//! Packing and publication under injected failures, through a recording `Disk` and a fake release.

use bench_tools::evidence::publication::{self, PART_LIMIT};
use bench_tools::evidence::{self, Disk, Error, Remote, Store, System, Upload};
use bench_tools::pyjson::{self, Duplicates, Value};
use bench_tools::pytext::PyPath;
use bench_tools::sha256;
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const MEMBERS: [(&str, &str, u32, &[u8]); 3] = [
    (
        "probe.sh",
        "artifacts/workspaces/r1/probe.sh",
        0o755,
        b"#!/bin/sh\nexit 0\n",
    ),
    (
        "review.md",
        "bench/runs/r1/attempts/att-001/review.md",
        0o644,
        b"# Review\n\nNo findings.\n",
    ),
    (
        "notes.txt",
        "docs/evidence/r1/notes.txt",
        0o600,
        b"private notes\n",
    ),
];

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "bench-tools-publication-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::canonicalize(&path).unwrap()
}

fn py(path: &Path) -> PyPath {
    PyPath::new(path.as_os_str().as_bytes())
}

/// The sources under `root/src` and a selection naming them.
fn sources(root: &Path) -> Value {
    let mut files = Vec::new();
    for (name, path, mode, data) in MEMBERS {
        let source = root.join("src").join(name);
        std::fs::create_dir_all(source.parent().unwrap()).unwrap();
        std::fs::write(&source, data).unwrap();
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(mode)).unwrap();
        files.push(format!(
            r#"{{"source": "{}", "path": "{path}", "kind": "evidence"}}"#,
            source.display()
        ));
    }
    pyjson::loads(
        &format!(
            r#"{{"repository": "canonical/bench", "tag": "evidence-test", "subjects": ["r1"], "files": [{}]}}"#,
            files.join(", ")
        ),
        Duplicates::KeepLast,
    )
    .unwrap()
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Create(PathBuf),
    Sync(PathBuf),
    Link(PathBuf, PathBuf),
    SyncDir(PathBuf),
}

/// The host filesystem, recording each effect and failing the effect numbered `fail_at`, as a crash would stop it.
struct Recording {
    events: Vec<Event>,
    fail_at: Option<usize>,
}

impl Recording {
    fn new(fail_at: Option<usize>) -> Self {
        Self {
            events: Vec::new(),
            fail_at,
        }
    }

    fn record(&mut self, event: Event) -> io::Result<()> {
        if self.fail_at == Some(self.events.len()) {
            return Err(io::Error::other("injected interruption"));
        }
        self.events.push(event);
        Ok(())
    }
}

impl Disk for Recording {
    fn create(&mut self, path: &Path) -> io::Result<File> {
        self.record(Event::Create(path.to_path_buf()))?;
        System.create(path)
    }

    fn sync(&mut self, file: &File, path: &Path) -> io::Result<()> {
        self.record(Event::Sync(path.to_path_buf()))?;
        System.sync(file, path)
    }

    fn link(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        self.record(Event::Link(from.to_path_buf(), to.to_path_buf()))?;
        System.link(from, to)
    }

    fn sync_dir(&mut self, path: &Path) -> io::Result<()> {
        self.record(Event::SyncDir(path.to_path_buf()))?;
        System.sync_dir(path)
    }
}

fn pack(root: &Path, disk: &mut dyn Disk) -> Result<Value, Error> {
    let selection = sources(root);
    publication::pack(
        disk,
        &selection,
        &py(&root.join("out")),
        &PART_LIMIT.to_string(),
        &py(root),
    )
}

/// Each archive is synced before it is linked, every link is followed by a sync of its directory, and the manifest
/// is the last name linked.
fn assert_durable(events: &[Event]) {
    let links: Vec<(usize, &PathBuf, &PathBuf)> = events
        .iter()
        .enumerate()
        .filter_map(|(at, event)| match event {
            Event::Link(from, to) => Some((at, from, to)),
            _ => None,
        })
        .collect();
    for (at, from, to) in &links {
        assert!(
            events[..*at].contains(&Event::Sync((*from).clone())),
            "{from:?} is linked at {to:?} before it is synced: {events:?}"
        );
        assert!(
            events[at + 1..].contains(&Event::SyncDir(to.parent().unwrap().to_path_buf())),
            "{to:?} is linked but its directory is never synced: {events:?}"
        );
    }
}

#[test]
fn pack_syncs_each_archive_before_linking_it_and_links_the_manifest_last() {
    let root = scratch("pack");
    let mut disk = Recording::new(None);
    let manifest = pack(&root, &mut disk).unwrap();
    assert_durable(&disk.events);
    let last = disk
        .events
        .iter()
        .rev()
        .find_map(|event| match event {
            Event::Link(_, to) => Some(to.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(last, root.join("out/manifest.json"));
    assert!(
        disk.events.contains(&Event::SyncDir(root.clone())),
        "the new out directory"
    );
    let mut store = Store {
        remote: &mut Release::default(),
        disk: &mut System,
        now: &fixed_clock,
        temporary: py(&root),
    };
    publication::check_package(&mut store, &py(&root.join("out"))).unwrap();
    let package = &manifest
        .as_object()
        .unwrap()
        .get("packages")
        .unwrap()
        .as_array()
        .unwrap()[0];
    let modes: Vec<String> = package
        .as_object()
        .unwrap()
        .get("members")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|member| member.as_object().unwrap().get("mode").unwrap().py_str())
        .collect();
    assert_eq!(
        modes,
        ["493", "420", "384"],
        "members keep each source's mode, in logical-path order"
    );
}

#[test]
fn an_interrupted_pack_leaves_only_synced_whole_files() {
    let total = {
        let root = scratch("pack-count");
        let mut disk = Recording::new(None);
        pack(&root, &mut disk).unwrap();
        disk.events.len()
    };
    for fail_at in 0..total {
        let root = scratch(&format!("pack-{fail_at}"));
        let mut disk = Recording::new(Some(fail_at));
        assert!(pack(&root, &mut disk).is_err(), "effect {fail_at}");
        let manifest = root.join("out/manifest.json");
        let linked = disk
            .events
            .iter()
            .any(|event| matches!(event, Event::Link(_, to) if *to == manifest));
        assert_eq!(
            manifest.exists(),
            linked,
            "effect {fail_at}: a manifest exists exactly when its synced bytes were linked"
        );
        for entry in std::fs::read_dir(root.join("out")).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_str().unwrap().to_string();
            assert!(
                name == "manifest.json"
                    || name.ends_with(".tar.gz")
                        && sha256::hex_digest(&std::fs::read(&path).unwrap()) + ".tar.gz" == name,
                "effect {fail_at} left {name}"
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn a_failed_rename_of_the_package_fails_the_pack() {
    let root = scratch("pack-link");
    let mut counting = Recording::new(None);
    pack(&scratch("pack-link-count"), &mut counting).unwrap();
    let first_link = counting
        .events
        .iter()
        .position(|event| matches!(event, Event::Link(..)))
        .unwrap();
    let mut disk = Recording::new(Some(first_link));
    assert_eq!(
        pack(&root, &mut disk).err(),
        Some(Error::Io(format!(
            "injected interruption: {}",
            pyquote(&format!(
                "{}/out/{}",
                root.display(),
                asset_of(&counting.events[first_link])
            ))
        )))
    );
    assert_eq!(std::fs::read_dir(root.join("out")).unwrap().count(), 0);
}

fn asset_of(event: &Event) -> String {
    let Event::Link(_, to) = event else {
        unreachable!()
    };
    to.file_name().unwrap().to_str().unwrap().to_string()
}

fn pyquote(text: &str) -> String {
    format!("'{text}'")
}

#[test]
fn an_input_that_changes_after_it_is_hashed_is_refused() {
    for replacement in [&b"#!/bin/sh\nexit 1\n"[..], b"#!/bin/sh\n"] {
        let root = scratch("changed");
        let selection = sources(&root);
        let output = py(&root.join("out"));
        let selected =
            publication::select(&selection, &output, &PART_LIMIT.to_string(), &py(&root)).unwrap();
        std::fs::write(root.join("src/probe.sh"), replacement).unwrap();
        assert_eq!(
            publication::write(&mut System, selected, &output).map(|_| ()),
            Err(Error::Violation(format!(
                "missing or changed evidence: {}/src/probe.sh",
                root.display()
            )))
        );
        assert!(!root.join("out/manifest.json").exists());
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Fault {
    #[default]
    None,
    /// The asset is stored, then the upload fails, as a lost response does.
    LostResponse,
    /// Half the asset is stored, then the upload fails.
    PartialAsset,
    /// Nothing is stored and the upload fails.
    Rejected,
    /// Downloads return other bytes of the same size.
    CorruptDownload,
}

/// A release that stores uploads, refusing to replace one.
#[derive(Default)]
struct Release {
    assets: Vec<(u64, String, Vec<u8>)>,
    uploads: u32,
    downloads: u32,
    fault: Fault,
}

impl Remote for Release {
    fn release(&mut self, _repository: &str, _tag: &str) -> Result<Value, Error> {
        let assets: Vec<String> = self
            .assets
            .iter()
            .map(|(id, name, data)| {
                format!(
                    r#"{{"id": {id}, "name": "{name}", "size": {}}}"#,
                    data.len()
                )
            })
            .collect();
        Ok(pyjson::loads(
            &format!(
                r#"{{"id": 7, "draft": false, "assets": [{}]}}"#,
                assets.join(", ")
            ),
            Duplicates::KeepLast,
        )
        .unwrap())
    }

    fn download(
        &mut self,
        _repository: &str,
        asset_id: &str,
        destination: &mut dyn Write,
        limit: u64,
    ) -> Result<(), Error> {
        self.downloads += 1;
        let (_, _, data) = self
            .assets
            .iter()
            .find(|(id, ..)| id.to_string() == asset_id)
            .unwrap();
        assert!(data.len() as u64 <= limit);
        let mut data = data.clone();
        if self.fault == Fault::CorruptDownload {
            let last = data.len() - 1;
            data[last] ^= 0xff;
        }
        destination.write_all(&data).unwrap();
        Ok(())
    }

    fn upload(&mut self, _repository: &str, _tag: &str, archive: &PyPath) -> Result<Upload, Error> {
        self.uploads += 1;
        let path = archive.to_path_buf();
        let name = path.file_name().unwrap().to_str().unwrap().to_string();
        assert!(
            self.assets.iter().all(|(_, existing, _)| *existing != name),
            "attempted to replace {name}"
        );
        let data = std::fs::read(&path).unwrap();
        let failed = Upload::Failed(Error::Io("upload failed".to_string()));
        let id = 101 + self.assets.len() as u64;
        match self.fault {
            Fault::Rejected => return Ok(failed),
            Fault::PartialAsset => {
                self.assets
                    .push((id, name, data[..data.len() / 2].to_vec()));
                return Ok(failed);
            }
            _ => self.assets.push((id, name, data)),
        }
        Ok(if self.fault == Fault::LostResponse {
            failed
        } else {
            Upload::Done
        })
    }
}

fn fixed_clock() -> String {
    evidence::isoformat(1_792_411_200, 0)
}

fn publish(root: &Path, release: &mut Release, disk: &mut dyn Disk) -> Result<Value, Error> {
    let temporary = root.join("tmp");
    std::fs::create_dir_all(&temporary).unwrap();
    let mut store = Store {
        remote: release,
        disk,
        now: &fixed_clock,
        temporary: py(&temporary),
    };
    publication::publish(
        &mut store,
        &py(&root.join("out/manifest.json")),
        &py(&root.join("published.json")),
    )
}

fn packed(label: &str) -> PathBuf {
    let root = scratch(label);
    pack(&root, &mut System).unwrap();
    root
}

#[test]
fn publish_uploads_once_restores_the_asset_and_writes_the_manifest_last() {
    let root = packed("publish");
    let mut release = Release::default();
    let mut disk = Recording::new(None);
    let published = publish(&root, &mut release, &mut disk).unwrap();
    assert_eq!((release.uploads, release.downloads), (1, 1));
    assert_durable(&disk.events);
    let written = std::fs::read_to_string(root.join("published.json")).unwrap();
    let package = &published
        .as_object()
        .unwrap()
        .get("packages")
        .unwrap()
        .as_array()
        .unwrap()[0];
    let sha = package.as_object().unwrap().get("sha256").unwrap().py_str();
    assert!(
        written.contains(&format!(
            "\"publication\": {{\n        \"release_id\": 7,\n        \"asset_id\": 101,\n        \"sha256\": \"{sha}\",\n        \
             \"verified_at\": \"2026-10-19T12:00:00+00:00\"\n      }}"
        )),
        "{written}"
    );
    let unpublished = std::fs::read_to_string(root.join("out/manifest.json")).unwrap();
    assert_eq!(
        written.replace(
            &format!(
                "{{\n        \"release_id\": 7,\n        \"asset_id\": 101,\n        \"sha256\": \"{sha}\",\n        \
                 \"verified_at\": \"2026-10-19T12:00:00+00:00\"\n      }}"
            ),
            "null"
        ),
        unpublished,
        "only the publication receipt changes"
    );
    publish(&root, &mut release, &mut System).unwrap();
    assert_eq!(
        (release.uploads, release.downloads),
        (1, 2),
        "an existing published manifest is verified, not uploaded again"
    );
}

#[test]
fn a_lost_upload_response_is_reconciled_by_reading_the_release() {
    let root = packed("lost");
    let mut release = Release {
        fault: Fault::LostResponse,
        ..Release::default()
    };
    publish(&root, &mut release, &mut System).unwrap();
    assert_eq!((release.uploads, release.assets.len()), (1, 1));
}

#[test]
fn a_partial_asset_is_refused_and_never_replaced() {
    let root = packed("partial");
    let mut release = Release {
        fault: Fault::PartialAsset,
        ..Release::default()
    };
    let refused = Some(Error::Violation(
        "release asset identity or size differs".to_string(),
    ));
    assert_eq!(publish(&root, &mut release, &mut System).err(), refused);
    release.fault = Fault::None;
    assert_eq!(publish(&root, &mut release, &mut System).err(), refused);
    assert_eq!(release.uploads, 1);
    assert!(!root.join("published.json").exists());
}

#[test]
fn a_rejected_upload_fails_with_its_command_and_writes_nothing() {
    let root = packed("rejected");
    let mut release = Release {
        fault: Fault::Rejected,
        ..Release::default()
    };
    assert_eq!(
        publish(&root, &mut release, &mut System).err(),
        Some(Error::Io("upload failed".to_string()))
    );
    assert!(release.assets.is_empty());
    assert!(!root.join("published.json").exists());
}

#[test]
fn a_corrupted_download_writes_no_published_manifest() {
    let root = packed("corrupt");
    let mut release = Release {
        fault: Fault::CorruptDownload,
        ..Release::default()
    };
    assert!(matches!(
        publish(&root, &mut release, &mut System),
        Err(Error::Violation(message)) if message.starts_with("missing or changed evidence: ")
    ));
    assert!(!root.join("published.json").exists());
}

#[test]
fn an_interrupted_publish_leaves_no_manifest_and_its_retry_uploads_nothing_more() {
    let total = {
        let root = packed("publish-count");
        let mut disk = Recording::new(None);
        publish(&root, &mut Release::default(), &mut disk).unwrap();
        disk.events.len()
    };
    for fail_at in 0..total {
        let root = packed(&format!("publish-{fail_at}"));
        let mut release = Release::default();
        let mut disk = Recording::new(Some(fail_at));
        assert!(
            publish(&root, &mut release, &mut disk).is_err(),
            "effect {fail_at}"
        );
        let linked = disk
            .events
            .iter()
            .any(|event| matches!(event, Event::Link(_, to) if *to == root.join("published.json")));
        assert_eq!(
            root.join("published.json").exists(),
            linked,
            "effect {fail_at}: a manifest exists exactly when its synced bytes were linked"
        );
        if !linked {
            publish(&root, &mut release, &mut System).unwrap();
        }
        assert_eq!(release.uploads, 1, "effect {fail_at}");
        std::fs::remove_dir_all(&root).unwrap();
    }
}

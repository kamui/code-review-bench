//! Durability and interruption of evidence restoration, through a recording `Disk` and a fake `Remote`.

use bench_tools::archive::PackageWriter;
use bench_tools::evidence::{self, Disk, Error, Remote, Store, System};
use bench_tools::pyjson::{self, Duplicates, Value};
use bench_tools::pytext::PyPath;
use bench_tools::sha256;
use std::cell::Cell;
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const MEMBERS: [(&str, u32, &[u8]); 3] = [
    (
        "artifacts/workspaces/r1/probe.sh",
        0o755,
        b"#!/bin/sh\nexit 0\n",
    ),
    (
        "bench/runs/r1/attempts/att-001/review.md",
        0o644,
        b"# Review\n\nNo findings.\n",
    ),
    ("docs/evidence/r1/notes.txt", 0o600, b"private notes\n"),
];

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "bench-tools-evidence-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    std::fs::canonicalize(&path).unwrap()
}

fn py(path: &Path) -> PyPath {
    PyPath::new(path.as_os_str().as_bytes())
}

fn package() -> Vec<u8> {
    let mut writer = PackageWriter::new(Vec::new());
    for (name, mode, data) in MEMBERS {
        writer
            .append(name.as_bytes(), data.len() as u64, mode, &mut &data[..])
            .unwrap();
    }
    writer.finish().unwrap()
}

fn manifest(archive: &[u8]) -> Value {
    let sha = sha256::hex_digest(archive);
    let members: Vec<String> = MEMBERS
        .iter()
        .map(|(name, mode, data)| {
            format!(
                r#"{{"path": "{name}", "sha256": "{}", "bytes": {}, "mode": {mode}, "kind": "evidence"}}"#,
                sha256::hex_digest(data),
                data.len()
            )
        })
        .collect();
    let text = format!(
        r#"{{"schema_version": 1, "repository": "canonical/bench", "tag": "evidence-test", "subjects": ["r1"],
        "packages": [{{"asset": "{sha}.tar.gz", "sha256": "{sha}", "bytes": {}, "members": [{}],
        "publication": {{"release_id": 7, "asset_id": 101, "sha256": "{sha}", "verified_at": "2026-10-19T12:00:00+00:00"}}}}]}}"#,
        archive.len(),
        members.join(", ")
    );
    pyjson::loads(&text, Duplicates::KeepLast).unwrap()
}

/// A release holding one asset, which may fail its download.
struct FakeRemote {
    archive: Vec<u8>,
    downloads: Cell<u32>,
    fail_download: bool,
}

impl FakeRemote {
    fn new(archive: &[u8]) -> Self {
        Self {
            archive: archive.to_vec(),
            downloads: Cell::new(0),
            fail_download: false,
        }
    }
}

impl Remote for FakeRemote {
    fn release(&mut self, _repository: &str, _tag: &str) -> Result<Value, Error> {
        let sha = sha256::hex_digest(&self.archive);
        let text = format!(
            r#"{{"id": 7, "draft": false, "assets": [{{"id": 101, "name": "{sha}.tar.gz", "size": {}}}]}}"#,
            self.archive.len()
        );
        Ok(pyjson::loads(&text, Duplicates::KeepLast).unwrap())
    }

    fn download(
        &mut self,
        _repository: &str,
        asset_id: &str,
        destination: &mut dyn Write,
        limit: u64,
    ) -> Result<(), Error> {
        assert_eq!(asset_id, "101");
        assert!(self.archive.len() as u64 <= limit);
        self.downloads.set(self.downloads.get() + 1);
        if self.fail_download {
            destination
                .write_all(&self.archive[..self.archive.len() / 2])
                .unwrap();
            return Err(Error::Io("connection reset by peer".to_string()));
        }
        destination.write_all(&self.archive).unwrap();
        Ok(())
    }
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

fn fixed_clock() -> String {
    evidence::isoformat(1_792_411_200, 0)
}

fn fetch(
    root: &Path,
    document: &Value,
    remote: &mut FakeRemote,
    disk: &mut dyn Disk,
) -> Result<(), Error> {
    let mut store = Store {
        remote,
        disk,
        now: &fixed_clock,
    };
    evidence::materialize(&mut store, document, &py(root), None, false).map(|_| ())
}

fn directories(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() && !path.is_symlink() {
                found.push(path.clone());
                pending.push(path);
            }
        }
    }
    found
}

fn assert_restored(root: &Path) {
    for (name, mode, data) in MEMBERS {
        let path = root.join(name);
        assert_eq!(std::fs::read(&path).unwrap(), data, "{name}");
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            mode,
            "{name}"
        );
    }
}

#[test]
fn fetch_syncs_each_file_before_linking_it_and_each_changed_directory_before_returning() {
    let root = scratch("sync-order");
    let archive = package();
    let document = manifest(&archive);
    let mut remote = FakeRemote::new(&archive);
    let mut disk = Recording::new(None);
    fetch(&root, &document, &mut remote, &mut disk).unwrap();
    assert_restored(&root);
    let events = &disk.events;
    let links: Vec<(usize, &PathBuf, &PathBuf)> = events
        .iter()
        .enumerate()
        .filter_map(|(at, event)| match event {
            Event::Link(from, to) => Some((at, from, to)),
            _ => None,
        })
        .collect();
    assert_eq!(
        links.len(),
        1 + MEMBERS.len(),
        "the archive and every member are linked: {events:?}"
    );
    for (at, from, to) in &links {
        let synced = events[..*at]
            .iter()
            .rposition(|event| *event == Event::Sync((*from).clone()));
        assert!(
            synced.is_some(),
            "{from:?} is linked at {to:?} before it is synced: {events:?}"
        );
        let directory = to.parent().unwrap().to_path_buf();
        assert!(
            events[at + 1..].contains(&Event::SyncDir(directory.clone())),
            "{directory:?} is not synced after {to:?} is linked into it: {events:?}"
        );
    }
    for directory in directories(&root) {
        if directory.starts_with(root.join(".cache/evidence"))
            && directory != root.join(".cache/evidence")
        {
            continue;
        }
        let parent = directory.parent().unwrap().to_path_buf();
        assert!(
            events.contains(&Event::SyncDir(parent.clone())),
            "{parent:?} gained {directory:?} but is never synced: {events:?}"
        );
    }
}

#[test]
fn an_interruption_at_any_disk_effect_leaves_nothing_partial_and_a_retry_completes() {
    let archive = package();
    let document = manifest(&archive);
    let total = {
        let root = scratch("count");
        let mut disk = Recording::new(None);
        fetch(&root, &document, &mut FakeRemote::new(&archive), &mut disk).unwrap();
        disk.events.len()
    };
    for fail_at in 0..total {
        let root = scratch(&format!("interrupt-{fail_at}"));
        let mut remote = FakeRemote::new(&archive);
        let mut disk = Recording::new(Some(fail_at));
        assert!(
            fetch(&root, &document, &mut remote, &mut disk).is_err(),
            "effect {fail_at}"
        );
        for (name, _, data) in MEMBERS {
            let path = root.join(name);
            assert!(
                !path.exists() || std::fs::read(&path).unwrap() == data,
                "effect {fail_at} left a partial {name}"
            );
        }
        let cached = root
            .join(".cache/evidence")
            .join(format!("{}.tar.gz", sha256::hex_digest(&archive)));
        assert!(
            !cached.exists() || std::fs::read(&cached).unwrap() == archive,
            "effect {fail_at}"
        );
        let had_cache = cached.exists();
        let mut retry = FakeRemote::new(&archive);
        fetch(&root, &document, &mut retry, &mut System).unwrap();
        assert_restored(&root);
        assert_eq!(
            retry.downloads.get(),
            u32::from(!had_cache),
            "effect {fail_at}"
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}

#[test]
fn a_failed_download_is_neither_cached_nor_restored() {
    let root = scratch("failed-download");
    let archive = package();
    let document = manifest(&archive);
    let mut remote = FakeRemote::new(&archive);
    remote.fail_download = true;
    assert_eq!(
        fetch(&root, &document, &mut remote, &mut System),
        Err(Error::Io("connection reset by peer".to_string()))
    );
    assert!(!root.join(MEMBERS[0].0).exists());
    let cache = root.join(".cache/evidence");
    assert_eq!(
        std::fs::read_dir(&cache).unwrap().count(),
        0,
        "the temporary download is removed"
    );
}

fn receipt(root: &Path, remote: &mut FakeRemote, disk: &mut dyn Disk) -> Result<Value, Error> {
    let manifest_path = root.join("published.json");
    let mut store = Store {
        remote,
        disk,
        now: &fixed_clock,
    };
    evidence::restoration_receipt(
        &mut store,
        &py(&manifest_path),
        &py(&root.join("receipts/receipt.json")),
    )
}

fn write_manifest(root: &Path, document: &Value) {
    let text = pyjson::dumps(
        document,
        &pyjson::Dumps {
            indent: Some(2),
            ..pyjson::Dumps::default()
        },
    );
    std::fs::write(root.join("published.json"), format!("{text}\n")).unwrap();
}

#[test]
fn the_receipt_is_synced_before_it_is_linked_and_spells_the_clock_as_isoformat() {
    let root = scratch("receipt");
    let archive = package();
    let document = manifest(&archive);
    write_manifest(&root, &document);
    let mut disk = Recording::new(None);
    receipt(&root, &mut FakeRemote::new(&archive), &mut disk).unwrap();
    let path = root.join("receipts/receipt.json");
    let link = disk
        .events
        .iter()
        .position(|event| matches!(event, Event::Link(_, to) if *to == path))
        .expect("the receipt is linked into place");
    let Event::Link(temporary, _) = &disk.events[link] else {
        unreachable!()
    };
    assert!(disk.events[..link].contains(&Event::Sync(temporary.clone())));
    assert!(disk.events[link + 1..].contains(&Event::SyncDir(root.join("receipts"))));
    assert!(
        disk.events[link + 1..].contains(&Event::SyncDir(root.clone())),
        "the new receipts directory"
    );
    assert!(!temporary.exists(), "the temporary name is removed");
    let manifest_sha = sha256::hex_digest(&std::fs::read(root.join("published.json")).unwrap());
    let package_sha = sha256::hex_digest(&archive);
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        format!(
            "{{\n  \"schema_version\": 1,\n  \"status\": \"verified\",\n  \"manifest_sha256\": \"{manifest_sha}\",\n  \
             \"at\": \"2026-10-19T12:00:00+00:00\",\n  \"subjects\": [\n    \"r1\"\n  ],\n  \"packages\": [\n    \
             \"{package_sha}\"\n  ],\n  \"method\": \"empty temporary directory; remote identity, archive hash, \
             member bytes and permissions\"\n}}\n"
        )
    );
    assert_eq!(
        evidence::isoformat(1_792_411_200, 120),
        "2026-10-19T12:00:00.000120+00:00"
    );
}

#[test]
fn interrupting_verify_receipt_at_download_unpack_or_receipt_write_leaves_no_receipt() {
    let archive = package();
    let document = manifest(&archive);
    let total = {
        let root = scratch("receipt-count");
        write_manifest(&root, &document);
        let mut disk = Recording::new(None);
        receipt(&root, &mut FakeRemote::new(&archive), &mut disk).unwrap();
        disk.events.len()
    };
    let root = scratch("receipt-download");
    write_manifest(&root, &document);
    let mut remote = FakeRemote::new(&archive);
    remote.fail_download = true;
    assert!(receipt(&root, &mut remote, &mut System).is_err());
    assert!(!root.join("receipts/receipt.json").exists());
    for fail_at in 0..total {
        let root = scratch(&format!("receipt-{fail_at}"));
        write_manifest(&root, &document);
        let mut disk = Recording::new(Some(fail_at));
        let outcome = receipt(&root, &mut FakeRemote::new(&archive), &mut disk);
        let linked = disk
            .events
            .iter()
            .any(|event| matches!(event, Event::Link(..)));
        assert!(outcome.is_err(), "effect {fail_at}");
        assert_eq!(
            root.join("receipts/receipt.json").exists(),
            linked,
            "effect {fail_at}: a receipt exists exactly when its synced bytes were linked: {:?}",
            disk.events
        );
        if !linked {
            receipt(&root, &mut FakeRemote::new(&archive), &mut System).unwrap();
            assert!(root.join("receipts/receipt.json").exists());
        }
        std::fs::remove_dir_all(&root).unwrap();
    }
}

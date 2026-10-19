//! Interrupted native artifact filing and transcript archiving, through a `Disk` that fails one effect as a crash
//! would stop it.

use bench_tools::evidence::transcripts::archive_transcripts;
use bench_tools::evidence::{self, Disk, System};
use bench_tools::native_artifacts::{self, ARCHIVE, INDEX, STORAGE};
use bench_tools::pyjson::{self, Dumps, Duplicates};
use bench_tools::pytext::PyPath;
use bench_tools::sha256;
use std::fs::File;
use std::io;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "bench-tools-native-artifacts-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path.canonicalize().unwrap()
}

fn py(path: &Path) -> PyPath {
    PyPath::new(path.as_os_str().as_bytes())
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Create(PathBuf),
    Sync(PathBuf),
    Link(PathBuf, PathBuf),
    SyncDir(PathBuf),
    Rename(PathBuf, PathBuf),
}

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

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        self.record(Event::Rename(from.to_path_buf(), to.to_path_buf()))?;
        std::fs::rename(from, to)
    }
}

const FILES: [(&str, &[u8], u32); 3] = [
    ("run-1/review.json", b"{\"status\": \"complete\"}", 0o644),
    ("run-1/scratch/notes.txt", b"notes\n", 0o600),
    ("run-1/scratch/tool.sh", b"#!/bin/sh\n", 0o755),
];

/// A report folder named `reports` and its index, under a new `base`.
fn report(base: &Path) -> (PathBuf, PathBuf) {
    let root = base.join("reports");
    let mut rows = Vec::new();
    for (name, data, mode) in FILES {
        let path = root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, data).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        rows.push(format!(
            "{{\"path\": \"{name}\", \"bytes\": {}, \"sha256\": \"{}\"}}",
            data.len(),
            sha256::hex_digest(data)
        ));
    }
    let index = base.join(INDEX);
    std::fs::write(&index, format!("{{\"files\": [{}]}}", rows.join(", "))).unwrap();
    (root, index)
}

fn file(disk: &mut dyn Disk, base: &Path) -> Result<String, native_artifacts::Error> {
    let (root, index) = (base.join("reports"), base.join(INDEX));
    native_artifacts::file_artifacts(
        disk,
        &py(&root),
        &py(&index),
        &py(&base.join("out")),
        Some(b"run-1/review.json"),
    )
    .map(|pin| pyjson::dumps(&pin.expect("scratch files are archived"), &Dumps::default()))
}

fn tree(root: &Path) -> Vec<(PathBuf, Vec<u8>, u32)> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
                found.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(&path).unwrap(),
                    mode,
                ));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn an_interrupted_filing_leaves_no_storage_record_and_a_retry_completes_it() {
    let base = scratch("count");
    report(&base);
    let mut disk = Recording::new(None);
    let pin = file(&mut disk, &base).unwrap();
    let out = base.join("out");
    let storage = out.join(STORAGE);
    let last = disk
        .events
        .iter()
        .rposition(|event| matches!(event, Event::Link(..)))
        .unwrap();
    assert!(matches!(&disk.events[last], Event::Link(_, to) if *to == storage));
    for name in [INDEX, ARCHIVE, "reports"] {
        let placed = disk.events.iter().position(|event| match event {
            Event::Link(_, to) | Event::Rename(_, to) => *to == out.join(name),
            _ => false,
        });
        assert!(
            placed.unwrap() < last,
            "{name} is placed before the storage record"
        );
    }
    let total = disk.events.len();
    for fail_at in 0..total {
        let base = scratch(&format!("interrupted-{fail_at}"));
        report(&base);
        let mut disk = Recording::new(Some(fail_at));
        assert!(file(&mut disk, &base).is_err(), "effect {fail_at}");
        assert_eq!(
            base.join("out").join(STORAGE).exists(),
            fail_at > last,
            "effect {fail_at}: {:?}",
            disk.events.last()
        );
        let mut retry = Recording::new(None);
        assert_eq!(file(&mut retry, &base).unwrap(), pin, "effect {fail_at}");
        assert!(
            retry.events.contains(&Event::SyncDir(base.join("out"))),
            "effect {fail_at}: the retry syncs the filing's directory"
        );
        let pin = pyjson::loads(&pin, Duplicates::KeepLast).unwrap();
        native_artifacts::verify(&py(&base.join("out")), Some(&pin)).unwrap();
    }
}

#[test]
fn a_retry_refuses_an_interrupted_filing_whose_leftovers_differ() {
    let base = scratch("leftover");
    report(&base);
    let mut disk = Recording::new(None);
    file(&mut disk, &base).unwrap();
    let stop = disk
        .events
        .iter()
        .rposition(|event| matches!(event, Event::Link(..)))
        .unwrap();
    let base = scratch("leftover-differs");
    report(&base);
    assert!(file(&mut Recording::new(Some(stop)), &base).is_err());
    std::fs::write(base.join("out/reports/run-1/review.json"), b"changed").unwrap();
    let error = file(&mut System, &base).unwrap_err();
    assert_eq!(
        error.0,
        "scratch packing needs unused artifact output paths"
    );
    assert!(!base.join("out").join(STORAGE).exists());
}

#[test]
fn restoring_a_rust_filing_gives_back_every_byte_and_mode() {
    let base = scratch("restore");
    let (root, _) = report(&base);
    let pin = file(&mut System, &base).unwrap();
    let record = format!("{{\"native_artifact_storage\": {pin}}}");
    std::fs::write(base.join("out/attempt.json"), record).unwrap();
    let restored = base.join("restored");
    native_artifacts::restore(&mut System, &py(&base.join("out")), &py(&restored)).unwrap();
    assert_eq!(tree(&restored), tree(&root));
}

#[test]
fn an_interrupted_transcript_archive_leaves_no_archive_and_a_retry_writes_it_once() {
    let base = scratch("transcripts");
    let attempt = base.join("attempt");
    std::fs::create_dir_all(attempt.join("home/projects")).unwrap();
    let transcript = attempt.join("home/projects/s1.jsonl");
    std::fs::write(&transcript, b"{\"type\": \"user\"}\n").unwrap();
    let paths = vec![transcript.as_os_str().as_bytes().to_vec()];
    let archive = |disk: &mut dyn Disk, dest: &Path| {
        archive_transcripts(
            disk,
            &paths,
            attempt.as_os_str().as_bytes(),
            dest.as_os_str().as_bytes(),
            base.join("out").as_os_str().as_bytes(),
            None,
        )
    };
    let mut disk = Recording::new(None);
    let first = archive(&mut disk, &base.join("count/a.tar.gz")).unwrap();
    assert!(first.passed);
    let linked = disk
        .events
        .iter()
        .position(|event| matches!(event, Event::Link(..)))
        .unwrap();
    for fail_at in 0..disk.events.len() {
        let dest = base.join(format!("interrupted-{fail_at}/a.tar.gz"));
        assert!(archive(&mut Recording::new(Some(fail_at)), &dest).is_err());
        assert_eq!(dest.exists(), fail_at > linked, "effect {fail_at}");
        assert_eq!(archive(&mut System, &dest).unwrap().sha256, first.sha256);
    }
    let dest = base.join("count/a.tar.gz");
    let mut again = Recording::new(None);
    assert_eq!(archive(&mut again, &dest).unwrap().sha256, first.sha256);
    assert!(
        !again
            .events
            .iter()
            .any(|event| matches!(event, Event::Link(..) | Event::Create(..))),
        "{:?}",
        again.events
    );
    std::fs::write(&transcript, b"{\"type\": \"assistant\"}\n").unwrap();
    let error = archive(&mut System, &dest).unwrap_err();
    assert!(matches!(error, evidence::Error::Violation(_)), "{error:?}");
    assert!(error.message().starts_with("refusing to replace "));
    assert_eq!(
        sha256::hex_digest(&std::fs::read(&dest).unwrap()),
        first.sha256
    );
}

//! Durability and interruption of `import-benchmark --preserve`, through a recording `Disk`.

use bench_tools::evidence::{self, Disk, Error, Remote, Store, System, Upload};
use bench_tools::import_benchmark;
use bench_tools::pyjson::Value;
use bench_tools::pytext::PyPath;
use bench_tools::sha256;
use std::fs::File;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const NAME: &str = "bench/tools/imported.py";
const PRESERVED: &str = "artifacts/import-source/bench/tools/imported.py";
const ORIGINAL: &[u8] = b"imported source\n";

fn scratch(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("bench-tools-import-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(path.join("bench/tools")).unwrap();
    std::fs::canonicalize(&path).unwrap()
}

fn py(path: &Path) -> PyPath {
    PyPath::new(path.as_os_str().as_bytes())
}

fn old_manifest() -> String {
    format!(
        "{{\"files\": [{{\"path\": \"{NAME}\", \"sha256\": \"{}\", \"bytes\": 16}}], \"transcripts\": []}}\n",
        sha256::hex_digest(ORIGINAL)
    )
}

fn new_manifest() -> String {
    format!(
        "{{\n  \"files\": [\n    {{\n      \"path\": \"{NAME}\",\n      \"sha256\": \"{}\",\n      \"bytes\": 16,\n      \
         \"preserved_path\": \"{PRESERVED}\"\n    }}\n  ],\n  \"transcripts\": []\n}}\n",
        sha256::hex_digest(ORIGINAL)
    )
}

fn repository(label: &str) -> PathBuf {
    let root = scratch(label);
    std::fs::write(root.join(NAME), ORIGINAL).unwrap();
    let manifest = root.join(import_benchmark::MANIFEST);
    std::fs::write(&manifest, old_manifest()).unwrap();
    std::fs::set_permissions(&manifest, std::fs::Permissions::from_mode(0o640)).unwrap();
    root
}

/// No case here reaches the remote: every file the check reads is present.
struct NoRemote;

impl Remote for NoRemote {
    fn release(&mut self, _: &str, _: &str) -> Result<Value, Error> {
        Err(Error::Io(
            "the remote is not reachable in this test".to_string(),
        ))
    }

    fn download(&mut self, _: &str, _: &str, _: &mut dyn Write, _: u64) -> Result<(), Error> {
        Err(Error::Io(
            "the remote is not reachable in this test".to_string(),
        ))
    }

    fn upload(&mut self, _: &str, _: &str, _: &PyPath) -> Result<Upload, Error> {
        Err(Error::Io(
            "the remote is not reachable in this test".to_string(),
        ))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Create(PathBuf),
    Sync(PathBuf),
    Link(PathBuf, PathBuf),
    SyncDir(PathBuf),
    Rename(PathBuf, PathBuf),
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

    fn rename(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        self.record(Event::Rename(from.to_path_buf(), to.to_path_buf()))?;
        System.rename(from, to)
    }
}

fn preserve(root: &Path, disk: &mut dyn Disk) -> Result<String, import_benchmark::Refusal> {
    let mut store = Store {
        remote: &mut NoRemote,
        disk,
        now: &evidence::now_isoformat,
        temporary: py(&std::env::temp_dir()),
    };
    import_benchmark::preserve(&mut store, &py(root), &[NAME.as_bytes().to_vec()])
}

fn leftovers(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else if path.to_string_lossy().ends_with(".tmp") {
                found.push(path);
            }
        }
    }
    found
}

#[test]
fn preserve_links_a_synced_copy_and_then_renames_a_synced_manifest_over_the_old_one() {
    let root = repository("order");
    let mut disk = Recording::new(None);
    let summary = preserve(&root, &mut disk).unwrap();
    assert_eq!(
        summary,
        "Verified copied bytes for 1 source files and 0 archives; 0 superseded source-hash mismatches, 0 unavailable"
    );
    let manifest = root.join(import_benchmark::MANIFEST);
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), new_manifest());
    assert_eq!(
        std::fs::metadata(&manifest).unwrap().permissions().mode() & 0o777,
        0o640,
        "the manifest keeps its permission bits"
    );
    assert_eq!(std::fs::read(root.join(PRESERVED)).unwrap(), ORIGINAL);
    assert_eq!(leftovers(&root), Vec::<PathBuf>::new());
    let events = &disk.events;
    let position = |wanted: &dyn Fn(&Event) -> bool| events.iter().position(wanted).unwrap();
    let copy = root.join(PRESERVED);
    let linked = position(&|event| matches!(event, Event::Link(_, to) if *to == copy));
    let Event::Link(copy_temporary, _) = &events[linked] else {
        unreachable!()
    };
    let synced = position(&|event| *event == Event::Sync(copy_temporary.clone()));
    assert!(
        synced < linked,
        "the copy is synced before it is linked: {events:?}"
    );
    let copy_directory = position(&|event| *event == Event::SyncDir(copy.parent().unwrap().into()));
    assert!(linked < copy_directory, "{events:?}");
    let renamed = position(&|event| matches!(event, Event::Rename(_, to) if *to == manifest));
    let Event::Rename(manifest_temporary, _) = &events[renamed] else {
        unreachable!()
    };
    assert_eq!(manifest_temporary.parent(), manifest.parent());
    let manifest_synced = position(&|event| *event == Event::Sync(manifest_temporary.clone()));
    assert!(
        copy_directory < manifest_synced && manifest_synced < renamed,
        "{events:?}"
    );
    assert_eq!(
        events.last(),
        Some(&Event::SyncDir(manifest.parent().unwrap().into())),
        "the manifest's directory is synced last: {events:?}"
    );
}

#[test]
fn an_interruption_at_each_effect_leaves_the_old_or_new_manifest_and_a_retry_completes() {
    let mut clean = Recording::new(None);
    preserve(&repository("count"), &mut clean).unwrap();
    for fail_at in 0..clean.events.len() {
        let root = repository("interrupted");
        let manifest = root.join(import_benchmark::MANIFEST);
        let refusal = preserve(&root, &mut Recording::new(Some(fail_at)))
            .expect_err("the injected interruption is reported");
        assert!(
            refusal.0.contains("injected interruption"),
            "{fail_at}: {refusal:?}"
        );
        let text = std::fs::read_to_string(&manifest).unwrap();
        assert!(
            text == old_manifest() || text == new_manifest(),
            "{fail_at}: {text}"
        );
        if text == new_manifest() {
            assert_eq!(std::fs::read(root.join(PRESERVED)).unwrap(), ORIGINAL);
        }
        match std::fs::read(root.join(PRESERVED)) {
            Ok(copy) => assert_eq!(copy, ORIGINAL, "{fail_at}: a copy is complete or absent"),
            Err(error) => assert_eq!(error.kind(), io::ErrorKind::NotFound, "{fail_at}"),
        }
        assert_eq!(leftovers(&root), Vec::<PathBuf>::new(), "{fail_at}");
        preserve(&root, &mut System).unwrap();
        assert_eq!(std::fs::read_to_string(&manifest).unwrap(), new_manifest());
        assert_eq!(std::fs::read(root.join(PRESERVED)).unwrap(), ORIGINAL);
        assert_eq!(
            std::fs::metadata(&manifest).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}

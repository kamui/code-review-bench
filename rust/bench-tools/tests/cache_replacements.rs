//! A frozen cache archive replaced through a cache-replacement manifest: `bench-tools provision check` and `smoke`,
//! `grade prepare`, `grade check-prepared` and the offline grading preflight, on the `grading/pending` fixture with
//! its target revision moved to a real local repository. No reviewer, model or network is called.

#[path = "support/grading_fixture.rs"]
#[allow(dead_code, reason = "this file builds only the pending fixture")]
mod grading_fixture;
#[path = "support/runtime_store.rs"]
mod runtime_store;

use bench_tools::pyjson::{self, Dumps, Duplicates, Value};
use bench_tools::sha256;
use grading_fixture::{RUN, TARGET, Variant};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PENDING_HEAD: &str = "1111111111111111111111111111111111111111";
const PENDING_DIFF: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn parse(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn read(path: &Path) -> Value {
    parse(&fs::read_to_string(path).unwrap())
}

fn dumps(value: &Value) -> String {
    pyjson::dumps(value, &Dumps::default())
}

fn canonical(value: &Value) -> String {
    pyjson::dumps(
        value,
        &Dumps {
            sort_keys: true,
            ..Dumps::default()
        },
    )
}

/// The value at a dotted path, or null.
fn field<'a>(value: &'a Value, path: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    path.split('.').fold(value, |value, key| {
        value
            .as_object()
            .and_then(|object| object.get(key))
            .unwrap_or(&NULL)
    })
}

/// Sets the member at a dotted path whose parents exist.
fn set(value: &mut Value, path: &str, new: Value) {
    let (parents, last) = path.rsplit_once('.').unwrap_or(("", path));
    let mut node = value;
    for key in parents.split('.').filter(|key| !key.is_empty()) {
        node = match node {
            Value::Object(object) => object.get_mut(key).unwrap(),
            Value::Array(items) => &mut items[key.parse::<usize>().unwrap()],
            other => panic!("{other:?} has no member {key}"),
        };
    }
    match node {
        Value::Object(object) => object.insert(last.to_string(), new),
        other => panic!("{other:?} has no member {last}"),
    }
}

fn text(value: &Value) -> String {
    value.py_str()
}

fn digest(path: &Path) -> String {
    sha256::hex_digest(&fs::read(path).unwrap())
}

/// This build installed in a runtime store under `root`, as grading runs only from an installed runtime.
fn install(root: &Path) -> PathBuf {
    let (source, sha) = runtime_store::built();
    let entry = root.join("store/bench-tools").join(sha);
    fs::create_dir_all(&entry).unwrap();
    let executable = entry.join("bench-tools");
    if fs::hard_link(source, &executable).is_err() {
        fs::copy(source, &executable).unwrap();
    }
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let size = fs::metadata(&executable).unwrap().len();
    fs::write(
        entry.join("receipt.json"),
        runtime_store::receipt(sha, size),
    )
    .unwrap();
    executable
}

/// Runs from the repository, whose `bench/policies` grading reads relative to the working directory.
fn bench_tools(executable: &Path, arguments: &[&str]) -> Output {
    Command::new(executable)
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(arguments)
        .output()
        .unwrap()
}

fn both(done: &Output) -> String {
    format!(
        "exit {:?}\nstdout: {}\nstderr: {}",
        done.status.code(),
        String::from_utf8_lossy(&done.stdout),
        String::from_utf8_lossy(&done.stderr)
    )
}

fn stderr(done: &Output) -> String {
    String::from_utf8_lossy(&done.stderr).into_owned()
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    let done = Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "-C",
        ])
        .arg(directory)
        .args(arguments)
        .output()
        .unwrap();
    assert!(done.status.success(), "{}", both(&done));
    String::from_utf8(done.stdout).unwrap().trim().to_string()
}

/// The grade corpora's pending fixture root with its target's revision moved to a real source repository and its
/// provisioning given a fixture cache, mirrored and built, and a replacement manifest naming that build.
struct Replacements {
    root: PathBuf,
    executable: PathBuf,
    directory: PathBuf,
    target_path: PathBuf,
    original: Vec<u8>,
    cache: PathBuf,
    receipt: PathBuf,
    manifest: PathBuf,
    document: Value,
    work: PathBuf,
    key: PathBuf,
}

impl Drop for Replacements {
    fn drop(&mut self) {
        let _ = Command::new("chmod")
            .arg("-R")
            .arg("u+w")
            .arg(&self.root)
            .output();
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Replacements {
    /// None where bubblewrap, which the cache build runs under, is missing.
    fn new(name: &str) -> Option<Self> {
        Command::new("bwrap").arg("--version").output().ok()?;
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("cache-replacements-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let executable = install(&root);
        for (path, bytes) in grading_fixture::build(Variant::Pending).files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        let directory = root.join("bench/targets").join(TARGET);
        let source = directory.join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("main.go"), "package main\n").unwrap();
        git(&source, &["init", "-q", "-b", "main"]);
        git(&source, &["add", "-A"]);
        git(&source, &["commit", "-qm", "fixture"]);
        let head = git(&source, &["rev-parse", "HEAD"]);
        let identity = bench_tools(
            &executable,
            &["diff-identity", "--quiet", s(&source), "HEAD", "HEAD"],
        );
        assert!(identity.status.success(), "{}", both(&identity));
        let identity = String::from_utf8(identity.stdout)
            .unwrap()
            .trim()
            .to_string();
        for name in [
            format!("bench/targets/{TARGET}/target.json"),
            "bench/scoreboard.current.json".to_string(),
            format!("bench/runs/{RUN}/manifest.json"),
            "bench/grading/current/references.json".to_string(),
        ] {
            let path = root.join(name);
            let pinned = fs::read_to_string(&path)
                .unwrap()
                .replace(PENDING_HEAD, &head)
                .replace(PENDING_DIFF, &identity);
            fs::write(path, pinned).unwrap();
        }
        let target_path = directory.join("target.json");
        let mut target = read(&target_path);
        set(
            &mut target,
            "provisioning.dependency_identity",
            parse(&format!(
                r#"[{{"name": "cache archive (fixture)", "sha256": "{}"}}]"#,
                "0".repeat(64)
            )),
        );
        set(
            &mut target,
            "provisioning.cache",
            parse(
                r#"{"kind": "fixture", "build": ["cp main.go {cache}/source"], "post_clone": [], "env": {}, "smoke": []}"#,
            ),
        );
        let indented = Dumps {
            indent: Some(2),
            ..Dumps::default()
        };
        fs::write(&target_path, pyjson::dumps(&target, &indented) + "\n").unwrap();
        let inventory = root.join("bench/grading/current/inventory.json");
        let done = bench_tools(
            &executable,
            &[
                "current-grading",
                "inventory",
                "--root",
                s(&root),
                "--out",
                s(&inventory),
            ],
        );
        assert!(done.status.success(), "{}", both(&done));
        let cache = root.join("cache");
        let mut fixture = Self {
            original: fs::read(&target_path).unwrap(),
            receipt: cache.join("caches").join(format!("{TARGET}.json")),
            manifest: root.join("replacements.json"),
            document: Value::Null,
            work: root.join("work"),
            key: root.join("keys/key.json"),
            target_path,
            cache,
            directory,
            executable,
            root,
        };
        for command in [&["mirror", "--staging", s(&source)][..], &["cache"]] {
            let done = fixture.provision(command);
            assert!(done.status.success(), "{}", both(&done));
        }
        let archive = text(field(&read(&fixture.receipt), "archive.sha256"));
        let receipt_path = fixture
            .receipt
            .strip_prefix(&fixture.root)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        fixture.document = parse(&format!(
            r#"{{"schema_version": 1, "version": 1, "reason": "Original archive deleted.", "targets": [{{"target": "{TARGET}", "target_sha256": "{}", "original_sha256": "{}", "sha256": "{archive}", "build_receipt": {{"path": "{receipt_path}", "sha256": "{}"}}}}]}}"#,
            digest(&fixture.target_path),
            "0".repeat(64),
            digest(&fixture.receipt)
        ));
        fixture.save();
        Some(fixture)
    }

    fn save(&self) {
        fs::write(&self.manifest, dumps(&self.document)).unwrap();
    }

    fn provision(&self, arguments: &[&str]) -> Output {
        let mut command = vec!["provision", arguments[0], "--target", s(&self.directory)];
        command.extend(&arguments[1..]);
        command.extend(["--cache-root", s(&self.cache)]);
        bench_tools(&self.executable, &command)
    }

    /// Why `provision check` refuses the target with the replacement manifest.
    fn refusal(&self) -> String {
        let done = self.provision(&["check", "--cache-replacements", s(&self.manifest)]);
        assert_ne!(done.status.code(), Some(0), "{}", both(&done));
        both(&done)
    }

    fn archive(&self) -> PathBuf {
        self.cache.join("archives").join(format!("{TARGET}.tar.gz"))
    }

    fn prepare(&self, extra: &[&str]) -> Output {
        let run = format!("runs/{RUN}");
        let mut command = vec![
            "grade",
            "prepare",
            "--root",
            s(&self.root),
            "--run",
            &run,
            "--target",
            TARGET,
            "--work",
            s(&self.work),
            "--key",
            s(&self.key),
            "--cache-root",
            s(&self.cache),
        ];
        command.extend(extra);
        bench_tools(&self.executable, &command)
    }

    fn check_prepared(&self, extra: &[&str]) -> Output {
        let mut command = vec![
            "grade",
            "check-prepared",
            "--work",
            s(&self.work),
            "--key",
            s(&self.key),
        ];
        command.extend(extra);
        bench_tools(&self.executable, &command)
    }

    fn target_entry(&self) -> &Value {
        &field(&self.document, "targets").as_array().unwrap()[0]
    }

    fn targets(&mut self) -> &mut Vec<Value> {
        match &mut self.document {
            Value::Object(document) => match document.get_mut("targets") {
                Some(Value::Array(targets)) => targets,
                other => panic!("targets is {other:?}"),
            },
            other => panic!("the manifest is {other:?}"),
        }
    }
}

#[test]
fn replacement_restores_without_changing_frozen_target_and_pins_private_provenance() {
    let Some(f) = Replacements::new("restores") else {
        return;
    };
    let refused = f.prepare(&[]);
    assert_ne!(refused.status.code(), Some(0), "{}", both(&refused));
    assert!(!f.key.exists());
    assert!(fs::read_dir(&f.work).map_or(true, |mut entries| entries.next().is_none()));
    let done = f.prepare(&["--cache-replacements", s(&f.manifest)]);
    assert!(done.status.success(), "{}", both(&done));
    assert_eq!(
        fs::read_to_string(f.work.join("clone-cache/source")).unwrap(),
        "package main\n"
    );
    assert_eq!(fs::read(&f.target_path).unwrap(), f.original);
    let key = read(&f.key);
    let snapshot = field(&key, "runner_deviation.provisioning");
    assert_eq!(
        text(field(snapshot, "manifest.sha256")),
        digest(&f.manifest)
    );
    assert_eq!(
        canonical(&parse(&text(field(snapshot, "manifest_text")))),
        canonical(&f.document)
    );
    assert_eq!(
        text(field(
            &parse(&text(field(snapshot, "build_receipt_text"))),
            "archive.sha256"
        )),
        text(field(f.target_entry(), "sha256"))
    );
    let checked = f.check_prepared(&["--dispatching"]);
    assert!(checked.status.success(), "{}", both(&checked));
    fs::write(&f.manifest, dumps(&f.document) + "\n").unwrap();
    let changed = f.check_prepared(&["--dispatching"]);
    assert_ne!(changed.status.code(), Some(0), "{}", both(&changed));
    assert!(
        both(&changed).contains("cache replacement changed after preparation"),
        "{}",
        both(&changed)
    );
    let undispatched = f.check_prepared(&[]);
    assert_eq!(
        undispatched.status.code(),
        Some(0),
        "{}",
        both(&undispatched)
    );
    assert!(
        !fs::read_to_string(f.work.join("prompt.md"))
            .unwrap()
            .contains(s(&f.manifest))
    );
}

#[test]
fn missing_duplicate_or_scope_expanding_replacements_are_refused() {
    let Some(mut f) = Replacements::new("refused") else {
        return;
    };
    let entry = f.target_entry().clone();
    for entries in [vec![], vec![entry.clone(), entry.clone()]] {
        *f.targets() = entries;
        f.save();
        f.refusal();
    }
    let mut expanded = entry;
    set(&mut expanded, "allowance", Value::Str("network".into()));
    *f.targets() = vec![expanded];
    f.save();
    let refusal = f.refusal();
    assert!(refusal.contains("unexpected key"), "{refusal}");
}

#[test]
fn frozen_target_receipt_and_archive_tampering_are_refused() {
    let Some(f) = Replacements::new("tampered") else {
        return;
    };
    fs::write(&f.target_path, [f.original.as_slice(), b"\n"].concat()).unwrap();
    let refusal = f.refusal();
    assert!(refusal.contains("frozen target changed"), "{refusal}");
    fs::write(&f.target_path, &f.original).unwrap();
    let saved = fs::read(&f.receipt).unwrap();
    fs::write(&f.receipt, [saved.as_slice(), b"\n"].concat()).unwrap();
    let refusal = f.refusal();
    assert!(refusal.contains("build receipt changed"), "{refusal}");
    fs::write(&f.receipt, &saved).unwrap();
    let archive = f.archive();
    let bytes = fs::read(&archive).unwrap();
    fs::write(&archive, [bytes.as_slice(), b"changed"].concat()).unwrap();
    let done = f.prepare(&["--cache-replacements", s(&f.manifest)]);
    assert_ne!(done.status.code(), Some(0), "{}", both(&done));
    assert!(stderr(&done).contains("cache archive"), "{}", both(&done));
    assert!(!f.key.exists());
}

#[test]
fn different_recipe_or_failed_build_cannot_supply_a_replacement() {
    let Some(mut f) = Replacements::new("recipe") else {
        return;
    };
    let original = read(&f.receipt);
    for (member, value) in [
        ("command", Value::Str("download something else".into())),
        ("exit_code", Value::Int("1".into())),
    ] {
        let mut receipt = original.clone();
        set(&mut receipt, &format!("build.0.{member}"), value);
        fs::write(&f.receipt, dumps(&receipt)).unwrap();
        let receipt_sha256 = digest(&f.receipt);
        set(
            &mut f.targets()[0],
            "build_receipt.sha256",
            Value::Str(receipt_sha256),
        );
        f.save();
        let refusal = f.refusal();
        assert!(refusal.contains("frozen recipe"), "{member}: {refusal}");
    }
}

#[test]
fn receipt_paths_cannot_leave_manifest_directory() {
    let Some(mut f) = Replacements::new("outside") else {
        return;
    };
    set(
        &mut f.targets()[0],
        "build_receipt.path",
        Value::Str("../receipt.json".into()),
    );
    f.save();
    let refusal = f.refusal();
    assert!(refusal.contains("relative to"), "{refusal}");
}

#[test]
fn replacement_smoke_refuses_missing_or_frozen_output_before_work() {
    let Some(f) = Replacements::new("smoke-refused") else {
        return;
    };
    let frozen = f.directory.join("smoke.json");
    fs::write(&frozen, "{\"source\": \"recorded\"}\n").unwrap();
    let original = fs::read(&frozen).unwrap();
    let alias = f.root.join("smoke-alias.json");
    symlink(&frozen, &alias).unwrap();
    let scratch = f.cache.join("scratch").join(format!("{TARGET}-smoke"));
    fs::create_dir_all(&scratch).unwrap();
    let marker = scratch.join("preserve");
    fs::write(&marker, "existing scratch").unwrap();
    for output in [vec![], vec!["--out", s(&frozen)], vec!["--out", s(&alias)]] {
        let mut arguments = vec!["smoke", "--cache-replacements", s(&f.manifest)];
        arguments.extend(&output);
        let done = f.provision(&arguments);
        assert_eq!(done.status.code(), Some(2), "{output:?}: {}", both(&done));
        assert!(
            stderr(&done).contains("frozen"),
            "{output:?}: {}",
            both(&done)
        );
        assert_eq!(fs::read(&frozen).unwrap(), original);
        assert_eq!(fs::read_to_string(&marker).unwrap(), "existing scratch");
    }
}

#[test]
fn replacement_smoke_preserves_frozen_evidence_and_names_consumed_manifest() {
    let Some(f) = Replacements::new("smoke") else {
        return;
    };
    let frozen = f.directory.join("smoke.json");
    fs::write(&frozen, "{\"source\": \"recorded\"}\n").unwrap();
    let original = fs::read(&frozen).unwrap();
    let output = f.root.join("replacement-smoke.json");
    let done = f.provision(&[
        "smoke",
        "--cache-replacements",
        s(&f.manifest),
        "--out",
        s(&output),
    ]);
    assert!(done.status.success(), "{}", both(&done));
    let smoke = read(&output);
    assert!(matches!(
        field(&smoke, "provisioning.tree_clean_after"),
        Value::Bool(true)
    ));
    let archive = text(field(f.target_entry(), "sha256"));
    assert!(
        text(field(&smoke, "provisioning.recipe")).contains(&archive),
        "{smoke:?}"
    );
    let named = digest(&f.manifest);
    assert!(
        field(&smoke, "notes")
            .as_array()
            .unwrap()
            .iter()
            .map(text)
            .any(|note| note.contains(s(&f.manifest)) && note.contains(&named)),
        "{smoke:?}"
    );
    assert_eq!(fs::read(&frozen).unwrap(), original);
    assert_eq!(fs::read(&f.target_path).unwrap(), f.original);
}

#[test]
fn offline_queue_checks_real_inputs_without_client_or_credentials() {
    let Some(f) = Replacements::new("preflight") else {
        return;
    };
    let run = format!("runs/{RUN}");
    let neutral = f.root.join("neutral");
    let (work_root, key_root) = (neutral.join("work"), neutral.join("keys"));
    let preflight = |command: &str, extra: &[&str]| {
        let mut arguments = vec![
            "grade",
            command,
            "--root",
            s(&f.root),
            "--run",
            &run,
            "--work-root",
            s(&work_root),
            "--key-root",
            s(&key_root),
            "--cache-root",
            s(&f.cache),
            "--cache-replacements",
            s(&f.manifest),
        ];
        arguments.extend(extra);
        bench_tools(&f.executable, &arguments)
    };
    let done = preflight("preflight-inputs", &[]);
    assert!(done.status.success(), "{}", both(&done));
    assert!(!neutral.exists());
    let done = preflight("preflight", &["--expected-cli-version", "9.9.9"]);
    assert_eq!(done.status.code(), Some(2), "{}", both(&done));
    assert!(stderr(&done).contains("--model"), "{}", both(&done));
    fs::remove_file(f.archive()).unwrap();
    let done = preflight("preflight-inputs", &[]);
    assert_ne!(done.status.code(), Some(0), "{}", both(&done));
    assert!(
        both(&done).contains("archive missing or changed"),
        "{}",
        both(&done)
    );
}

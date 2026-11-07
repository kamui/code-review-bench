//! `bench-tools runtime` and `bench/tools/bench-tools` against a disposable repository whose `rust/bench-tools` is a
//! small crate built by the active cargo, and a store under the target's temporary directory. Nothing outside it is
//! written.

#[path = "support/runtime_store.rs"]
mod runtime_store;

use bench_tools::sha256::hex_digest;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const CHANNEL: &str = include_str!("../rust-toolchain.toml");

/// A repository holding a `bench-tools` crate that prints its version word and arguments, exits with its first
/// argument, and answers `runtime --help` as a launcher must.
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    store: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn main_rs(word: &str) -> String {
    format!(
        r#"fn main() {{
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments == ["runtime", "--help"] {{
        return;
    }}
    println!("{word} {{}}", arguments.join(" "));
    std::process::exit(arguments.first().and_then(|first| first.parse().ok()).unwrap_or(0));
}}
"#
    )
}

fn text(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The `"name": "value"` string field of printed JSON.
fn json_field(output: &str, name: &str) -> String {
    let key = format!("\"{name}\": \"");
    let start = output
        .find(&key)
        .unwrap_or_else(|| panic!("no {name} in {output}"))
        + key.len();
    output[start..start + output[start..].find('"').unwrap()].to_string()
}

impl Fixture {
    fn new(label: &str) -> Self {
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("runtime-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let root = fs::canonicalize(&root).unwrap();
        let repo = root.join("repo");
        let fixture = Self {
            store: root.join("store"),
            repo,
            root,
        };
        let crate_dir = fixture.crate_dir();
        fs::create_dir_all(crate_dir.join("src")).unwrap();
        fixture.git(&["init", "-q"]);
        fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"bench-tools\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        fs::write(crate_dir.join("rust-toolchain.toml"), CHANNEL).unwrap();
        fs::write(crate_dir.join(".gitignore"), "/target/\n").unwrap();
        fs::write(crate_dir.join("src/main.rs"), main_rs("one")).unwrap();
        let lock = Command::new("cargo")
            .args(["generate-lockfile", "--offline"])
            .current_dir(&crate_dir)
            .output()
            .unwrap();
        assert!(lock.status.success(), "{}", text(&lock));
        fixture.commit("one");
        fixture
    }

    fn crate_dir(&self) -> PathBuf {
        self.repo.join("rust/bench-tools")
    }

    fn git(&self, arguments: &[&str]) -> String {
        let done = Command::new("git")
            .arg("-C")
            .arg(&self.repo)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(arguments)
            .output()
            .unwrap();
        assert!(done.status.success(), "git {arguments:?}: {}", text(&done));
        stdout(&done).trim().to_string()
    }

    fn commit(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-qm", message]);
    }

    fn version(&self, word: &str) {
        fs::write(self.crate_dir().join("src/main.rs"), main_rs(word)).unwrap();
        self.commit(word);
    }

    /// `bench-tools runtime ARGUMENTS` from the repository, with no launch variables.
    fn runtime(&self, arguments: &[&str]) -> Output {
        self.runtime_with(arguments, &[])
    }

    fn runtime_with(&self, arguments: &[&str], environment: &[(&str, &str)]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .arg("runtime")
            .args(arguments)
            .current_dir(&self.repo)
            .env("BENCH_RUNTIME_STORE", &self.store)
            .env_remove("BENCH_RUST_BENCH_TOOLS")
            .env_remove("BENCH_RUST_BENCH_TOOLS_SHA256")
            .envs(environment.iter().copied())
            .output()
            .unwrap()
    }

    /// Install the committed crate and return its SHA-256.
    fn install(&self, select: bool) -> String {
        let arguments: &[&str] = if select {
            &["install", "--select"]
        } else {
            &["install"]
        };
        let done = self.runtime(arguments);
        assert!(done.status.success(), "{}", text(&done));
        json_field(&stdout(&done), "sha256")
    }

    fn entries(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.store.join("bench-tools"))
            .map(|entries| {
                entries
                    .flatten()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .filter(|name| name.len() == 64)
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    fn run_dir(&self, name: &str, pins: Option<&str>) -> PathBuf {
        let run = self.root.join("runs").join(name);
        fs::create_dir_all(&run).unwrap();
        fs::write(
            run.join("manifest.json"),
            r#"{"freeze_commit": "1234567890abcdef1234567890abcdef12345678"}"#,
        )
        .unwrap();
        if let Some(pins) = pins {
            fs::write(run.join("runtimes.frozen.json"), pins).unwrap();
        }
        run
    }

    fn exec(&self, run: Option<&Path>, arguments: &[&str]) -> Output {
        let mut all = vec!["exec"];
        let shown = run.map(|run| run.to_string_lossy().into_owned());
        if let Some(run) = &shown {
            all.extend(["--run", run]);
        }
        all.push("--");
        all.extend(arguments);
        self.runtime(&all)
    }

    /// Commit this checkout's `bench/tools/bench-tools` into the repository and return a way to run it.
    fn launcher_script(&self) -> impl Fn(&[&str]) -> Output + '_ {
        let script = self.repo.join("bench/tools/bench-tools");
        fs::create_dir_all(script.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/tools/bench-tools"),
            &script,
        )
        .unwrap();
        self.commit("the launcher");
        move |arguments| {
            Command::new(&script)
                .args(arguments)
                .current_dir(&self.root)
                .env("BENCH_RUNTIME_STORE", &self.store)
                .env_remove("BENCH_RUST_BENCH_TOOLS")
                .env_remove("BENCH_RUST_BENCH_TOOLS_SHA256")
                .output()
                .unwrap()
        }
    }
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// Retain and select `executable` in `store` by running it, as a store with no launcher yet takes its first entry.
fn restore_and_select(executable: &Path, sha: &str, receipt: &Path, store: &Path) {
    for arguments in [
        vec![
            "restore".as_ref(),
            "--receipt".as_ref(),
            receipt.as_os_str(),
            "--executable".as_ref(),
            executable.as_os_str(),
        ],
        vec!["select".as_ref(), std::ffi::OsStr::new(sha)],
    ] {
        let done = Command::new(executable)
            .arg("runtime")
            .args(&arguments)
            .env("BENCH_RUNTIME_STORE", store)
            .output()
            .unwrap();
        assert!(done.status.success(), "{}", text(&done));
    }
}

#[test]
fn install_retains_read_only_entries_and_rollback_returns_to_the_replaced_selection() {
    let fixture = Fixture::new("rollback");
    let refused = fixture.runtime(&["rollback"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(stderr(&refused).contains("no previously selected version"));

    let one = fixture.install(true);
    let entry = fixture.store.join("bench-tools").join(&one);
    assert_eq!(mode(&entry.join("bench-tools")), 0o555);
    assert_eq!(mode(&entry.join("receipt.json")), 0o444);
    let receipt = fs::read_to_string(entry.join("receipt.json")).unwrap();
    assert!(
        !receipt.contains(fixture.root.to_str().unwrap()),
        "{receipt}"
    );
    assert!(receipt.contains(&fixture.git(&["rev-parse", "HEAD:rust/bench-tools"])));
    assert_eq!(
        fs::read_link(fixture.store.join("bench-tools/launcher")).unwrap(),
        Path::new(&one).join("bench-tools")
    );
    let again = fixture.runtime(&["install"]);
    assert!(
        stdout(&again).contains("\"added\": false"),
        "{}",
        text(&again)
    );

    fixture.version("two");
    let two = fixture.install(true);
    assert_ne!(one, two);
    assert_eq!(fixture.entries().len(), 2);
    let pinned = fixture.runtime(&["pin"]);
    assert_eq!(json_field(&stdout(&pinned), "sha256"), two);

    let rolled = fixture.runtime(&["rollback"]);
    assert!(rolled.status.success(), "{}", text(&rolled));
    assert!(stdout(&rolled).contains("\"action\": \"rollback\""));
    assert_eq!(
        json_field(&stdout(&fixture.runtime(&["pin"])), "sha256"),
        one
    );
    for (sha, word) in [(&one, "one"), (&two, "two")] {
        let ran = Command::new(
            fixture
                .store
                .join("bench-tools")
                .join(sha)
                .join("bench-tools"),
        )
        .output()
        .unwrap();
        assert_eq!(stdout(&ran), format!("{word} \n"));
        let verified = fixture.runtime(&["verify", sha]);
        assert!(verified.status.success(), "{}", text(&verified));
    }
    let history = fs::read_to_string(fixture.store.join("bench-tools/selections.jsonl")).unwrap();
    assert_eq!(history.lines().count(), 3, "{history}");
}

#[test]
fn the_selection_runs_only_while_its_crate_tree_is_the_one_at_head() {
    let fixture = Fixture::new("head");
    let missing = fixture.exec(None, &["3"]);
    assert_eq!(missing.status.code(), Some(1), "{}", text(&missing));
    assert!(
        stderr(&missing)
            .contains("install it with: bench/tools/bench-tools runtime install --select"),
        "{}",
        text(&missing)
    );

    fixture.install(true);
    let ran = fixture.exec(None, &["7", "--run", "x"]);
    assert_eq!(ran.status.code(), Some(7), "{}", text(&ran));
    assert_eq!(stdout(&ran), "one 7 --run x\n");
    let resolved = fixture.runtime(&["resolve"]);
    assert!(resolved.status.success(), "{}", text(&resolved));

    fixture.version("two");
    let stale = fixture.exec(None, &["0"]);
    assert_eq!(stale.status.code(), Some(1), "{}", text(&stale));
    assert!(
        stderr(&stale).contains("was built from crate tree"),
        "{}",
        text(&stale)
    );
    assert!(stderr(&stale).contains("runtime install --select"));
}

#[test]
fn a_run_executes_exactly_its_frozen_pin_and_refuses_a_run_that_pins_none() {
    let fixture = Fixture::new("frozen");
    fixture.install(true);
    let pin_one = stdout(&fixture.runtime(&["pin"]));
    fixture.version("two");
    fixture.install(true);

    let frozen = fixture.run_dir("frozen", Some(&pin_one));
    let ran = fixture.exec(Some(&frozen), &["0", "a"]);
    assert_eq!(stdout(&ran), "one 0 a\n", "{}", text(&ran));

    for (name, pins) in [
        ("unpinned", None),
        ("other-runtime", Some("{\"other\": {}}")),
    ] {
        let run = fixture.run_dir(name, pins);
        let refused = fixture.exec(Some(&run), &["0"]);
        assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
        assert!(
            stderr(&refused).contains(&format!(
                "run {name} pins no bench-tools runtime; run it from a checkout of its freeze commit \
                 1234567890abcdef1234567890abcdef12345678"
            )),
            "{}",
            text(&refused)
        );
        assert_eq!(stdout(&refused), "");
    }

    let changed = pin_one.replace(&json_field(&pin_one, "receipt_sha256"), &"0".repeat(64));
    let run = fixture.run_dir("changed", Some(&changed));
    let refused = fixture.exec(Some(&run), &["0"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(stderr(&refused).contains("no longer hashes to its pin"));
}

#[test]
fn a_launch_variable_is_used_only_with_its_matching_hash() {
    let fixture = Fixture::new("launch");
    let sha = fixture.install(false);
    let executable = fixture
        .store
        .join("bench-tools")
        .join(&sha)
        .join("bench-tools");
    let path = executable.to_str().unwrap();
    for (environment, expected) in [
        (
            vec![("BENCH_RUST_BENCH_TOOLS", path)],
            "must be set together",
        ),
        (
            vec![
                ("BENCH_RUST_BENCH_TOOLS", path),
                ("BENCH_RUST_BENCH_TOOLS_SHA256", &"0".repeat(64)),
            ],
            "does not hash to BENCH_RUST_BENCH_TOOLS_SHA256",
        ),
        (
            vec![
                ("BENCH_RUST_BENCH_TOOLS", "bench-tools"),
                ("BENCH_RUST_BENCH_TOOLS_SHA256", &sha),
            ],
            "not the absolute path of an executable file",
        ),
    ] {
        let refused = fixture.runtime_with(&["exec", "--", "0"], &environment);
        assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
        assert!(stderr(&refused).contains(expected), "{}", text(&refused));
    }
    let ran = fixture.runtime_with(
        &["exec", "--", "0"],
        &[
            ("BENCH_RUST_BENCH_TOOLS", path),
            ("BENCH_RUST_BENCH_TOOLS_SHA256", &sha),
        ],
    );
    assert_eq!(stdout(&ran), "one 0\n", "{}", text(&ran));
}

#[test]
fn an_entry_is_never_replaced_and_a_wrong_hash_is_never_retained() {
    let fixture = Fixture::new("restore");
    let sha = fixture.install(false);
    let entry = fixture.store.join("bench-tools").join(&sha);
    let receipt = fs::read_to_string(entry.join("receipt.json")).unwrap();
    let copies = fixture.root.join("copies");
    fs::create_dir_all(&copies).unwrap();
    let copy = |name: &str, content: &str| {
        let path = copies.join(name);
        fs::write(&path, content).unwrap();
        path.to_string_lossy().into_owned()
    };
    let executable = entry.join("bench-tools").to_string_lossy().into_owned();

    let rebuilt = copy(
        "rebuilt.json",
        &receipt.replace("\"built_at\": \"", "\"built_at\": \"1"),
    );
    let kept = fixture.runtime(&[
        "restore",
        "--receipt",
        &rebuilt,
        "--executable",
        &executable,
    ]);
    assert!(
        stdout(&kept).contains("\"added\": false"),
        "{}",
        text(&kept)
    );

    let other = copy(
        "other.json",
        &receipt.replace("\"cargo\": \"cargo ", "\"cargo\": \"other "),
    );
    let refused = fixture.runtime(&["restore", "--receipt", &other, "--executable", &executable]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(stderr(&refused).contains("entries are never replaced"));

    let wrong = copy("wrong-bytes", "#!/bin/sh\nexit 3\n");
    let original = copy("receipt.json", &receipt);
    let refused = fixture.runtime(&["restore", "--receipt", &original, "--executable", &wrong]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(stderr(&refused).contains("does not hash to the receipt's"));
    assert_eq!(fixture.entries(), std::slice::from_ref(&sha));
    assert_eq!(
        fs::read_to_string(entry.join("receipt.json")).unwrap(),
        receipt
    );

    // Another store takes the copied entry; selecting it makes it the launcher, and a selected stand-in without these
    // commands never becomes the launcher.
    let elsewhere = Fixture {
        root: fixture.root.join("elsewhere"),
        repo: fixture.repo.clone(),
        store: fixture.root.join("elsewhere/store"),
    };
    let restored = elsewhere.runtime(&[
        "restore",
        "--receipt",
        &original,
        "--executable",
        &executable,
    ]);
    assert!(restored.status.success(), "{}", text(&restored));
    let selected = elsewhere.runtime(&["select", &sha]);
    assert!(selected.status.success(), "{}", text(&selected));
    let script = "#!/bin/sh\nexit 3\n";
    let stand_in_sha = hex_digest(script.as_bytes());
    let stand_in = copy("stand-in", script);
    fs::set_permissions(&stand_in, fs::Permissions::from_mode(0o755)).unwrap();
    let stand_in_receipt = copy(
        "stand-in.json",
        &runtime_store::receipt(&stand_in_sha, script.len() as u64),
    );
    let retained = elsewhere.runtime(&[
        "restore",
        "--receipt",
        &stand_in_receipt,
        "--executable",
        &stand_in,
    ]);
    assert!(retained.status.success(), "{}", text(&retained));
    let selected = elsewhere.runtime(&["select", &stand_in_sha]);
    assert!(selected.status.success(), "{}", text(&selected));
    assert_eq!(
        fs::read_link(elsewhere.store.join("bench-tools/launcher")).unwrap(),
        Path::new(&sha).join("bench-tools")
    );
}

#[test]
fn install_refuses_what_a_receipt_cannot_restore() {
    let fixture = Fixture::new("refusals");
    let crate_dir = fixture.crate_dir();
    let refuses = |expected: &str, environment: &[(&str, &str)]| {
        let done = fixture.runtime_with(&["install", "--select"], environment);
        assert_eq!(done.status.code(), Some(1), "{expected}: {}", text(&done));
        assert!(
            stderr(&done).contains(expected),
            "{expected}: {}",
            text(&done)
        );
        assert_eq!(fixture.entries(), [] as [String; 0]);
    };

    fs::write(crate_dir.join("src/extra.rs"), "\n").unwrap();
    refuses("uncommitted or untracked files", &[]);
    fs::remove_file(crate_dir.join("src/extra.rs")).unwrap();

    for name in ["HOME", "CARGO_HOME", "RUSTUP_HOME", "TMPDIR"] {
        refuses(
            &format!("{name} must be absolute"),
            &[(name, "relative-home")],
        );
    }

    fs::create_dir_all(crate_dir.join(".cargo")).unwrap();
    fs::write(
        crate_dir.join(".cargo/config.toml"),
        "[profile.release]\ndebug-assertions = true\n",
    )
    .unwrap();
    fixture.commit("a tracked cargo configuration");
    refuses(
        "cargo configuration a receipt cannot restore would apply",
        &[],
    );
    fixture.git(&["rm", "-rq", "rust/bench-tools/.cargo"]);
    fixture.commit("no cargo configuration");

    let manifest = fs::read_to_string(crate_dir.join("Cargo.toml")).unwrap();
    fs::write(
        crate_dir.join("Cargo.toml"),
        format!("{manifest}[patch.crates-io]\nserde = {{ path = \"vendor/serde\" }}\n"),
    )
    .unwrap();
    fixture.commit("a source override");
    refuses("Cargo.toml has [patch] sections", &[]);
    fs::write(crate_dir.join("Cargo.toml"), &manifest).unwrap();
    fixture.commit("no source override");

    fixture.git(&["rm", "-q", "--cached", "rust/bench-tools/Cargo.lock"]);
    fs::write(crate_dir.join(".gitignore"), "/target/\n/Cargo.lock\n").unwrap();
    fixture.commit("ignore the lockfile");
    refuses("does not track Cargo.lock", &[]);
    fs::write(crate_dir.join(".gitignore"), "/target/\n").unwrap();
    fixture.commit("track the lockfile");

    std::os::unix::fs::symlink("main.rs", crate_dir.join("src/link.rs")).unwrap();
    fixture.commit("a tracked symbolic link");
    refuses("tracks symbolic links or submodules", &[]);
    fs::remove_file(crate_dir.join("src/link.rs")).unwrap();
    fixture.commit("no symbolic link");

    fs::write(
        fixture.repo.join("rust/Cargo.toml"),
        "[workspace]\nmembers = [\"bench-tools\"]\n",
    )
    .unwrap();
    fixture.commit("a parent workspace");
    refuses("belongs to the Cargo workspace", &[]);
}

#[test]
fn a_build_that_reads_the_environment_or_an_untracked_file_is_refused() {
    let fixture = Fixture::new("inputs");
    let crate_dir = fixture.crate_dir();
    let refuses = |expected: &str| {
        let done = fixture.runtime(&["install"]);
        assert_eq!(done.status.code(), Some(1), "{expected}: {}", text(&done));
        assert!(
            stderr(&done).contains(expected),
            "{expected}: {}",
            text(&done)
        );
        assert_eq!(fixture.entries(), [] as [String; 0]);
    };
    fs::write(
        crate_dir.join("src/main.rs"),
        "fn main() { println!(\"{:?}\", option_env!(\"HOME\")); }\n",
    )
    .unwrap();
    fixture.commit("read HOME");
    refuses("read host environment variables its receipt cannot pin: HOME");

    fs::write(crate_dir.join("src/main.rs"), main_rs("one")).unwrap();
    fs::write(
        crate_dir.join("build.rs"),
        "fn main() { println!(\"cargo:rerun-if-env-changed=PATH\"); }\n",
    )
    .unwrap();
    fixture.commit("a build script that reads PATH");
    refuses("receipt cannot pin: PATH");
    fs::remove_file(crate_dir.join("build.rs")).unwrap();

    let outside = fixture.root.join("outside.txt");
    fs::write(&outside, "outside\n").unwrap();
    fs::write(
        crate_dir.join("src/main.rs"),
        format!(
            "fn main() {{ print!(\"{{}}\", include_str!({:?})); }}\n",
            outside.to_str().unwrap()
        ),
    )
    .unwrap();
    fixture.commit("include an absolute path");
    refuses("not tracked in rust/bench-tools");
}

#[test]
fn a_runtime_deviation_records_both_identities_once_and_moves_the_runs_pin() {
    let fixture = Fixture::new("deviation");
    let one = fixture.install(true);
    let pin_one = stdout(&fixture.runtime(&["pin"]));
    fixture.version("two");
    let two = fixture.install(false);
    let run = fixture.run_dir("deviating", Some(&pin_one));
    let record = |old: &str, new: &str, reason: &str| {
        fixture.runtime(&[
            "deviation",
            "--run",
            run.to_str().unwrap(),
            "--from",
            old,
            "--to",
            new,
            "--reason",
            reason,
            "--scope",
            "every cell",
            "--validation",
            "corpora",
            "--accounting",
            "none",
        ])
    };
    assert_eq!(record(&one, &two, " ").status.code(), Some(2));
    assert_eq!(record(&one, &one, "why").status.code(), Some(2));
    let stale = record(&two, &one, "why");
    assert_eq!(stale.status.code(), Some(2), "{}", text(&stale));
    assert!(stderr(&stale).contains(&format!("now pins bench-tools {one}, not {two}")));
    assert_eq!(stdout(&fixture.exec(Some(&run), &["0", "a"])), "one 0 a\n");
    let done = record(&one, &two, "a fix");
    assert!(done.status.success(), "{}", text(&done));
    let path = json_field(&stdout(&done), "recorded");
    let saved = fs::read_to_string(&path).unwrap();
    for expected in [
        "\"schema\": \"rust-runtime-deviation/1\"",
        &format!("\"sha256\": \"{one}\""),
        &format!("\"sha256\": \"{two}\""),
        "\"reason\": \"a fix\"",
    ] {
        assert!(saved.contains(expected), "{expected}: {saved}");
    }
    let moved = fixture.exec(Some(&run), &["0", "a"]);
    assert_eq!(stdout(&moved), "two 0 a\n", "{}", text(&moved));
    let again = record(&one, &two, "a fix");
    assert_eq!(again.status.code(), Some(2), "{}", text(&again));
    assert!(stderr(&again).contains(&format!("now pins bench-tools {two}, not {one}")));
    let back = record(&two, &one, "roll back");
    assert!(back.status.success(), "{}", text(&back));
    assert_eq!(stdout(&fixture.exec(Some(&run), &["0", "a"])), "one 0 a\n");
    let again = record(&one, &two, "a fix");
    assert_eq!(again.status.code(), Some(1), "{}", text(&again));
    assert!(stderr(&again).contains("a record is never overwritten"));
}

#[test]
fn the_launcher_script_names_the_install_step_and_runs_a_runs_pin_through_the_store() {
    let fixture = Fixture::new("script");
    let launch = fixture.launcher_script();
    let empty = launch(&["roster"]);
    assert_eq!(empty.status.code(), Some(1), "{}", text(&empty));
    assert!(
        stderr(&empty).contains(
            "has no installed bench-tools; install it with: bench/tools/bench-tools runtime install --select"
        ),
        "{}",
        text(&empty)
    );

    let (built, sha) = runtime_store::built();
    let receipt = fixture.root.join("receipt.json");
    let size = fs::metadata(built).unwrap().len();
    fs::write(&receipt, runtime_store::receipt(sha, size)).unwrap();
    // A store with no launcher yet takes and selects an entry through the copied executable itself.
    restore_and_select(built, sha, &receipt, &fixture.store);
    let selected = launch(&["runtime", "select", sha]);
    assert!(selected.status.success(), "{}", text(&selected));

    let stale = launch(&["roster"]);
    assert_eq!(stale.status.code(), Some(1), "{}", text(&stale));
    assert!(
        stderr(&stale).contains("was built from crate tree"),
        "{}",
        text(&stale)
    );

    let pins = stdout(&launch(&["runtime", "pin"]));
    let run = fixture.run_dir("pinned", Some(&pins));
    let listed = launch(&["--run", run.to_str().unwrap(), "--", "runtime", "list"]);
    assert!(listed.status.success(), "{}", text(&listed));
    assert!(stdout(&listed).contains(sha.as_str()), "{}", text(&listed));
    let unpinned = fixture.run_dir("unpinned", None);
    let refused = launch(&["--run", unpinned.to_str().unwrap(), "runtime", "list"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(stderr(&refused).contains("pins no bench-tools runtime"));
}

fn linked(fixture: &Fixture) -> PathBuf {
    fs::read_link(fixture.store.join("bench-tools/launcher")).unwrap()
}

#[test]
fn an_install_without_select_leaves_the_launcher_and_a_selection_moves_it() {
    let fixture = Fixture::new("install-launcher");
    let one = fixture.install(true);
    assert_eq!(linked(&fixture), Path::new(&one).join("bench-tools"));

    fixture.version("two");
    let installed = fixture.runtime(&["install"]);
    assert!(installed.status.success(), "{}", text(&installed));
    assert!(
        stdout(&installed).contains("\"launcher\": false"),
        "{}",
        text(&installed)
    );
    let two = json_field(&stdout(&installed), "sha256");
    assert_eq!(linked(&fixture), Path::new(&one).join("bench-tools"));

    let selected = fixture.runtime(&["select", &two]);
    assert!(selected.status.success(), "{}", text(&selected));
    assert_eq!(linked(&fixture), Path::new(&two).join("bench-tools"));
    let rolled = fixture.runtime(&["rollback"]);
    assert!(rolled.status.success(), "{}", text(&rolled));
    assert_eq!(linked(&fixture), Path::new(&one).join("bench-tools"));
}

#[test]
fn a_restored_entry_is_not_the_launcher_until_it_is_selected() {
    let fixture = Fixture::new("restore-launcher");
    let one = fixture.install(true);
    fixture.version("two");
    let elsewhere = Fixture {
        root: fixture.root.join("elsewhere"),
        repo: fixture.repo.clone(),
        store: fixture.root.join("elsewhere/store"),
    };
    let two = elsewhere.install(false);
    let entry = elsewhere.store.join("bench-tools").join(&two);
    let restored = fixture.runtime(&[
        "restore",
        "--receipt",
        entry.join("receipt.json").to_str().unwrap(),
        "--executable",
        entry.join("bench-tools").to_str().unwrap(),
    ]);
    assert!(restored.status.success(), "{}", text(&restored));
    assert_eq!(linked(&fixture), Path::new(&one).join("bench-tools"));

    let selected = fixture.runtime(&["select", &two]);
    assert!(selected.status.success(), "{}", text(&selected));
    assert_eq!(linked(&fixture), Path::new(&two).join("bench-tools"));
}

#[test]
fn a_frozen_run_and_rollback_never_run_on_an_unselected_entry() {
    let fixture = Fixture::new("unselected");
    let launch = fixture.launcher_script();
    let (built, sha) = runtime_store::built();
    let receipt = fixture.root.join("receipt.json");
    fs::write(
        &receipt,
        runtime_store::receipt(sha, fs::metadata(built).unwrap().len()),
    )
    .unwrap();
    restore_and_select(built, sha, &receipt, &fixture.store);

    // The fixture crate answers `runtime --help`, so an unselected entry of it could pass for a launcher.
    let installed = fixture.runtime(&["install"]);
    assert!(installed.status.success(), "{}", text(&installed));
    let output = stdout(&installed);
    let pins = format!(
        r#"{{"bench-tools": {{"receipt_sha256": "{}", "sha256": "{}"}}}}"#,
        json_field(&output, "receipt_sha256"),
        json_field(&output, "sha256")
    );
    let run = fixture.run_dir("frozen", Some(&pins));
    let ran = launch(&["--run", run.to_str().unwrap(), "--", "0", "a"]);
    assert_eq!(ran.status.code(), Some(0), "{}", text(&ran));
    assert_eq!(stdout(&ran), "one 0 a\n", "{}", text(&ran));

    let rolled = launch(&["runtime", "rollback"]);
    assert_eq!(rolled.status.code(), Some(1), "{}", text(&rolled));
    assert!(
        stderr(&rolled).contains("no previously selected version"),
        "{}",
        text(&rolled)
    );
    assert_eq!(stdout(&rolled), "");
}

/// The `"name": {...}` object field of a selection row, or `null`.
fn row_object(row: &str, name: &str) -> String {
    let key = format!("\"{name}\": ");
    let start = row
        .find(&key)
        .unwrap_or_else(|| panic!("no {name} in {row}"))
        + key.len();
    let rest = &row[start..];
    if rest.starts_with("null") {
        return "null".to_string();
    }
    rest[..=rest.find('}').unwrap()].to_string()
}

#[test]
fn concurrent_selects_and_rollbacks_leave_an_unbroken_selection_chain() {
    let fixture = Fixture::new("concurrent");
    let one = fixture.install(true);
    fixture.version("two");
    let two = fixture.install(true);
    let children: Vec<_> = (0..24)
        .map(|index| {
            let mut command = Command::new(env!("CARGO_BIN_EXE_bench-tools"));
            command.arg("runtime");
            match index % 3 {
                0 => command.args(["select", one.as_str()]),
                1 => command.args(["select", two.as_str()]),
                _ => command.arg("rollback"),
            };
            command
                .current_dir(&fixture.repo)
                .env("BENCH_RUNTIME_STORE", &fixture.store)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    for child in children {
        let done = child.wait_with_output().unwrap();
        assert!(done.status.success(), "{}", text(&done));
    }
    let history = fs::read_to_string(fixture.store.join("bench-tools/selections.jsonl")).unwrap();
    let rows: Vec<&str> = history.lines().collect();
    assert!(rows.len() > 2, "{history}");
    for pair in rows.windows(2) {
        assert_eq!(
            row_object(pair[1], "previous"),
            row_object(pair[0], "pin"),
            "{history}"
        );
    }
    let last = row_object(rows[rows.len() - 1], "pin");
    let selected = fs::read_to_string(fixture.store.join("bench-tools/selected.json")).unwrap();
    assert!(
        selected.contains(&format!("\"sha256\": \"{}\"", json_field(&last, "sha256"))),
        "{selected}\n{history}"
    );
}

#[test]
fn an_installer_that_does_not_build_names_the_fetch_step() {
    let fixture = Fixture::new("bootstrap");
    let launch = fixture.launcher_script();
    fs::write(fixture.crate_dir().join("src/main.rs"), "fn main() {\n").unwrap();
    let refused = launch(&["runtime", "install", "--select"]);
    assert_eq!(refused.status.code(), Some(1), "{}", text(&refused));
    assert!(
        stderr(&refused).contains("run `cargo fetch --locked` in rust/bench-tools"),
        "{}",
        text(&refused)
    );
    assert_eq!(fixture.entries(), [] as [String; 0]);
}

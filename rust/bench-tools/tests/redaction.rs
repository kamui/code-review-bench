//! `bench-tools redaction` against a temporary git repository, with a values file of made-up values.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VALUES: &str = r#"{"schema_version": 1, "email": ["person@example.com"], "hosts": {"examplehost": "host-01-xyz"}}"#;

struct Repository {
    root: PathBuf,
    values: PathBuf,
}

fn git(root: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Example")
        .env("GIT_AUTHOR_EMAIL", "example@example.com")
        .env("GIT_COMMITTER_NAME", "Example")
        .env("GIT_COMMITTER_EMAIL", "example@example.com")
        .status()
        .unwrap();
    assert!(status.success(), "git {arguments:?}");
}

impl Repository {
    fn new(label: &str) -> Self {
        let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("redaction-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("repository");
        std::fs::create_dir_all(&root).unwrap();
        let values = base.join("values.json");
        std::fs::write(&values, VALUES).unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        Self { root, values }
    }

    fn write(&self, name: &str, content: &str) {
        let path = self.root.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn run(&self, arguments: &[&str]) -> Output {
        self.run_with(&self.values, arguments)
    }

    fn run_with(&self, values: &Path, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .arg("redaction")
            .args(arguments)
            .current_dir(&self.root)
            .env("BENCH_REDACTION_VALUES", values)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap()
    }
}

fn shown(output: &Output) -> (i32, String, String) {
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    let (stdout, stderr) = (text(&output.stdout), text(&output.stderr));
    assert!(
        !stdout.contains("examplehost") && !stderr.contains("examplehost"),
        "a matched value was printed: {stdout} {stderr}"
    );
    assert!(!stdout.contains("person@") && !stderr.contains("person@"));
    (output.status.code().unwrap(), stdout, stderr)
}

#[test]
fn staged_content_is_checked_not_the_working_tree() {
    let repository = Repository::new("staged");
    repository.write("notes/a.txt", "ssh examplehost\n");
    repository.write("notes/b.txt", "nothing\n");
    git(&repository.root, &["add", "notes"]);
    repository.write("notes/a.txt", "fixed in the working tree only\n");
    let (code, stdout, _) = shown(&repository.run(&["check", "--staged", "--require-values"]));
    assert_eq!((code, stdout.as_str()), (1, "would change: notes/a.txt\n"));

    repository.write("notes/b.txt", "mail person@example.com\n");
    git(&repository.root, &["add", "notes/a.txt"]);
    let (code, stdout, _) = shown(&repository.run(&["check", "--staged"]));
    assert_eq!((code, stdout.as_str()), (0, ""));
}

#[test]
fn a_commit_message_is_checked() {
    let repository = Repository::new("message");
    repository.write("message.txt", "fix(bench): rerun on examplehost\n");
    let (code, stdout, _) = shown(&repository.run(&["check", "--message", "message.txt"]));
    assert_eq!((code, stdout.as_str()), (1, "would change: message.txt\n"));
    repository.write("message.txt", "fix(bench): rerun on host-01\n");
    let (code, _, _) = shown(&repository.run(&["check", "--message", "message.txt"]));
    assert_eq!(code, 0);
}

#[test]
fn require_values_fails_without_the_values_file() {
    let repository = Repository::new("require");
    let missing = repository.root.join("absent.json");
    let (code, _, stderr) =
        shown(&repository.run_with(&missing, &["check", "--staged", "--require-values"]));
    assert_eq!(code, 2);
    assert!(stderr.contains("no redaction values file"), "{stderr}");
    let (code, _, _) = shown(&repository.run_with(&missing, &["check", "--staged"]));
    assert_eq!(code, 0);
}

#[test]
fn changed_files_are_checked_against_the_merge_base_and_apply_fixes_them() {
    let repository = Repository::new("changed");
    repository.write("old.txt", "examplehost\n");
    git(&repository.root, &["add", "."]);
    git(&repository.root, &["commit", "-q", "-m", "base"]);
    git(&repository.root, &["checkout", "-q", "-b", "topic"]);
    repository.write("new.txt", "examplehost\n");
    git(&repository.root, &["add", "."]);
    git(&repository.root, &["commit", "-q", "-m", "topic"]);
    let (code, stdout, _) = shown(&repository.run(&["check", "--changed", "main"]));
    assert_eq!(
        (code, stdout.as_str()),
        (
            1,
            format!(
                "would change: {}\n",
                repository.root.join("new.txt").display()
            )
            .as_str()
        )
    );

    let (code, stdout, _) = shown(&repository.run(&["apply", "new.txt"]));
    assert_eq!((code, stdout.as_str()), (0, "redacted: new.txt\n"));
    assert_eq!(
        std::fs::read_to_string(repository.root.join("new.txt")).unwrap(),
        "host-01-xyz\n"
    );
    let (code, _, _) = shown(&repository.run(&["check", "--changed", "main", "new.txt"]));
    assert_eq!(code, 0);
}

fn write_database(repository: &Repository, name: &str) {
    let mut bytes = b"SQLite format 3\0".to_vec();
    bytes.extend_from_slice(b" mail person@example.com");
    let path = repository.root.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn a_refusal_advises_by_the_kind_of_file_listed() {
    let repository = Repository::new("advice");
    repository.write("notes/a.txt", "ssh examplehost\n");
    let (code, stdout, stderr) = shown(&repository.run(&["check", "notes"]));
    assert_eq!(code, 1);
    assert!(stdout.starts_with("would change: "), "{stdout}");
    assert!(stderr.contains("redaction apply"), "{stderr}");
    assert!(!stderr.contains("leave it out"), "{stderr}");

    std::fs::remove_file(repository.root.join("notes/a.txt")).unwrap();
    write_database(&repository, "notes/a.db");
    let (code, stdout, stderr) = shown(&repository.run(&["check", "notes"]));
    assert_eq!(code, 1);
    assert!(stdout.starts_with("cannot redact: "), "{stdout}");
    assert!(
        stderr.contains("leave it out") && !stderr.contains("redaction apply"),
        "{stderr}"
    );

    repository.write("notes/a.txt", "ssh examplehost\n");
    let (_, _, stderr) = shown(&repository.run(&["check", "notes"]));
    assert!(
        stderr.contains("redaction apply") && stderr.contains("leave it out"),
        "{stderr}"
    );
}

#[test]
fn apply_says_what_to_do_with_a_file_it_cannot_rewrite() {
    let repository = Repository::new("apply-advice");
    write_database(&repository, "a.db");
    let (code, stdout, stderr) = shown(&repository.run(&["apply", "a.db"]));
    assert_eq!(code, 1);
    assert!(stdout.starts_with("cannot redact: "), "{stdout}");
    assert!(stderr.contains("leave it out"), "{stderr}");

    repository.write("b.txt", "ssh examplehost\n");
    let (code, _, stderr) = shown(&repository.run(&["apply", "b.txt"]));
    assert_eq!((code, stderr.as_str()), (0, ""));
}

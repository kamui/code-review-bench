//! Attempt launch: the clean context, skill provenance, offline provisioning and the skill runners' refusals of a
//! run they are not pinned by. No client, model or network is called; every fixture lives under the system
//! temporary directory.

use bench_tools::clean_context;
use bench_tools::pyjson::{self, Duplicates, Value};
use bench_tools::sha256;
use bench_tools::skill_provenance::{self, Request};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Temp(PathBuf);

impl Temp {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("bench-tools-launch-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(fs::canonicalize(&path).unwrap())
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = Command::new("chmod")
            .arg("-R")
            .arg("u+w")
            .arg(&self.0)
            .output();
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, content: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn parse(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn field<'a>(value: &'a Value, path: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    path.split('.').fold(value, |value, key| {
        value
            .as_object()
            .and_then(|object| object.get(key))
            .unwrap_or(&NULL)
    })
}

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn bench_tools(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .args(arguments)
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn both(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn git(directory: &Path, arguments: &[&str]) -> String {
    git_at(directory, arguments, None)
}

fn git_at(directory: &Path, arguments: &[&str], date: Option<&str>) -> String {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
        ])
        .args(arguments);
    if let Some(date) = date {
        command
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "git {arguments:?}: {}",
        both(&output)
    );
    stdout(&output).trim().to_string()
}

fn entries(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
fn codex_configuration_disables_ambient_guidance_and_skills() {
    let temp = Temp::new("codex-config");
    let receipt = clean_context::prepare(&temp.0).unwrap();
    let home = PathBuf::from(text(field(&receipt, "home")));
    let clone = temp.0.join("clone");
    let skill = clone.join(".agents/skills/ambient");
    write(&skill.join("SKILL.md"), "Ambient instructions");
    let config = clean_context::configure_codex(&home, &clone, None, None).unwrap();
    let settings = fs::read_to_string(&config).unwrap();
    let lines: Vec<&str> = settings.lines().collect();
    for line in [
        "project_doc_max_bytes = 0",
        "project_doc_fallback_filenames = []",
        "features.apps = false",
    ] {
        assert!(lines.contains(&line), "{line} missing from {settings}");
    }
    let disabled = format!("path = \"{}\"", skill.display());
    let at = lines.iter().position(|line| *line == disabled).unwrap();
    assert_eq!(lines[at + 1], "enabled = false");
    let project = format!("[projects.\"{}\"]", clone.display());
    let at = lines.iter().position(|line| *line == project).unwrap();
    assert_eq!(lines[at + 1], "trust_level = \"untrusted\"");
    assert!(settings.contains("Treat AGENTS.md"));
}

#[test]
fn a_fresh_attempt_records_an_empty_home_and_a_unique_context() {
    let temp = Temp::new("fresh");
    let mut contexts = Vec::new();
    for name in ["first", "second"] {
        let attempt = temp.0.join(name);
        fs::create_dir(&attempt).unwrap();
        let receipt = clean_context::prepare(&attempt).unwrap();
        assert!(entries(Path::new(text(field(&receipt, "home")))).is_empty());
        let saved = parse(&fs::read_to_string(attempt.join("clean-context.json")).unwrap());
        assert!(saved.py_eq(&receipt));
        contexts.push(text(field(&receipt, "context_id")).to_string());
    }
    assert_ne!(contexts[0], contexts[1]);
}

#[test]
fn an_existing_home_or_a_linked_attempt_is_refused_without_changing_evidence() {
    let temp = Temp::new("reused");
    let attempt = temp.0.join("attempt");
    fs::create_dir(&attempt).unwrap();
    let receipt = clean_context::prepare(&attempt).unwrap();
    let history = attempt.join("home/sessions.jsonl");
    fs::write(&history, "prior review").unwrap();
    assert!(clean_context::prepare(&attempt).is_err());
    assert_eq!(fs::read_to_string(&history).unwrap(), "prior review");
    let saved = parse(&fs::read_to_string(attempt.join("clean-context.json")).unwrap());
    assert!(saved.py_eq(&receipt));

    let empty = temp.0.join("empty");
    fs::create_dir_all(empty.join("home")).unwrap();
    assert!(clean_context::prepare(&empty).is_err());
    fs::remove_dir(empty.join("home")).unwrap();
    std::os::unix::fs::symlink(&empty, empty.join("home")).unwrap();
    assert!(clean_context::prepare(&empty).is_err());
    let link = temp.0.join("linked-attempt");
    std::os::unix::fs::symlink(&empty, &link).unwrap();
    assert_eq!(
        clean_context::prepare(&link).unwrap_err(),
        "attempt directory must not be a symlink"
    );
}

#[test]
fn the_neutral_directory_is_fresh_and_outside_every_repository() {
    let temp = Temp::new("neutral");
    let neutral = || {
        Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .args(["clean-context", "neutral-directory"])
            .env("TMPDIR", &temp.0)
            .output()
            .unwrap()
    };
    let made = neutral();
    assert!(made.status.success(), "{}", both(&made));
    let directory = PathBuf::from(stdout(&made).trim_end());
    assert_eq!(directory.parent().unwrap(), temp.0);
    assert!(entries(&directory).is_empty());
    git(&temp.0, &["init", "-q"]);
    let refused = neutral();
    assert_eq!(refused.status.code(), Some(2));
    assert!(
        both(&refused).contains(&format!(
            "inside the git repository {}; set TMPDIR",
            temp.0.display()
        )),
        "{}",
        both(&refused)
    );
    let left: Vec<String> = entries(&temp.0)
        .into_iter()
        .filter(|name| name.starts_with("client-"))
        .collect();
    assert_eq!(
        left,
        [directory
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned()]
    );
}

#[test]
fn the_claude_account_keeps_only_account_fields() {
    let temp = Temp::new("account");
    let source = temp.0.join("source.json");
    fs::write(
        &source,
        r#"{"oauthAccount": {"emailAddress": "a@example.test"}, "userID": "u", "projects": {"/x": {}}, "mcpServers": {}}"#,
    )
    .unwrap();
    clean_context::claude_account(&source, &temp.0).unwrap();
    let account = parse(&fs::read_to_string(temp.0.join(".claude.json")).unwrap());
    let mut keys: Vec<&str> = account
        .as_object()
        .unwrap()
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["hasCompletedOnboarding", "oauthAccount", "userID"]);
}

struct Skill {
    temp: Temp,
}

impl Skill {
    fn new(name: &str) -> Self {
        let skill = Self {
            temp: Temp::new(name),
        };
        let root = skill.root();
        git(root, &["init", "-q"]);
        git(
            root,
            &[
                "remote",
                "add",
                "origin",
                "https://github.com/example/skills.git",
            ],
        );
        skill.write(
            "SKILL.md",
            "---\nname: review\n---\nRead references/rubric.md.\n",
        );
        skill.write("references/rubric.md", "Review criteria\n");
        skill.commit("2026-01-01T12:00:00Z");
        skill
    }

    fn root(&self) -> &Path {
        &self.temp.0
    }

    fn write(&self, path: &str, contents: &str) {
        write(&self.root().join("skill").join(path), contents);
    }

    fn commit(&self, date: &str) {
        git(self.root(), &["add", "."]);
        git_at(
            self.root(),
            &["commit", "-qm", "Fixture change"],
            Some(date),
        );
    }

    fn collect(
        &self,
        frozen: Option<&Path>,
        release: Option<&Value>,
        inclusions: &[String],
    ) -> Result<Value, String> {
        skill_provenance::collect(&Request {
            repository: self.root(),
            revision: "HEAD",
            skill_path: "skill",
            frozen,
            benchmark_tree: None,
            release,
            exclusions: &[],
            inclusions,
        })
    }

    fn date(&self) -> String {
        text(field(&self.collect(None, None, &[]).unwrap(), "date")).to_string()
    }
}

#[test]
fn design_history_and_tests_do_not_change_the_fallback_date() {
    let skill = Skill::new("fallback-date");
    for name in [
        "DESIGN.md",
        "CHANGELOG.md",
        "HISTORY.md",
        "scripts/test_review.py",
    ] {
        skill.write(name, "Documentation or skill tests\n");
    }
    skill.commit("2026-02-01T12:00:00Z");
    let record = skill.collect(None, None, &[]).unwrap();
    assert_eq!(text(field(&record, "date")), "2026-01-01");
    assert!(matches!(field(&record, "version"), Value::Null));
    skill.write("references/rubric.md", "New review criteria\n");
    skill.commit("2026-03-01T12:00:00Z");
    assert_eq!(skill.date(), "2026-03-01");
}

#[test]
fn a_deleted_runtime_file_changes_the_date() {
    let skill = Skill::new("deleted");
    fs::remove_file(skill.root().join("skill/references/rubric.md")).unwrap();
    skill.commit("2026-04-01T12:00:00Z");
    assert_eq!(skill.date(), "2026-04-01");
}

#[test]
fn explicit_runtime_dependencies_override_documentation_exclusions() {
    let skill = Skill::new("inclusions");
    skill.write("references/README.md", "Instructions the skill reads\n");
    skill.commit("2026-02-01T12:00:00Z");
    let record = skill
        .collect(None, None, &["references/*.md".to_string()])
        .unwrap();
    assert_eq!(text(field(&record, "date")), "2026-02-01");
    let paths: Vec<&str> = field(&record, "runtime_files")
        .as_array()
        .unwrap()
        .iter()
        .map(|file| text(field(file, "path")))
        .collect();
    assert!(paths.contains(&"references/README.md"), "{paths:?}");
}

#[test]
fn the_runtime_snapshot_must_match_the_recorded_revision() {
    let skill = Skill::new("snapshot");
    let frozen = skill.root().join("skill");
    skill.collect(Some(&frozen), None, &[]).unwrap();
    skill.write("references/rubric.md", "Uncommitted change\n");
    let error = skill.collect(Some(&frozen), None, &[]).unwrap_err();
    assert!(error.contains("differs from upstream"), "{error}");
}

#[test]
fn a_release_date_and_version_require_the_matching_runtime() {
    let skill = Skill::new("release");
    git(skill.root(), &["tag", "v1.2.3"]);
    let release = parse(
        r#"{"tag_name": "v1.2.3", "published_at": "2026-01-10T00:00:00Z", "html_url": "https://github.com/example/skills/releases/tag/v1.2.3"}"#,
    );
    let record = skill.collect(None, Some(&release), &[]).unwrap();
    assert_eq!(
        [
            text(field(&record, "version")),
            text(field(&record, "date")),
            text(field(&record, "date_source"))
        ],
        ["1.2.3", "2026-01-10", "release"]
    );
    skill.write("references/rubric.md", "New review criteria\n");
    skill.commit("2026-02-01T12:00:00Z");
    let error = skill.collect(None, Some(&release), &[]).unwrap_err();
    assert!(error.contains("release runtime differs"), "{error}");
}

#[test]
fn a_declared_version_is_kept_without_a_release_date() {
    let skill = Skill::new("declared");
    skill.write(
        "SKILL.md",
        "---\nname: review\nversion: \"2.3.4\"\n---\nReview\n",
    );
    skill.commit("2026-02-01T12:00:00Z");
    let record = skill.collect(None, None, &[]).unwrap();
    assert_eq!(skill_provenance::label(&record), "2.3.4 · 2026-02-01");
    assert_eq!(text(field(&record, "date_source")), "commit");
}

#[test]
fn empty_and_null_versions_are_omitted() {
    let skill = Skill::new("versions");
    for version in ["", "null", "\"\"", "# undeclared"] {
        skill.write(
            "SKILL.md",
            &format!("---\nname: review\nversion: {version}\ndescription: Review\n---\nReview\n"),
        );
        skill.commit("2026-02-01T12:00:00Z");
        let record = skill.collect(None, None, &[]).unwrap();
        assert!(
            matches!(field(&record, "version"), Value::Null),
            "{version}"
        );
    }
}

#[test]
fn a_frozen_pin_refuses_changed_metadata_and_a_wrong_tree() {
    let skill = Skill::new("pin");
    let record = skill.collect(None, None, &[]).unwrap();
    let path = skill.root().join("release.json");
    skill_provenance::write(&path, &record).unwrap();
    let pinned = |tree: &str| {
        parse(&format!(
            r#"{{"resolved_skill_tree": "{tree}", "skill_provenance": {{"path": "release.json", "sha256": "{}"}}}}"#,
            sha256::hex_digest(&fs::read(&path).unwrap())
        ))
    };
    let tree = text(field(&record, "benchmark_tree")).to_string();
    skill_provenance::verify_pin(skill.root(), &pinned(&tree)).unwrap();
    let error = skill_provenance::verify_pin(skill.root(), &pinned("another-tree")).unwrap_err();
    assert!(error.contains("different benchmark tree"), "{error}");
    let arm = pinned(&tree);
    fs::write(&path, "{}").unwrap();
    let error = skill_provenance::verify_pin(skill.root(), &arm).unwrap_err();
    assert!(error.contains("frozen hash"), "{error}");
}

#[test]
fn verify_pin_checks_every_arm_of_the_manifest_and_names_the_failing_one() {
    let temp = Temp::new("verify-pin");
    write(
        &temp.0.join("manifest.json"),
        r#"{"arms": [{"id": "plain"}, {"id": "pinned", "resolved_skill_tree": "t", "skill_provenance": {"path": "missing.json", "sha256": "0"}}]}"#,
    );
    let run = s(&temp.0);
    let plain = bench_tools(&[
        "skill-provenance",
        "verify-pin",
        "--run",
        run,
        "--arm",
        "plain",
    ]);
    assert!(plain.status.success(), "{}", both(&plain));
    let every = bench_tools(&["skill-provenance", "verify-pin", "--run", run]);
    assert_eq!(every.status.code(), Some(2));
    assert!(
        both(&every).starts_with("skill-provenance: pinned: "),
        "{}",
        both(&every)
    );
    let unknown = bench_tools(&[
        "skill-provenance",
        "verify-pin",
        "--run",
        run,
        "--arm",
        "other",
    ]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(
        both(&unknown).contains("the manifest has no arm other"),
        "{}",
        both(&unknown)
    );
}

struct Provisioned {
    temp: Temp,
    target_dir: PathBuf,
    target: String,
    leak: String,
    base: String,
}

impl Provisioned {
    fn new(name: &str) -> Self {
        let temp = Temp::new(name);
        let staging = temp.0.join("staging");
        fs::create_dir(&staging).unwrap();
        git(&staging, &["init", "-q", "-b", "trunk"]);
        let commit = |content: &str, message: &str| {
            fs::write(staging.join("f.txt"), content).unwrap();
            git(&staging, &["add", "-A"]);
            git(&staging, &["commit", "-q", "-m", message]);
            git(&staging, &["rev-parse", "HEAD"])
        };
        let base = commit("base\n", "base");
        let head = commit("head\n", "head");
        let leak = commit("fix\n", "fix that reveals the answer");
        let identity = bench_tools(&["diff-identity", "--quiet", s(&staging), &base, &head]);
        assert!(identity.status.success(), "{}", both(&identity));
        let target = format!(
            r#"{{"id": "t-1", "head": "{head}", "merge_base": "{base}", "local_base_branch": "main", "negative_shas": [{{"sha": "{leak}", "reason": "the fix"}}], "diff_manifest_sha256": "{}""#,
            stdout(&identity).trim()
        );
        let target_dir = temp.0.join("targets/t-1");
        let provisioned = Self {
            temp,
            target_dir,
            target,
            leak,
            base,
        };
        provisioned.set_target("");
        let mirrored = provisioned.run(&["mirror", "--staging", s(&staging)]);
        assert!(mirrored.status.success(), "{}", both(&mirrored));
        provisioned
    }

    fn cache(&self) -> PathBuf {
        self.temp.0.join("cache")
    }

    /// target.json with `extra` members appended after the identity fields.
    fn set_target(&self, extra: &str) {
        write(
            &self.target_dir.join("target.json"),
            format!("{}{extra}}}", self.target),
        );
    }

    fn run(&self, arguments: &[&str]) -> Output {
        let cache = self.cache();
        let mut command = vec!["provision", arguments[0], "--target", s(&self.target_dir)];
        command.extend(&arguments[1..]);
        command.extend(["--cache-root", s(&cache)]);
        bench_tools(&command)
    }
}

#[test]
fn the_mirror_drops_the_leak_and_a_clone_has_the_review_branches() {
    let provisioned = Provisioned::new("mirror");
    let mirror = provisioned.cache().join("mirrors/t-1.git");
    assert!(provisioned.cache().join("mirrors/t-1.json").is_file());
    let leak = Command::new("git")
        .arg("-C")
        .arg(&mirror)
        .args(["cat-file", "-e", &provisioned.leak])
        .output()
        .unwrap();
    assert!(
        !leak.status.success(),
        "the leak commit must not be in the mirror"
    );
    assert_eq!(git(&mirror, &["symbolic-ref", "HEAD"]), "refs/heads/main");
    let checked = provisioned.run(&["check"]);
    assert!(
        checked.status.success() && stdout(&checked).contains("ok"),
        "{}",
        both(&checked)
    );
    let clone = provisioned.temp.0.join("clone");
    let cloned = provisioned.run(&["clone", "--out", s(&clone)]);
    assert!(cloned.status.success(), "{}", both(&cloned));
    assert_eq!(
        git(&clone, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "review-head"
    );
    assert_eq!(git(&clone, &["rev-parse", "main"]), provisioned.base);
    let again = provisioned.run(&["clone", "--out", s(&clone)]);
    assert_eq!(
        again.status.code(),
        Some(2),
        "an existing clone directory must be refused"
    );
    let nothing = provisioned.run(&["cache"]);
    assert!(
        nothing.status.success() && stdout(&nothing).contains("nothing to build"),
        "{}",
        both(&nothing)
    );
}

#[test]
fn prepare_restores_only_the_recorded_cache_archive_into_its_own_copy() {
    if Command::new("bwrap").arg("--version").output().is_err() {
        return;
    }
    let provisioned = Provisioned::new("cache");
    let cache_config = r#"{"kind": "test",
        "build": ["mkdir -p {cache}/store {cache}/ro/sub && printf hi > {cache}/store/f && printf r > {cache}/ro/sub/g && ln -s store {cache}/inside && chmod 555 {cache}/ro/sub {cache}/ro", "test -f {clone}/f.txt && test ! -e {cache}/../../mirrors"],
        "post_clone": ["test -f {cache}/store/f && printf x > {work}/marker && test \"$BENCH_TEST_ENV\" = {cache}/store && printf w > {cache}/store/written"],
        "env": {"BENCH_TEST_ENV": "{cache}/store"},
        "smoke": [{"name": "read", "command": "cat $BENCH_TEST_ENV/f", "revision": "head"}]}"#;
    provisioned.set_target(&format!(r#", "provisioning": {{"cache": {cache_config}}}"#));
    let built = provisioned.run(&["cache"]);
    let output = stdout(&built);
    assert!(
        built.status.success() && output.contains("dependency_identity entry"),
        "{}",
        both(&built)
    );
    let entry = output
        .split("dependency_identity entry: ")
        .nth(1)
        .unwrap()
        .trim();
    let archive = provisioned.cache().join("archives/t-1.tar.gz");
    let cache = provisioned.cache();
    assert_eq!(
        fs::read_to_string(cache.join("caches/t-1/store/f")).unwrap(),
        "hi"
    );
    let receipt = parse(&fs::read_to_string(cache.join("caches/t-1.json")).unwrap());
    let archive_sha = sha256::hex_digest(&fs::read(&archive).unwrap());
    assert_eq!(text(field(&receipt, "archive.sha256")), archive_sha);
    assert_eq!(field(&receipt, "build").as_array().unwrap().len(), 2);
    assert_eq!(
        text(field(&receipt, "sandbox.profile")),
        "provision-bwrap-v1"
    );
    assert_eq!(text(field(&receipt, "sandbox.network")), "shared");
    assert!(
        !cache.join("scratch/t-1-cache").exists(),
        "the scratch clone must be removed"
    );

    let prepared = provisioned.temp.0.join("prepared");
    let unrecorded = provisioned.run(&["prepare", "--out", s(&prepared)]);
    assert_eq!(unrecorded.status.code(), Some(1));
    assert!(
        stdout(&unrecorded).contains("expected 1"),
        "{}",
        both(&unrecorded)
    );
    let zeros = "0".repeat(64);
    let wrong = entry.replace(&archive_sha, &zeros);
    provisioned.set_target(&format!(
        r#", "provisioning": {{"cache": {cache_config}, "dependency_identity": [{wrong}]}}"#
    ));
    let mismatched = provisioned.run(&[
        "prepare",
        "--out",
        s(&provisioned.temp.0.join("mismatched")),
    ]);
    assert_eq!(mismatched.status.code(), Some(1));
    assert!(
        stdout(&mismatched).contains(&format!("target.json records {zeros}")),
        "{}",
        both(&mismatched)
    );

    provisioned.set_target(&format!(
        r#", "provisioning": {{"cache": {cache_config}, "dependency_identity": [{entry}]}}"#
    ));
    let _ = Command::new("chmod")
        .arg("-R")
        .arg("u+w")
        .arg(&prepared)
        .output();
    let _ = fs::remove_dir_all(&prepared);
    let done = provisioned.run(&["prepare", "--out", s(&prepared)]);
    assert!(done.status.success(), "{}", both(&done));
    let record = parse(&stdout(&done));
    assert!(matches!(
        field(&record, "tree_clean_after"),
        Value::Bool(true)
    ));
    assert_eq!(text(field(&record, "sandbox.network")), "off");
    assert!(
        text(field(&record, "recipe"))
            .starts_with(&format!("restore cache archive sha256:{archive_sha}")),
        "{}",
        stdout(&done)
    );
    let sibling = |suffix: &str| PathBuf::from(format!("{}{suffix}", prepared.display()));
    assert!(sibling("-work").join("marker").is_file());
    assert!(
        sibling("-cache").join("store/written").is_file(),
        "the clone writes into its own restore"
    );
    assert!(
        !cache.join("caches/t-1/store/written").exists(),
        "the build directory must stay untouched"
    );
    assert_eq!(
        sha256::hex_digest(&fs::read(&archive).unwrap()),
        archive_sha
    );

    let smoke = provisioned.temp.0.join("smoke.json");
    let smoked = provisioned.run(&["smoke", "--out", s(&smoke)]);
    assert!(smoked.status.success(), "{}", both(&smoked));
    let schema =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/schema/smoke.v2.schema.json");
    let checked = bench_tools(&["check-manifest", s(&schema), s(&smoke)]);
    assert!(checked.status.success(), "{}", both(&checked));
    let receipt = parse(&fs::read_to_string(&smoke).unwrap());
    assert_eq!(text(field(&receipt, "sandbox.network")), "off");
    assert_eq!(text(field(&receipt, "provisioning.sandbox.network")), "off");
}

#[test]
fn a_skill_runner_refuses_a_run_it_is_not_pinned_by_before_it_writes_anything() {
    let temp = Temp::new("unpinned");
    let run = temp.0.join("bench/runs/2026-01-01-unpinned");
    write(
        &run.join("manifest.json"),
        r#"{"run_id": "2026-01-01-unpinned", "freeze_commit": "ffff", "frozen_at": "2026-01-01T00:00:00Z", "arms": []}"#,
    );
    let attempt = temp.0.join("attempt");
    fs::create_dir(&attempt).unwrap();
    for name in ["clone/.keep", "packet.md", "arm.json"] {
        write(&temp.0.join(name), "");
    }
    let launch = |client: &str| {
        bench_tools(&[
            "skill-runner",
            client,
            "--run",
            s(&run),
            "--attempt-dir",
            s(&attempt),
            "--clone",
            s(&temp.0.join("clone")),
            "--packet",
            s(&temp.0.join("packet.md")),
            "--arm",
            s(&temp.0.join("arm.json")),
            "--target",
            "fixture",
        ])
    };
    let frozen = run.join("runtimes.frozen.json");
    for (pin, problem) in [
        (
            None,
            format!("{} pins no bench-tools runtime", frozen.display()),
        ),
        (
            Some(r#"{"bench-tools": {"sha256": "0000"}}"#),
            format!("is not the bench-tools that {} pins", frozen.display()),
        ),
    ] {
        if let Some(pin) = pin {
            fs::write(&frozen, pin).unwrap();
        }
        for client in ["codex", "claude"] {
            let refused = launch(client);
            assert_eq!(refused.status.code(), Some(2), "{}", both(&refused));
            let message = both(&refused);
            assert!(
                message.starts_with(&format!("{client} skill runner: ")),
                "{message}"
            );
            assert!(message.contains(&problem), "{message}");
            assert!(
                message.contains("launch it from a checkout of its freeze commit ffff"),
                "{message}"
            );
            assert!(entries(&attempt).is_empty());
        }
    }
}

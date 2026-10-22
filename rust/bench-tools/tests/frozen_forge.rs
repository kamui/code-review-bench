use bench_tools::pyjson::{self, Duplicates, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const CUTOFF: &str = "2024-01-10T00:00:00Z";
const LATER: &str = "LATER-ONLY-PROVENANCE-MARKER";

fn bench(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .args(arguments)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn sha256(path: &Path) -> String {
    bench_tools::sha256::hex_digest(&std::fs::read(path).unwrap())
}

fn git(directory: &Path, arguments: &[&str], date: &str) -> String {
    let done = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(arguments)
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
        .env("GIT_AUTHOR_DATE", date)
        .env("GIT_COMMITTER_DATE", date)
        .output()
        .unwrap();
    assert!(
        done.status.success(),
        "git {arguments:?}: {}",
        text(&done.stderr)
    );
    text(&done.stdout).trim().to_string()
}

/// A task whose pull request `o/r#1` takes branch `feature` into `trunk`, with its mirror, capture and pins.
struct Fixture {
    root: PathBuf,
    target: PathBuf,
    cache: PathBuf,
    captures: PathBuf,
    head: String,
    base: String,
    excluded: String,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "bench-tools-frozen-forge-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let staging = root.join("staging");
        std::fs::create_dir_all(&staging).unwrap();
        git(&staging, &["init", "-q"], "2024-01-01T00:00:00Z");
        std::fs::write(staging.join("a.txt"), "one\n").unwrap();
        git(&staging, &["add", "-A"], "2024-01-01T00:00:00Z");
        git(
            &staging,
            &["commit", "-q", "-m", "base"],
            "2024-01-01T00:00:00Z",
        );
        let base = git(&staging, &["rev-parse", "HEAD"], "2024-01-01T00:00:00Z");
        git(
            &staging,
            &["checkout", "-q", "-b", "feature"],
            "2024-01-02T00:00:00Z",
        );
        std::fs::write(staging.join("a.txt"), "two\n").unwrap();
        std::fs::write(staging.join("b.txt"), "new\n").unwrap();
        git(&staging, &["add", "-A"], "2024-01-02T00:00:00Z");
        git(
            &staging,
            &["commit", "-q", "-m", "change"],
            "2024-01-02T00:00:00Z",
        );
        let head = git(&staging, &["rev-parse", "HEAD"], "2024-01-02T00:00:00Z");
        git(
            &staging,
            &["checkout", "-q", "-b", "fix"],
            "2024-02-01T00:00:00Z",
        );
        std::fs::write(staging.join("a.txt"), "fixed\n").unwrap();
        git(
            &staging,
            &["commit", "-q", "-am", "the later fix"],
            "2024-02-01T00:00:00Z",
        );
        let excluded = git(&staging, &["rev-parse", "HEAD"], "2024-02-01T00:00:00Z");
        let identity = bench(&["diff-identity", staging.to_str().unwrap(), &base, &head]);
        let identity = text(&identity.stdout)
            .lines()
            .last()
            .unwrap()
            .trim_start_matches("sha256 ")
            .to_string();

        let target = root.join("bench/targets/t1");
        std::fs::create_dir_all(target.join("forge")).unwrap();
        std::fs::write(
            target.join("target.json"),
            format!(
                r#"{{"id": "t1", "head": "{head}", "merge_base": "{base}", "local_base_branch": "main",
                "negative_shas": [{{"sha": "{excluded}", "reason": "the fix"}}], "diff_manifest_sha256": "{identity}"}}"#
            ),
        )
        .unwrap();
        let comment = r#"{"id": "IC_1", "fullDatabaseId": "11", "url": "https://github.com/o/r/pull/1#issuecomment-11", "author": {"login": "reviewer"}, "authorAssociation": "MEMBER", "body": "please add a test", "createdAt": "2024-01-03T00:00:00Z", "lastEditedAt": null}"#;
        let thread = r#"{"id": "PRRT_1", "isResolved": null, "isOutdated": false, "path": "a.txt", "line": 1, "originalLine": 1, "startLine": null, "originalStartLine": null, "diffSide": "RIGHT", "startDiffSide": null, "subjectType": "LINE", "comments": {"totalCount": 1, "nodes": [{"id": "PRRC_1", "fullDatabaseId": "21", "url": "https://github.com/o/r/pull/1#discussion_r21", "author": {"login": "reviewer"}, "authorAssociation": "MEMBER", "body": "why two?", "createdAt": "2024-01-04T00:00:00Z", "lastEditedAt": null, "path": "a.txt", "diffHunk": "@@ -1 +1 @@", "position": 1, "originalPosition": 1, "line": 1, "originalLine": 1, "startLine": null, "originalStartLine": null, "subjectType": "LINE", "commit": {"oid": "HEAD"}, "originalCommit": {"oid": "HEAD"}, "replyTo": null, "pullRequestReview": {"id": "PRR_1", "fullDatabaseId": "31", "submittedAt": "2024-01-04T00:00:00Z"}}]}}"#;
        let graphql = format!(
            r#"{{"data": {{"repository": {{"url": "https://github.com/o/r", "nameWithOwner": "o/r", "pullRequest": {{
            "id": "PR_1", "number": 1, "url": "https://github.com/o/r/pull/1", "title": "Change a", "body": "Changes a.",
            "state": "OPEN", "merged": false, "mergedAt": null, "closed": false, "closedAt": null, "isDraft": false,
            "baseRefName": "trunk", "baseRefOid": "{base}", "headRefName": "feature", "headRefOid": "{head}",
            "isCrossRepository": false, "maintainerCanModify": true, "createdAt": "2024-01-02T00:00:00Z", "lastEditedAt": null,
            "additions": 2, "deletions": 1, "changedFiles": 2, "author": {{"login": "author"}}, "authorAssociation": "CONTRIBUTOR",
            "baseRepository": {{"url": "https://github.com/o/r", "nameWithOwner": "o/r"}},
            "headRepository": {{"url": "https://github.com/o/r", "nameWithOwner": "o/r"}}, "headRepositoryOwner": {{"login": "o"}},
            "labels": {{"totalCount": 0, "nodes": []}},
            "commits": {{"totalCount": 1, "nodes": [{{"commit": {{"oid": "{head}", "messageHeadline": "change", "messageBody": "",
              "authoredDate": "2024-01-02T00:00:00Z", "committedDate": "2024-01-02T00:00:00Z",
              "author": {{"name": "Fixture", "email": "fixture@example.invalid", "date": "2024-01-02T00:00:00Z", "user": null}}}}}}]}},
            "headCommit": {{"nodes": [{{"commit": {{"oid": "{head}", "checkSuites": {{"totalCount": 1, "nodes": [{{"checkRuns": {{"totalCount": 1,
              "nodes": [{{"name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS", "startedAt": "2024-01-02T00:00:00Z",
              "completedAt": "2024-01-02T00:01:35Z", "detailsUrl": "https://ci.example/1", "title": null}}]}}}}]}}, "status": null}}}}]}},
            "timelineItems": {{"pageInfo": {{"hasNextPage": false}}, "nodes": []}},
            "reviews": {{"totalCount": 0, "nodes": []}},
            "reviewThreads": {{"totalCount": 1, "nodes": [{thread}]}},
            "comments": {{"totalCount": 1, "nodes": [{comment}]}},
            "closingIssuesReferences": {{"totalCount": 0, "nodes": []}}}}}}}}}}"#
        );
        std::fs::write(target.join("forge/graphql.json"), graphql).unwrap();
        std::fs::write(target.join("forge/issues.json"), r#"{"issues": []}"#).unwrap();
        std::fs::write(
            target.join("forge/provenance.json"),
            format!(r#"{{"restored": [{{"at_fetch": "{LATER}"}}]}}"#),
        )
        .unwrap();
        std::fs::write(
            target.join("packet.v2.md"),
            "| Merge-base | `base` (**differs from the base SHA**: the base branch moved before the merge; review against the merge-base) |\n\
             | `state` | `MERGED` |\n| `merged` | **`true`** (merged 2024-02-01T00:00:00Z) |\n\
             **1.** 2024-01-04T00:00:00Z \u{b7} `reviewer` \u{b7} `a.txt:1` \u{b7} on commit `HEAD` \u{b7} thread unresolved\n",
        )
        .unwrap();
        let captures = root.join("bench/forge-captures.v1.json");
        std::fs::write(
            &captures,
            format!(
                r#"{{"schema_version": 1, "targets": [{{"target": "t1", "target_sha256": "{}", "cutoff": "{CUTOFF}",
                "packet": {{"path": "bench/targets/t1/packet.v2.md", "sha256": "{}"}},
                "files": {{"graphql.json": "{}", "issues.json": "{}", "provenance.json": "{}"}}}}]}}"#,
                sha256(&target.join("target.json")),
                sha256(&target.join("packet.v2.md")),
                sha256(&target.join("forge/graphql.json")),
                sha256(&target.join("forge/issues.json")),
                sha256(&target.join("forge/provenance.json")),
            ),
        )
        .unwrap();
        let cache = root.join("cache");
        let mirrored = bench(&[
            "provision",
            "mirror",
            "--target",
            target.to_str().unwrap(),
            "--cache-root",
            cache.to_str().unwrap(),
            "--staging",
            staging.to_str().unwrap(),
        ]);
        assert!(
            mirrored.status.success(),
            "{}{}",
            text(&mirrored.stdout),
            text(&mirrored.stderr)
        );
        Self {
            root,
            target,
            cache,
            captures,
            head,
            base,
            excluded,
        }
    }

    /// A new attempt with a clone of the mirror and its frozen forge.
    fn attempt(&self, name: &str) -> PathBuf {
        let attempt = self.clone(name);
        let served = self.setup(&attempt, &[]);
        assert!(served.status.success(), "{}", text(&served.stderr));
        attempt
    }

    /// A new attempt with a clone of the mirror and no forge yet.
    fn clone(&self, name: &str) -> PathBuf {
        let attempt = self.root.join(name);
        let clone = attempt.join("clone");
        std::fs::create_dir_all(&attempt).unwrap();
        let cloned = bench(&[
            "provision",
            "clone",
            "--target",
            self.target.to_str().unwrap(),
            "--cache-root",
            self.cache.to_str().unwrap(),
            "--out",
            clone.to_str().unwrap(),
        ]);
        assert!(cloned.status.success(), "{}", text(&cloned.stdout));
        attempt
    }

    fn setup(&self, attempt: &Path, extra: &[&str]) -> Output {
        let clone = attempt.join("clone");
        let mut arguments = vec![
            "frozen-forge",
            "setup",
            "--target",
            self.target.to_str().unwrap(),
            "--attempt",
            attempt.to_str().unwrap(),
            "--clone",
            clone.to_str().unwrap(),
            "--captures",
            self.captures.to_str().unwrap(),
            "--captures-sha256",
        ];
        let pinned = sha256(&self.captures);
        arguments.push(&pinned);
        arguments.extend_from_slice(extra);
        bench(&arguments)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// Run `command` through `sh -c` in the clone with the forge's `gh` first on `PATH`.
fn shell(attempt: &Path, command: &str) -> Output {
    let path = format!(
        "{}:{}",
        attempt.join("forge/bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    Command::new("sh")
        .args(["-c", command])
        .current_dir(attempt.join("clone"))
        .env("PATH", path)
        .output()
        .unwrap()
}

fn log(attempt: &Path) -> Vec<Value> {
    std::fs::read_to_string(attempt.join("forge-log/calls.jsonl"))
        .unwrap()
        .lines()
        .map(|line| pyjson::loads(line, Duplicates::KeepLast).unwrap())
        .collect()
}

fn files_under(directory: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(directory).unwrap().flatten() {
        let path = entry.path();
        if entry.file_type().unwrap().is_dir() {
            found.extend(files_under(&path));
        } else {
            found.push(path);
        }
    }
    found
}

#[test]
fn an_attempt_reads_the_pull_request_through_gh_and_origin_as_at_the_cutoff() {
    let fixture = Fixture::new("serve");
    let attempt = fixture.attempt("att-001");
    let clone = attempt.join("clone");
    let origin = attempt.join("forge/origin.git");

    assert_eq!(
        git(&clone, &["remote", "get-url", "origin"], CUTOFF),
        origin.to_str().unwrap()
    );
    let refs = git(
        &origin,
        &["for-each-ref", "--format=%(refname) %(objectname)"],
        CUTOFF,
    );
    assert_eq!(
        refs,
        format!(
            "refs/heads/feature {}\nrefs/heads/trunk {}\nrefs/pull/1/head {}",
            fixture.head, fixture.base, fixture.head
        )
    );
    assert_eq!(
        git(
            &clone,
            &["rev-parse", "--abbrev-ref", "origin/HEAD"],
            CUTOFF
        ),
        "origin/trunk"
    );
    assert_eq!(
        git(
            &clone,
            &["rev-parse", "--abbrev-ref", "main@{upstream}"],
            CUTOFF
        ),
        "origin/trunk"
    );
    assert_eq!(
        git(&clone, &["config", "branch.review-head.merge"], CUTOFF),
        "refs/pull/1/head"
    );
    assert!(
        git(&clone, &["branch", "-r"], CUTOFF)
            .lines()
            .all(|line| !line.contains("review-head"))
    );

    for path in files_under(&attempt) {
        let bytes = std::fs::read(&path).unwrap();
        assert!(
            !text(&bytes).contains(LATER),
            "{} holds the provenance marker",
            path.display()
        );
    }
    assert!(!attempt.join("forge/provenance.json").exists());

    let viewed = shell(
        &attempt,
        "gh pr view 1 --json number,state,files --jq '.files[].path'",
    );
    assert!(viewed.status.success(), "{}", text(&viewed.stderr));
    assert_eq!(text(&viewed.stdout), "a.txt\nb.txt\n");
    let current = shell(&attempt, "gh pr view --json number,headRefName");
    assert_eq!(
        text(&current.stdout),
        "{\"headRefName\":\"feature\",\"number\":1}\n"
    );
    let url = shell(
        &attempt,
        "gh pr view https://github.com/o/r/pull/1 --json title --jq .title",
    );
    assert_eq!(text(&url.stdout), "Change a\n");
    let other = shell(&attempt, "gh pr view 2 --json title");
    assert_eq!(other.status.code(), Some(1));
    assert!(text(&other.stderr).contains("number of 2"));
    let diff = shell(&attempt, "gh pr diff 1 --color=never");
    assert!(
        text(&diff.stdout).contains("diff --git a/b.txt b/b.txt"),
        "{}",
        text(&diff.stdout)
    );
    let checks = shell(&attempt, "gh pr checks 1");
    assert_eq!(
        (checks.status.code(), text(&checks.stdout)),
        (
            Some(0),
            "ci\tpass\t1m35s\thttps://ci.example/1\t\n".to_string()
        )
    );
    let comments = shell(
        &attempt,
        "gh api repos/{owner}/{repo}/pulls/1/comments --jq '.[] | {path, body, user: .user.login, updated_at}'",
    );
    assert_eq!(
        text(&comments.stdout),
        "{\"path\":\"a.txt\",\"body\":\"why two?\",\"user\":\"reviewer\",\"updated_at\":null}\n"
    );
    let graphql = shell(
        &attempt,
        "gh api graphql -F owner='{owner}' -F name='{repo}' -F number=1 -f query='query($owner:String!,$name:String!,$number:Int!){ repository(owner:$owner,name:$name){ pullRequest(number:$number){ reviewThreads(first:5){ nodes{ isResolved } } comments(first:5){ nodes{ body updatedAt } } } } }'",
    );
    assert_eq!(
        text(&graphql.stdout),
        "{\"data\":{\"repository\":{\"pullRequest\":{\"reviewThreads\":{\"nodes\":[{\"isResolved\":null}]},\"comments\":{\"nodes\":[{\"body\":\"please add a test\",\"updatedAt\":null}]}}}}}\n"
    );
    for (write, needle) in [
        ("gh pr comment 1 --body hi", "read-only"),
        (
            "gh api -X POST repos/o/r/issues/1/comments -f body=hi",
            "read-only",
        ),
        ("gh api graphql -f query='mutation{ x }'", "read-only"),
        ("gh repo view", "not answered by the frozen forge"),
        ("gh pr view 1 --json mergeable", "not in the frozen capture"),
    ] {
        let refused = shell(&attempt, write);
        assert_eq!(refused.status.code(), Some(1), "{write}");
        assert!(
            text(&refused.stderr).contains(needle),
            "{write}: {}",
            text(&refused.stderr)
        );
    }

    let fetched = shell(
        &attempt,
        "git fetch --no-tags origin feature:refs/review/pr-1-head && git rev-parse refs/review/pr-1-head",
    );
    assert!(fetched.status.success(), "{}", text(&fetched.stderr));
    assert_eq!(text(&fetched.stdout).trim(), fixture.head);
    let pushed = shell(&attempt, "git push origin HEAD:refs/heads/x");
    assert!(!pushed.status.success());
    assert!(
        text(&pushed.stderr).contains("pre-receive hook declined"),
        "{}",
        text(&pushed.stderr)
    );
    let missing = shell(&attempt, &format!("git fetch origin {}", fixture.excluded));
    assert!(!missing.status.success());

    let calls = log(&attempt);
    let tools: Vec<String> = calls
        .iter()
        .map(|call| call.as_object().unwrap().get("tool").unwrap().py_str())
        .collect();
    assert_eq!(tools.iter().filter(|tool| *tool == "gh").count(), 13);
    assert!(
        tools
            .iter()
            .filter(|tool| *tool == "git-upload-pack")
            .count()
            >= 2
    );
    let first = calls[0].as_object().unwrap();
    assert_eq!(
        first.get("argv").unwrap().py_repr(),
        "['pr', 'view', '1', '--json', 'number,state,files', '--jq', '.files[].path']"
    );
    assert_eq!(first.get("exit").unwrap().py_str(), "0");

    let receipt: Value = pyjson::loads(
        &std::fs::read_to_string(attempt.join("forge.json")).unwrap(),
        Duplicates::KeepLast,
    )
    .unwrap();
    let receipt = receipt.as_object().unwrap();
    assert_eq!(receipt.get("cutoff").unwrap().py_str(), CUTOFF);
    let served = receipt.get("served").unwrap().as_object().unwrap();
    assert_eq!(
        served
            .iter()
            .map(|(key, _)| key.as_str())
            .collect::<Vec<_>>(),
        ["graphql.json", "issues.json"]
    );
}

#[test]
fn setup_refuses_a_changed_capture_and_a_reused_attempt() {
    let fixture = Fixture::new("refuse");
    let attempt = fixture.attempt("att-001");
    let again = fixture.setup(&attempt, &[]);
    assert_eq!(again.status.code(), Some(2));
    assert!(
        text(&again.stderr).contains("never reused"),
        "{}",
        text(&again.stderr)
    );

    let pins = std::fs::read_to_string(&fixture.captures).unwrap();
    std::fs::write(
        fixture.target.join("forge/issues.json"),
        r#"{"issues": [{}]}"#,
    )
    .unwrap();
    let second = fixture.root.join("att-002");
    let cloned = bench(&[
        "provision",
        "clone",
        "--target",
        fixture.target.to_str().unwrap(),
        "--cache-root",
        fixture.cache.to_str().unwrap(),
        "--out",
        second.join("clone").to_str().unwrap(),
    ]);
    assert!(cloned.status.success());
    let changed = fixture.setup(&second, &[]);
    assert_eq!(changed.status.code(), Some(2));
    assert!(
        text(&changed.stderr).contains("issues.json differs from its pin"),
        "{}",
        text(&changed.stderr)
    );
    assert!(!second.join("forge").exists());

    std::fs::write(&fixture.captures, pins.replace("\"t1\"", "\"t2\"")).unwrap();
    let unpinned = bench(&[
        "frozen-forge",
        "setup",
        "--target",
        fixture.target.to_str().unwrap(),
        "--attempt",
        second.to_str().unwrap(),
        "--clone",
        second.join("clone").to_str().unwrap(),
        "--captures",
        fixture.captures.to_str().unwrap(),
        "--captures-sha256",
        &"0".repeat(64),
    ]);
    assert_eq!(unpinned.status.code(), Some(2));
    assert!(
        text(&unpinned.stderr).contains("but the run pins"),
        "{}",
        text(&unpinned.stderr)
    );
}

#[test]
fn the_probe_runs_an_inventory_and_reports_each_call() {
    let fixture = Fixture::new("probe");
    let inventory = fixture.root.join("bench/frozen-forge-inventory.v1.json");
    std::fs::write(
        &inventory,
        r#"{"schema_version": 1, "arm_kinds": {"claude-builtin": ["demo"]}, "sources": {"demo": {"tree": "bench", "calls": [
          {"id": "view", "source": "demo", "command": "gh pr view {{number}} --json comments", "expect": "answer"},
          {"id": "fetch-head", "source": "demo", "command": "git fetch --no-tags origin {{head_ref}}", "expect": {"same_repository": "answer", "cross_repository": "refusal"}},
          {"id": "upstream", "source": "demo", "command": "git diff @{upstream}...HEAD", "expect": "refusal"},
          {"id": "thread", "source": "demo", "command": "gh api graphql -f thread={{thread}} -f query='query($thread:ID!){ node(id:$thread){ id } }'", "expect": "answer"},
          {"id": "issue", "source": "demo", "command": "gh pr view {{issue}}", "expect": "answer"}
        ]}}}"#,
    )
    .unwrap();
    let out = fixture.root.join("probe");
    let done = bench(&[
        "frozen-forge",
        "probe",
        "--target",
        fixture.target.to_str().unwrap(),
        "--kind",
        "claude-builtin",
        "--out",
        out.to_str().unwrap(),
        "--captures",
        fixture.captures.to_str().unwrap(),
        "--inventory",
        inventory.to_str().unwrap(),
        "--cache-root",
        fixture.cache.to_str().unwrap(),
    ]);
    assert!(
        done.status.success(),
        "{}{}",
        text(&done.stdout),
        text(&done.stderr)
    );
    let report: Value = pyjson::loads(
        &std::fs::read_to_string(out.join("probe.json")).unwrap(),
        Duplicates::KeepLast,
    )
    .unwrap();
    let report = report.as_object().unwrap();
    assert!(matches!(report.get("ok"), Some(Value::Bool(true))));
    let calls = report.get("calls").unwrap().as_array().unwrap();
    let field = |index: usize, key: &str| {
        calls[index]
            .as_object()
            .unwrap()
            .get(key)
            .map(Value::py_str)
    };
    assert_eq!(field(0, "log_lines").as_deref(), Some("1"));
    assert_eq!(field(1, "exit").as_deref(), Some("0"));
    assert_eq!(field(2, "expect").as_deref(), Some("refusal"));
    assert_eq!(field(3, "exit").as_deref(), Some("0"));
    assert_eq!(
        field(4, "skipped").as_deref(),
        Some("the pull request has no closing issue")
    );
    assert!(!out.join("clone").exists() && !out.join("forge").exists());
    assert!(out.join("forge-log/calls.jsonl").is_file());
}

#[test]
fn setup_refuses_a_packet_that_states_a_thread_resolution_or_a_merge() {
    let fixture = Fixture::new("packet");
    let pinned = fixture.target.join("packet.v2.md");
    let v2 = std::fs::read_to_string(&pinned).unwrap();
    let threads_unknown = v2.replace("thread unresolved", "thread resolution unknown");
    let merged = fixture.target.join("packet.merged.md");
    std::fs::write(&merged, &threads_unknown).unwrap();
    let open = threads_unknown.replace("`MERGED`", "`OPEN`").replace(
        "**`true`** (merged 2024-02-01T00:00:00Z)",
        "**`false`** (not merged)",
    );
    let merge_prose = fixture.target.join("packet.merge-prose.md");
    std::fs::write(&merge_prose, &open).unwrap();
    for (name, stated) in [
        ("att-001", &pinned),
        ("att-003", &merged),
        ("att-004", &merge_prose),
    ] {
        let attempt = fixture.clone(name);
        let refused = fixture.setup(&attempt, &["--packet", stated.to_str().unwrap()]);
        assert_eq!(refused.status.code(), Some(2), "{}", text(&refused.stdout));
        assert!(
            text(&refused.stderr).contains("does not state what the capture holds"),
            "{}",
            text(&refused.stderr)
        );
        assert!(!attempt.join("forge").exists());
    }

    let unknown = fixture.target.join("packet.v3.md");
    std::fs::write(
        &unknown,
        open.replace(
            "the base branch moved before the merge",
            "the base branch moved after the head branched from it",
        ),
    )
    .unwrap();
    let attempt = fixture.clone("att-002");
    let served = fixture.setup(&attempt, &["--packet", unknown.to_str().unwrap()]);
    assert!(served.status.success(), "{}", text(&served.stderr));
    let receipt = pyjson::loads(
        &std::fs::read_to_string(attempt.join("forge.json")).unwrap(),
        Duplicates::KeepLast,
    )
    .unwrap();
    let packet = receipt.as_object().unwrap().get("packet").unwrap();
    assert_eq!(
        packet.as_object().unwrap().get("sha256").unwrap().py_str(),
        sha256(&unknown)
    );
}

/// The bwrap arguments that give a command the write policy of `settings`, as Claude's sandbox applies it on
/// Linux: a read-only root, each `allowWrite` path writable, and each `denyWrite` path read-only again.
fn write_policy(settings: &Value) -> Vec<String> {
    let paths = |key: &str| -> Vec<String> {
        let filesystem = settings.as_object().unwrap().get("sandbox").unwrap();
        let filesystem = filesystem.as_object().unwrap().get("filesystem").unwrap();
        let list = filesystem.as_object().unwrap().get(key).unwrap();
        list.as_array().unwrap().iter().map(Value::py_str).collect()
    };
    let mut arguments: Vec<String> = ["--ro-bind", "/", "/", "--dev", "/dev"]
        .map(str::to_string)
        .to_vec();
    for path in paths("allowWrite") {
        if Path::new(&path).exists() {
            arguments.extend(["--bind".to_string(), path.clone(), path]);
        }
    }
    for path in paths("denyWrite") {
        arguments.extend(["--ro-bind".to_string(), path.clone(), path]);
    }
    arguments
}

#[test]
fn a_fetch_from_origin_works_under_the_strict_write_policy() {
    if !Command::new("bwrap")
        .args(["--ro-bind", "/", "/", "true"])
        .output()
        .is_ok_and(|output| output.status.success())
    {
        eprintln!("skipped: bwrap cannot run here");
        return;
    }
    let fixture = Fixture::new("strict-fetch");
    let attempt = fixture.attempt("att-001");
    let clone = attempt.join("clone");
    for name in ["clone-work", "tmp", "artifacts"] {
        std::fs::create_dir_all(attempt.join(name)).unwrap();
    }
    let out = attempt.join("isolation-settings.json");
    let written = bench(&[
        "review-isolation",
        "settings",
        "--attempt",
        attempt.to_str().unwrap(),
        "--clone",
        clone.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    if text(&written.stderr).contains("required executable is missing") {
        eprintln!("skipped: {}", text(&written.stderr));
        return;
    }
    assert!(written.status.success(), "{}", text(&written.stderr));
    let settings = pyjson::loads(
        &std::fs::read_to_string(&out).unwrap(),
        Duplicates::KeepLast,
    )
    .unwrap();
    let sandboxed = |command: &str| {
        Command::new("bwrap")
            .args(write_policy(&settings))
            .args(["--chdir", clone.to_str().unwrap(), "sh", "-c", command])
            .output()
            .unwrap()
    };

    let fetched = sandboxed(
        "git fetch origin && git fetch --no-tags origin trunk && git rev-parse FETCH_HEAD",
    );
    assert!(fetched.status.success(), "{}", text(&fetched.stderr));
    assert_eq!(text(&fetched.stdout).trim(), fixture.base);
    let written = sandboxed("echo changed > a.txt");
    assert!(!written.status.success());
    assert_ne!(
        std::fs::read_to_string(clone.join("a.txt")).unwrap(),
        "changed\n"
    );
}

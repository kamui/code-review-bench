use bench_tools::pyjson::{self, Duplicates, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TASK: &str = "z-demo-7";
const HEAD: &str = "1111111111111111111111111111111111111111";
const BASE: &str = "2222222222222222222222222222222222222222";
const EXCLUDED: &str = "deadbeefcafe0000000000000000000000000000";
const CUTOFF: &str = "2024-01-10T00:00:00Z";

/// A checkout with one task, its cutoff and a hand-written packet, beside saved forge responses.
struct Fixture {
    root: PathBuf,
    replay: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "bench-tools-forge-capture-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let task = root.join("bench/targets").join(TASK);
        let replay = root.join("replay");
        for directory in [
            &task,
            &replay,
            &root.join("docs/research/last-push-recut-2026-10-07"),
        ] {
            std::fs::create_dir_all(directory).unwrap();
        }
        write(
            &task.join("target.json"),
            &format!(
                r#"{{"id":"{TASK}","repo":"o/r","pr":7,"head":"{HEAD}","merge_base":"{BASE}","base_sha":"{BASE}",
                "negative_shas":[{{"sha":"{EXCLUDED}","reason":"the fix"}}],"originating_issue":"o/r#3"}}"#
            ),
        );
        write(
            &task.join("packet.v2.md"),
            &format!(
                "# Task\n\nRepository: `o/r`.\nPull request: https://github.com/o/r/pull/7.\n\
                 Base revision: `{BASE}`.\nHead revision: `{HEAD}`.\nRevision cutoff: `{CUTOFF}`.\n"
            ),
        );
        write(
            &root.join("docs/research/last-push-recut-2026-10-07/packet-replacements.v1.json"),
            &format!(
                r#"{{"schema_version":1,"targets":[{{"target":"{TASK}","replacement":{{
                "path":"bench/targets/{TASK}/packet.v2.md","cutoff":"{CUTOFF}",
                "cutoff_is":"the last push","cutoff_source":"a test"}}}}]}}"#
            ),
        );
        write(&replay.join("graphql.json"), &pull_response(2));
        write(
            &replay.join("edits.json"),
            r#"{"data":{"nodes":[{"id":"PR_7","userContentEdits":{"totalCount":2,"nodes":[
            {"editedAt":"2024-01-20T00:00:00Z","deletedAt":null,"diff":"The body now."},
            {"editedAt":"2024-01-01T00:00:00Z","deletedAt":null,"diff":"The body then."}]}}]}}"#,
        );
        write(
            &replay.join("issue.json"),
            r#"{"data":{"repository":{"url":"https://github.com/o/r","nameWithOwner":"o/r","issue":{
            "id":"I_3","number":3,"url":"https://github.com/o/r/issues/3","title":"A bug","body":"It breaks.",
            "state":"CLOSED","closed":true,"closedAt":"2024-02-01T00:00:00Z","createdAt":"2023-12-01T00:00:00Z",
            "lastEditedAt":null,"repository":{"nameWithOwner":"o/r","url":"https://github.com/o/r"},
            "timelineItems":{"pageInfo":{"hasNextPage":false},"nodes":[{"__typename":"ClosedEvent","createdAt":"2024-02-01T00:00:00Z"}]},
            "comments":{"totalCount":0,"nodes":[]}}}}}"#,
        );
        Self { root, replay }
    }

    fn task(&self) -> PathBuf {
        self.root.join("bench/targets").join(TASK)
    }

    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .arg("forge-capture")
            .args(arguments)
            .arg("--root")
            .arg(&self.root)
            .output()
            .unwrap()
    }

    fn fetch(&self) -> Output {
        let replay = self.replay.to_str().unwrap();
        self.run(&["fetch", "--target", TASK, "--replay", replay])
    }

    fn captured(&self, name: &str) -> Value {
        read(&self.task().join("forge").join(name))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The pull request as the forge reports it after the merge; `reviews` is the reported review count.
fn pull_response(reviews: usize) -> String {
    format!(
        r#"{{"data":{{"repository":{{"url":"https://github.com/o/r","nameWithOwner":"o/r","pullRequest":{{
        "id":"PR_7","number":7,"url":"https://github.com/o/r/pull/7","title":"Later title","body":"The body now.",
        "state":"MERGED","merged":true,"mergedAt":"2024-02-01T00:00:00Z","closed":true,"closedAt":"2024-02-01T00:00:00Z",
        "isDraft":false,"baseRefName":"main","baseRefOid":"{BASE}","headRefOid":"{HEAD}",
        "createdAt":"2024-01-01T00:00:00Z","lastEditedAt":"2024-01-20T00:00:00Z",
        "additions":1,"deletions":1,"changedFiles":1,"author":{{"login":"alice"}},
        "labels":{{"totalCount":0,"nodes":[]}},
        "commits":{{"totalCount":1,"nodes":[{{"commit":{{"oid":"{HEAD}","committedDate":"2024-01-09T00:00:00Z",
          "authoredDate":"2024-01-09T00:00:00Z","messageHeadline":"Fix it"}}}}]}},
        "headCommit":{{"nodes":[{{"commit":{{"oid":"{HEAD}","checkSuites":{{"totalCount":0,"nodes":[]}},"status":null}}}}]}},
        "timelineItems":{{"pageInfo":{{"hasNextPage":false}},"nodes":[
          {{"__typename":"RenamedTitleEvent","createdAt":"2024-01-15T00:00:00Z","previousTitle":"Early title","currentTitle":"Later title"}}]}},
        "closingIssuesReferences":{{"totalCount":0,"nodes":[]}},
        "reviews":{{"totalCount":{reviews},"nodes":[
          {{"id":"R1","fullDatabaseId":1,"author":{{"login":"bob"}},"state":"COMMENTED","body":"Before the cutoff.",
            "submittedAt":"2024-01-05T00:00:00Z","lastEditedAt":null,"commit":{{"oid":"{HEAD}"}}}},
          {{"id":"R2","fullDatabaseId":2,"author":{{"login":"bob"}},"state":"APPROVED","body":"After the fix in deadbeefcafe.",
            "submittedAt":"2024-01-12T00:00:00Z","lastEditedAt":null,"commit":{{"oid":"{HEAD}"}}}}]}},
        "reviewThreads":{{"totalCount":1,"nodes":[{{"id":"T1","isResolved":true,"path":"a.py","comments":{{"totalCount":1,"nodes":[
          {{"id":"RC1","fullDatabaseId":5,"author":{{"login":"bob"}},"body":"Rename this.","createdAt":"2024-01-05T00:00:00Z","lastEditedAt":null}}]}}}}]}},
        "comments":{{"totalCount":1,"nodes":[
          {{"id":"C2","fullDatabaseId":3,"author":{{"login":"carol"}},"body":"Merged.","createdAt":"2024-02-01T00:00:00Z","lastEditedAt":null}}]}}
        }}}}}}}}"#
    )
}

fn write(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap();
}

fn read(path: &Path) -> Value {
    pyjson::loads(
        &std::fs::read_to_string(path).unwrap(),
        Duplicates::KeepLast,
    )
    .unwrap()
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(value, |value, key| {
        value
            .as_object()
            .and_then(|object| object.get(key))
            .unwrap_or_else(|| panic!("no {key} in {}", value.py_str()))
    })
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn fetch_freezes_the_pull_request_at_the_cutoff() {
    let fixture = Fixture::new("freeze");
    let output = fixture.fetch();
    assert!(output.status.success(), "{}", stderr(&output));

    let pull = at(
        &fixture.captured("graphql.json"),
        &["data", "repository", "pullRequest"],
    )
    .clone();
    let field = |key: &str| at(&pull, &[key]).clone();
    assert_eq!(field("title").py_str(), "Early title");
    assert_eq!(field("body").py_str(), "The body then.");
    assert_eq!(field("state").py_str(), "OPEN");
    assert!(!field("merged").truthy());
    assert!(matches!(field("mergedAt"), Value::Null));
    let reviews = at(&pull, &["reviews"]);
    let kept: Vec<String> = at(reviews, &["nodes"])
        .as_array()
        .unwrap()
        .iter()
        .map(|review| at(review, &["id"]).py_str())
        .collect();
    assert_eq!(kept, ["R1"]);
    assert_eq!(at(reviews, &["totalCount"]).py_str(), "1");
    assert!(
        at(&pull, &["comments", "nodes"])
            .as_array()
            .unwrap()
            .is_empty()
    );

    let issues = fixture.captured("issues.json");
    let issue = &at(&issues, &["issues"]).as_array().unwrap()[0];
    assert_eq!(at(issue, &["issue", "state"]).py_str(), "OPEN");
    assert!(matches!(at(issue, &["issue", "closedAt"]), Value::Null));

    let provenance = fixture.captured("provenance.json");
    let omitted: Vec<String> = at(&provenance, &["omitted_records"])
        .as_array()
        .unwrap()
        .iter()
        .map(|record| at(record, &["kind"]).py_str())
        .collect();
    assert!(omitted.contains(&"review".to_string()), "{omitted:?}");
    assert!(
        omitted.contains(&"conversation_comment".to_string()),
        "{omitted:?}"
    );
    let restored = at(&provenance, &["text_as_of_cutoff"]).as_array().unwrap();
    assert_eq!(
        at(&restored[0], &["text_as_of"]).py_str(),
        "2024-01-01T00:00:00Z"
    );
    let thread = &at(&pull, &["reviewThreads", "nodes"]).as_array().unwrap()[0];
    assert!(matches!(at(thread, &["isResolved"]), Value::Null));
    let resolution = at(&provenance, &["restored"])
        .as_array()
        .unwrap()
        .iter()
        .find(|record| at(record, &["field"]).py_str() == "isResolved")
        .expect("the fetched thread state is recorded");
    assert_eq!(at(resolution, &["record"]).py_str(), "review thread T1");
    assert!(at(resolution, &["at_fetch"]).truthy());

    let checked = fixture.run(&["check"]);
    assert!(checked.status.success(), "{}", stderr(&checked));
    let stdout = String::from_utf8_lossy(&checked.stdout);
    assert!(stdout.contains(&format!("ok {TASK}")), "{stdout}");
}

#[test]
fn check_reports_later_dates_excluded_commits_and_stated_facts() {
    let fixture = Fixture::new("tamper");
    assert!(fixture.fetch().status.success());
    let graphql = fixture.task().join("forge/graphql.json");
    let captured = std::fs::read_to_string(&graphql).unwrap();
    let tampered = captured
        .replace("2024-01-05T00:00:00Z", "2024-01-11T00:00:00Z")
        .replace("Before the cutoff.", "See deadbeefcafe.")
        .replace("\"isResolved\": null", "\"isResolved\": true")
        .replacen(
            "\"nameWithOwner\": \"o/r\",",
            "\"nameWithOwner\": \"o/r\", \"description\": \"Later text.\",",
            1,
        );
    write(&graphql, &tampered);
    let packet = fixture.task().join("packet.v2.md");
    let stated = std::fs::read_to_string(&packet)
        .unwrap()
        .replace(HEAD, BASE);
    write(&packet, &stated);

    let output = fixture.run(&["check", "--target", TASK]);
    assert_eq!(output.status.code(), Some(1));
    let report = stderr(&output);
    for expected in [
        "forge/graphql.json differs from its pin",
        "2024-01-11T00:00:00Z is after the cutoff",
        "abbreviates excluded commit",
        "packet.v2.md differs from its pin",
        "the packet states the head",
        "reviewThreads[0].isResolved is set",
        "graphql.json.data.repository.description is not a field the capture query requests",
    ] {
        assert!(
            report.contains(expected),
            "missing {expected:?} in {report}"
        );
    }
}

#[test]
fn fetch_refuses_an_incomplete_connection_and_a_task_without_a_cutoff() {
    let fixture = Fixture::new("refuse");
    write(&fixture.replay.join("graphql.json"), &pull_response(3));
    let output = fixture.fetch();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("the capture needs the whole connection"),
        "{}",
        stderr(&output)
    );
    assert!(!fixture.task().join("forge").exists());

    let other = fixture.root.join("bench/targets/t-other-1");
    std::fs::create_dir_all(&other).unwrap();
    std::fs::copy(
        fixture.task().join("target.json"),
        other.join("target.json"),
    )
    .unwrap();
    let missing = fixture.run(&["fetch", "--target", "t-other-1"]);
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        stderr(&missing).contains("holds no last-push cutoff"),
        "{}",
        stderr(&missing)
    );
}

#[test]
fn fetch_refuses_a_saved_response_with_fields_the_query_does_not_request() {
    let fixture = Fixture::new("unrequested");
    let response = pull_response(2).replacen(
        r#""id":"PR_7","#,
        r#""id":"PR_7","reactionGroups":[{"content":"THUMBS_UP","reactors":{"nodes":[{"login":"later"}]}}],"#,
        1,
    );
    write(&fixture.replay.join("graphql.json"), &response);
    let output = fixture.fetch();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output)
            .contains("pullRequest.reactionGroups is not a field the capture query requests"),
        "{}",
        stderr(&output)
    );
    assert!(!fixture.task().join("forge").exists());
}

#[test]
fn check_fails_when_the_named_mirrors_lack_a_generated_packet_mirror() {
    let fixture = Fixture::new("mirrors");
    let packet = fixture.task().join("packet.v2.md");
    write(
        &packet,
        "# Task\n\n## 6. Prior review state through the frozen cutoff `2024-01-10T00:00:00Z`\n",
    );
    assert!(fixture.fetch().status.success());

    let unchecked = fixture.run(&["check"]);
    assert!(unchecked.status.success(), "{}", stderr(&unchecked));

    let absent = fixture.root.join("no-mirrors");
    let output = fixture.run(&["check", "--mirrors", absent.to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("--mirrors holds no mirror at"),
        "{}",
        stderr(&output)
    );
}

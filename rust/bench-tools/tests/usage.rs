use bench_tools::pyjson::{self, Duplicates, Value};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/usage")
}

/// `usage claude --json` over the given transcripts, with the parent transcripts read for receipts only.
fn meter(transcripts: &[PathBuf], receipts: &[PathBuf]) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bench-tools"));
    command.args(["usage", "claude"]).args(transcripts);
    if !receipts.is_empty() {
        command.arg("--receipts").args(receipts);
    }
    let output = command
        .args(["--prices", "5,25", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    pyjson::loads(
        &String::from_utf8(output.stdout).unwrap(),
        Duplicates::KeepLast,
    )
    .unwrap()
}

/// Every transcript of one minimized attempt: the root session, then its subagents in name order.
fn attempt(name: &str) -> Vec<PathBuf> {
    let root = fixtures().join(name);
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            paths.push(path);
        } else if path.is_dir() {
            let mut agents: Vec<PathBuf> = std::fs::read_dir(path.join("subagents"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            agents.sort();
            paths.extend(agents);
        }
    }
    paths
}

fn complete(data: &Value) -> bool {
    matches!(at(data, &["completeness", "complete"]), Value::Bool(true))
}

fn at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter()
        .fold(value, |value, key| match key.parse::<usize>() {
            Ok(index) => &value.as_array().unwrap()[index],
            Err(_) => value
                .as_object()
                .unwrap()
                .get(key)
                .unwrap_or_else(|| panic!("no {key}")),
        })
}

fn int(value: &Value) -> i64 {
    match value {
        Value::Int(digits) => digits.parse().unwrap(),
        other => panic!("not an integer: {}", other.py_repr()),
    }
}

fn problems(value: &Value) -> Vec<String> {
    at(value, &["completeness", "problems"])
        .as_array()
        .unwrap()
        .iter()
        .map(|problem| problem.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_sole_matching_receipt_completes_a_truncated_subagent() {
    let data = meter(&attempt("att-019"), &[]);
    let repairs = at(&data, &["completeness", "repairs"]).as_array().unwrap();
    assert_eq!(repairs.len(), 1);
    assert_eq!(
        at(&repairs[0], &["agent"]).as_str(),
        Some("a8161a8c3b5dc90d0")
    );
    assert_eq!(int(at(&repairs[0], &["output_from"])), 759);
    assert_eq!(int(at(&repairs[0], &["output_to"])), 2908);
    assert!(complete(&data));
    assert!(problems(&data).is_empty(), "{:?}", problems(&data));
    assert_eq!(
        int(at(&data, &["total", "turns"])),
        143,
        "a repair adds no request"
    );
}

#[test]
fn a_repair_does_not_cover_other_unfinished_requests() {
    let data = meter(&attempt("att-029"), &[]);
    let repairs = at(&data, &["completeness", "repairs"]).as_array().unwrap();
    assert_eq!(repairs.len(), 1);
    assert_eq!(int(at(&repairs[0], &["output_to"])), 834);
    assert!(!complete(&data));
    assert_eq!(
        at(&data, &["completeness", "unfinalized"])
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let gaps = at(&data, &["completeness", "gaps"]).as_array().unwrap();
    assert_eq!(gaps.len(), 1);
    assert_eq!(at(&gaps[0], &["category"]).as_str(), Some("output"));
    assert_eq!(int(at(&gaps[0], &["gap"])), 2665);
}

/// A scratch directory holding a parent transcript and one subagent, `agent-a1.jsonl`.
struct Case {
    directory: PathBuf,
}

impl Case {
    fn new(name: &str) -> Self {
        let directory =
            std::env::temp_dir().join(format!("bench-tools-usage-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        Self { directory }
    }

    fn write(&self, name: &str, lines: &[String]) -> PathBuf {
        let path = self.directory.join(name);
        std::fs::write(
            &path,
            lines.iter().fold(String::new(), |mut out, line| {
                let _ = writeln!(out, "{line}");
                out
            }),
        )
        .unwrap();
        path
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn usage(input: u64, output: u64) -> String {
    format!(
        r#"{{"input_tokens":{input},"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":{output}}}"#
    )
}

fn assistant(request: &str, input: u64, output: u64, stop: Option<&str>, text: &str) -> String {
    let stop = stop.map_or("null".to_string(), |stop| format!("\"{stop}\""));
    format!(
        r#"{{"type":"assistant","requestId":"{request}","timestamp":"2026-10-11T00:00:00Z","message":{{"model":"m","stop_reason":{stop},"usage":{},"content":[{{"type":"text","text":"{text}"}}]}}}}"#,
        usage(input, output)
    )
}

fn receipt(input: u64, output: u64, text: &str) -> String {
    format!(
        r#"{{"type":"user","toolUseResult":{{"status":"completed","agentId":"a1","usage":{},"content":[{{"type":"text","text":"{text}"}}]}}}}"#,
        usage(input, output)
    )
}

fn session(input: u64, output: u64) -> String {
    format!(
        r#"{{"type":"cost-state","sessionId":"s","modelUsage":{{"m":{{"inputTokens":{input},"outputTokens":{output},"cacheReadInputTokens":0,"cacheCreationInputTokens":0}}}}}}"#
    )
}

fn refusals(data: &Value) -> Vec<String> {
    at(data, &["completeness", "refusals"])
        .as_array()
        .unwrap()
        .iter()
        .map(|refusal| at(refusal, &["reason"]).as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_matching_receipt_replaces_the_partial_output() {
    let case = Case::new("match");
    let agent = case.write("agent-a1.jsonl", &[assistant("r1", 10, 3, None, "done")]);
    let parent = case.write("parent.jsonl", &[receipt(10, 8, "done"), session(10, 8)]);
    let data = meter(&[agent], &[parent]);
    assert!(complete(&data), "{:?}", problems(&data));
    assert_eq!(int(at(&data, &["total", "output"])), 8);
    assert_eq!(int(at(&data, &["total", "turns"])), 1);
}

#[test]
fn an_ambiguous_or_mismatched_receipt_repairs_nothing() {
    let cases = [
        (
            vec![receipt(10, 8, "done"), receipt(10, 8, "done")],
            "found 2 completion receipts for the agent",
        ),
        (
            vec![receipt(11, 8, "done")],
            "receipt input counts differ from the final request",
        ),
        (
            vec![receipt(10, 8, "other")],
            "receipt text differs from the final request",
        ),
        (
            vec![receipt(10, 2, "done")],
            "receipt output 2 is below the recorded 3",
        ),
    ];
    for (index, (records, reason)) in cases.into_iter().enumerate() {
        let case = Case::new(&format!("refuse-{index}"));
        let agent = case.write("agent-a1.jsonl", &[assistant("r1", 10, 3, None, "done")]);
        let mut lines = records;
        lines.push(session(10, 8));
        let parent = case.write("parent.jsonl", &lines);
        let data = meter(&[agent], &[parent]);
        assert_eq!(refusals(&data), [reason]);
        assert_eq!(int(at(&data, &["total", "output"])), 3, "{reason}");
        assert!(!complete(&data), "{reason}");
    }
}

#[test]
fn a_transcript_that_continues_after_the_final_request_is_not_repaired() {
    let case = Case::new("continues");
    let error_line = r#"{"type":"assistant","isApiErrorMessage":true,"message":{"model":"<synthetic>","usage":{"input_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0,"output_tokens":0},"content":[]}}"#;
    let agent = case.write(
        "agent-a1.jsonl",
        &[assistant("r1", 10, 3, None, "done"), error_line.to_string()],
    );
    let parent = case.write("parent.jsonl", &[receipt(10, 8, "done"), session(10, 8)]);
    let data = meter(&[agent], &[parent]);
    assert_eq!(
        refusals(&data),
        ["the transcript continues after the final request"]
    );
    assert!(!complete(&data));
}

#[test]
fn a_final_end_turn_does_not_hide_an_earlier_unfinished_request() {
    let case = Case::new("earlier");
    let agent = case.write(
        "agent-a1.jsonl",
        &[
            assistant("r1", 10, 3, None, "a"),
            assistant("r2", 5, 4, Some("end_turn"), "b"),
        ],
    );
    let parent = case.write("parent.jsonl", &[session(15, 7)]);
    let data = meter(&[agent], &[parent]);
    assert_eq!(refusals(&data), [] as [String; 0]);
    let unfinalized = at(&data, &["completeness", "unfinalized"])
        .as_array()
        .unwrap();
    assert_eq!(unfinalized.len(), 1);
    assert_eq!(at(&unfinalized[0], &["request"]).as_str(), Some("r1"));
    assert!(!complete(&data));
}

#[test]
fn sums_that_differ_from_the_session_totals_are_incomplete() {
    let case = Case::new("gap");
    let agent = case.write(
        "agent-a1.jsonl",
        &[assistant("r1", 10, 3, Some("end_turn"), "a")],
    );
    let parent = case.write("parent.jsonl", &[session(10, 9)]);
    let data = meter(std::slice::from_ref(&agent), &[parent]);
    let gaps = at(&data, &["completeness", "gaps"]).as_array().unwrap();
    assert_eq!(gaps.len(), 1);
    assert_eq!(int(at(&gaps[0], &["gap"])), 6);
    assert!(!complete(&data));

    let alone = meter(&[agent], &[]);
    assert_eq!(problems(&alone), ["no session totals to reconcile against"]);
}

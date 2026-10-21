//! `delegation::filing_faults` has no `bench-tools ruling` command, because `current_grading` calls it while it checks
//! filed decisions. These cases state its faults on the agent-settled root of the
//! `ruling-agents-settlement-checks-clean` corpus case, as the retained `delegation.py` gave them before #241 removed it.

use bench_tools::delegation;
use bench_tools::pyjson::{self, Dumps, Duplicates, Value};
use bench_tools::ruling::Repo;
use bench_tools::sha256::hex_digest;
use std::path::{Path, PathBuf};

const BEFORE: &str = "docs/research/current-2026-02-01/rulings/case-1.before.json";
const AFTER: &str = "docs/research/current-2026-02-01/rulings/case-1.after.json";

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn json(text: &str) -> Value {
    pyjson::loads(text, Duplicates::KeepLast).unwrap()
}

fn dumps(value: &Value) -> String {
    pyjson::dumps(value, &Dumps::default())
}

fn decode(text: &str) -> Vec<u8> {
    let digit = |byte: u8| match byte {
        b'A'..=b'Z' => byte - b'A',
        b'a'..=b'z' => byte - b'a' + 26,
        b'0'..=b'9' => byte - b'0' + 52,
        b'+' => 62,
        _ => 63,
    };
    let bytes: Vec<u8> = text.bytes().filter(|byte| *byte != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let mut bits = 0u32;
        for (at, byte) in chunk.iter().enumerate() {
            bits |= u32::from(digit(*byte)) << (18 - 6 * at);
        }
        out.extend_from_slice(&bits.to_be_bytes()[1..chunk.len()]);
    }
    out
}

struct Root(PathBuf);

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The corpus case's root, with a filing on the before-record and the after-record's pin moved to match it.
fn root() -> Root {
    let corpus = std::fs::read_to_string(
        repository().join("tools/behavior_contracts/ruling-record.v1.json"),
    )
    .unwrap();
    let corpus = json(&corpus);
    let case = corpus
        .as_object()
        .unwrap()
        .get("cases")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .find(|case| {
            case.as_object().unwrap().get("id").unwrap().as_str()
                == Some("ruling-agents-settlement-checks-clean")
        })
        .unwrap();
    let path = std::env::temp_dir().join(format!("ruling-filing-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    for entry in case
        .as_object()
        .unwrap()
        .get("initial")
        .unwrap()
        .as_array()
        .unwrap()
    {
        let entry = entry.as_object().unwrap();
        let name =
            String::from_utf8(decode(entry.get("path_b64").unwrap().as_str().unwrap())).unwrap();
        let file = path.join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(
            file,
            decode(entry.get("bytes_b64").unwrap().as_str().unwrap()),
        )
        .unwrap();
    }
    let mut before = std::fs::read_to_string(path.join(BEFORE)).unwrap();
    before = before.replacen(
        "\"filing\": null",
        "\"filing\": {\"revision\": {\"head\": \"h1\"}, \"review\": \"r1\", \"attempt_id\": \"att-001\", \"item_id\": \"item-0\", \"family_id\": \"F1\"}",
        1,
    );
    std::fs::write(path.join(BEFORE), &before).unwrap();
    let after = std::fs::read_to_string(path.join(AFTER)).unwrap();
    let pinned = after.splitn(2, "\"sha256\": \"").collect::<Vec<_>>();
    let rest = &pinned[1][64..];
    std::fs::write(
        path.join(AFTER),
        format!(
            "{}\"sha256\": \"{}{rest}",
            pinned[0],
            hex_digest(before.as_bytes())
        ),
    )
    .unwrap();
    Root(path)
}

fn pin(root: &Path, path: &str) -> String {
    format!(
        "{{\"path\": \"{path}\", \"sha256\": \"{}\"}}",
        hex_digest(&std::fs::read(root.join(path)).unwrap())
    )
}

fn rust(root: &Path, decision: &Value, kind: &str) -> String {
    let repo = Repo::new(root.as_os_str().as_encoded_bytes());
    match delegation::filing_faults(&repo, decision, kind) {
        Ok(found) => dumps(&Value::Array(found.into_iter().map(Value::Str).collect())),
        Err(raised) => format!("RAISED {}", raised.describe()),
    }
}

#[test]
fn filing_faults_match_the_recorded_outcomes() {
    let root = root();
    let root = root.0.as_path();
    let receipt = json(&std::fs::read_to_string(root.join(delegation::POLICY)).unwrap());
    let receipt = dumps(receipt.as_object().unwrap().get("receipt").unwrap());
    let settlement = pin(root, AFTER);
    let credit = |change: &str| {
        format!(
            "{{\"target\": \"t-1\", \"revision\": {{\"head\": \"h1\"}}, \"review\": \"r1\", \"attempt_id\": \"att-001\", \
             \"item_id\": \"item-0\", \"family_id\": \"F1\", \"settled_by\": \"agents\", \"settlement\": {settlement}, \
             \"says_what\": \"yes\", \"identifies_cause\": \"yes\", \"receipt\": {receipt}, \
             \"receipt_scope\": \"Adopt policy v2 as written\", \"reason\": \"Settled by agents under policy v2.\"{change}}}"
        )
    };
    let cases = [
        (credit(""), "recovery", "[]"),
        (
            credit(", \"family_id\": \"F2\""),
            "recovery",
            r#"["before-record filing does not bind this exact subject, revision and source"]"#,
        ),
        (
            credit(", \"identifies_cause\": \"no\""),
            "recovery",
            r#"["credit filing differs from the settled facts"]"#,
        ),
        (
            credit(", \"reason\": \"Settled.\""),
            "recovery",
            r#"["reason must say agents settled under policy v2"]"#,
        ),
        (
            credit(", \"receipt_scope\": \"Adopt\""),
            "recovery",
            r#"["agent settlement must cite the owner's saved policy adoption"]"#,
        ),
        (
            credit(", \"target\": \"t-2\""),
            "recovery",
            r#"["settlement is for another decision type or target"]"#,
        ),
        (
            credit(""),
            "candidate",
            r#"["settlement is for another decision type or target", "before-record filing does not bind this exact subject, revision and source", "candidate filing differs from the settled eligibility outcome"]"#,
        ),
        (
            credit(", \"settlement\": {\"path\": \"missing.json\", \"sha256\": \"0\"}"),
            "recovery",
            r#"["agent-settled current decision needs its pinned after-record"]"#,
        ),
        (
            credit(", \"settled_by\": \"owner\""),
            "recovery",
            r#"["a settlement pin or policy adoption receipt requires settled_by agents"]"#,
        ),
        (
            credit(", \"settled_by\": \"owner\", \"settlement\": null"),
            "recovery",
            r#"["a settlement pin or policy adoption receipt requires settled_by agents"]"#,
        ),
        (
            "{\"settled_by\": \"owner\", \"settlement\": null, \"receipt\": null}".to_string(),
            "recovery",
            "[]",
        ),
        (
            "{\"settled_by\": \"agents\"}".to_string(),
            "recovery",
            r#"["agent-settled current decision needs its pinned after-record"]"#,
        ),
        (
            credit("").replace("\"review\": \"r1\", ", ""),
            "recovery",
            "RAISED KeyError: 'review'",
        ),
    ];
    for (decision, kind, wanted) in cases {
        let decision = json(&decision);
        assert_eq!(
            rust(root, &decision, kind),
            wanted,
            "{} {kind}",
            dumps(&decision)
        );
    }
}

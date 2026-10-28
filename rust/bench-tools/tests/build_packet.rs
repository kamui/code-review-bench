#[allow(dead_code, reason = "only decode is used here")]
#[path = "behavior_contracts/b64.rs"]
mod b64;

use std::path::{Path, PathBuf};
use std::process::Command;

use bench_tools::pyjson::{self, Duplicates, Value};

const REFERENCE_CASE: &str =
    "build-packet-an-originating-pull-request-reference-renders-as-the-spec";

fn scratch(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "bench-tools-build-packet-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(path.join(".home")).unwrap();
    path
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.as_object().unwrap().get(key).unwrap()
}

fn text(value: &Value, key: &str) -> String {
    field(value, key).as_str().unwrap().to_string()
}

fn decoded(value: &Value, key: &str) -> Vec<u8> {
    b64::decode(&text(value, key)).unwrap()
}

/// Lays out the reference case's inputs in `root` with `edit` applied to each file, and returns its argv.
fn reference_case(root: &Path, edit: impl Fn(&str, Vec<u8>) -> Vec<u8>) -> Vec<String> {
    let contracts = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/behavior_contracts/build-packet.v1.json");
    let corpus = pyjson::loads(
        &std::fs::read_to_string(contracts).unwrap(),
        Duplicates::Refuse,
    )
    .unwrap();
    let case = field(&corpus, "cases")
        .as_array()
        .unwrap()
        .iter()
        .find(|case| text(case, "id") == REFERENCE_CASE)
        .unwrap();
    for entry in field(case, "initial").as_array().unwrap() {
        let name = String::from_utf8(decoded(entry, "path_b64")).unwrap();
        let path = root.join(&name);
        match text(entry, "kind").as_str() {
            "directory" => std::fs::create_dir_all(path).unwrap(),
            "file" => {
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, edit(&name, decoded(entry, "bytes_b64"))).unwrap();
            }
            kind => panic!("{name}: {kind}"),
        }
    }
    field(case, "argv")
        .as_array()
        .unwrap()
        .iter()
        .map(|argument| argument.as_str().unwrap().to_string())
        .collect()
}

fn build(root: &Path, argv: &[String]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_bench-tools"))
        .arg("build-packet")
        .args(argv)
        .current_dir(root)
        .env_clear()
        .env("PATH", "/bin:/usr/bin")
        .env("HOME", root.join(".home"))
        .env("LC_ALL", "C.UTF-8")
        .env("TZ", "UTC")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(root.join("packets/a/packet.md")).unwrap()
}

#[test]
fn a_reference_on_an_unmerged_target_names_its_close_without_a_merge() {
    let root = scratch("unmerged-reference");
    let argv = reference_case(&root, |name, bytes| {
        if name != "replay/graphql.json" {
            return bytes;
        }
        let merged = r#""state": "MERGED", "merged": true, "mergedAt": "2026-03-10T12:00:00Z""#;
        let open = r#""state": "OPEN", "merged": false, "mergedAt": null"#;
        let graphql = String::from_utf8(bytes).unwrap();
        assert!(graphql.contains(merged));
        graphql.replace(merged, open).into_bytes()
    });
    let packet = build(&root, &argv);
    std::fs::remove_dir_all(&root).unwrap();
    assert!(packet.contains("| `merged` | **`false`** (not merged) |"));
    assert!(packet.contains(
        "Opened 2026-02-20 by `carol`; state `closed`, not merged; closed 2026-03-10T12:00:05Z.\n"
    ));
    assert!(!packet.contains("when the reviewed pull request merged"));
}

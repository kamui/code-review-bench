//! The one fixture root of the #156 grading corpora: a minimal repository with the `bench/` layout the grading
//! tools read. It writes inputs only. Files a tool derives, such as `inventory.json`, come from the program a
//! case runs. Corpus cases name their variant with `"fixture": "grading/<variant>"`, and `grading_fixture.rs`
//! fails when their `initial` entries or leading setup steps differ from what is built here.

use bench_tools::pyjson::{self, Dumps, Object, Value};
use bench_tools::sha256::hex_digest;
use std::collections::BTreeMap;

pub const RUN: &str = "2026-01-01-grade-test";
pub const TARGET: &str = "t-grade-1";
pub const SELECTED: &str = "codex-high";
pub const OTHER: &str = "codex-low";
/// `behavior_contract.FREEZE_PLACEHOLDER`, which the harness's `fixture-git` replaces with the commit it makes.
pub const FREEZE: &str = "<freeze commit>";
const HEAD: &str = "1111111111111111111111111111111111111111";
const DIFF: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const CURRENT: &str = "bench/grading/current";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// Both families await an eligibility ruling.
    Pending,
    /// An automated eligibility decision is proposed and not approved.
    Proposed,
    /// An approved decision whose pinned receipt was never saved.
    ApprovedWithoutReceipt,
    /// A canonical claim's equivalent link whose blind parties disagree, with no after-record.
    DisputedWithoutRuling,
    /// The same link once the owner ruled: the saved ruling and an after-record that pins the before-record.
    Ruled,
    /// A saved grade whose input fingerprint no longer matches its inputs.
    StaleFingerprint,
    /// `packet.md` changed after `target.json` pinned it.
    ChangedPinnedFile,
    /// A grading workspace whose clone has an uncommitted change.
    ModifiedClone,
    /// A run frozen on a re-cut packet through `packet_replacements`, committed by `fixture-git`.
    PacketReplacements,
}

impl Variant {
    pub const ALL: [Variant; 9] = [
        Variant::Pending,
        Variant::Proposed,
        Variant::ApprovedWithoutReceipt,
        Variant::DisputedWithoutRuling,
        Variant::Ruled,
        Variant::StaleFingerprint,
        Variant::ChangedPinnedFile,
        Variant::ModifiedClone,
        Variant::PacketReplacements,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Variant::Pending => "pending",
            Variant::Proposed => "proposed",
            Variant::ApprovedWithoutReceipt => "approved-without-receipt",
            Variant::DisputedWithoutRuling => "disputed-without-ruling",
            Variant::Ruled => "ruled",
            Variant::StaleFingerprint => "stale-fingerprint",
            Variant::ChangedPinnedFile => "changed-pinned-file",
            Variant::ModifiedClone => "modified-clone",
            Variant::PacketReplacements => "packet-replacements",
        }
    }

    pub fn parse(name: &str) -> Option<Variant> {
        Variant::ALL
            .into_iter()
            .find(|variant| variant.name() == name)
    }
}

/// A setup step a case runs before its own commands, in the harness's `steps` form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Setup {
    Command { program: String, argv: Vec<String> },
    Write { path: String, bytes: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fixture {
    pub files: BTreeMap<String, Vec<u8>>,
    pub setup: Vec<Setup>,
}

fn s(text: &str) -> Value {
    Value::Str(text.to_string())
}

fn n(number: i64) -> Value {
    Value::Int(number.to_string())
}

fn a(items: Vec<Value>) -> Value {
    Value::Array(items)
}

fn o(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect::<Object>(),
    )
}

/// `json.dumps(value, indent=2) + "\n"`, the record serialization of the grading tools.
pub fn json(value: &Value) -> Vec<u8> {
    let options = Dumps {
        indent: Some(2),
        ..Dumps::default()
    };
    let mut text = pyjson::dumps(value, &options);
    text.push('\n');
    text.into_bytes()
}

struct Builder {
    files: BTreeMap<String, Vec<u8>>,
}

impl Builder {
    fn put(&mut self, path: &str, bytes: Vec<u8>) {
        self.files.insert(path.to_string(), bytes);
    }

    fn put_json(&mut self, path: &str, value: &Value) {
        self.put(path, json(value));
    }

    fn sha(&self, path: &str) -> String {
        hex_digest(&self.files[path])
    }

    fn pin(&self, path: &str) -> Value {
        o(vec![("path", s(path)), ("sha256", s(&self.sha(path)))])
    }
}

fn revision(packet_sha256: &str) -> Value {
    o(vec![
        ("head", s(HEAD)),
        ("base_sha", s(HEAD)),
        ("packet_sha256", s(packet_sha256)),
        ("diff_manifest_sha256", s(DIFF)),
    ])
}

fn item(claim: &str, fix: Option<&str>, kind: &str) -> Value {
    o(vec![
        ("file", s("main.go")),
        ("line_start", n(3)),
        ("line_end", n(3)),
        ("claim", s(claim)),
        ("consequence", s("It breaks.")),
        ("proposed_fix", fix.map_or(Value::Null, s)),
        ("native_priority", s("P1")),
        ("native_action", s("must-fix")),
        ("native_confidence", Value::Null),
        ("kind", s(kind)),
    ])
}

fn review(items: Vec<Value>) -> Value {
    let parse_status = if items.is_empty() { "empty" } else { "parsed" };
    o(vec![
        ("arm", s("x")),
        ("parse_status", s(parse_status)),
        ("native_verdict", s("findings")),
        ("verdict_source", s("x")),
        ("items", a(items)),
        ("parse_notes", a(vec![])),
    ])
}

struct Attempt {
    id: &'static str,
    arm: &'static str,
    replicate: i64,
    disposition: &'static str,
    predecessor: Option<&'static str>,
    review: Option<Value>,
}

/// The attempts of `test_grade.ATTEMPTS`: a resolved cell, an invalid attempt and its replacement, an unselected
/// arm, and a stopped attempt with no saved output.
fn attempts() -> Vec<Attempt> {
    vec![
        Attempt {
            id: "att-001",
            arm: SELECTED,
            replicate: 1,
            disposition: "valid completed",
            predecessor: None,
            review: Some(review(vec![
                item(
                    "Races on close",
                    Some("Hold the lock while closing."),
                    "finding",
                ),
                item("Rename x", None, "finding"),
                item("Lock order is new", None, "observation"),
            ])),
        },
        Attempt {
            id: "att-002",
            arm: SELECTED,
            replicate: 2,
            disposition: "harness-invalid: read audit: 1 violation(s)",
            predecessor: None,
            review: Some(review(vec![item("Leaks the conn", None, "finding")])),
        },
        Attempt {
            id: "att-003",
            arm: SELECTED,
            replicate: 2,
            disposition: "valid completed",
            predecessor: Some("att-002"),
            review: Some(review(vec![])),
        },
        Attempt {
            id: "att-004",
            arm: OTHER,
            replicate: 1,
            disposition: "valid completed",
            predecessor: None,
            review: Some(review(vec![item("Nil map write", None, "finding")])),
        },
        Attempt {
            id: "att-005",
            arm: SELECTED,
            replicate: 3,
            disposition: "stopped: timeout",
            predecessor: None,
            review: None,
        },
    ]
}

fn attempt_record(attempt: &Attempt, payload: Option<Value>) -> Value {
    let cell = o(vec![
        ("target", s(TARGET)),
        ("arm", s(attempt.arm)),
        ("replicate", n(attempt.replicate)),
    ]);
    let saved = attempt.review.is_some();
    let normalized = o(vec![
        (
            "path",
            if saved {
                s("normalized.json")
            } else {
                Value::Null
            },
        ),
        (
            "parse_status",
            match &attempt.review {
                Some(Value::Object(review)) => {
                    review.get("parse_status").cloned().unwrap_or(Value::Null)
                }
                _ => s("unresolved"),
            },
        ),
        ("reason", if saved { Value::Null } else { s("No output.") }),
    ]);
    o(vec![
        ("schema_version", n(1)),
        ("attempt_id", s(attempt.id)),
        ("run_id", s(RUN)),
        ("cell", cell),
        ("predecessor", attempt.predecessor.map_or(Value::Null, s)),
        (
            "retry_reason",
            attempt
                .predecessor
                .map_or(Value::Null, |_| s("Replace the invalid attempt.")),
        ),
        ("replacement_index", Value::Null),
        ("continuity", s("fresh")),
        ("dispatched_at", s("2026-01-01T00:00:00Z")),
        (
            "observed",
            o(vec![
                ("harness", s("codex")),
                ("cli_version", s("0.160.0")),
                ("models", a(vec![s("example")])),
                ("effort", s("high")),
                ("prompt_hash", s(&hex_digest(b"Review the change.\n"))),
                ("prompt_header", Value::Null),
                ("prompt_registry_match", Value::Null),
                ("skill_tree", Value::Null),
                ("diff_commands", a(vec![s("git diff main...HEAD")])),
                ("sandbox", Value::Null),
                ("subagent_count", n(0)),
                ("tree_identity_before", s(&hex_digest(b"tree"))),
                ("tree_identity_after", s(&hex_digest(b"tree"))),
            ]),
        ),
        ("phase_reached", s(if saved { "result" } else { "primary" })),
        ("disposition", s(attempt.disposition)),
        ("arm_reported_complete", Value::Bool(true)),
        (
            "audit",
            o(vec![
                ("violations", a(vec![])),
                ("guidance_probes", a(vec![])),
                ("network_commands", a(vec![])),
                ("audit_file", s("audit.json")),
            ]),
        ),
        (
            "usage",
            o(vec![
                ("requests", s("usage-requests.jsonl")),
                ("priced_total_usd", Value::Float(0.04)),
                (
                    "cost_bounds_usd",
                    o(vec![
                        ("low", Value::Float(0.04)),
                        ("high", Value::Float(0.04)),
                    ]),
                ),
                ("rates_as_of", s("2026-06-24")),
                ("billing", s("api-dollars")),
                ("quota_consumed", Value::Null),
                ("metering_status", s("complete")),
            ]),
        ),
        (
            "timing",
            o(vec![
                ("dispatched_at", s("2026-01-01T00:00:00Z")),
                ("payload_validated_at", Value::Null),
                ("completed_at", s("2026-01-01T00:05:00Z")),
                ("stopped_at", Value::Null),
            ]),
        ),
        ("native_payload", payload.unwrap_or(Value::Null)),
        ("normalized", normalized),
        (
            "transcript_archive",
            o(vec![
                ("path", Value::Null),
                ("sha256", Value::Null),
                ("restoration_check", s("not-run")),
            ]),
        ),
        ("notes", a(vec![])),
    ])
}

fn family(identifier: &str, evidence: &Value) -> Value {
    o(vec![
        ("id", s(identifier)),
        ("title", s(&format!("Title of {identifier}"))),
        ("obligation", s("Preserve the behavior")),
        ("trigger", s("Concurrent use")),
        ("mechanism", s("Shared state is mutated")),
        ("grouping_reason", s("One mechanism")),
        ("evidence", a(vec![evidence.clone()])),
        (
            "eligibility",
            o(vec![
                ("state", s("pending")),
                ("reason", s("Awaiting eligibility approval")),
                ("adjudication", Value::Null),
            ]),
        ),
        (
            "impact",
            o(vec![
                ("band", s("unknown")),
                ("reason", s("Awaiting calibration")),
                ("adjudication", Value::Null),
            ]),
        ),
    ])
}

fn document(field: &str, items: Vec<Value>) -> Value {
    o(vec![("schema_version", n(1)), (field, a(items))])
}

/// The decision a ruling receipt saves, in the shape of `test_current_grading.approved`.
fn decision(status: &str, authority: &str, receipt: &str, packet_sha256: &str) -> Value {
    let pin = o(vec![
        ("path", s(receipt)),
        ("sha256", s(&hex_digest(RECEIPT_SCOPE.as_bytes()))),
    ]);
    o(vec![
        ("id", s("AD-GT-t1-eligibility")),
        ("target", s(TARGET)),
        ("revision", revision(packet_sha256)),
        ("subject", s("GT-t1")),
        ("dimension", s("eligibility")),
        ("status", s(status)),
        ("outcome", s("eligible")),
        ("authority", s(authority)),
        ("reason", s("Saved ruling")),
        ("receipt", pin.clone()),
        ("receipt_scope", s(RECEIPT_SCOPE)),
        ("boundary", Value::Null),
        ("evidence", a(vec![pin])),
        ("independent_checks", a(vec![])),
    ])
}

const RECEIPT: &str = "bench/grading/rulings/receipts/AD-GT-t1-eligibility.md";
const RECEIPT_SCOPE: &str = "Human approves GT-t1 for eligibility: eligible.";
const CASE: &str = "bench/grading/rulings/fixture/cases/CL-t-grade-1-close.md";
const RULE: &str = "bench/grading/rulings/fixture/rule.v1.md";
const BRIEF: &str = "bench/grading/rulings/fixture/brief.v1.md";
const INDEX: &str = "bench/grading/rulings/fixture/rulings.index.v1.json";
pub const RULING: &str = "bench/grading/rulings/fixture/fixture-01.md";
pub const BEFORE: &str = "bench/grading/rulings/fixture/fixture-01.before.json";
pub const AFTER: &str = "bench/grading/rulings/fixture/fixture-01.after.json";

/// The fixture root of one variant. Every call returns the same bytes.
pub fn build(variant: Variant) -> Fixture {
    let mut b = Builder {
        files: BTreeMap::new(),
    };
    let target_dir = format!("bench/targets/{TARGET}");
    let packet = format!("{target_dir}/packet.md");
    b.put(&packet, b"# Packet\n\nThe pull request.\n".to_vec());
    let original_packet = b.sha(&packet);
    let target = o(vec![
        ("id", s(TARGET)),
        ("head", s(HEAD)),
        ("base_sha", s(HEAD)),
        ("merge_base", s(HEAD)),
        ("local_base_branch", s("main")),
        ("negative_shas", a(vec![])),
        ("packet_sha256", s(&original_packet)),
        ("diff_manifest_sha256", s(DIFF)),
        ("repo", s("example/repo")),
        ("pr", n(1)),
        ("language", s("Go")),
        ("shape", s("buggy")),
        (
            "provisioning",
            o(vec![
                (
                    "allowance",
                    s("Run go test from <clone> with GOMODCACHE=<cache>/gomodcache."),
                ),
                ("unavailable", s("network")),
            ]),
        ),
    ]);
    b.put_json(&format!("{target_dir}/target.json"), &target);
    let mut selected_packet = original_packet.clone();
    if variant == Variant::PacketReplacements {
        let recut = format!("{target_dir}/packet.v2.md");
        b.put(&recut, b"Re-cut task packet\n".to_vec());
        let replacements = "docs/packet-replacements.v1.json";
        let value = o(vec![
            ("schema_version", n(1)),
            (
                "targets",
                a(vec![o(vec![
                    ("target", s(TARGET)),
                    (
                        "target_sha256",
                        s(&b.sha(&format!("{target_dir}/target.json"))),
                    ),
                    ("original", b.pin(&packet)),
                    ("replacement", b.pin(&recut)),
                ])]),
            ),
        ]);
        b.put_json(replacements, &value);
        selected_packet = b.sha(&recut);
    }

    for (name, text) in [
        ("scoring.md", "# Scoring\n\nA fixture rubric.\n"),
        ("rules.md", "# Rules\n\nA fixture rule set.\n"),
        (
            "grader.md",
            "# Grade {TARGET}\n\nFamilies: {FAMILY_IDS}\n\n{REVIEWS}\n\nAllowance: {ALLOWANCE}\n",
        ),
        (
            "scoring.second.md",
            "# Scoring\n\nThe second grader's rubric.\n",
        ),
        (
            "grader.second.md",
            "# Inventory {TARGET}\n\nFamilies: {FAMILY_IDS}\n\n{REVIEWS}\n\nAllowance: {ALLOWANCE}\n",
        ),
    ] {
        b.put(&format!("bench/rubric/{name}"), text.as_bytes().to_vec());
    }
    let mut arms = Vec::new();
    for arm in [SELECTED, OTHER] {
        let path = format!("bench/arms/{arm}.json");
        b.put_json(
            &path,
            &o(vec![
                ("id", s(arm)),
                ("model", s("example")),
                ("effort", s("high")),
            ]),
        );
        arms.push(o(vec![
            ("id", s(arm)),
            ("arm_file_sha256", s(&b.sha(&path))),
            ("resolved_skill_tree", Value::Null),
        ]));
    }

    let run_dir = format!("bench/runs/{RUN}");
    let mut cells: Vec<(String, i64)> = Vec::new();
    for attempt in attempts() {
        let directory = format!("{run_dir}/attempts/{}", attempt.id);
        let payload = attempt.review.as_ref().map(|doc| {
            b.put_json(&format!("{directory}/normalized.json"), doc);
            b.put_json(
                &format!("{directory}/payload.json"),
                &o(vec![("fixture", Value::Bool(true))]),
            );
            o(vec![
                ("path", s("payload.json")),
                ("sha256", s(&b.sha(&format!("{directory}/payload.json")))),
            ])
        });
        b.put_json(
            &format!("{directory}/attempt.json"),
            &attempt_record(&attempt, payload),
        );
        b.put(
            &format!("{directory}/usage-requests.jsonl"),
            b"{\"output_tokens\": 3}\n".to_vec(),
        );
        if !cells.contains(&(attempt.arm.to_string(), attempt.replicate)) {
            cells.push((attempt.arm.to_string(), attempt.replicate));
        }
    }
    let cohort = o(vec![
        ("target", s(TARGET)),
        ("head", s(HEAD)),
        ("base_sha", s(HEAD)),
        ("packet_sha256", s(&selected_packet)),
        ("diff_manifest_sha256", s(DIFF)),
    ]);
    let mut manifest = vec![
        ("run_id", s(RUN)),
        ("arms", a(arms)),
        ("cohort", a(vec![cohort])),
        (
            "planned_cells",
            a(cells
                .iter()
                .map(|(arm, replicate)| {
                    o(vec![
                        ("target", s(TARGET)),
                        ("arm", s(arm)),
                        ("replicate", n(*replicate)),
                    ])
                })
                .collect()),
        ),
        (
            "execution_policy",
            o(vec![
                ("allowance", s("Five minutes per command.")),
                ("branch_layout", s("`main` is the merge-base.")),
            ]),
        ),
    ];
    if variant == Variant::PacketReplacements {
        manifest.push(("frozen_at", s("2026-01-01T00:00:00Z")));
        manifest.push(("freeze_commit", s(FREEZE)));
        manifest.push((
            "packet_replacements",
            b.pin("docs/packet-replacements.v1.json"),
        ));
    }
    b.put_json(&format!("{run_dir}/manifest.json"), &o(manifest));

    b.put_json(
        "bench/scoreboard.current.json",
        &o(vec![
            ("schema_version", n(1)),
            ("contract", s("current-cohort-input/v1")),
            ("description", s("Selected fixture sources")),
            (
                "tasks",
                a(vec![o(vec![
                    ("id", s(TARGET)),
                    ("revision", revision(&selected_packet)),
                ])]),
            ),
            (
                "configurations",
                a(vec![o(vec![
                    ("id", s("setup-0")),
                    ("label", s("Setup")),
                    ("short", s("Setup")),
                    ("method", s("codex")),
                    ("version", s("fixture")),
                    ("experimental", Value::Bool(false)),
                    ("review_edition", s("builtin")),
                    ("review_change", Value::Null),
                    ("note", s("Fixture")),
                ])]),
            ),
            (
                "sources",
                a(vec![o(vec![
                    ("run", s(&format!("runs/{RUN}"))),
                    ("arm", s(SELECTED)),
                    ("configuration", s("setup-0")),
                    ("tasks", a(vec![s(TARGET)])),
                ])]),
            ),
            (
                "suites",
                a(vec![o(vec![
                    ("id", s("suite")),
                    ("title", s("Fixture")),
                    ("tasks", a(vec![s(TARGET)])),
                    ("configurations", a(vec![s("setup-0")])),
                ])]),
            ),
        ]),
    );

    let evidence = b.pin(&packet);
    let mut families = vec![family("GT-t1", &evidence), family("GT-t2", &evidence)];
    let mut decisions = Vec::new();
    match variant {
        Variant::Proposed => {
            b.put(RECEIPT, RECEIPT_SCOPE.as_bytes().to_vec());
            decisions.push(decision(
                "proposed",
                "automation",
                RECEIPT,
                &selected_packet,
            ));
        }
        Variant::ApprovedWithoutReceipt => {
            decisions.push(decision("approved", "human", RECEIPT, &selected_packet));
            if let Value::Object(first) = &mut families[0] {
                first.insert(
                    "eligibility".to_string(),
                    o(vec![
                        ("state", s("approved")),
                        ("reason", s("Saved human eligibility ruling")),
                        ("adjudication", s("AD-GT-t1-eligibility")),
                    ]),
                );
            }
        }
        _ => {}
    }
    b.put_json(
        &format!("{CURRENT}/references.json"),
        &o(vec![
            ("schema_version", n(1)),
            (
                "targets",
                a(vec![o(vec![
                    ("target", s(TARGET)),
                    ("revision", revision(&selected_packet)),
                    ("families", a(families)),
                    (
                        "control",
                        o(vec![
                            ("status", s("known-problems")),
                            ("reason", s("Provisional")),
                            ("adjudication", Value::Null),
                            ("evidence", a(vec![evidence.clone()])),
                        ]),
                    ),
                ])]),
            ),
        ]),
    );
    b.put_json(
        &format!("{CURRENT}/adjudications.json"),
        &document("decisions", decisions),
    );

    let mut claims = Vec::new();
    if matches!(variant, Variant::DisputedWithoutRuling | Variant::Ruled) {
        let review = b.pin(&format!("{run_dir}/attempts/att-001/normalized.json"));
        claims.push(o(vec![
            ("id", s("CL-t-grade-1-close")),
            ("target", s(TARGET)),
            ("revision", revision(&selected_packet)),
            (
                "claim",
                o(vec![
                    ("trigger", s("Concurrent close")),
                    ("mechanism", s("Unsynchronized close")),
                    ("consequence", s("A send can panic")),
                    ("change_relation", s("Introduced")),
                    ("settlement_question", s("Is close serialized?")),
                ]),
            ),
            (
                "evidence",
                a(vec![o(vec![
                    ("source", evidence.clone()),
                    ("stance", s("supports")),
                    ("summary", s("The packet shows the change")),
                ])]),
            ),
            (
                "links",
                a(vec![o(vec![
                    ("review", review),
                    ("attempt_id", s("att-001")),
                    ("item_id", s("item-0")),
                    ("relation", s("equivalent")),
                    ("reason", s("Same trigger and mechanism")),
                ])]),
            ),
            ("adjudication", Value::Null),
            ("family_id", Value::Null),
        ]));
        b.put(
            CASE,
            b"# CL-t-grade-1-close\n\nDoes item 0 of att-001 name the close race?\n".to_vec(),
        );
        b.put(
            RULE,
            b"# Grouping rule\n\nA link is equivalent when the item names the trigger, mechanism and consequence.\n"
                .to_vec(),
        );
        b.put(
            BRIEF,
            b"# Brief\n\nAnswer from the case file and the rule alone.\n".to_vec(),
        );
        b.put_json(
            INDEX,
            &o(vec![
                ("about", s("No earlier ruling applies to the fixture.")),
                ("rulings", o(vec![])),
            ]),
        );
        let brief = b.pin(BRIEF);
        let answer = |by: &str, outcome: &str, blind: bool| {
            o(vec![
                ("by", s(by)),
                ("model", s("example")),
                ("family", s(if blind { "example-b" } else { "example-a" })),
                ("effort", s("high")),
                ("blind", Value::Bool(blind)),
                ("exposure", s("none")),
                ("brief", if blind { brief.clone() } else { Value::Null }),
                ("written", s("before-question")),
                ("outcome", s(outcome)),
                (
                    "same_fault_as",
                    if outcome == "same-family" {
                        s("CL-t-grade-1-close")
                    } else {
                        Value::Null
                    },
                ),
                ("clauses", a(vec![])),
                ("nearest", a(vec![])),
                ("conflict", Value::Null),
                ("rule_gap", Value::Null),
                ("confidence", s("medium")),
                ("short_of_high", a(vec![s("no-precedent")])),
                ("would_settle", Value::Bool(false)),
                ("reason", s("The case file settles it.")),
            ])
        };
        b.put_json(
            BEFORE,
            &o(vec![
                ("contract", n(2)),
                ("ruling", s("fixture-01")),
                ("decision_type", s("grouping")),
                ("target", s(TARGET)),
                ("group", s("N1")),
                ("reconstructed", Value::Bool(false)),
                ("dossier", Value::Null),
                (
                    "case",
                    o(vec![
                        ("path", s(CASE)),
                        ("sha256", s(&b.sha(CASE))),
                        ("written_by", s("example")),
                        ("precedents", Value::Null),
                    ]),
                ),
                ("rule", b.pin(RULE)),
                ("clauses", Value::Null),
                ("rulings", s(INDEX)),
                ("reviews", Value::Null),
                (
                    "answers",
                    a(vec![
                        answer("recommender", "same-family", false),
                        answer("assessor-1", "separate", true),
                        answer("assessor-2", "same-family", true),
                    ]),
                ),
            ]),
        );
        if variant == Variant::Ruled {
            b.put(
                RULING,
                b"# fixture-01\n\nThe owner rules that item 0 of att-001 names the close race: same family.\n"
                    .to_vec(),
            );
            b.put_json(
                AFTER,
                &o(vec![
                    ("contract", n(2)),
                    ("ruling", s("fixture-01")),
                    ("before", b.pin(BEFORE)),
                    ("settled_by", s("owner")),
                    ("policy", Value::Null),
                    ("asked", n(1)),
                    ("outcome", s("same-family")),
                    ("by_default", Value::Bool(false)),
                    ("ground", s("Item 0 names the close race.")),
                    ("miss", Value::Null),
                    ("lesson", Value::Null),
                    ("same_fault_as", s("CL-t-grade-1-close")),
                ]),
            );
        }
    }
    b.put_json(
        &format!("{CURRENT}/claims.json"),
        &document("claims", claims),
    );

    let mut batches = Vec::new();
    if variant == Variant::StaleFingerprint {
        let directory = format!("{CURRENT}/assessments/{RUN}/{TARGET}/assessment-1");
        b.put_json(
            &format!("{directory}/verdicts.json"),
            &o(vec![("reviews", o(vec![]))]),
        );
        let stale = "0".repeat(64);
        b.put_json(
            &format!("{directory}/receipt.json"),
            &o(vec![
                ("contract", s("current-assessment-receipt/v1")),
                ("run", s(&format!("runs/{RUN}"))),
                ("target", s(TARGET)),
                ("input_fingerprint", s(&stale)),
                ("mapped_at", s("2026-01-01T00:10:00Z")),
                (
                    "provenance",
                    o(vec![("kind", s("manual")), ("identity", s("fixture"))]),
                ),
                (
                    "verdicts_sha256",
                    s(&b.sha(&format!("{directory}/verdicts.json"))),
                ),
            ]),
        );
        batches.push(o(vec![
            ("run", s(&format!("runs/{RUN}"))),
            ("target", s(TARGET)),
            ("input_fingerprint", s(&stale)),
            (
                "assessor",
                o(vec![
                    ("kind", s("manual")),
                    ("receipt", b.pin(&format!("{directory}/receipt.json"))),
                    ("verdicts", b.pin(&format!("{directory}/verdicts.json"))),
                ]),
            ),
            ("reviews", a(vec![])),
        ]));
    }
    b.put_json(
        &format!("{CURRENT}/grades.json"),
        &document("batches", batches),
    );
    b.put_json(
        &format!("{CURRENT}/candidates.json"),
        &document("candidates", vec![]),
    );
    b.put_json(
        &format!("{CURRENT}/credits.json"),
        &document("rulings", vec![]),
    );
    b.put_json(
        &format!("{CURRENT}/audits.json"),
        &o(vec![("state", s("unassessed")), ("evidence", a(vec![]))]),
    );
    b.put_json(
        &format!("{CURRENT}/validation-policy.json"),
        &o(vec![
            ("contract", s("current-validation/v1")),
            ("rubric", b.pin("bench/rubric/scoring.md")),
            ("grader", b.pin("bench/rubric/grader.md")),
            ("verdicts", s("current-verdicts/v3")),
            ("rules", b.pin("bench/rubric/rules.md")),
        ]),
    );
    b.put_json(
        "bench/rubric/remedy-policy.fixture.json",
        &o(vec![
            ("contract", s("remedy-assessment/v1")),
            (
                "validation_policy",
                b.pin(&format!("{CURRENT}/validation-policy.json")),
            ),
            (
                "first",
                o(vec![
                    ("grader", b.pin("bench/rubric/grader.md")),
                    ("rubric", b.pin("bench/rubric/scoring.md")),
                ]),
            ),
            (
                "second",
                o(vec![
                    ("grader", b.pin("bench/rubric/grader.second.md")),
                    ("rubric", b.pin("bench/rubric/scoring.second.md")),
                ]),
            ),
        ]),
    );
    b.put_json(
        &format!("{CURRENT}/remedy-policy.json"),
        &b.pin("bench/rubric/remedy-policy.fixture.json"),
    );

    let mut setup = Vec::new();
    match variant {
        Variant::ChangedPinnedFile => {
            b.put(&packet, b"# Packet\n\nThe pull request, edited.\n".to_vec());
        }
        Variant::ModifiedClone => {
            b.put("work/clone/main.go", b"package main\n".to_vec());
            setup.push(Setup::Command {
                program: "fixture-git".to_string(),
                argv: vec!["--directory".to_string(), "work/clone".to_string()],
            });
            setup.push(Setup::Write {
                path: "work/clone/main.go".to_string(),
                bytes: b"package main\n\nfunc main() {}\n".to_vec(),
            });
        }
        Variant::PacketReplacements => {
            setup.push(Setup::Command {
                program: "fixture-git".to_string(),
                argv: vec![format!("{run_dir}/manifest.json")],
            });
        }
        _ => {}
    }
    Fixture {
        files: b.files,
        setup,
    }
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard base64 with padding, as the corpus format's `*_b64` fields use.
pub fn base64(bytes: &[u8]) -> String {
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let word = chunk.iter().enumerate().fold(0u32, |word, (at, byte)| {
            word | u32::from(*byte) << (16 - 8 * at)
        });
        for at in 0..4 {
            if at <= chunk.len() {
                text.push(char::from(ALPHABET[(word >> (18 - 6 * at) & 63) as usize]));
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// The harness's `initial` entries for the fixture's files, in path order.
pub fn initial(fixture: &Fixture) -> Value {
    a(fixture
        .files
        .iter()
        .map(|(path, bytes)| {
            o(vec![
                ("kind", s("file")),
                ("path_b64", s(&base64(path.as_bytes()))),
                ("bytes_b64", s(&base64(bytes))),
            ])
        })
        .collect())
}

/// The harness's leading `steps` entries for the fixture's setup, without the expectations a case adds.
pub fn setup_steps(fixture: &Fixture) -> Vec<Value> {
    fixture
        .setup
        .iter()
        .map(|step| match step {
            Setup::Command { program, argv } => o(vec![
                ("program", s(program)),
                ("argv", a(argv.iter().map(|arg| s(arg)).collect())),
            ]),
            Setup::Write { path, bytes } => o(vec![(
                "write",
                a(vec![o(vec![
                    ("kind", s("file")),
                    ("path_b64", s(&base64(path.as_bytes()))),
                    ("bytes_b64", s(&base64(bytes))),
                ])]),
            )]),
        })
        .collect()
}

//! `ruling replay`: which saved decisions a delegation policy's agreement gates would have left to agents, and
//! how often the decision that stands is the answer the parties agreed on. It reads records, settles nothing
//! and grants no authority.

use super::summary::Pair;
use super::{Repo, record};
use crate::delegation::v3::{self, Bar, Counted, Gate};
use crate::pyjson::{Object, Value};
use crate::pyvalue::{self, Py, item};

pub use crate::delegation::v3::{POLICY, Policy, policy};

const OWNER: &str = "owner";

struct Difference {
    path: String,
    agreed: String,
    decided: Vec<String>,
}

/// One decision type's replay.
struct Row {
    kind: &'static str,
    gate: Option<Gate>,
    decisions: u64,
    never_needed: u64,
    unread: u64,
    /// Decisions on which the parties gave the agreement the gate names.
    agreeing: u64,
    passed: u64,
    agreed: u64,
    pull_requests: Vec<Value>,
    /// Each agreed outcome with how many decisions passed the gate on it and how many of those stand.
    outcomes: Vec<(Value, u64, u64)>,
    differences: Vec<Difference>,
    /// Each reason an agreed answer still went to the owner, with its decisions; one decision can have several.
    held: Vec<(String, u64)>,
}

impl Row {
    fn met(&self, bar: &Bar) -> bool {
        bar.met(self.passed, self.pull_requests.len() as u64, self.agreed)
    }
}

pub struct Replay {
    source: String,
    status: Value,
    bar: Bar,
    gaps_ask: bool,
    rows: Vec<Row>,
}

/// `replay(pairs, policy)`: every decided pair counted under its decision type's gate.
pub fn replay(repo: &Repo, pairs: &[Pair], policy: Policy, source: &str) -> Py<Replay> {
    let mut rows: Vec<Row> = policy
        .gates
        .iter()
        .map(|(kind, gate)| Row {
            kind,
            gate: *gate,
            decisions: 0,
            never_needed: 0,
            unread: 0,
            agreeing: 0,
            passed: 0,
            agreed: 0,
            pull_requests: Vec::new(),
            outcomes: Vec::new(),
            differences: Vec::new(),
            held: Vec::new(),
        })
        .collect();
    for (position, pair) in pairs.iter().enumerate() {
        if !pair.after.truthy() {
            continue;
        }
        let before = &pair.before;
        let kind = item(before, "decision_type")?;
        let Some(row) = rows.iter_mut().find(|row| pyvalue::is(kind, row.kind)) else {
            continue;
        };
        row.decisions += 1;
        let Some(gate) = row.gate else {
            continue;
        };
        let (first, decisions, stands) =
            match v3::count(repo, position, pairs, gate, policy.gaps_ask)? {
                Counted::NeverNeeded => {
                    row.never_needed += 1;
                    continue;
                }
                Counted::Unread => {
                    row.unread += 1;
                    continue;
                }
                Counted::Disagreed => continue,
                Counted::Held(reasons) => {
                    row.agreeing += 1;
                    for reason in reasons {
                        match row.held.iter_mut().find(|(known, _)| *known == reason) {
                            Some(slot) => slot.1 += 1,
                            None => row.held.push((reason, 1)),
                        }
                    }
                    continue;
                }
                Counted::Passed {
                    first,
                    decisions,
                    stands,
                } => (first, decisions, stands),
            };
        row.agreeing += 1;
        row.passed += 1;
        row.agreed += u64::from(stands);
        let target = item(before, "target")?;
        pyvalue::hashable(target)?;
        if !row.pull_requests.iter().any(|known| known.py_eq(target)) {
            row.pull_requests.push(target.clone());
        }
        let outcome = item(&first, "outcome")?;
        pyvalue::hashable(outcome)?;
        let at = match row
            .outcomes
            .iter()
            .position(|(known, _, _)| known.py_eq(outcome))
        {
            Some(at) => at,
            None => {
                row.outcomes.push((outcome.clone(), 0, 0));
                row.outcomes.len() - 1
            }
        };
        row.outcomes[at].1 += 1;
        row.outcomes[at].2 += u64::from(stands);
        if !stands {
            let mut decided = Vec::new();
            for decision in &decisions {
                decided.push(record::pick(decision)?);
            }
            row.differences.push(Difference {
                path: pair.path.clone(),
                agreed: record::pick(&first)?,
                decided,
            });
        }
    }
    for row in &mut rows {
        row.pull_requests = pyvalue::sorted(std::mem::take(&mut row.pull_requests))?;
        let outcomes = pyvalue::sorted(
            row.outcomes
                .iter()
                .map(|(outcome, _, _)| outcome.clone())
                .collect(),
        )?;
        row.outcomes = outcomes
            .into_iter()
            .filter_map(|outcome| {
                row.outcomes
                    .iter()
                    .find(|(known, _, _)| known.py_eq(&outcome))
                    .cloned()
            })
            .collect();
    }
    Ok(Replay {
        source: source.to_string(),
        status: policy.status,
        bar: policy.bar,
        gaps_ask: policy.gaps_ask,
        rows,
    })
}

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn int(value: impl ToString) -> Value {
    Value::Int(value.to_string())
}

impl Replay {
    /// The replay as `--json` prints it.
    pub fn to_value(&self) -> Value {
        let mut types = Object::default();
        for row in &self.rows {
            let mut members = vec![
                (
                    "gate",
                    row.gate.map_or_else(
                        || pyvalue::text(OWNER),
                        |gate| pyvalue::text(&gate.describe()),
                    ),
                ),
                ("decisions", int(row.decisions)),
            ];
            if row.gate.is_some() {
                members.extend([
                    ("never_needed", int(row.never_needed)),
                    ("settled_by_agents_unread", int(row.unread)),
                    ("parties_agreed", int(row.agreeing)),
                    (
                        "held",
                        Value::Array(
                            row.held
                                .iter()
                                .map(|(reason, decisions)| {
                                    object(vec![
                                        ("reason", pyvalue::text(reason)),
                                        ("decisions", int(decisions)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                    ("passed", int(row.passed)),
                    ("stands", int(row.agreed)),
                    ("pull_requests", Value::Array(row.pull_requests.clone())),
                    ("entry_bar_met", Value::Bool(row.met(&self.bar))),
                    (
                        "outcomes",
                        Value::Array(
                            row.outcomes
                                .iter()
                                .map(|(outcome, passed, agreed)| {
                                    object(vec![
                                        ("outcome", outcome.clone()),
                                        ("passed", int(passed)),
                                        ("stands", int(agreed)),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                    (
                        "differences",
                        Value::Array(
                            row.differences
                                .iter()
                                .map(|difference| {
                                    object(vec![
                                        ("path", pyvalue::text(&difference.path)),
                                        ("agreed", pyvalue::text(&difference.agreed)),
                                        (
                                            "decided",
                                            Value::Array(
                                                difference
                                                    .decided
                                                    .iter()
                                                    .map(|pick| pyvalue::text(pick))
                                                    .collect(),
                                            ),
                                        ),
                                    ])
                                })
                                .collect(),
                        ),
                    ),
                ]);
            }
            types.insert(row.kind.to_string(), object(members));
        }
        object(vec![
            ("policy", pyvalue::text(&self.source)),
            ("status", self.status.clone()),
            ("gap_or_conflict_asks", Value::Bool(self.gaps_ask)),
            (
                "entry",
                object(vec![
                    ("decisions", int(self.bar.decisions)),
                    ("pull_requests", int(self.bar.pull_requests)),
                    ("agreement_percent", int(self.bar.agreement_percent)),
                ]),
            ),
            ("decision_types", Value::Object(types)),
        ])
    }

    pub fn render(&self) -> String {
        let mut lines = vec![
            format!("Policy: {}, {}", self.source, self.status.py_str()),
            format!(
                "Entry bar: {} decisions that pass the gate on {} pull requests, {}% of them the decision that stands.",
                self.bar.decisions, self.bar.pull_requests, self.bar.agreement_percent
            ),
            format!(
                "A gap or conflict that any party states: {}.",
                if self.gaps_ask {
                    "the decision goes to the owner"
                } else {
                    "an agreed answer still passes"
                }
            ),
            String::new(),
            "| Decision type | Gate | Decisions | Never needed | Parties agreed | Passed the gate | Stands | Pull requests | Entry bar |".into(),
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- |".into(),
        ];
        for row in &self.rows {
            lines.push(match row.gate {
                None => format!("| {} | {OWNER} | {} | | | | | | |", row.kind, row.decisions),
                Some(gate) => format!(
                    "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                    row.kind,
                    gate.describe(),
                    row.decisions,
                    row.never_needed,
                    row.agreeing,
                    row.passed,
                    row.agreed,
                    row.pull_requests.len(),
                    if row.met(&self.bar) { "met" } else { "not met" }
                ),
            });
        }
        let mut section = |title: &str, items: Vec<String>| {
            if !items.is_empty() {
                lines.extend([String::new(), title.to_string()]);
                lines.extend(items);
            }
        };
        section(
            "Agreed answers that still go to the owner, by reason; one decision can have several:",
            self.rows
                .iter()
                .flat_map(|row| {
                    row.held
                        .iter()
                        .map(|(reason, decisions)| format!("- {} {reason}: {decisions}", row.kind))
                })
                .collect(),
        );
        section(
            "Answers that passed the gate, by outcome, as stands / passed:",
            self.rows
                .iter()
                .flat_map(|row| {
                    row.outcomes.iter().map(|(outcome, passed, agreed)| {
                        format!("- {} {}: {agreed} / {passed}", row.kind, outcome.py_str())
                    })
                })
                .collect(),
        );
        section(
            "Decided differently from the agreed answer:",
            self.rows
                .iter()
                .flat_map(|row| &row.differences)
                .map(|difference| {
                    format!(
                        "- {}: agreed {}; decided {}",
                        difference.path,
                        difference.agreed,
                        difference.decided.join(" and ")
                    )
                })
                .collect(),
        );
        section(
            "Settled by agents and not read by the owner, counted on neither side:",
            self.rows
                .iter()
                .filter(|row| row.unread > 0)
                .map(|row| format!("- {}: {}", row.kind, row.unread))
                .collect(),
        );
        lines.extend([
            String::new(),
            "The owner made these decisions with the parties' picks visible, so agreement here is an upper bound. The replay settles nothing and grants no authority.".into(),
        ]);
        lines.join("\n")
    }
}

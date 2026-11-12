//! `ruling replay`: which saved decisions a delegation policy's agreement gates would have left to agents, and
//! how often the decision that stands is the answer the parties agreed on. It reads records, settles nothing
//! and grants no authority.

use super::summary::{self, Pair};
use super::{Repo, record};
use crate::delegation;
use crate::pyjson::{Object, Value};
use crate::pytext::PyPath;
use crate::pyvalue::{self, Py, Raised, get, item};

pub const POLICY: &str = "bench/grading/delegation.v3.json";
const OWNER: &str = "owner";
const AFTER_ADMISSION: &str = "after-admission";
const NEVER_NEEDED: &str = "not-applicable";
const GATE_KEYS: [&str; 3] = ["blind_answers", "recommender_confidence", "asked"];
const GAP_NAMES: [&str; 2] = ["gap", "conflict"];

#[derive(Clone, Copy, PartialEq)]
enum Confidence {
    Any,
    MediumOrHigh,
}

#[derive(Clone, Copy)]
struct Gate {
    blind_answers: usize,
    recommender: Confidence,
    after_admission: bool,
}

impl Gate {
    fn describe(self) -> String {
        let parties = match self.blind_answers {
            0 => "the recommender's answer".to_string(),
            1 => "the recommender and 1 blind answer agree".to_string(),
            count => format!("the recommender and {count} blind answers agree"),
        };
        let confidence = match self.recommender {
            Confidence::Any => "",
            Confidence::MediumOrHigh => ", recommender at medium or high confidence",
        };
        let asked = if self.after_admission {
            ", asked after admission"
        } else {
            ""
        };
        format!("{parties}{confidence}{asked}")
    }
}

struct Bar {
    decisions: u64,
    pull_requests: u64,
    agreement_percent: u64,
}

pub struct Policy {
    status: Value,
    bar: Bar,
    /// Whether a gap or conflict that any party states sends an agreed answer to the owner.
    gaps_ask: bool,
    gates: Vec<(&'static str, Option<Gate>)>,
}

fn refused<T>(what: impl AsRef<str>) -> Py<T> {
    Err(Raised::Value(format!(
        "unsupported replay policy: {}",
        what.as_ref()
    )))
}

fn whole(value: &Value, name: &str) -> Py<u64> {
    match value {
        Value::Int(digits) => digits
            .parse()
            .or_else(|_| refused(format!("`{name}` must be a whole number"))),
        _ => refused(format!("`{name}` must be a whole number")),
    }
}

fn gate(name: &str, blind: usize, rule: &Value) -> Py<Option<Gate>> {
    if pyvalue::is(rule, OWNER) {
        return Ok(None);
    }
    let Value::Object(members) = rule else {
        return refused(format!("`{name}` must be \"{OWNER}\" or a gate"));
    };
    if members
        .iter()
        .any(|(key, _)| !GATE_KEYS.contains(&key.as_str()))
    {
        return refused(format!(
            "`{name}` has a gate field the replay does not apply"
        ));
    }
    if whole(get(rule, "blind_answers")?, "blind_answers")? != blind as u64 {
        return refused(format!(
            "`{name}` must name the {blind} blind answers its records hold"
        ));
    }
    let confidence = get(rule, "recommender_confidence")?;
    let recommender = if pyvalue::is(confidence, "any") {
        Confidence::Any
    } else if pyvalue::is(confidence, "medium-or-high") {
        Confidence::MediumOrHigh
    } else {
        return refused(format!(
            "`{name}` needs `recommender_confidence` of \"any\" or \"medium-or-high\""
        ));
    };
    let asked = get(rule, "asked")?;
    if asked.truthy() && !pyvalue::is(asked, AFTER_ADMISSION) {
        return refused(format!("`{name}` has an `asked` the replay does not apply"));
    }
    Ok(Some(Gate {
        blind_answers: blind,
        recommender,
        after_admission: asked.truthy(),
    }))
}

/// The version 3 policy at `source`, proposed or adopted. A rule the replay would not apply as written is
/// refused, so a replayed gate is never looser than the policy's text.
pub fn policy(repo: &Repo, source: &PyPath) -> Py<Policy> {
    let value = repo.load(source)?;
    if !get(&value, "version")?.py_eq(&Value::Int("3".into())) {
        return refused("the replay reads version 3");
    }
    if !pyvalue::one_of(get(&value, "status")?, &["proposed", "adopted"]) {
        return refused("`status` must be \"proposed\" or \"adopted\"");
    }
    let asks = get(&value, "always_ask")?;
    let full = delegation::always_ask();
    let agreed_through_gaps = Value::Array(
        pyvalue::iterate(&full)?
            .into_iter()
            .filter(|name| !pyvalue::one_of(name, &GAP_NAMES))
            .collect(),
    );
    let gaps_ask = asks.py_eq(&full);
    if !gaps_ask && !asks.py_eq(&agreed_through_gaps) {
        return refused(
            "`always_ask` must be policy v2's list in its order, with or without `gap` and `conflict`",
        );
    }
    if !pyvalue::is(get(&value, "written")?, "before-question") {
        return refused("`written` must keep policy v2's value");
    }
    let entry = get(&value, "entry")?;
    if !matches!(entry, Value::Object(_)) {
        return refused("`entry` must give the entry bar");
    }
    let bar = Bar {
        decisions: whole(get(entry, "decisions")?, "decisions")?,
        pull_requests: whole(get(entry, "pull_requests")?, "pull_requests")?,
        agreement_percent: whole(get(entry, "agreement_percent")?, "agreement_percent")?,
    };
    if bar.agreement_percent > 100 {
        return refused("`agreement_percent` is at most 100");
    }
    let types = get(&value, "decision_types")?;
    let Value::Object(named) = types else {
        return refused("`decision_types` must name every decision type");
    };
    let mut gates = Vec::new();
    for (name, _, blind) in record::DECISION_TYPES {
        if named.get(name).is_none() {
            return refused(format!("`decision_types` does not name `{name}`"));
        }
        gates.push((name, gate(name, blind, get(types, name)?)?));
    }
    if named.len() != gates.len() {
        return refused("`decision_types` names a decision type the records do not have");
    }
    Ok(Policy {
        status: get(&value, "status")?.clone(),
        bar,
        gaps_ask,
        gates,
    })
}

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
        self.passed >= bar.decisions
            && self.pull_requests.len() as u64 >= bar.pull_requests
            && self.agreed * 100 >= bar.agreement_percent * self.passed
    }
}

pub struct Replay {
    source: String,
    status: Value,
    bar: Bar,
    gaps_ask: bool,
    rows: Vec<Row>,
}

const UNCOUNTED: &str =
    "first answers that do not count: contract 1, reconstructed, same-family or written late";
const REVIEW: &str = "a review of a saved ruling";
const RESERVED: &str = "an outcome reserved to the owner";

/// Whether the parties gave the agreement the gate names.
fn agreement(repo: &Repo, before: &Value, gate: Gate) -> Py<bool> {
    let (_, first) = record::recommender(before)?;
    Ok(summary::unanimous(repo, before, gate.blind_answers)?
        && (gate.recommender == Confidence::Any
            || pyvalue::one_of(item(&first, "confidence")?, &["medium", "high"])))
}

/// Why an agreed answer still goes to the owner: first answers that do not count, or what `always_ask` reserves.
fn held(before: &Value, gaps_ask: bool) -> Py<Vec<String>> {
    let mut reasons = Vec::new();
    if !summary::eligible(before)? {
        reasons.push(UNCOUNTED.to_string());
    }
    if item(before, "reviews")?.truthy() {
        reasons.push(REVIEW.to_string());
    }
    for reason in delegation::answer_reasons(before)? {
        let reason = match reason.split_once(": ") {
            Some((_, said)) if said.ends_with("stays with the owner") => RESERVED.to_string(),
            Some((_, said)) => said.to_string(),
            None => reason,
        };
        if !reasons.contains(&reason) && (gaps_ask || reason != delegation::GAP_REASON) {
            reasons.push(reason);
        }
    }
    Ok(reasons)
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
        let decisions = summary::final_decisions(repo, position, pairs)?;
        let mut unneeded = gate.after_admission && !decisions.is_empty();
        for decision in &decisions {
            unneeded = unneeded && pyvalue::is(item(decision, "outcome")?, NEVER_NEEDED);
        }
        if unneeded {
            row.never_needed += 1;
            continue;
        }
        if pyvalue::is(get(&pair.after, "settled_by")?, "agents")
            && summary::successors(repo, position, pairs)?.is_empty()
        {
            row.unread += 1;
            continue;
        }
        if !agreement(repo, before, gate)? {
            continue;
        }
        row.agreeing += 1;
        let reasons = held(before, policy.gaps_ask)?;
        if !reasons.is_empty() {
            for reason in reasons {
                match row.held.iter_mut().find(|(known, _)| *known == reason) {
                    Some(slot) => slot.1 += 1,
                    None => row.held.push((reason, 1)),
                }
            }
            continue;
        }
        let (_, first) = record::recommender(before)?;
        let stands = summary::agrees(&first, &decisions)?;
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

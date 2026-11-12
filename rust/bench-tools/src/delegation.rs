//! `delegation.py`: earned-confidence routing and the common filing boundary for policy v2. A decision goes to
//! agents only through an adopted policy with its saved receipt, so a missing or changed policy keeps every
//! decision with the owner.

use crate::pyjson::Value;
use crate::pytext::PyPath;
use crate::pyvalue::{self, Py, Raised, get, item};
use crate::ruling::summary::{self, Pair};
use crate::ruling::{Repo, record};

pub mod v3;

pub const POLICY: &str = "bench/grading/delegation.v2.json";
const GATE: &str = r#"{"blind_answers": 2, "blind_confidence": "high", "recommender": "same-outcome",
    "recommender_confidence": "medium-or-high", "written": "before-question"}"#;
const ALWAYS_ASK: &str =
    r#"["cannot-tell", "gap", "conflict", "relied-on", "problem", "reviews-saved-ruling"]"#;
const TYPES: [(&str, &str); 6] = [
    ("recovery", "open"),
    ("candidate", "open"),
    (
        "reconciliation",
        "reconciler, 5 per audit round to the owner",
    ),
    ("band", "owner"),
    ("grouping", "owner"),
    ("control", "owner"),
];
const LEVELS: &str = r#"[{"level": 1, "decisions": 15, "pull_requests": 4, "owner_reads": "digest"},
    {"level": 2, "decisions": 30, "pull_requests": 5, "owner_reads": "1-in-5"}]"#;

fn literal(text: &str) -> Value {
    pyvalue::loads(text).unwrap_or(Value::Null)
}

/// The `always_ask` list that [`answer_reasons`] and the saved-ruling check apply.
pub fn always_ask() -> Value {
    literal(ALWAYS_ASK)
}

fn types() -> Value {
    Value::Object(
        TYPES
            .iter()
            .map(|(name, route)| ((*name).to_string(), pyvalue::text(route)))
            .collect(),
    )
}

/// The adopted policy at `source`, `None` when there is none or it is still proposed without a receipt. A
/// changed rule, or an adoption without its saved receipt and verbatim scope, is refused.
pub fn policy(repo: &Repo, source: &PyPath) -> Py<Option<Value>> {
    if !pyvalue::is_file(source) {
        return Ok(None);
    }
    let value = repo.load(source)?;
    let supported = get(&value, "version")?.py_eq(&Value::Int("2".into()))
        && get(&value, "gate")?.py_eq(&literal(GATE))
        && get(&value, "always_ask")?.py_eq(&literal(ALWAYS_ASK))
        && get(&value, "levels")?.py_eq(&literal(LEVELS))
        && get(&value, "decision_types")?.py_eq(&types());
    if !supported {
        return Err(Raised::Value(
            "unsupported delegation policy; a changed rule needs a new version".into(),
        ));
    }
    if pyvalue::is(get(&value, "status")?, "proposed")
        && matches!(get(&value, "receipt")?, Value::Null)
    {
        return Ok(None);
    }
    let receipt = get(&value, "receipt")?;
    let adopted = pyvalue::is(get(&value, "status")?, "adopted")
        && record::pinned(repo, receipt)?
        && get(&value, "adoption_scope")?.truthy()
        && {
            let text = pyvalue::read_text(&repo.path(item(receipt, "path")?)?)?;
            pyvalue::contains(&Value::Str(text), item(&value, "adoption_scope")?)?
        };
    if !adopted {
        return Err(Raised::Value(
            "policy v2 needs the saved adoption receipt and its verbatim scope".into(),
        ));
    }
    Ok(Some(value.clone()))
}

/// The recommender's model and the blind assessors' models, sorted.
pub fn setup(before: &Value) -> Py<(Value, Vec<Value>)> {
    let (_, first) = record::recommender(before)?;
    let model = item(&first, "model")?.clone();
    let mut blind = Vec::new();
    for answer in pyvalue::iterate(item(before, "answers")?)? {
        if item(&answer, "blind")?.truthy() {
            blind.push(item(&answer, "model")?.clone());
        }
    }
    Ok((model, pyvalue::sorted(blind)?))
}

pub fn same_setup(left: &(Value, Vec<Value>), right: &(Value, Vec<Value>)) -> bool {
    left.0.py_eq(&right.0)
        && left.1.len() == right.1.len()
        && left.1.iter().zip(&right.1).all(|(a, b)| a.py_eq(b))
}

/// The rounds the opening snapshot of the pair's round pins as closed. Each failure is a `ValueError` that
/// `route` gives as its reason.
pub fn opening_rounds(repo: &Repo, current: usize, pairs: &[Pair]) -> Py<Vec<Value>> {
    let pair = &pairs[current];
    let pin = &pair.opening;
    if !record::pinned(repo, pin)? || !get(&pair.before, "round_opening")?.py_eq(pin) {
        return Err(Raised::Value(
            "the before-record must pin its round's immutable opening snapshot".into(),
        ));
    }
    let opening = repo.load(&repo.path(item(pin, "path")?)?)?;
    let names_round = get(&opening, "round")?.py_eq(&pair.round);
    let commit_named = names_round && {
        match pyvalue::get_or(&opening, "source_commit", &pyvalue::text(""))? {
            Value::Str(commit) => {
                commit.len() == 40
                    && commit
                        .bytes()
                        .all(|byte| matches!(byte, b'a'..=b'f' | b'0'..=b'9'))
            }
            other => {
                return Err(Raised::Type(format!(
                    "expected string or bytes-like object, got '{}'",
                    other.type_name()
                )));
            }
        }
    };
    if !commit_named {
        return Err(Raised::Value(
            "round opening must name its round and the source commit".into(),
        ));
    }
    let mut closed: Vec<Value> = Vec::new();
    for prior in pyvalue::iterate(item(&opening, "closed_rounds")?)? {
        if !record::pinned(repo, &prior)? {
            return Err(Raised::Value(
                "a closed round pinned at opening changed".into(),
            ));
        }
        let sheet = repo.load(&repo.path(item(&prior, "path")?)?)?;
        let distinct = matches!(get(&sheet, "closed")?, Value::Bool(true))
            && !item(&sheet, "round")?.py_eq(&pair.round)
            && {
                let round = item(&sheet, "round")?;
                pyvalue::hashable(round)?;
                !closed.iter().any(|known| known.py_eq(round))
            };
        if !distinct {
            return Err(Raised::Value(
                "opening snapshot needs distinct earlier closed rounds".into(),
            ));
        }
        let round = item(&sheet, "round")?;
        for other in pairs {
            if other.round.py_eq(round) {
                let directory = summary::parent(&other.path);
                let sheet_path = if directory == "." {
                    "round.json".to_string()
                } else {
                    format!("{directory}/round.json")
                };
                if !pyvalue::text(&sheet_path).py_eq(item(&prior, "path")?) {
                    return Err(Raised::Value(
                        "opening snapshot names a different round directory".into(),
                    ));
                }
            }
        }
        closed.push(round.clone());
    }
    Ok(closed)
}

/// The level a decision earned and every reason it goes to the owner; an empty list means agents settle it.
pub struct Routed {
    pub level: u64,
    /// The policy version that routed it, 0 when its round's policy is refused; version 3 settles on agreement,
    /// with no level.
    pub version: u64,
    pub reasons: Vec<String>,
}

/// The `policy` pin of the opening the before-record pins, `None` for a round that runs under policy v2 as it
/// always has. An opening that cannot be read is left to policy v2, which refuses it.
fn round_pin(repo: &Repo, before: &Value) -> Option<Value> {
    let opening = get(before, "round_opening").ok()?;
    if !record::pinned(repo, opening).ok()? {
        return None;
    }
    let snapshot = repo
        .load(&repo.path(item(opening, "path").ok()?).ok()?)
        .ok()?;
    snapshot.as_object()?.get("policy").cloned()
}

/// The policy a round opening pins, read for routing.
enum Governing {
    V2(PyPath),
    V3(Box<v3::Policy>),
    /// Why the pin grants nothing: every decision of the round goes to the owner.
    Refused(String),
}

fn governing(repo: &Repo, pin: &Value) -> Py<(Option<PyPath>, Governing)> {
    if !record::pinned(repo, pin).unwrap_or(false) {
        return Ok((
            None,
            Governing::Refused(
                "the round opening's `policy` must pin a policy file by path and sha256".into(),
            ),
        ));
    }
    let named = item(pin, "path")?;
    let path = repo.path(named)?;
    let read = if pyvalue::is(named, POLICY) {
        policy(repo, &path).map(|_| Governing::V2(path.clone()))
    } else if pyvalue::is(named, v3::POLICY) {
        v3::policy(repo, &path).and_then(|policy| {
            Ok(if v3::enforced_terms(repo, &path)? {
                Governing::V3(Box::new(policy))
            } else {
                Governing::Refused(
                    "the round opening's policy v3 states terms other than those this tool enforces; a changed rule needs a new version".into(),
                )
            })
        })
    } else {
        Ok(Governing::Refused(format!(
            "the round opening's `policy` must pin {POLICY} or {}",
            v3::POLICY
        )))
    };
    let governing = read.unwrap_or_else(|raised| {
        Governing::Refused(format!(
            "the round opening's policy is refused: {}",
            raised
                .caught()
                .map_or_else(|| raised.describe(), str::to_string)
        ))
    });
    Ok((Some(path), governing))
}

/// Whether the before-record's round runs under an adopted policy v3: the same reading of the opening's pin that
/// routing uses. A round whose pin is refused runs under no policy.
pub fn under_adopted_v3(repo: &Repo, before: &Value) -> Py<bool> {
    let Some(pin) = round_pin(repo, before) else {
        return Ok(false);
    };
    match governing(repo, &pin)?.1 {
        Governing::V3(policy) => v3::adopted(repo, &policy),
        Governing::V2(_) | Governing::Refused(_) => Ok(false),
    }
}

/// `route(before, root, policy_path, pairs, before_path)`, with `policy_path` named by the caller.
pub fn route(
    repo: &Repo,
    before: &Value,
    policy_path: &PyPath,
    pairs: Option<&[Pair]>,
    before_path: Option<&PyPath>,
) -> Py<Routed> {
    route_under(repo, before, Some(policy_path), pairs, before_path)
}

/// Route a before-record under the policy its round opening pins, or policy v2 at `named` (by default
/// [`POLICY`]) for a round whose opening pins none. A `named` policy that is not the pinned one is a reason to ask,
/// never an override.
pub fn route_under(
    repo: &Repo,
    before: &Value,
    named: Option<&PyPath>,
    pairs: Option<&[Pair]>,
    before_path: Option<&PyPath>,
) -> Py<Routed> {
    let Some(pin) = round_pin(repo, before) else {
        let default = repo.join(POLICY);
        return route_v2(repo, before, named.unwrap_or(&default), pairs, before_path);
    };
    let (path, governing) = governing(repo, &pin)?;
    let mut reasons = Vec::new();
    if let Some(named) = named
        && path.as_ref().is_none_or(|path| {
            crate::pyfs::resolve(named.as_bytes()) != crate::pyfs::resolve(path.as_bytes())
        })
    {
        reasons.push(format!(
            "the policy named for routing, {}, is not the one the round opening pins",
            crate::pytext::stderr_text(named.as_bytes())
        ));
    }
    let mut routed = match governing {
        Governing::V2(path) => route_v2(repo, before, &path, pairs, before_path)?,
        Governing::V3(policy) => route_v3(repo, before, &policy, pairs, before_path)?,
        Governing::Refused(reason) => {
            let mut refused = vec![reason];
            refused.extend(record::reasons(repo, before)?);
            Routed {
                level: 0,
                version: 0,
                reasons: refused,
            }
        }
    };
    reasons.append(&mut routed.reasons);
    routed.reasons = reasons;
    Ok(routed)
}

/// The position of the before-record among `pairs`, by `before_path` when given.
fn position(
    repo: &Repo,
    before: &Value,
    pairs: &[Pair],
    before_path: Option<&PyPath>,
) -> Py<Option<usize>> {
    Ok(match before_path {
        Some(path) => {
            let resolved = crate::pyfs::resolve(path.as_bytes());
            let name =
                crate::pyfs::relative_to(&resolved, repo.root.as_bytes()).map_err(Raised::Value)?;
            pairs
                .iter()
                .position(|pair| pair.path.as_bytes() == name.as_slice())
        }
        None => pairs.iter().position(|pair| pair.before.py_eq(before)),
    })
}

fn route_v2(
    repo: &Repo,
    before: &Value,
    policy_path: &PyPath,
    pairs: Option<&[Pair]>,
    before_path: Option<&PyPath>,
) -> Py<Routed> {
    if policy(repo, policy_path)?.is_none() {
        let mut reasons =
            vec!["no policy is adopted, so agents settle nothing beyond ADR-0006".to_string()];
        reasons.extend(record::reasons(repo, before)?);
        return Ok(Routed {
            level: 0,
            version: 2,
            reasons,
        });
    }
    let mut reasons = Vec::new();
    let kind = item(before, "decision_type")?;
    pyvalue::hashable(kind)?;
    let open = TYPES
        .iter()
        .find(|(name, _)| pyvalue::is(kind, name))
        .is_some_and(|(_, route)| *route == "open");
    if !open {
        reasons.push(format!(
            "{} stays on its existing owner or reconciler route",
            kind.py_str()
        ));
    }
    if record::same_family(before)? {
        reasons.push(record::SAME_FAMILY_REASON.into());
    } else if !summary::eligible(before)? {
        reasons.push("only contract 2 first answers written before the question qualify".into());
    }
    if item(before, "reviews")?.truthy() {
        reasons.push("a review of a saved ruling always goes to the owner".into());
    }
    reasons.extend(answer_reasons(before)?);
    if !summary::consensus(repo, before)? {
        reasons.push(
            "two blind high-confidence answers and the medium-or-high recommender must agree"
                .into(),
        );
    }
    let owned;
    let pairs = match pairs {
        Some(pairs) => pairs,
        None => {
            owned = summary::records(repo)?;
            &owned
        }
    };
    let Some(position) = position(repo, before, pairs, before_path)? else {
        reasons.push("the before-record must belong to a recorded round".into());
        return Ok(Routed {
            level: 0,
            version: 2,
            reasons,
        });
    };
    let history = &pairs[..position];
    for pair in history {
        if item(&pair.before, "decision_type")?.py_eq(kind) && matches!(pair.after, Value::Null) {
            reasons.push("an earlier decision of this type is still pending".into());
            break;
        }
    }
    let closed = match opening_rounds(repo, position, pairs) {
        Ok(closed) => closed,
        Err(Raised::Value(message)) => {
            reasons.push(message);
            return Ok(Routed {
                level: 0,
                version: 2,
                reasons,
            });
        }
        Err(other) => return Err(other),
    };
    let round = &pairs[position].round;
    let opening = summary::earned(repo, &pairs[..=position], kind, Some(round), Some(&closed))?;
    let running = summary::earned(repo, history, kind, None, None)?;
    let mut level = opening.level;
    let in_round: Vec<&str> = history
        .iter()
        .filter(|pair| pair.round.py_eq(round))
        .map(|pair| pair.path.as_str())
        .collect();
    if running
        .resets
        .iter()
        .any(|(path, _)| in_round.contains(&path.as_str()))
    {
        level = 0;
        reasons.push("a miss or model change reset this decision type during the round".into());
    }
    let mut prior = None;
    for pair in history.iter().rev() {
        if item(&pair.before, "decision_type")?.py_eq(kind) && summary::eligible(&pair.before)? {
            prior = Some(pair);
            break;
        }
    }
    if let Some(prior) = prior
        && !same_setup(&setup(&prior.before)?, &setup(before)?)
    {
        level = 0;
        reasons.push("a model changed; this decision type starts again".into());
    }
    if level == 0 {
        reasons.push("this decision type has not earned level 1 from earlier closed rounds".into());
    }
    Ok(Routed {
        level,
        version: 2,
        reasons,
    })
}

/// ADR-0008: agents settle an open type when the parties give the agreement its gate names, the type met the entry
/// bar in the rounds closed at opening, and no owner change suspended it earlier in the round.
fn route_v3(
    repo: &Repo,
    before: &Value,
    policy: &v3::Policy,
    pairs: Option<&[Pair]>,
    before_path: Option<&PyPath>,
) -> Py<Routed> {
    let routed = |reasons| Routed {
        level: 0,
        version: 3,
        reasons,
    };
    if !v3::adopted(repo, policy)? {
        let mut reasons =
            vec!["policy v3 is not adopted, so agents settle nothing beyond ADR-0006".to_string()];
        reasons.extend(record::reasons(repo, before)?);
        return Ok(routed(reasons));
    }
    let mut reasons = Vec::new();
    let kind = item(before, "decision_type")?;
    pyvalue::hashable(kind)?;
    let gate = policy.gate(kind);
    match gate {
        None => reasons.push(format!(
            "{} stays with the owner under policy v3",
            kind.py_str()
        )),
        Some(_) if !v3::ENFORCED.iter().any(|name| pyvalue::is(kind, name)) => {
            reasons.push(format!(
                "policy v3 filing for {} is not enforced yet",
                kind.py_str()
            ));
        }
        Some(_) => {}
    }
    if record::same_family(before)? {
        reasons.push(record::SAME_FAMILY_REASON.into());
    } else if !summary::eligible(before)? {
        reasons.push("only contract 2 first answers written before the question qualify".into());
    }
    if item(before, "reviews")?.truthy() {
        reasons.push("a review of a saved ruling always goes to the owner".into());
    }
    reasons.extend(v3::answer_reasons(before, policy.gaps_ask)?);
    if let Some(gate) = gate
        && !v3::agreement(repo, before, gate)?
    {
        reasons.push(format!(
            "the parties did not give the agreement policy v3 needs: {}",
            gate.describe()
        ));
    }
    let owned;
    let pairs = match pairs {
        Some(pairs) => pairs,
        None => {
            owned = summary::records(repo)?;
            &owned
        }
    };
    let Some(position) = position(repo, before, pairs, before_path)? else {
        reasons.push("the before-record must belong to a recorded round".into());
        return Ok(routed(reasons));
    };
    let closed = match opening_rounds(repo, position, pairs) {
        Ok(closed) => closed,
        Err(Raised::Value(message)) => {
            reasons.push(message);
            return Ok(routed(reasons));
        }
        Err(other) => return Err(other),
    };
    if let Some(gate) = gate {
        let entry = v3::entry(repo, pairs, kind, gate, policy.gaps_ask, &closed)?;
        if !policy
            .bar
            .met(entry.passed, entry.pull_requests, entry.stands)
        {
            let percent = (entry.stands * 100).checked_div(entry.passed).unwrap_or(0);
            reasons.push(format!(
                "{} has not met policy v3's entry bar in the rounds closed at opening: {} decisions passed the gate (needs {}), on {} pull requests (needs {}), and {percent}% of them stand (needs {}%)",
                kind.py_str(),
                entry.passed,
                policy.bar.decisions,
                entry.pull_requests,
                policy.bar.pull_requests,
                policy.bar.agreement_percent
            ));
        }
    }
    let round = &pairs[position].round;
    let seen = if pairs[position].after.truthy() {
        &pairs[..position]
    } else {
        pairs
    };
    for (at, pair) in seen.iter().enumerate() {
        let settled = pair.after.truthy()
            && item(&pair.before, "decision_type")?.py_eq(kind)
            && pyvalue::is(get(&pair.after, "settled_by")?, "agents");
        if settled && v3::changed(repo, at, seen, Some(round))? {
            reasons.push(
                "a review in this round changed an agent settlement of this decision type, so policy v3 suspends it for the rest of the round"
                    .into(),
            );
            break;
        }
    }
    Ok(routed(reasons))
}

pub const GAP_REASON: &str = "gap or conflict in the rule";

/// Why the first answers alone keep a decision with the owner under `always_ask`: an outcome reserved to the
/// owner, a gap or conflict in the rule, or a credit fact nobody could tell.
pub fn answer_reasons(before: &Value) -> Py<Vec<String>> {
    let mut reasons = Vec::new();
    for answer in pyvalue::iterate(item(before, "answers")?)? {
        let outcome = item(&answer, "outcome")?;
        let owner_outcomes =
            [record::COULD_NOT_TELL.as_slice(), &["relied-on", "problem"]].concat();
        if pyvalue::one_of(outcome, &owner_outcomes) {
            reasons.push(format!(
                "{}: {} stays with the owner",
                item(&answer, "by")?.py_str(),
                outcome.py_str()
            ));
        }
        if item(&answer, "rule_gap")?.truthy() || item(&answer, "conflict")?.truthy() {
            reasons.push(format!("{}: {GAP_REASON}", item(&answer, "by")?.py_str()));
        }
        let facts = get(&answer, "facts")?;
        let could_not_tell = match facts {
            Value::Object(facts) => facts
                .iter()
                .any(|(_, value)| pyvalue::is(value, "cannot-tell")),
            other if !other.truthy() => false,
            other => {
                return Err(Raised::Attribute(format!(
                    "'{}' object has no attribute 'values'",
                    other.type_name()
                )));
            }
        };
        if could_not_tell {
            reasons.push(format!(
                "{}: could not tell a credit fact",
                item(&answer, "by")?.py_str()
            ));
        }
    }
    Ok(reasons)
}

/// Why an after-record settled by agents could not be: empty when the owner settled it, or when the policy its
/// round runs under routes it to agents and the settlement repeats the agreed answer.
pub fn settlement_faults(
    repo: &Repo,
    before: &Value,
    after: &Value,
    before_path: Option<&PyPath>,
    pairs: Option<&[Pair]>,
) -> Py<Vec<String>> {
    if !pyvalue::is(get(after, "settled_by")?, "agents") {
        return Ok(Vec::new());
    }
    let round = round_pin(repo, before);
    let policy_path = repo.join(POLICY);
    if round.is_none() && policy(repo, &policy_path)?.is_none() {
        return Ok(vec!["`settled_by` is `agents`, and the decision is the user's: no policy is adopted, so agents settle nothing beyond ADR-0006".into()]);
    }
    let two = Value::Int("2".into());
    if !get(before, "contract")?.py_eq(&two) || !get(after, "contract")?.py_eq(&two) {
        return Ok(vec![
            "agent settlement needs contract 2 before and after records".into(),
        ]);
    }
    let pin = get(after, "policy")?;
    match &round {
        None => {
            if !record::pinned(repo, pin)? || !pyvalue::is(item(pin, "path")?, POLICY) {
                return Ok(vec![
                    "agent settlement must pin the policy v2 in force".into(),
                ]);
            }
        }
        Some(round) => {
            let same = record::pinned(repo, pin)?
                && get(pin, "path")?.py_eq(get(round, "path")?)
                && get(pin, "sha256")?.py_eq(get(round, "sha256")?);
            if !same {
                return Ok(vec![
                    "agent settlement must pin the policy its round opening pins".into(),
                ]);
            }
        }
    }
    if let Some(path) = before_path {
        let pinned =
            crate::pyfs::resolve(repo.path(item(item(after, "before")?, "path")?)?.as_bytes());
        if pinned != crate::pyfs::resolve(path.as_bytes()) {
            return Ok(vec![
                "agent settlement pins a different before-record path".into(),
            ]);
        }
    }
    let routed = match round {
        None => route(repo, before, &policy_path, pairs, before_path)?,
        Some(_) => route_under(repo, before, None, pairs, before_path)?,
    };
    let mut found: Vec<String> = routed
        .reasons
        .iter()
        .map(|reason| format!("agents cannot settle: {reason}"))
        .collect();
    let (_, first) = record::recommender(before)?;
    for field in ["outcome", "same_fault_as", "facts"] {
        if !get(after, field)?.py_eq(get(&first, field)?) {
            found.push(format!("agent settlement changes the agreed {field}"));
        }
    }
    if !item(after, "asked")?.py_eq(&Value::Int("0".into()))
        || !matches!(item(after, "ground")?, Value::Null)
        || item(after, "by_default")?.truthy()
    {
        found
            .push("agent settlement has zero asks, no owner's ground and no default answer".into());
    }
    Ok(found)
}

const CANDIDATE_FILINGS: [(&str, &str); 6] = [
    ("suggestion", "advisory"),
    ("minor-defect", "inconsequential"),
    ("refuted", "refuted"),
    ("unproven", "unsupported"),
    ("outside-scope", "scope-excluded"),
    ("duplicate", "eligible"),
];

/// Why a current candidate or credit decision filed as agent-settled does not bind its saved settlement; a
/// decision filed as the owner's may carry no settlement pin or policy receipt.
pub fn filing_faults(repo: &Repo, decision: &Value, decision_type: &str) -> Py<Vec<String>> {
    let settlement = get(decision, "settlement")?;
    if !pyvalue::is(get(decision, "settled_by")?, "agents") {
        let adopted = policy(repo, &repo.join(POLICY))?;
        let cites = settlement.truthy()
            || match &adopted {
                Some(adopted) => get(decision, "receipt")?.py_eq(item(adopted, "receipt")?),
                None => false,
            }
            || match v3_receipt(repo) {
                Some(receipt) => get(decision, "receipt")?.py_eq(&receipt),
                None => false,
            };
        return Ok(if cites {
            vec!["a settlement pin or policy adoption receipt requires settled_by agents".into()]
        } else {
            Vec::new()
        });
    }
    if !record::pinned(repo, settlement)? {
        return Ok(vec![
            "agent-settled current decision needs its pinned after-record".into(),
        ]);
    }
    let after = repo.load(&repo.path(item(settlement, "path")?)?)?;
    if !record::pinned(repo, get(&after, "before")?)? {
        return Ok(vec!["settlement needs its pinned before-record".into()]);
    }
    let before_path = repo.path(item(item(&after, "before")?, "path")?)?;
    let before = record::read_before(repo, &before_path)?;
    let mut found = record::faults(repo, &before)?;
    if !found.is_empty() {
        return Ok(found);
    }
    found.extend(record::after_faults(
        repo,
        &before,
        &after,
        &before_path,
        true,
        None,
    )?);
    if !pyvalue::is(get(&after, "settled_by")?, "agents") {
        found.push("the pinned record was not settled by agents".into());
    }
    if !pyvalue::is(item(&before, "decision_type")?, decision_type)
        || !item(&before, "target")?.py_eq(item(decision, "target")?)
    {
        found.push("settlement is for another decision type or target".into());
    }
    let filing = get(&before, "filing")?;
    let expected: Vec<(&str, Value)> = if decision_type == "candidate" {
        vec![
            ("revision", item(decision, "revision")?.clone()),
            ("subject", get(decision, "subject")?.clone()),
        ]
    } else {
        let mut members = Vec::new();
        for key in ["revision", "review", "attempt_id", "item_id", "family_id"] {
            members.push((key, item(decision, key)?.clone()));
        }
        members
    };
    let expected = Value::Object(
        expected
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    );
    if !filing.py_eq(&expected) {
        found.push(
            "before-record filing does not bind this exact subject, revision and source".into(),
        );
    }
    if decision_type == "candidate" {
        let differs = !pyvalue::is(get(decision, "dimension")?, "eligibility")
            || !pyvalue::is(get(decision, "status")?, "approved")
            || !pyvalue::is(get(decision, "authority")?, "human")
            || {
                let outcome = item(&after, "outcome")?;
                pyvalue::hashable(outcome)?;
                let mapped = CANDIDATE_FILINGS
                    .iter()
                    .find(|(settled, _)| pyvalue::is(outcome, settled))
                    .map_or(Value::Null, |(_, filed)| pyvalue::text(filed));
                !mapped.py_eq(item(decision, "outcome")?)
            };
        if differs {
            found.push("candidate filing differs from the settled eligibility outcome".into());
        }
    } else {
        let empty = Value::Object(crate::pyjson::Object::default());
        let facts = pyvalue::get_or(&after, "facts", &empty)?;
        for field in record::FACTS {
            if !get(decision, field)?.py_eq(get(facts, field)?) {
                found.push("credit filing differs from the settled facts".into());
                break;
            }
        }
    }
    let adoption = filed_under(repo, &before)?;
    let cited = match &adoption.cited {
        Some((receipt, scope)) => {
            item(decision, "receipt")?.py_eq(receipt)
                && item(decision, "receipt_scope")?.py_eq(scope)
        }
        None => false,
    };
    if !cited {
        found.push("agent settlement must cite the owner's saved policy adoption".into());
    }
    if !adoption.credit && decision_type != "candidate" {
        found.push(format!(
            "credit stays with the owner under {}; agents settle no credit decision",
            adoption.name
        ));
    }
    let reason = item(decision, "reason")?;
    if !pyvalue::contains(reason, &pyvalue::text(adoption.name))?
        || !pyvalue::contains(reason, &pyvalue::text("agents"))?
    {
        found.push(format!(
            "reason must say agents settled under {}",
            adoption.name
        ));
    }
    Ok(found)
}

/// The receipt of the policy v3 file at [`v3::POLICY`] when the owner adopted it; a file that cannot be read as
/// the enforced policy adopts nothing.
fn v3_receipt(repo: &Repo) -> Option<Value> {
    let path = repo.join(v3::POLICY);
    if !pyvalue::is_file(&path) || !v3::enforced_terms(repo, &path).ok()? {
        return None;
    }
    let policy = v3::policy(repo, &path).ok()?;
    v3::adopted(repo, &policy)
        .ok()?
        .then(|| policy.receipt.clone())
}

/// The adoption an agent-settled filing cites: the policy its before-record's round runs under.
struct Adoption {
    name: &'static str,
    /// The receipt and `adoption_scope`, `None` when that policy is not adopted.
    cited: Option<(Value, Value)>,
    /// Whether agents may settle credit under it.
    credit: bool,
}

fn filed_under(repo: &Repo, before: &Value) -> Py<Adoption> {
    let v2 = |adopted: Option<Value>| -> Py<Adoption> {
        Ok(Adoption {
            name: "policy v2",
            cited: match adopted {
                Some(adopted) => Some((
                    item(&adopted, "receipt")?.clone(),
                    item(&adopted, "adoption_scope")?.clone(),
                )),
                None => None,
            },
            credit: true,
        })
    };
    let Some(pin) = round_pin(repo, before) else {
        return v2(policy(repo, &repo.join(POLICY))?);
    };
    match governing(repo, &pin)?.1 {
        Governing::V2(path) => v2(policy(repo, &path)?),
        Governing::V3(policy) => Ok(Adoption {
            name: "policy v3",
            cited: v3::adopted(repo, &policy)?
                .then(|| (policy.receipt.clone(), policy.adoption_scope.clone())),
            credit: false,
        }),
        Governing::Refused(_) => Ok(Adoption {
            name: "policy v3",
            cited: None,
            credit: false,
        }),
    }
}

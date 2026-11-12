//! Policy v3, settlement on agreement (ADR-0008): the policy file, its gates and how a decided pair counts under
//! them. `ruling replay` and routing both read the policy through this module, so a replayed gate and an enforced
//! one cannot differ.

use crate::pyjson::Value;
use crate::pytext::PyPath;
use crate::pyvalue::{self, Py, Raised, get, item};
use crate::ruling::summary::{self, Pair};
use crate::ruling::{Repo, record};

pub const POLICY: &str = "bench/grading/delegation.v3.json";
/// Where the owner's adoption receipt for policy v3 is saved; a receipt anywhere else adopts nothing.
pub const RECEIPT: &str = "bench/grading/rulings/delegation.v3.md";
/// The terms routing enforces. The replay reads variants of them; a policy file that states other terms is
/// refused for routing, so a changed rule needs a new version.
const TERMS: &str = r#"{"written": "before-question",
    "always_ask": ["cannot-tell", "relied-on", "problem", "reviews-saved-ruling"],
    "entry": {"decisions": 15, "pull_requests": 4, "agreement_percent": 90},
    "owner_reads": {"first_round": "digest", "later_rounds": "1-in-5"},
    "on_owner_change": "suspend-type-for-round", "on_model_replacement": "digest-for-one-round",
    "audit_calls_to_owner": 5,
    "decision_types": {
        "pre-existing-admission": {"blind_answers": 2, "recommender_confidence": "any"},
        "pre-existing-raise": "owner", "pre-existing-grouping": "owner",
        "candidate": {"blind_answers": 2, "recommender_confidence": "any"},
        "recovery": "owner", "grouping": "owner",
        "band": {"blind_answers": 1, "recommender_confidence": "any", "asked": "after-admission"},
        "control": "owner",
        "reconciliation": {"blind_answers": 0, "recommender_confidence": "medium-or-high"}}}"#;
const ADOPTION: [&str; 4] = ["version", "status", "receipt", "adoption_scope"];
const OWNER: &str = "owner";
const AFTER_ADMISSION: &str = "after-admission";
const NEVER_NEEDED: &str = "not-applicable";
const GATE_KEYS: [&str; 3] = ["blind_answers", "recommender_confidence", "asked"];
const GAP_NAMES: [&str; 2] = ["gap", "conflict"];

/// The decision types whose agent settlements can be filed under policy v3; every other open type still asks.
pub const ENFORCED: [&str; 3] = ["candidate", "reconciliation", "band"];
/// The reconciliations of each audit round that go to the owner without the reconciler's call.
pub const AUDIT_CALLS: usize = 5;

/// The owner's audit calls among `units`, the disputed units of the audit drawn with `seed`: the ones whose
/// SHA-256 of `<seed>:reconciliation:<unit id>` is lowest. Neither the preparer nor the records can move them.
pub fn audit_calls(seed: &str, units: &[String]) -> Vec<String> {
    let mut ranked: Vec<(String, &String)> = units
        .iter()
        .map(|unit| {
            (
                crate::sha256::hex_digest(format!("{seed}:reconciliation:{unit}").as_bytes()),
                unit,
            )
        })
        .collect();
    ranked.sort();
    ranked
        .into_iter()
        .take(AUDIT_CALLS)
        .map(|(_, unit)| unit.clone())
        .collect()
}

/// An evaluator audit as its comparison pins it: the seed of its sample and the ids of the units its graders
/// disagree on.
#[derive(Clone)]
pub struct Audit {
    pub seed: String,
    pub disputed: Vec<String>,
}

impl Audit {
    pub fn calls(&self) -> Vec<String> {
        audit_calls(&self.seed, &self.disputed)
    }
}

/// Where the current records keep the audit plan and each audit round's directory.
pub const AUDITS: &str = "bench/grading/current/audits.json";
const CURRENT: &str = "bench/grading/current";
const AUDIT_ROUNDS: &str = "bench/grading/current/audit";

/// A comparison by its path and SHA-256, with the audit it was checked to be.
type Checked = ((Vec<u8>, Value), Option<Audit>);

thread_local! {
    /// Each comparison checked in this process, by its path and SHA-256, so routing a round derives it once.
    static CHECKED: std::cell::RefCell<Vec<Checked>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The audit whose `comparison.json` the pin names, `None` unless it is the comparison `evaluator-audit conclude`
/// would accept for a round the audit plan records: at `bench/grading/current/audit/<seed>/comparison.json` for the
/// plan's seed or a superseded round's, pinning its sibling `sample.json` (the one a superseded round's evidence
/// pins), and derived from that sample and the latest second assessments. The plan's sample must also be the draw
/// redrawn from the inventory, and its comparison derives from the live grades alone.
pub fn audit(repo: &Repo, comparison: &Value) -> Py<Option<Audit>> {
    if !record::pinned(repo, comparison)? {
        return Ok(None);
    }
    let key = (
        repo.path(item(comparison, "path")?)?.as_bytes().to_vec(),
        item(comparison, "sha256")?.clone(),
    );
    let known = CHECKED.with(|checked| {
        checked
            .borrow()
            .iter()
            .find(|(seen, _)| seen.0 == key.0 && seen.1.py_eq(&key.1))
            .map(|(_, audit)| audit.clone())
    });
    if let Some(audit) = known {
        return Ok(audit);
    }
    let audit = checked_audit(repo, comparison).unwrap_or(None);
    CHECKED.with(|checked| checked.borrow_mut().push((key, audit.clone())));
    Ok(audit)
}

/// Whether the plan file records the plan's round as assessed, its evidence pinning this comparison and the round's
/// `conclusion.json` as they are. `conclude` writes that evidence, and only an assessed round has it; a round that is
/// concluded but not assessed is anchored by nothing outside its own directory, so it must still redraw.
fn assessed_with(repo: &Repo, plans: &Value, comparison: &Value, directory: &str) -> Py<bool> {
    if !pyvalue::is(get(plans, "state")?, "assessed") {
        return Ok(false);
    }
    let evidence = pyvalue::iterate(get(plans, "evidence")?)?;
    let concluded = format!("{directory}/conclusion.json");
    let mut conclusion = false;
    for pin in &evidence {
        if pyvalue::is(get(pin, "path")?, &concluded) && record::pinned(repo, pin)? {
            conclusion = true;
        }
    }
    Ok(conclusion && evidence.iter().any(|pin| pin.py_eq(comparison)))
}

fn checked_audit(repo: &Repo, comparison: &Value) -> Py<Option<Audit>> {
    let Value::Str(path) = item(comparison, "path")? else {
        return Ok(None);
    };
    let plans = repo.load(&repo.join(AUDITS))?;
    let mut rounds = vec![(item(item(&plans, "plan")?, "seed")?.clone(), None)];
    let none = Value::Array(Vec::new());
    for round in pyvalue::iterate(pyvalue::get_or(&plans, "superseded", &none)?)? {
        rounds.push((
            item(item(&round, "plan")?, "seed")?.clone(),
            Some(item(&round, "evidence")?.clone()),
        ));
    }
    let Some((seed, evidence)) = rounds.into_iter().find(|(seed, _)| {
        matches!(seed, Value::Str(seed) if *path == format!("{AUDIT_ROUNDS}/{seed}/comparison.json"))
    }) else {
        return Ok(None);
    };
    let Value::Str(seed) = seed else {
        return Ok(None);
    };
    let directory = format!("{AUDIT_ROUNDS}/{seed}");
    let compared = repo.load(&repo.path(item(comparison, "path")?)?)?;
    let sample = get(&compared, "sample")?;
    let sibling = format!("{directory}/sample.json");
    if !record::pinned(repo, sample)? || !pyvalue::is(item(sample, "path")?, &sibling) {
        return Ok(None);
    }
    let Ok(root) = crate::current_grading::Root::new(repo.root.as_bytes()) else {
        return Ok(None);
    };
    let drawing = match evidence {
        Some(evidence) => {
            let pinned_there = pyvalue::iterate(&evidence)?
                .iter()
                .any(|pin| pin.py_eq(sample));
            if !pinned_there {
                return Ok(None);
            }
            false
        }
        None if assessed_with(repo, &plans, comparison, &directory)? => false,
        None => {
            if !crate::evaluator_audit::redraws_plan_sample(&root, CURRENT.as_bytes()) {
                return Ok(None);
            }
            true
        }
    };
    if !crate::evaluator_audit::derives(&root, &repo.join(&directory), drawing).unwrap_or(false) {
        return Ok(None);
    }
    let drawn = repo.load(&repo.path(item(sample, "path")?)?)?;
    let Value::Str(drawn_seed) = get(&drawn, "seed")? else {
        return Ok(None);
    };
    if *drawn_seed != seed {
        return Ok(None);
    }
    let mut disputed = Vec::new();
    for unit in pyvalue::iterate(get(&compared, "units")?)? {
        let Value::Str(id) = get(&unit, "id")? else {
            return Ok(None);
        };
        if !get(&unit, "agreement")?.truthy() {
            disputed.push(id.clone());
        }
    }
    Ok(Some(Audit {
        seed: seed.clone(),
        disputed,
    }))
}

#[derive(Clone, Copy, PartialEq)]
enum Confidence {
    Any,
    MediumOrHigh,
}

#[derive(Clone, Copy)]
pub struct Gate {
    blind_answers: usize,
    recommender: Confidence,
    pub(crate) after_admission: bool,
}

impl Gate {
    pub fn describe(self) -> String {
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

pub struct Bar {
    pub(crate) decisions: u64,
    pub(crate) pull_requests: u64,
    pub(crate) agreement_percent: u64,
}

impl Bar {
    pub fn met(&self, passed: u64, pull_requests: u64, stands: u64) -> bool {
        passed >= self.decisions
            && pull_requests >= self.pull_requests
            && stands * 100 >= self.agreement_percent * passed
    }
}

pub struct Policy {
    pub(crate) status: Value,
    pub(crate) bar: Bar,
    /// Whether a gap or conflict that any party states sends an agreed answer to the owner.
    pub(crate) gaps_ask: bool,
    pub(crate) gates: Vec<(&'static str, Option<Gate>)>,
    pub(crate) receipt: Value,
    pub(crate) adoption_scope: Value,
}

impl Policy {
    /// The gate of a decision type, `None` for one the policy keeps with the owner.
    pub fn gate(&self, kind: &Value) -> Option<Gate> {
        self.gates
            .iter()
            .find(|(name, _)| pyvalue::is(kind, name))
            .and_then(|(_, gate)| *gate)
    }
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
    let full = super::always_ask();
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
        receipt: get(&value, "receipt")?.clone(),
        adoption_scope: get(&value, "adoption_scope")?.clone(),
    })
}

/// Whether the policy file at `source` states exactly the terms routing enforces, besides its adoption.
pub fn enforced_terms(repo: &Repo, source: &PyPath) -> Py<bool> {
    let value = repo.load(source)?;
    let (Value::Object(stated), Ok(Value::Object(terms))) = (&value, pyvalue::loads(TERMS)) else {
        return Ok(false);
    };
    Ok(stated.len() == terms.len() + ADOPTION.len()
        && ADOPTION.iter().all(|key| stated.get(key).is_some())
        && terms
            .iter()
            .all(|(key, term)| stated.get(key).is_some_and(|said| said.py_eq(term))))
}

/// Whether the owner adopted the policy: `adopted`, with the saved receipt at [`RECEIPT`] pinned and its
/// `adoption_scope` written in the receipt verbatim. A proposed policy grants nothing.
pub fn adopted(repo: &Repo, policy: &Policy) -> Py<bool> {
    let Value::Str(scope) = &policy.adoption_scope else {
        return Ok(false);
    };
    let at_receipt = match &policy.receipt {
        Value::Object(pin) => pin
            .get("path")
            .is_some_and(|path| pyvalue::is(path, RECEIPT)),
        _ => false,
    };
    if !pyvalue::is(&policy.status, "adopted")
        || scope.trim().is_empty()
        || !at_receipt
        || !record::pinned(repo, &policy.receipt)?
    {
        return Ok(false);
    }
    let text = pyvalue::read_text(&repo.path(item(&policy.receipt, "path")?)?)?;
    Ok(text.contains(scope.as_str()))
}

/// Whether the parties gave the agreement the gate names.
pub fn agreement(repo: &Repo, before: &Value, gate: Gate) -> Py<bool> {
    let (_, first) = record::recommender(before)?;
    Ok(summary::unanimous(repo, before, gate.blind_answers)?
        && (gate.recommender == Confidence::Any
            || pyvalue::one_of(item(&first, "confidence")?, &["medium", "high"])))
}

/// Each party's answer that `always_ask` sends to the owner, as [`super::answer_reasons`] words it; a stated gap or
/// conflict only while the policy keeps `gap` and `conflict`.
pub fn answer_reasons(before: &Value, gaps_ask: bool) -> Py<Vec<String>> {
    Ok(super::answer_reasons(before)?
        .into_iter()
        .filter(|reason| {
            gaps_ask || reason.split_once(": ").map(|(_, said)| said) != Some(super::GAP_REASON)
        })
        .collect())
}

const UNCOUNTED: &str =
    "first answers that do not count: contract 1, reconstructed, same-family or written late";
const REVIEW: &str = "a review of a saved ruling";
const RESERVED: &str = "an outcome reserved to the owner";

/// Why an agreed answer still goes to the owner: first answers that do not count, or what `always_ask` reserves.
pub fn held(before: &Value, gaps_ask: bool) -> Py<Vec<String>> {
    let mut reasons = Vec::new();
    if !summary::eligible(before)? {
        reasons.push(UNCOUNTED.to_string());
    }
    if item(before, "reviews")?.truthy() {
        reasons.push(REVIEW.to_string());
    }
    for reason in answer_reasons(before, gaps_ask)? {
        let reason = match reason.split_once(": ") {
            Some((_, said)) if said.ends_with("stays with the owner") => RESERVED.to_string(),
            Some((_, said)) => said.to_string(),
            None => reason,
        };
        if !reasons.contains(&reason) {
            reasons.push(reason);
        }
    }
    Ok(reasons)
}

/// How a decided pair counts under its decision type's gate.
pub enum Counted {
    /// A label asked after admission whose every decision that stands is `not-applicable`.
    NeverNeeded,
    /// Settled by agents with no later owner record, so it counts on neither side.
    Unread,
    /// The parties did not give the agreement the gate names.
    Disagreed,
    /// Agreed, and still the owner's for these reasons.
    Held(Vec<String>),
    /// Passed the gate; `stands` when the recommender's first answer is every decision that stands.
    Passed {
        first: Value,
        decisions: Vec<Value>,
        stands: bool,
    },
}

/// `count(pair)`: the pair at `position` counted under `gate`, with later reviews in `pairs` deciding what stands.
pub fn count(
    repo: &Repo,
    position: usize,
    pairs: &[Pair],
    gate: Gate,
    gaps_ask: bool,
) -> Py<Counted> {
    let pair = &pairs[position];
    let before = &pair.before;
    let decisions = summary::final_decisions(repo, position, pairs)?;
    let mut unneeded = gate.after_admission && !decisions.is_empty();
    for decision in &decisions {
        unneeded = unneeded && pyvalue::is(item(decision, "outcome")?, NEVER_NEEDED);
    }
    if unneeded {
        return Ok(Counted::NeverNeeded);
    }
    if settled_by_agents(pair)? && summary::successors(repo, position, pairs)?.is_empty() {
        return Ok(Counted::Unread);
    }
    if !agreement(repo, before, gate)? {
        return Ok(Counted::Disagreed);
    }
    let reasons = held(before, gaps_ask)?;
    if !reasons.is_empty() {
        return Ok(Counted::Held(reasons));
    }
    let (_, first) = record::recommender(before)?;
    let stands = summary::agrees(&first, &decisions)?;
    Ok(Counted::Passed {
        first,
        decisions,
        stands,
    })
}

fn settled_by_agents(pair: &Pair) -> Py<bool> {
    Ok(pyvalue::is(get(&pair.after, "settled_by")?, "agents"))
}

/// Whether any review in the chain descending from the settlement at `at` decided other than it. With `round`, the
/// walk keeps to the reviews saved in that round.
pub fn changed(repo: &Repo, at: usize, pairs: &[Pair], round: Option<&Value>) -> Py<bool> {
    let mut chain = summary::successors(repo, at, pairs)?;
    while let Some(review) = chain.pop() {
        if round.is_some_and(|round| !pairs[review].round.py_eq(round)) {
            continue;
        }
        if !summary::agrees(&pairs[at].after, std::slice::from_ref(&pairs[review].after))? {
            return Ok(true);
        }
        chain.extend(summary::successors(repo, review, pairs)?);
    }
    Ok(false)
}

/// What a decision type brings to a round's entry bar.
pub struct Entry {
    pub passed: u64,
    pub pull_requests: u64,
    pub stands: u64,
}

/// The decided pairs of `kind` in the `closed` rounds, counted as the replay counts them over those rounds alone,
/// so a review saved after the round opened changes nothing. Unlike the replay, an agent settlement that a review
/// changed does not stand even when a later review restored its outcome (ADR-0008 term 3).
pub fn entry(
    repo: &Repo,
    pairs: &[Pair],
    kind: &Value,
    gate: Gate,
    gaps_ask: bool,
    closed: &[Value],
) -> Py<Entry> {
    let closed_pairs: Vec<Pair> = pairs
        .iter()
        .filter(|pair| closed.iter().any(|round| round.py_eq(&pair.round)))
        .cloned()
        .collect();
    let (mut passed, mut stands) = (0, 0);
    let mut targets: Vec<Value> = Vec::new();
    for (position, pair) in closed_pairs.iter().enumerate() {
        if !pair.after.truthy() || !item(&pair.before, "decision_type")?.py_eq(kind) {
            continue;
        }
        if let Counted::Passed {
            stands: standing, ..
        } = count(repo, position, &closed_pairs, gate, gaps_ask)?
        {
            let standing = standing
                && !(settled_by_agents(pair)? && changed(repo, position, &closed_pairs, None)?);
            passed += 1;
            stands += u64::from(standing);
            let target = item(&pair.before, "target")?;
            pyvalue::hashable(target)?;
            if !targets.iter().any(|known| known.py_eq(target)) {
                targets.push(target.clone());
            }
        }
    }
    Ok(Entry {
        passed,
        pull_requests: targets.len() as u64,
        stands,
    })
}

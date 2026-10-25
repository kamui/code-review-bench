//! Claim-level grading rules for current verdicts, and the item projection the historical scorer reads, ported
//! from `bench/tools/claim_grading.py`. Each rule reads JSON values as Python does and fails with the exception
//! Python would raise. `scoring_items` has no caller and is not ported.

use crate::pyjson::{Object, Value};
use crate::pyobject::{self as py, PyResult, Raised};
use crate::pytext;
use crate::schema;
use std::path::Path;

pub const OUTCOMES: [&str; 7] = [
    "eligible",
    "refuted",
    "unsupported",
    "advisory",
    "inconsequential",
    "scope-excluded",
    "unresolved",
];
pub const AXES: [(&str, &[&str]); 4] = [
    (
        "support",
        &["supported", "contradicted", "unsupported", "unsettled"],
    ),
    (
        "attribution",
        &[
            "introduced",
            "worsened",
            "new-obligation",
            "pre-existing",
            "out-of-scope",
            "unsettled",
        ],
    ),
    ("reachability", &["reachable", "unreachable", "unsettled"]),
    ("materiality", &["material", "below-threshold", "unsettled"]),
];
pub const CLAIM_FIELDS: [&str; 10] = [
    "id",
    "quote",
    "outcome",
    "family",
    "canonical_claim_id",
    "duplicate_group",
    "candidate",
    "notes",
    "evidence",
    "assessment",
];
const ATTRIBUTED: [&str; 3] = ["introduced", "worsened", "new-obligation"];
const REQUIRED_SUPPORT: [(&str, &str); 5] = [
    ("refuted", "contradicted"),
    ("unsupported", "unsupported"),
    ("advisory", "supported"),
    ("inconsequential", "supported"),
    ("scope-excluded", "supported"),
];
pub const V2_CLAIM_FIELDS: [&str; 16] = [
    "id",
    "quote",
    "true",
    "this_change",
    "promised",
    "promise_source",
    "delivered",
    "outcome",
    "kind",
    "known_problems",
    "canonical_claim_id",
    "duplicate_group",
    "candidate",
    "open",
    "notes",
    "evidence",
];
const V2_ANSWERS: [(&str, &[&str]); 4] = [
    ("true", &["yes", "no", "not-shown", "cannot-check"]),
    ("this_change", &["yes", "no"]),
    ("promised", &["yes", "no", "cannot-tell"]),
    ("delivered", &["yes", "no", "cannot-tell"]),
];
const FACTS: [&str; 3] = ["yes", "no", "cannot-tell"];
/// The open kind of a claim whose needed check the grader could not make: the `run` tool refused its `command`, or
/// it needs a `service` graders do not have. `grade map` files no batch holding one that names a command.
pub const BLOCKED_CHECK: &str = "blocked-check";

/// `", ".join(sorted(fields))`.
pub fn sorted_names(fields: &[&str]) -> String {
    let mut names = fields.to_vec();
    names.sort_unstable();
    names.join(", ")
}

/// `isinstance(value, dict) and set(value) == set(fields)`.
pub fn has_exactly(value: &Value, fields: &[&str]) -> bool {
    value.as_object().is_some_and(|object| {
        object.len() == fields.len() && fields.iter().all(|field| object.get(field).is_some())
    })
}

/// `isinstance(value, str) and value.strip()`.
pub fn filled(value: &Value) -> bool {
    value
        .as_str()
        .is_some_and(|text| !pytext::strip(text).is_empty())
}

/// `value == literal` for a `str` literal.
pub fn is(value: &Value, literal: &str) -> bool {
    value.as_str() == Some(literal)
}

/// `value in choices` for a tuple of `str` literals.
pub fn one_of(value: &Value, choices: &[&str]) -> bool {
    choices.iter().any(|choice| is(value, choice))
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    py::getitem(value, key).unwrap_or(&Value::Null)
}

fn evidence_given(evidence: &Value) -> bool {
    evidence
        .as_array()
        .is_some_and(|items| !items.is_empty() && items.iter().all(filled))
}

/// Problems with an outcome under the four eligibility tests: support, change attribution, supported
/// reachability and material consequence.
pub fn assessment_problems(outcome: &Value, assessment: &Value) -> PyResult<Vec<String>> {
    let axes: Vec<&str> = AXES.iter().map(|(axis, _)| *axis).collect();
    if !has_exactly(assessment, &axes) {
        return Ok(vec![format!(
            "assessment needs exactly {}",
            axes.join(", ")
        )]);
    }
    let problems: Vec<String> = AXES
        .iter()
        .filter(|(axis, states)| !one_of(field(assessment, axis), states))
        .map(|(axis, states)| {
            format!(
                "{axis} {} is not one of {}",
                field(assessment, axis).py_repr(),
                states.join(", ")
            )
        })
        .collect();
    if !problems.is_empty() {
        return Ok(problems);
    }
    let mut problems = problems;
    let axis = |name: &str| field(assessment, name);
    if is(outcome, "eligible")
        && !(is(axis("support"), "supported")
            && one_of(axis("attribution"), &ATTRIBUTED)
            && is(axis("reachability"), "reachable")
            && is(axis("materiality"), "material"))
    {
        problems.push("an eligible claim must satisfy all four eligibility tests".to_string());
    }
    if !py::hashable(outcome) {
        return Err(Raised::TypeError);
    }
    if let Some((name, support)) = REQUIRED_SUPPORT.iter().find(|(name, _)| is(outcome, name))
        && !is(axis("support"), support)
    {
        problems.push(format!("{name} has inconsistent support"));
    }
    if one_of(outcome, &["advisory", "inconsequential"])
        && !is(axis("materiality"), "below-threshold")
    {
        problems.push("advice and observations must be below the correction threshold".to_string());
    }
    if is(outcome, "scope-excluded")
        && !one_of(axis("attribution"), &["pre-existing", "out-of-scope"])
    {
        problems.push("a scope exclusion needs a scope reason".to_string());
    }
    Ok(problems)
}

/// Problems with one blinded claim verdict, given the target's causal family ids.
pub fn verdict_problems(claim: &Value, families: &Value) -> PyResult<Vec<String>> {
    if !has_exactly(claim, &CLAIM_FIELDS) {
        return Ok(vec![format!(
            "needs exactly {}",
            sorted_names(&CLAIM_FIELDS)
        )]);
    }
    let get = |name: &str| field(claim, name);
    let mut problems: Vec<String> = ["id", "quote", "notes"]
        .into_iter()
        .filter(|name| !filled(get(name)))
        .map(|name| format!("{name} is empty"))
        .collect();
    problems.extend(
        [
            "family",
            "canonical_claim_id",
            "duplicate_group",
            "candidate",
        ]
        .into_iter()
        .filter(|name| !matches!(get(name), Value::Null) && !filled(get(name)))
        .map(|name| format!("{name} must be null or a non-empty string")),
    );
    if !evidence_given(get("evidence")) {
        problems.push("needs inspected evidence or an explicit evidence limitation".to_string());
    }
    let (outcome, family) = (get("outcome"), get("family"));
    if !one_of(outcome, &OUTCOMES) {
        problems.push(format!(
            "outcome {} is not one of {}",
            outcome.py_repr(),
            OUTCOMES.join(", ")
        ));
        return Ok(problems);
    }
    problems.extend(assessment_problems(outcome, get("assessment"))?);
    if !matches!(family, Value::Null) && !py::contains(families, family)? {
        problems.push(format!(
            "family {} is not a causal family of this task",
            family.py_repr()
        ));
    }
    if is(outcome, "eligible") && matches!(family, Value::Null) {
        problems.push("an eligible claim names the causal family it identifies".to_string());
    }
    if !matches!(family, Value::Null) && !one_of(outcome, &["eligible", "unresolved"]) {
        problems.push("only an eligible or unresolved claim names a causal family".to_string());
    }
    if !matches!(get("candidate"), Value::Null) && !is(outcome, "unresolved") {
        problems.push("only an unresolved claim names a novel candidate".to_string());
    }
    Ok(problems)
}

/// One review's recovery of one causal family: an outcome, the claim ids behind it and a reason.
pub struct Recovery {
    pub outcome: &'static str,
    pub claim_ids: Vec<Value>,
    pub reason: &'static str,
    /// Version 2 only: no claim says what goes wrong and one identifies the cause.
    pub cause_only: bool,
}

fn approved(family: &Value) -> PyResult<bool> {
    Ok(is(
        py::getitem(py::getitem(family, "eligibility")?, "state")?,
        "approved",
    ))
}

/// One review's recovery of one causal family, independent of any remedy.
///
/// Any number of eligible claims recover the family once. A pending family, an unadmitted review or an
/// unresolved claim that names the family keeps the recovery unresolved rather than missed. An unresolved claim
/// that names no family, such as a novel candidate, leaves every family's recovery to the other claims.
pub fn family_recovery(family: &Value, claims: &[Value], admitted: bool) -> PyResult<Recovery> {
    let approved = approved(family)?;
    let mut recoveries = Vec::new();
    for claim in claims {
        if py::eq(py::getitem(claim, "family_id")?, py::getitem(family, "id")?)
            && is(py::getitem(claim, "outcome")?, "eligible")
        {
            recoveries.push(py::getitem(claim, "id")?.clone());
        }
    }
    let mut open_claims = Vec::new();
    for claim in claims {
        if is(py::getitem(claim, "outcome")?, "unresolved")
            && py::eq(py::getitem(claim, "family_id")?, py::getitem(family, "id")?)
        {
            open_claims.push(py::getitem(claim, "id")?.clone());
        }
    }
    let recovery = |outcome, claim_ids, reason| Recovery {
        outcome,
        claim_ids,
        reason,
        cause_only: false,
    };
    Ok(if !recoveries.is_empty() && approved && admitted {
        recovery(
            "caught",
            recoveries,
            "Original identifying wording satisfies all four eligibility tests.",
        )
    } else if !recoveries.is_empty() {
        recovery(
            "unresolved",
            recoveries,
            if approved {
                "The review was not admitted, so its eligible claim earns no recovery."
            } else {
                "The family's eligibility awaits a saved human ruling."
            },
        )
    } else if !approved {
        recovery(
            "unresolved",
            Vec::new(),
            "The family's eligibility awaits a saved human ruling.",
        )
    } else if !open_claims.is_empty() {
        recovery(
            "unresolved",
            open_claims,
            "Unresolved original claims could concern this family.",
        )
    } else {
        recovery(
            "missed",
            Vec::new(),
            "Every original item is accounted for and none identifies this family.",
        )
    })
}

/// Fix sufficiency of a caught family from the distinct recommendations that address it.
pub fn family_sufficiency(
    outcome: &str,
    remedies: &[Value],
    inventory_complete: bool,
) -> &'static str {
    let has = |name: &str| remedies.iter().any(|remedy| is(remedy, name));
    if outcome != "caught" {
        "unassessed"
    } else if has("sufficient") {
        "sufficient"
    } else if has("unassessed") {
        "unassessed"
    } else if has("partial") {
        "partial"
    } else if !remedies.is_empty() {
        "ineffective"
    } else if inventory_complete {
        "absent"
    } else {
        "unassessed"
    }
}

/// Problems with one claim under the ordered questions and known-problem facts.
pub fn verdict_problems_v2(claim: &Value, families: &Value) -> PyResult<Vec<String>> {
    if !has_exactly(claim, &V2_CLAIM_FIELDS) {
        return Ok(vec![format!(
            "needs exactly {}",
            sorted_names(&V2_CLAIM_FIELDS)
        )]);
    }
    let get = |name: &str| field(claim, name);
    let mut problems: Vec<String> = ["id", "quote", "notes"]
        .into_iter()
        .filter(|name| !filled(get(name)))
        .map(|name| format!("{name} is empty"))
        .collect();
    problems.extend(
        ["canonical_claim_id", "duplicate_group", "candidate"]
            .into_iter()
            .filter(|name| !matches!(get(name), Value::Null) && !filled(get(name)))
            .map(|name| format!("{name} must be null or a non-empty string")),
    );
    if !evidence_given(get("evidence")) {
        problems.push("needs inspected evidence or an explicit evidence limitation".to_string());
    }
    let Some(known) = get("known_problems").as_array() else {
        problems.push("known_problems must be a list".to_string());
        return Ok(problems);
    };
    let mut ids: Vec<&str> = Vec::new();
    for entry in known {
        let family = field(entry, "family");
        let valid = has_exactly(
            entry,
            &["family", "says_what", "identifies_cause", "reason"],
        ) && family.as_str().is_some()
            && py::contains(families, family)?
            && one_of(field(entry, "says_what"), &FACTS)
            && one_of(field(entry, "identifies_cause"), &FACTS)
            && filled(field(entry, "reason"));
        if !valid {
            problems.push(
                "known_problems entries need a known family, says_what, identifies_cause and a reason"
                    .to_string(),
            );
            continue;
        }
        let family = family.as_str().unwrap_or_default();
        if ids.contains(&family) {
            problems.push("known_problems names each family once".to_string());
        }
        ids.push(family);
    }
    if !problems.is_empty() {
        return Ok(problems);
    }
    let any_fact =
        |name: &str, answer: &str| known.iter().any(|entry| is(field(entry, name), answer));
    let says_what = any_fact("says_what", "yes");
    let uncertain = any_fact("says_what", "cannot-tell");
    let cause_only = !says_what && !uncertain && any_fact("identifies_cause", "yes");
    // A claim that only restates a known problem's cause is not sorted further; one that says more is.
    let restated = cause_only && is(get("true"), "yes") && is(get("kind"), "known-cause");
    let mut reached = vec!["true"];
    let mut expected: Option<&str> = None;
    let mut candidate_required = false;
    if says_what {
        expected = Some("problem");
        if !is(get("true"), "yes") {
            problems.push("says_what yes requires true yes".to_string());
        }
    } else if restated {
        expected = Some("suggestion");
    } else if is(get("true"), "no") {
        expected = Some("refuted");
    } else if is(get("true"), "not-shown") {
        expected = Some("unproven");
    } else if is(get("true"), "cannot-check") {
        expected = Some("unresolved");
    } else if is(get("true"), "yes") {
        reached.push("this_change");
        if is(get("this_change"), "no") {
            expected = Some("outside-this-change");
        } else if is(get("this_change"), "yes") {
            reached.push("promised");
            if is(get("promised"), "no") {
                // A relied-on use is the user's to settle unless a saved ruling on that use is already linked.
                candidate_required = is(get("kind"), "relied-on")
                    && matches!(get("canonical_claim_id"), Value::Null);
                expected = Some(if candidate_required {
                    "unresolved"
                } else {
                    "suggestion"
                });
            } else if is(get("promised"), "cannot-tell") {
                expected = Some("unresolved");
            } else if is(get("promised"), "yes") {
                reached.push("delivered");
                if is(get("delivered"), "yes") {
                    expected = Some("minor-defect");
                } else if one_of(get("delivered"), &["no", "cannot-tell"]) {
                    expected = Some("unresolved");
                    candidate_required = is(get("delivered"), "no");
                }
            }
        }
    }
    for (name, choices) in V2_ANSWERS {
        if reached.contains(&name) && !one_of(get(name), choices) {
            problems.push(format!("{name} must be one of {}", choices.join(", ")));
        } else if !reached.contains(&name) && !matches!(get(name), Value::Null) {
            problems.push(format!(
                "{name} must be null when an earlier answer settles the claim"
            ));
        }
    }
    if uncertain && !says_what {
        expected = Some("unresolved");
    }
    let outcome = get("outcome");
    let outcome_matches = match expected {
        Some(expected) => is(outcome, expected),
        None => matches!(outcome, Value::Null),
    };
    if !outcome_matches {
        problems.push(format!(
            "answers require outcome {}",
            expected.map_or_else(|| "None".to_string(), pytext::repr_str)
        ));
    }
    let sources = get("promise_source");
    if is(get("promised"), "yes") {
        let listed = sources.as_array().is_some_and(|sources| {
            !sources.is_empty()
                && sources
                    .iter()
                    .all(|source| one_of(source, &["written", "announced", "built"]))
        });
        if !listed {
            problems.push("promise_source must list written, announced or built".to_string());
        }
    } else if !py::eq(sources, &Value::Array(Vec::new())) {
        problems.push("promise_source must be empty unless promised is yes".to_string());
    }
    let kind = get("kind");
    let canonical = get("canonical_claim_id");
    let relied_on = is(get("true"), "yes")
        && is(get("this_change"), "yes")
        && is(get("promised"), "no")
        && is(outcome, "unresolved")
        && is(kind, "relied-on");
    if is(kind, "known-cause") {
        if !restated {
            problems.push(
                "kind known-cause is for a true claim that identifies a known problem's cause and does not say what goes wrong"
                    .to_string(),
            );
        }
    } else if is(outcome, "suggestion") {
        if !one_of(kind, &["improvement", "outside-supported-use"])
            && !(is(kind, "relied-on") && !matches!(canonical, Value::Null))
        {
            problems.push(
                "suggestion needs kind improvement or outside-supported-use, or relied-on under a saved ruling"
                    .to_string(),
            );
        }
    } else if !matches!(kind, Value::Null) && !relied_on {
        problems.push(
            "kind must be null except for a suggestion or an unresolved relied-on use".to_string(),
        );
    }
    let opened = get("open");
    if is(outcome, "unresolved") && is(field(opened, "kind"), BLOCKED_CHECK) {
        let described = ["command", "service"].iter().any(|blocker| {
            has_exactly(opened, &["kind", blocker, "would_settle"])
                && filled(field(opened, blocker))
        }) && filled(field(opened, "would_settle"));
        if !described {
            problems.push(
                "a blocked check needs open kind, would_settle and one of command or service"
                    .to_string(),
            );
        }
        if candidate_required && matches!(get("candidate"), Value::Null) {
            problems.push("a possible new problem or relied-on use needs a candidate".to_string());
        }
    } else if is(outcome, "unresolved") {
        let described = has_exactly(opened, &["kind", "would_settle"])
            && one_of(
                field(opened, "kind"),
                &[
                    "missing-fact",
                    "promise",
                    "delivery",
                    "new-problem",
                    "relied-on",
                    "credit",
                ],
            )
            && filled(field(opened, "would_settle"));
        if !described {
            problems.push("unresolved needs open kind and would_settle".to_string());
        }
        let opens_candidate = opened.as_object().is_some_and(|opened| {
            opened
                .get("kind")
                .is_some_and(|kind| one_of(kind, &["new-problem", "relied-on"]))
        });
        if (candidate_required || opens_candidate) && matches!(get("candidate"), Value::Null) {
            problems.push("a possible new problem or relied-on use needs a candidate".to_string());
        }
    } else {
        if !matches!(opened, Value::Null) {
            problems.push("open must be null unless unresolved".to_string());
        }
        if !matches!(get("candidate"), Value::Null) {
            problems.push("only an unresolved claim names a novel candidate".to_string());
        }
    }
    Ok(problems)
}

/// Recovery from a review's known-problem facts.
pub fn family_recovery_v2(family: &Value, claims: &[Value], admitted: bool) -> PyResult<Recovery> {
    let mut facts: Vec<(Value, &Value)> = Vec::new();
    for claim in claims {
        let known = py::getitem(claim, "known_problems")?;
        for entry in py::iterate(known)? {
            let Some(entry) = (match entry {
                std::borrow::Cow::Borrowed(entry) => Some(entry),
                std::borrow::Cow::Owned(_) => None,
            }) else {
                // A dict key or a character: indexing it by name raises.
                return Err(Raised::TypeError);
            };
            if py::eq(py::getitem(entry, "family")?, py::getitem(family, "id")?) {
                facts.push((py::getitem(claim, "id")?.clone(), entry));
            }
        }
    }
    let mut recoveries = Vec::new();
    for (identifier, entry) in &facts {
        if is(py::getitem(entry, "says_what")?, "yes") {
            recoveries.push(identifier.clone());
        }
    }
    let mut cause_only = false;
    if recoveries.is_empty() {
        for (_, entry) in &facts {
            if is(py::getitem(entry, "identifies_cause")?, "yes") {
                cause_only = true;
                break;
            }
        }
    }
    let approved = approved(family)?;
    let recovery = |outcome, claim_ids, reason| Recovery {
        outcome,
        claim_ids,
        reason,
        cause_only,
    };
    if !recoveries.is_empty() && approved && admitted {
        return Ok(recovery(
            "caught",
            recoveries,
            "Original wording says what goes wrong for this known problem.",
        ));
    }
    if !approved {
        return Ok(recovery(
            "unresolved",
            recoveries,
            "The family's eligibility awaits a saved human ruling.",
        ));
    }
    if !recoveries.is_empty() {
        return Ok(recovery(
            "unresolved",
            recoveries,
            "The review was not admitted, so its claim earns no recovery.",
        ));
    }
    let mut open_claims = Vec::new();
    for (identifier, entry) in &facts {
        if is(py::getitem(entry, "says_what")?, "cannot-tell") {
            open_claims.push(identifier.clone());
        }
    }
    if !open_claims.is_empty() {
        return Ok(recovery(
            "unresolved",
            open_claims,
            "Original claims leave credit for this known problem unresolved.",
        ));
    }
    Ok(recovery(
        "missed",
        Vec::new(),
        "No original claim says what goes wrong for this known problem.",
    ))
}

/// One item's answer on one fact of a known problem, from its claims' answers: "yes" when any claim says yes,
/// otherwise "cannot-tell" when any claim cannot tell, otherwise "no".
pub fn item_fact(answers: &[&Value]) -> &'static str {
    ["yes", "cannot-tell"]
        .into_iter()
        .find(|answer| answers.iter().any(|given| is(given, answer)))
        .unwrap_or("no")
}

/// Problems of one item's known-problem entries for a family against the user's ruling on that comment. The
/// ruling records "says what goes wrong"; it records "identifies the cause as a fault" only when the user ruled
/// on it. An item with no entry for the family says neither.
pub fn credit_problems(entries: &[&Value], ruling: &Value) -> PyResult<Vec<String>> {
    let mut problems = Vec::new();
    for name in ["says_what", "identifies_cause"] {
        let ruled = py::getitem(ruling, name)?;
        if matches!(ruled, Value::Null) {
            continue;
        }
        let answers = entries
            .iter()
            .map(|entry| py::getitem(entry, name))
            .collect::<PyResult<Vec<_>>>()?;
        if !is(ruled, item_fact(&answers)) {
            problems.push(format!(
                "the user ruled {name} {} for {} on this comment",
                ruled.py_repr(),
                py::getitem(ruling, "family")?.py_str()
            ));
        }
    }
    Ok(problems)
}

/// The rubric-v1 category of a rubric-v2 assignment, which the historical scorer reads.
pub fn legacy_assignment(assignment: &Value) -> PyResult<Value> {
    let Some(text) = assignment.as_str() else {
        return Err(Raised::AttributeError);
    };
    Ok(if text.starts_with("defect:") || text == "unresolved" {
        assignment.clone()
    } else if text == "refuted" || text == "unsupported" {
        Value::Str("false-finding".to_string())
    } else {
        Value::Str("non-material".to_string())
    })
}

/// The item-level fields of an item's most severe claim.
pub fn primary(claims: &[Value]) -> PyResult<Value> {
    for category in ["defect:", "false-finding", "unresolved", "non-material"] {
        for claim in claims {
            let assignment = legacy_assignment(py::getitem(claim, "assignment")?)?;
            if assignment
                .as_str()
                .is_some_and(|text| text.starts_with(category))
            {
                let mut projected = Object::default();
                projected.insert("assignment".to_string(), assignment);
                for name in ["duplicate_group", "fix_sufficiency", "notes"] {
                    projected.insert(name.to_string(), py::getitem(claim, name)?.clone());
                }
                return Ok(Value::Object(projected));
            }
        }
    }
    Err(Raised::ValueError)
}

/// `bench/schema/graded-claim.schema.json` under a repository root, which `claim_problems` checks each claim
/// against.
pub fn graded_claim_schema(root: &Path) -> Result<Value, String> {
    let path = root.join("bench/schema/graded-claim.schema.json");
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    crate::pyjson::loads(&text, crate::pyjson::Duplicates::KeepLast)
        .map_err(|error| error.to_string())
}

/// Problems with one rubric-v2 item's claims, given the register's defect ids and, when known, the item's text.
pub fn claim_problems(
    claims: &Value,
    defect_ids: &[&str],
    source_text: Option<&str>,
    schema: &Value,
) -> Result<Vec<String>, schema::SchemaError> {
    let Some(claims) = claims.as_array().filter(|claims| !claims.is_empty()) else {
        return Ok(vec!["an item needs a non-empty claims list".to_string()]);
    };
    let mut validator = schema::Validator::default();
    let mut problems = Vec::new();
    let mut ids: Vec<&str> = Vec::new();
    for (index, claim) in claims.iter().enumerate() {
        let number = index + 1;
        let found = validator.validate(schema, claim)?;
        problems.extend(
            found
                .iter()
                .map(|problem| format!("claim {number}: {problem}")),
        );
        if !found.is_empty() {
            continue;
        }
        let text = |name: &str| field(claim, name).as_str().unwrap_or_default();
        let identifier = text("id");
        if ids.contains(&identifier) {
            problems.push(format!("claim {number}: repeated id {identifier}"));
        }
        ids.push(identifier);
        for name in ["id", "quote", "notes"] {
            if pytext::strip(text(name)).is_empty() {
                problems.push(format!("claim {number}: {name} is empty"));
            }
        }
        for name in ["canonical_claim_id", "duplicate_group", "candidate"] {
            if let Value::Str(value) = field(claim, name)
                && pytext::strip(value).is_empty()
            {
                problems.push(format!("claim {number}: {name} must be null or non-empty"));
            }
        }
        let evidence = field(claim, "evidence").as_array().unwrap_or_default();
        if evidence.is_empty() || evidence.iter().any(|entry| !filled(entry)) {
            problems.push(format!(
                "claim {number}: needs inspected evidence or an explicit evidence limitation"
            ));
        }
        if let Some(source) = source_text
            && !source.contains(text("quote"))
        {
            problems.push(format!(
                "claim {number}: quote is not verbatim in the source item"
            ));
        }
        let assessment = field(claim, "assessment");
        let axis = |name: &str| field(assessment, name).as_str().unwrap_or_default();
        let assignment = text("assignment");
        let recovery = assignment.starts_with("defect:");
        if recovery {
            let defect = &assignment["defect:".len()..];
            if !defect_ids.contains(&defect) {
                problems.push(format!(
                    "claim {number}: {assignment} is not in the register"
                ));
            }
            if !(axis("support") == "supported"
                && ATTRIBUTED.contains(&axis("attribution"))
                && axis("reachability") == "reachable"
                && axis("materiality") == "material")
            {
                problems.push(format!(
                    "claim {number}: recovery must satisfy all four eligibility questions"
                ));
            }
        }
        let fix = text("fix_sufficiency");
        if recovery && !["sufficient", "partial", "absent"].contains(&fix) {
            problems.push(format!(
                "claim {number}: recovery needs a fix assessment, including absent"
            ));
        }
        if !recovery && fix != "n/a" {
            problems.push(format!(
                "claim {number}: only a recovery receives fix sufficiency"
            ));
        }
        if let Some((_, support)) = REQUIRED_SUPPORT
            .iter()
            .find(|(name, _)| *name == assignment)
            && axis("support") != *support
        {
            problems.push(format!(
                "claim {number}: {assignment} has inconsistent support"
            ));
        }
        if ["advisory", "inconsequential"].contains(&assignment)
            && axis("materiality") != "below-threshold"
        {
            problems.push(format!(
                "claim {number}: advisory/observation must be below threshold"
            ));
        }
        if assignment == "scope-excluded"
            && !["pre-existing", "out-of-scope"].contains(&axis("attribution"))
        {
            problems.push(format!(
                "claim {number}: scope exclusion needs a scope reason"
            ));
        }
        if !matches!(field(claim, "candidate"), Value::Null) && assignment != "unresolved" {
            problems.push(format!(
                "claim {number}: only unresolved claims name a novel candidate"
            ));
        }
    }
    Ok(problems)
}

/// Why a mapping cannot be checked: Python raised, or the schema could not be applied.
#[derive(Debug)]
pub enum MappingError {
    Raised(Raised),
    Schema(schema::SchemaError),
}

impl From<Raised> for MappingError {
    fn from(raised: Raised) -> Self {
        Self::Raised(raised)
    }
}

/// Problems with a mapping's rubric-v2 claims: each item's claims, its projected item fields, unique claim ids
/// within a review and consistent duplicate groups.
pub fn mapping_problems(
    mapping: &Value,
    defect_ids: &[&str],
    schema: &Value,
) -> Result<Vec<String>, MappingError> {
    let mut problems = Vec::new();
    let mut groups: Vec<(Value, Value, (Value, Value))> = Vec::new();
    for attempt in py::iterate(py::getitem(mapping, "attempts")?)? {
        let attempt = attempt.as_ref();
        let mut ids: Vec<Value> = Vec::new();
        for item in py::iterate(py::getitem(attempt, "items")?)? {
            let item = item.as_ref();
            if py::eq(
                py::getitem(mapping, "rubric_version")?,
                &Value::Int("1".into()),
            ) {
                if py::contains(item, &Value::Str("claims".into()))? {
                    problems.push("rubric v1 cannot contain claim-level assignments".to_string());
                }
                continue;
            }
            let claims = py::get(item, "claims")?.unwrap_or(&Value::Null);
            let found =
                claim_problems(claims, defect_ids, None, schema).map_err(MappingError::Schema)?;
            let label = || -> PyResult<String> {
                Ok(format!(
                    "{} {}",
                    py::getitem(attempt, "attempt_id")?.py_str(),
                    py::getitem(item, "item_id")?.py_str()
                ))
            };
            if !found.is_empty() {
                for problem in found {
                    problems.push(format!("{}: {problem}", label()?));
                }
                continue;
            }
            let claims = claims.as_array().unwrap_or_default();
            let projected = primary(claims)?;
            for (name, value) in projected.as_object().into_iter().flat_map(Object::iter) {
                if !py::eq(py::getitem(item, name)?, value) {
                    problems.push(format!("{}: incorrect {name} projection", label()?));
                }
            }
            for claim in claims {
                let identifier = py::getitem(claim, "id")?;
                if ids.iter().any(|seen| py::same(seen, identifier)) {
                    problems.push(format!(
                        "{}: claim IDs must be unique within a review",
                        py::getitem(attempt, "attempt_id")?.py_str()
                    ));
                }
                ids.push(identifier.clone());
                let group = py::getitem(claim, "duplicate_group")?;
                if !group.truthy() {
                    continue;
                }
                let attempt_id = py::getitem(attempt, "attempt_id")?;
                if !py::hashable(attempt_id) {
                    return Err(Raised::TypeError.into());
                }
                let signature = (
                    py::getitem(claim, "assignment")?.clone(),
                    py::getitem(claim, "canonical_claim_id")?.clone(),
                );
                let found = groups
                    .iter_mut()
                    .find(|entry| py::same(&entry.0, attempt_id) && py::same(&entry.1, group));
                match found {
                    Some(entry) => {
                        if !(py::same(&entry.2.0, &signature.0)
                            && py::same(&entry.2.1, &signature.1))
                        {
                            problems.push(format!(
                                "{}: duplicate group {} has conflicting verdicts",
                                attempt_id.py_str(),
                                group.py_str()
                            ));
                        }
                        entry.2 = signature;
                    }
                    None => groups.push((attempt_id.clone(), group.clone(), signature)),
                }
            }
        }
    }
    Ok(problems)
}

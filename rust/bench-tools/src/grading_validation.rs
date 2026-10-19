//! Validate blinded grader output without choosing judgments or reading an unblinding key, ported from
//! `bench/tools/grading_validation.py`. `validate` returns the violations Python prints, in its order, or the
//! exception Python would raise, which the command reports as unreadable or malformed input.

use crate::claim_grading::{self, filled, has_exactly, is, one_of, sorted_names};
use crate::pre_existing;
use crate::pyjson::{self, Duplicates, Value};
use crate::pyobject::{self as py, PyResult, Raised};
use crate::pytext;
use std::cmp::Ordering;

pub const CONTRACT: &str = "current-verdicts/v1";
pub const CONTRACT_V2: &str = "current-verdicts/v2";
pub const CONDITIONAL_SAFETY: &str = "conditional-safety/v1";
pub const NO_SAFETY: &str = "no-safety/v1";
pub const MALFORMED: &str = "cannot validate: unreadable or malformed JSON input";
const VERDICT_FIELDS: [&str; 3] = ["reviews", "new_candidates", "link_disputes"];
const REVIEW_FIELDS: [&str; 3] = ["items", "recommendations", "remedy_inventory"];
const RECOMMENDATION_FIELDS: [&str; 6] = [
    "id",
    "anchors",
    "addressed_claims",
    "duplicate_group",
    "sufficiency",
    "safety",
];
const CANDIDATE_FIELDS: [&str; 8] = [
    "id",
    "claim",
    "evidence",
    "limits",
    "relevance",
    "confidence",
    "would_settle",
    "items",
];
const DISPUTE_FIELDS: [&str; 4] = ["review", "item", "canonical_claim_id", "reason"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Contract {
    V1,
    V2,
    V3,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Safety {
    Legacy,
    Conditional,
    None,
}

fn texts(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|items| !items.is_empty() && items.iter().all(filled))
}

/// `isinstance(value, int) and not isinstance(value, bool)`.
fn number(value: &Value) -> bool {
    matches!(value, Value::Int(_))
}

fn int(value: usize) -> Value {
    Value::Int(value.to_string())
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    py::getitem(value, key).unwrap_or(&Value::Null)
}

/// `container[key]` for a `str` or `None` key.
fn subscript<'a>(container: &'a Value, key: &Value) -> PyResult<&'a Value> {
    match (container, key) {
        (Value::Object(object), Value::Str(key)) => object.get(key).ok_or(Raised::KeyError),
        (Value::Object(_), _) if py::hashable(key) => Err(Raised::KeyError),
        _ => Err(Raised::TypeError),
    }
}

/// `value.get(key, default)` where the default is an empty container.
fn get_or<'a>(value: &'a Value, key: &str, default: &'a Value) -> PyResult<&'a Value> {
    Ok(py::get(value, key)?.unwrap_or(default))
}

/// Whether the quote is verbatim inside one field of the original item.
fn quoted(quote: &Value, source: &Value) -> PyResult<bool> {
    if !filled(quote) {
        return Ok(false);
    }
    for segment in py::iterate(py::getitem(source, "segments")?)? {
        if py::contains(&segment, quote)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn item_numbers(count: usize) -> Vec<String> {
    (1..=count).map(|n| n.to_string()).collect()
}

/// Whether a dict's keys are exactly these names, in any order.
fn keys_are(object: &Value, names: &[String]) -> bool {
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    has_exactly(object, &names)
}

/// Problems with a pinned claim inventory against the workspace's reviews.
pub fn inventory_problems(inventory: &Value, reviews: &Value) -> PyResult<Vec<String>> {
    let Some(entries) = inventory.as_object() else {
        return Ok(vec![
            "inventory must cover exactly the workspace's reviews".to_string(),
        ]);
    };
    let members = py::set(reviews)?;
    let covered = members.len() == entries.len()
        && members.iter().all(|member| {
            member
                .as_str()
                .is_some_and(|key| entries.get(key).is_some())
        });
    if !covered {
        return Ok(vec![
            "inventory must cover exactly the workspace's reviews".to_string(),
        ]);
    }
    let Value::Object(reviews) = reviews else {
        return Err(Raised::AttributeError);
    };
    let mut problems = Vec::new();
    for (token, review) in reviews.iter() {
        let items = field(inventory, token);
        let sources = py::getitem(review, "items")?;
        let expected = item_numbers(py::len(sources)?);
        if !keys_are(items, &expected) {
            problems.push(format!(
                "{token}: inventory must cover exactly the review's items"
            ));
            continue;
        }
        for key in &expected {
            let item = field(items, key);
            let with_ids = item
                .as_object()
                .is_some_and(|item| item.get("ids").is_some());
            let fields: &[&str] = if with_ids {
                &["kind", "quotes", "ids"]
            } else {
                &["kind", "quotes"]
            };
            let quotes = field(item, "quotes");
            let shaped = has_exactly(item, fields)
                && one_of(field(item, "kind"), &["finding", "not-a-finding"])
                && quotes.as_array().is_some()
                && if is(field(item, "kind"), "finding") {
                    texts(quotes)
                } else {
                    py::eq(quotes, &Value::Array(Vec::new()))
                };
            if !shaped {
                problems.push(format!(
                    "{token} item {key}: inventory needs kind and matching quotes"
                ));
                continue;
            }
            let quotes = quotes.as_array().unwrap_or_default();
            if with_ids {
                let ids = field(item, "ids");
                let named = ids
                    .as_array()
                    .is_some_and(|ids| ids.len() == quotes.len() && ids.iter().all(filled));
                if !named {
                    problems.push(format!(
                        "{token} item {key}: inventory ids name each quoted claim"
                    ));
                    continue;
                }
            }
            let position = key.parse::<usize>().unwrap_or(1) - 1;
            for quote in quotes {
                if !quoted(quote, py::index(sources, position)?.as_ref())? {
                    problems.push(format!(
                        "{token} item {key}: inventory quote is not verbatim inside one source field"
                    ));
                    break;
                }
            }
        }
    }
    Ok(problems)
}

/// Whether a claim agrees with the decision pinned for its canonical claim, under verdict contract v2 or v3.
pub fn pinned_matches_v2(claim: &Value, pinned: &Value) -> PyResult<bool> {
    let pinned_outcome = py::getitem(pinned, "outcome")?;
    if is(pinned_outcome, "eligible") {
        if !is(py::getitem(claim, "outcome")?, "problem") {
            return Ok(false);
        }
        for entry in py::iterate(py::getitem(claim, "known_problems")?)? {
            let entry = entry.as_ref();
            if py::eq(
                py::getitem(entry, "family")?,
                py::getitem(pinned, "family")?,
            ) && is(py::getitem(entry, "says_what")?, "yes")
            {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    if !py::hashable(pinned_outcome) {
        return Err(Raised::TypeError);
    }
    let allowed: &[&str] = match pinned_outcome.as_str() {
        Some("advisory" | "inconsequential") => &["minor-defect", "suggestion"],
        Some("scope-excluded") => &["outside-this-change"],
        Some("refuted") => &["refuted"],
        Some("unsupported") => &["unproven"],
        Some("unresolved") => &["unresolved"],
        _ => return Err(Raised::KeyError),
    };
    Ok(one_of(py::getitem(claim, "outcome")?, allowed))
}

/// The equivalent links a grader disputes, as `(review, item, canonical claim)`.
#[derive(Default)]
struct Disputed(Vec<(Value, Value, Value)>);

impl Disputed {
    /// `(review, item, canonical) in disputed`.
    fn holds(&self, review: &str, item: usize, canonical: &Value) -> PyResult<bool> {
        if !py::hashable(canonical) {
            return Err(Raised::TypeError);
        }
        let review = Value::Str(review.to_string());
        let item = int(item);
        Ok(self
            .0
            .iter()
            .any(|(r, i, c)| py::same(r, &review) && py::same(i, &item) && py::same(c, canonical)))
    }
}

/// Which review items each named candidate came from, in the order claims first named them.
#[derive(Default)]
struct Naming(Vec<(String, Vec<(String, usize)>)>);

impl Naming {
    fn add(&mut self, name: &str, review: &str, item: usize) {
        let position = match self.0.iter().position(|(seen, _)| seen == name) {
            Some(position) => position,
            None => {
                self.0.push((name.to_string(), Vec::new()));
                self.0.len() - 1
            }
        };
        let items = &mut self.0[position].1;
        if !items.iter().any(|(r, i)| r == review && *i == item) {
            items.push((review.to_string(), item));
        }
    }

    fn items(&self, name: &str) -> &[(String, usize)] {
        self.0
            .iter()
            .find(|(seen, _)| seen == name)
            .map_or(&[], |(_, items)| items)
    }
}

#[derive(PartialEq)]
enum Facts {
    Family(Option<String>),
    Known(Vec<[String; 3]>),
}

#[derive(PartialEq)]
struct Signature {
    outcome: String,
    canonical: Option<String>,
    facts: Facts,
    pre_existing: Option<Vec<[String; 3]>>,
}

fn sorted_triples(entries: &Value, first: &str) -> Vec<[String; 3]> {
    let mut triples: Vec<[String; 3]> = entries
        .as_array()
        .unwrap_or_default()
        .iter()
        .map(|entry| {
            [first, "says_what", "identifies_cause"]
                .map(|name| field(entry, name).as_str().unwrap_or_default().to_string())
        })
        .collect();
    triples.sort();
    triples
}

/// One review's claims that passed their verdict rules, by id, with the item that holds each.
type Claims = Vec<(String, usize, Value)>;

fn claim_by_id<'a>(claims: &'a Claims, identifier: &str) -> Option<&'a Value> {
    claims
        .iter()
        .find(|(id, _, _)| id == identifier)
        .map(|(_, _, claim)| claim)
}

struct Snapshot<'a> {
    value: &'a Value,
    contract: Contract,
}

impl Snapshot<'_> {
    fn v2(&self) -> bool {
        self.contract != Contract::V1
    }

    fn v3(&self) -> bool {
        self.contract == Contract::V3
    }
}

/// Every item decomposed into claims with native quotations and pinned decisions.
fn claim_problems(
    token: &str,
    review: &Value,
    sources: &Value,
    snapshot: &Snapshot,
    disputed: &Disputed,
    naming: &mut Naming,
) -> PyResult<(Vec<String>, Claims)> {
    let items = py::getitem(review, "items")?;
    let count = py::len(sources)?;
    let expected = item_numbers(count);
    if !keys_are(items, &expected) {
        let keys = if expected.is_empty() {
            "none ({} for an empty review)".to_string()
        } else {
            format!("\"1\"..\"{count}\"")
        };
        return Ok((
            vec![format!("{token}: item keys must be {keys}")],
            Vec::new(),
        ));
    }
    let v2 = snapshot.v2();
    let mut problems = Vec::new();
    let mut claims: Claims = Vec::new();
    let mut groups: Vec<(String, Signature)> = Vec::new();
    let empty_object = Value::Object(pyjson::Object::default());
    let empty_list = Value::Array(Vec::new());
    let inventory = if v2 {
        py::get(get_or(snapshot.value, "inventory", &empty_object)?, token)?
    } else {
        None
    };
    if let Some(inventory) = inventory {
        let order = |value: &Value| -> Vec<String> {
            value
                .as_object()
                .map(|object| object.iter().map(|(key, _)| key.clone()).collect())
                .unwrap_or_default()
        };
        if order(items) != order(inventory) {
            problems.push(format!("{token}: items must follow the inventory order"));
        }
    }
    for key in &expected {
        let number: usize = key.parse().unwrap_or(1);
        let place = format!("{token} item {key}");
        let item = field(items, key);
        let claim_list = field(item, "claims");
        if v2 {
            let finding = is(field(item, "kind"), "finding");
            let shaped = has_exactly(item, &["kind", "note", "claims"])
                && field(item, "note").as_str().is_some()
                && claim_list.as_array().is_some()
                && one_of(field(item, "kind"), &["finding", "not-a-finding"])
                && if finding {
                    claim_list.truthy()
                } else {
                    !claim_list.truthy() && filled(field(item, "note"))
                };
            if !shaped {
                problems.push(format!(
                    "{place}: finding needs claims; not-a-finding needs no claims and a non-empty note"
                ));
                continue;
            }
            if let Some(inventory) = inventory {
                let pinned = field(inventory, key);
                let listed = |name: &str| {
                    Value::Array(
                        claim_list
                            .as_array()
                            .unwrap_or_default()
                            .iter()
                            .map(|claim| match claim {
                                Value::Object(claim) => {
                                    claim.get(name).cloned().unwrap_or(Value::Null)
                                }
                                _ => Value::Null,
                            })
                            .collect(),
                    )
                };
                let ids = listed("id");
                if !py::eq(field(item, "kind"), field(pinned, "kind"))
                    || !py::eq(&listed("quote"), field(pinned, "quotes"))
                {
                    problems.push(format!(
                        "{place}: kind and claim quotes must match the inventory in order"
                    ));
                } else if let Some(pinned_ids) =
                    pinned.as_object().and_then(|pinned| pinned.get("ids"))
                    && !py::eq(&ids, pinned_ids)
                {
                    problems.push(format!(
                        "{place}: claim ids must match the inventory in order"
                    ));
                }
            }
        } else if !has_exactly(item, &["notes", "claims"])
            || !filled(field(item, "notes"))
            || !claim_list.truthy()
            || claim_list.as_array().is_none()
        {
            problems.push(format!(
                "{place}: needs non-empty notes and a non-empty claims list"
            ));
            continue;
        }
        let links = py::getitem(snapshot.value, "links")?;
        let linked = get_or(get_or(links, token, &empty_object)?, key, &empty_list)?;
        let matches = py::getitem(snapshot.value, "matches")?;
        let equivalent = get_or(get_or(matches, token, &empty_object)?, key, &empty_list)?;
        let claim_list = claim_list.as_array().unwrap_or_default();
        for (index, claim) in claim_list.iter().enumerate() {
            let index = index + 1;
            let families = py::getitem(snapshot.value, "families")?;
            let found = if snapshot.v3() {
                let listed = py::getitem(snapshot.value, "pre_existing_problems")?;
                pre_existing::verdict_problems(claim, families, listed)?
            } else if v2 {
                claim_grading::verdict_problems_v2(claim, families)?
            } else {
                claim_grading::verdict_problems(claim, families)?
            };
            problems.extend(
                found
                    .iter()
                    .map(|problem| format!("{place} claim {index}: {problem}")),
            );
            if !found.is_empty() {
                continue;
            }
            let text = |name: &str| field(claim, name).as_str().unwrap_or_default().to_string();
            let identifier = text("id");
            if claims.iter().any(|(id, _, _)| *id == identifier) {
                problems.push(format!("{token}: claim ID {identifier} repeated"));
                claims.retain(|(id, _, _)| *id != identifier);
            }
            claims.push((identifier.clone(), number, claim.clone()));
            if !quoted(
                field(claim, "quote"),
                py::index(sources, number - 1)?.as_ref(),
            )? {
                problems.push(format!(
                    "{place} claim {index}: quote is not verbatim inside one field of the source item"
                ));
            }
            let canonical = field(claim, "canonical_claim_id");
            if !v2 && matches!(canonical, Value::Null) {
                let mut undisputed = false;
                for link in py::iterate(equivalent)? {
                    if !disputed.holds(token, number, &link)? {
                        undisputed = true;
                        break;
                    }
                }
                if undisputed {
                    problems.push(format!(
                        "{place} claim {index}: an equivalent item carries only its linked canonical claims; \
                         record a link dispute when it asserts something else"
                    ));
                }
            }
            if !matches!(canonical, Value::Null) && !py::contains(linked, canonical)? {
                problems.push(format!(
                    "{place} claim {index}: canonical claim is not linked to this item"
                ));
            } else if py::contains(equivalent, canonical)?
                && !disputed.holds(token, number, canonical)?
            {
                let pinned = subscript(py::getitem(snapshot.value, "canonical")?, canonical)?;
                let agrees = if v2 {
                    pinned_matches_v2(claim, pinned)?
                } else {
                    let pinned_outcome = py::getitem(pinned, "outcome")?;
                    let pinned_family = py::getitem(pinned, "family")?;
                    py::same(field(claim, "outcome"), pinned_outcome)
                        && py::same(field(claim, "family"), pinned_family)
                };
                if !agrees {
                    problems.push(format!(
                        "{place} claim {index}: canonical outcome or family disagrees with the pinned decision; \
                         record a link dispute when the wording does not identify that claim"
                    ));
                }
            }
            let group = text("duplicate_group");
            if !group.is_empty() {
                let signature = Signature {
                    outcome: text("outcome"),
                    canonical: canonical.as_str().map(str::to_string),
                    facts: if v2 {
                        Facts::Known(sorted_triples(field(claim, "known_problems"), "family"))
                    } else {
                        Facts::Family(field(claim, "family").as_str().map(str::to_string))
                    },
                    pre_existing: snapshot
                        .v3()
                        .then(|| sorted_triples(field(claim, "pre_existing_problems"), "problem")),
                };
                match groups.iter().find(|(name, _)| *name == group) {
                    Some((_, first)) if *first != signature => problems.push(format!(
                        "{token}: duplicate group {group} has conflicting verdicts"
                    )),
                    Some(_) => {}
                    None => groups.push((group, signature)),
                }
            }
            for name in ["candidate", "pre_existing_candidate"] {
                if let Some(candidate) = claim.as_object().and_then(|claim| claim.get(name))
                    && let Some(candidate) = candidate.as_str()
                {
                    naming.add(candidate, token, number);
                }
            }
            if snapshot.v3()
                && py::contains(equivalent, canonical)?
                && !disputed.holds(token, number, canonical)?
            {
                let pinned = subscript(py::getitem(snapshot.value, "canonical")?, canonical)?;
                let listed = py::get(pinned, "pre_existing_problem")?.unwrap_or(&Value::Null);
                let named = field(claim, "pre_existing_problems")
                    .as_array()
                    .unwrap_or_default()
                    .iter()
                    .any(|entry| py::eq(field(entry, "problem"), listed));
                if !matches!(listed, Value::Null) && !named {
                    problems.push(format!(
                        "{place}: canonical pre-existing-problem link is missing"
                    ));
                }
            }
        }
        for link in py::iterate(equivalent)? {
            if disputed.holds(token, number, &link)? {
                continue;
            }
            let carried = claim_list.iter().any(|claim| {
                claim.as_object().is_some_and(|claim| {
                    py::eq(
                        claim.get("canonical_claim_id").unwrap_or(&Value::Null),
                        &link,
                    )
                })
            });
            if !carried {
                problems.push(format!(
                    "{place}: equivalent item needs its canonical claim {}",
                    link.py_str()
                ));
            }
        }
        if v2 {
            let mut all_kept = true;
            for claim in claim_list {
                let Some(object) = claim.as_object() else {
                    all_kept = false;
                    break;
                };
                let identifier = object.get("id").unwrap_or(&Value::Null);
                if !py::hashable(identifier) {
                    return Err(Raised::TypeError);
                }
                if !identifier
                    .as_str()
                    .is_some_and(|identifier| claim_by_id(&claims, identifier).is_some())
                {
                    all_kept = false;
                    break;
                }
            }
            if all_kept {
                let credits = get_or(snapshot.value, "credits", &empty_object)?;
                let rulings = get_or(get_or(credits, token, &empty_object)?, key, &empty_list)?;
                for ruling in py::iterate(rulings)? {
                    let ruling = ruling.as_ref();
                    let mut entries: Vec<&Value> = Vec::new();
                    for claim in claim_list {
                        let pre_existing =
                            py::get(ruling, "pre_existing")?.is_some_and(Value::truthy);
                        let (list, name) = if pre_existing {
                            ("pre_existing_problems", "problem")
                        } else {
                            ("known_problems", "family")
                        };
                        let listed = claim
                            .as_object()
                            .and_then(|claim| claim.get(list))
                            .and_then(Value::as_array)
                            .unwrap_or_default();
                        for entry in listed {
                            if py::eq(field(entry, name), py::getitem(ruling, "family")?) {
                                entries.push(entry);
                            }
                        }
                    }
                    problems.extend(
                        claim_grading::credit_problems(&entries, ruling)?
                            .iter()
                            .map(|problem| format!("{place}: {problem}")),
                    );
                }
            }
        }
    }
    Ok((problems, claims))
}

/// Corrective requests as distinct recommendations: each anchored in original wording, assessed for sufficiency
/// per addressed family and for safety on its own.
fn remedy_problems(
    token: &str,
    review: &Value,
    sources: &Value,
    claims: &Claims,
    v2: bool,
    safety: Safety,
) -> PyResult<Vec<String>> {
    let recommendations = py::getitem(review, "recommendations")?;
    let inventory = py::getitem(review, "remedy_inventory")?;
    let Some(recommendations) = recommendations.as_array() else {
        return Ok(vec![format!("{token}: recommendations must be a list")]);
    };
    let mut problems = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    let mut groups: Vec<Value> = Vec::new();
    let mut anchored: Vec<Value> = Vec::new();
    let fields: Vec<&str> = RECOMMENDATION_FIELDS
        .into_iter()
        .filter(|name| {
            !(v2 && *name == "duplicate_group") && !(safety == Safety::None && *name == "safety")
        })
        .collect();
    let mut outcomes = vec!["sufficient", "partial", "unassessed"];
    if safety != Safety::Legacy {
        outcomes.push("ineffective");
    }
    let count = Value::Int(py::len(sources)?.to_string());
    for (index, recommendation) in recommendations.iter().enumerate() {
        let place = format!("{token} recommendation {}", index + 1);
        let get = |name: &str| field(recommendation, name);
        if !has_exactly(recommendation, &fields) || !filled(get("id")) {
            problems.push(format!("{place}: needs exactly {}", sorted_names(&fields)));
            continue;
        }
        let identifier = get("id").as_str().unwrap_or_default().to_string();
        if ids.contains(&identifier) {
            problems.push(format!("{place}: repeated id {identifier}"));
        }
        ids.push(identifier);
        let anchors = get("anchors");
        let one = Value::Int("1".to_string());
        let well_formed = anchors.as_array().is_some_and(|anchors| {
            !anchors.is_empty()
                && anchors.iter().all(|anchor| {
                    let item = field(anchor, "item");
                    has_exactly(anchor, &["item", "quote"])
                        && number(item)
                        && py::compare_numbers(&one, item) != Some(Ordering::Greater)
                        && py::compare_numbers(item, &count) != Some(Ordering::Greater)
                })
        });
        if well_formed {
            for anchor in anchors.as_array().unwrap_or_default() {
                let item = field(anchor, "item");
                let position = item.py_str().parse::<usize>().unwrap_or(1) - 1;
                if !quoted(
                    field(anchor, "quote"),
                    py::index(sources, position)?.as_ref(),
                )? {
                    problems.push(format!(
                        "{place}: anchor quote is not verbatim inside one field of item {}",
                        item.py_str()
                    ));
                }
            }
            for anchor in anchors.as_array().unwrap_or_default() {
                let item = field(anchor, "item");
                if !anchored.iter().any(|seen| py::same(seen, item)) {
                    anchored.push(item.clone());
                }
            }
        } else {
            problems.push(format!(
                "{place}: anchors must be a non-empty list of {{item, quote}} naming this review's items"
            ));
        }
        let addressed = get("addressed_claims");
        let named: Vec<&str> = addressed
            .as_array()
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        let listed = texts(addressed) || (v2 && py::eq(addressed, &Value::Array(Vec::new())));
        let distinct = named
            .iter()
            .enumerate()
            .all(|(at, name)| !named[..at].contains(name));
        if !(listed && distinct && named.iter().all(|name| claim_by_id(claims, name).is_some())) {
            problems.push(format!(
                "{place}: addressed_claims must name this review's claims, each once"
            ));
            continue;
        }
        let group = recommendation
            .as_object()
            .and_then(|recommendation| recommendation.get("duplicate_group"))
            .unwrap_or(&Value::Null);
        if !matches!(group, Value::Null)
            && (!filled(group) || groups.iter().any(|seen| py::same(seen, group)))
        {
            problems.push(format!(
                "{place}: a repeated remedy is one recommendation with all of its original anchors"
            ));
        }
        if !py::hashable(group) {
            return Err(Raised::TypeError);
        }
        if !groups.iter().any(|seen| py::same(seen, group)) {
            groups.push(group.clone());
        }
        let mut families: Vec<String> = Vec::new();
        for name in &named {
            let claim = claim_by_id(claims, name).unwrap_or(&Value::Null);
            if v2 {
                for entry in field(claim, "known_problems")
                    .as_array()
                    .unwrap_or_default()
                {
                    if is(field(entry, "says_what"), "yes")
                        || is(field(entry, "identifies_cause"), "yes")
                    {
                        families.push(
                            field(entry, "family")
                                .as_str()
                                .unwrap_or_default()
                                .to_string(),
                        );
                    }
                }
            } else if let Some(family) = field(claim, "family").as_str() {
                families.push(family.to_string());
            }
        }
        families.sort();
        families.dedup();
        let sufficiency = get("sufficiency");
        let assessed = sufficiency.as_array().is_some_and(|entries| {
            entries.iter().all(|entry| {
                let outcome = field(entry, "outcome");
                has_exactly(entry, &["family", "outcome", "reason", "evidence"])
                    && filled(field(entry, "reason"))
                    && filled(field(entry, "family"))
                    && one_of(outcome, &outcomes)
                    && (texts(field(entry, "evidence"))
                        || (is(outcome, "unassessed")
                            && py::eq(field(entry, "evidence"), &Value::Array(Vec::new()))))
            })
        });
        if !assessed {
            problems.push(format!(
                "{place}: each sufficiency entry needs family, outcome, reason and evidence; only unassessed may have none"
            ));
        } else {
            let mut covered: Vec<String> = sufficiency
                .as_array()
                .unwrap_or_default()
                .iter()
                .map(|entry| {
                    field(entry, "family")
                        .as_str()
                        .unwrap_or_default()
                        .to_string()
                })
                .collect();
            covered.sort();
            if covered != families {
                problems.push(format!(
                    "{place}: assess sufficiency once for every family its addressed claims name"
                ));
            }
        }
        if safety == Safety::None {
            continue;
        }
        let assessment = get("safety");
        let state = field(assessment, "state");
        let stated = has_exactly(assessment, &["state", "reason", "evidence"])
            && filled(field(assessment, "reason"))
            && one_of(state, &["safe", "unsafe", "unassessed"])
            && (texts(field(assessment, "evidence"))
                || (is(state, "unassessed")
                    && py::eq(field(assessment, "evidence"), &Value::Array(Vec::new()))));
        if !stated {
            problems.push(format!(
                "{place}: safety needs state, reason and evidence; only unassessed may have none"
            ));
        } else if safety == Safety::Conditional
            && let Some(entries) = sufficiency.as_array()
            && !entries.iter().any(|entry| {
                entry.as_object().is_some_and(|entry| {
                    entry
                        .get("outcome")
                        .is_some_and(|outcome| one_of(outcome, &["sufficient", "partial"]))
                })
            })
            && (!is(state, "unassessed")
                || !py::eq(field(assessment, "evidence"), &Value::Array(Vec::new())))
        {
            problems.push(format!(
                "{place}: skip safety unless a fix is sufficient or partial; use unassessed with no safety evidence"
            ));
        }
    }
    let state = field(inventory, "state");
    let reason = field(inventory, "reason");
    let complete_inventory = has_exactly(inventory, &["state", "reason"])
        && one_of(state, &["complete", "incomplete"])
        && (filled(reason) || (v2 && is(state, "complete") && is(reason, "")));
    if !complete_inventory {
        problems.push(format!(
            "{token}: remedy_inventory needs state complete or incomplete and a reason"
        ));
        return Ok(problems);
    }
    if is(state, "complete") {
        for (index, source) in py::iterate(sources)?.iter().enumerate() {
            let source: &Value = source;
            let number = index + 1;
            if py::getitem(source, "proposed_fix")?.truthy()
                && !anchored.iter().any(|seen| py::same(seen, &int(number)))
            {
                problems.push(format!(
                    "{token} item {number}: a complete remedy inventory covers this item's proposed fix"
                ));
            }
        }
    }
    Ok(problems)
}

fn candidate_problems(
    candidates: &[Value],
    naming: &Naming,
    v2: bool,
    pre_existing_names: Option<&[String]>,
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut ids: Vec<&str> = Vec::new();
    let mut fields: Vec<&str> = CANDIDATE_FIELDS
        .into_iter()
        .filter(|name| !(v2 && *name == "confidence"))
        .collect();
    if pre_existing_names.is_some() {
        fields.push("kind");
    }
    let texts_needed: Vec<&str> = fields
        .iter()
        .copied()
        .filter(|name| *name != "items")
        .collect();
    for (index, candidate) in candidates.iter().enumerate() {
        if !has_exactly(candidate, &fields)
            || !texts_needed
                .iter()
                .all(|name| filled(field(candidate, name)))
        {
            problems.push(format!(
                "new_candidates[{index}]: needs non-empty {} and items",
                sorted_names(&texts_needed)
            ));
            continue;
        }
        let name = field(candidate, "id").as_str().unwrap_or_default();
        if let Some(names) = pre_existing_names {
            let kind = if names.iter().any(|known| known == name) {
                "pre-existing-problem"
            } else {
                "ordinary"
            };
            if !is(field(candidate, "kind"), kind) {
                problems.push(format!(
                    "{name}: candidate kind contradicts the claims naming it"
                ));
            }
        }
        if ids.contains(&name) {
            problems.push(format!("new_candidates[{index}]: id {name} is repeated"));
        }
        ids.push(name);
        let items = field(candidate, "items");
        let listed = items.as_array().is_some_and(|items| {
            !items.is_empty()
                && items.iter().all(|item| {
                    has_exactly(item, &["review", "item"])
                        && field(item, "review").as_str().is_some()
                        && number(field(item, "item"))
                })
        });
        if !listed {
            problems.push(format!(
                "{name}: items must be a non-empty list of {{review, item}}"
            ));
            continue;
        }
        let mut given: Vec<(String, Value)> = Vec::new();
        for item in items.as_array().unwrap_or_default() {
            let pair = (
                field(item, "review")
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                field(item, "item").clone(),
            );
            if !given
                .iter()
                .any(|(r, i)| *r == pair.0 && py::same(i, &pair.1))
            {
                given.push(pair);
            }
        }
        let named = naming.items(name);
        let same_items = given.len() == named.len()
            && named.iter().all(|(r, i)| {
                given
                    .iter()
                    .any(|(gr, gi)| gr == r && py::same(gi, &int(*i)))
            });
        let repeated = v2 && given.len() != items.as_array().unwrap_or_default().len();
        if !same_items || repeated {
            problems.push(format!(
                "{name}: its items must be exactly the items whose claims name it"
            ));
        }
    }
    let mut missing: Vec<&str> = naming
        .0
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !ids.contains(name))
        .collect();
    missing.sort_unstable();
    problems.extend(
        missing
            .into_iter()
            .map(|name| format!("{name}: named by a claim but missing from new_candidates")),
    );
    problems
}

/// Equivalence links whose original wording the grader finds does not identify the claim.
fn dispute_problems(disputes: &[Value], snapshot: &Value) -> PyResult<(Vec<String>, Disputed)> {
    let mut problems = Vec::new();
    let mut disputed = Disputed::default();
    let empty_object = Value::Object(pyjson::Object::default());
    let empty_list = Value::Array(Vec::new());
    for (index, dispute) in disputes.iter().enumerate() {
        let review = field(dispute, "review");
        let item = field(dispute, "item");
        if !(has_exactly(dispute, &DISPUTE_FIELDS)
            && review.as_str().is_some()
            && number(item)
            && filled(field(dispute, "reason")))
        {
            problems.push(format!(
                "link_disputes[{index}]: needs review, item, canonical_claim_id and a reason"
            ));
            continue;
        }
        let canonical = field(dispute, "canonical_claim_id");
        let matches = py::getitem(snapshot, "matches")?;
        let links = get_or(
            get_or(matches, review.as_str().unwrap_or_default(), &empty_object)?,
            &item.py_str(),
            &empty_list,
        )?;
        if !py::contains(links, canonical)? {
            problems.push(format!("link_disputes[{index}]: no such equivalent link"));
        } else if !py::hashable(canonical) {
            return Err(Raised::TypeError);
        } else {
            disputed
                .0
                .push((review.clone(), item.clone(), canonical.clone()));
        }
    }
    Ok((problems, disputed))
}

/// The violations of a grader's verdicts against the workspace's validator inputs, or the exception Python
/// raises reading them.
pub fn validate(verdicts: &Value, snapshot: &Value) -> PyResult<Vec<String>> {
    let default_contract = Value::Str(CONTRACT.to_string());
    let contract = py::get(snapshot, "contract")?.unwrap_or(&default_contract);
    let contract = match contract.as_str() {
        Some(CONTRACT) => Contract::V1,
        Some(CONTRACT_V2) => Contract::V2,
        Some(pre_existing::CONTRACT) => Contract::V3,
        _ => {
            return Ok(vec![format!(
                "unsupported verdict contract {}",
                contract.py_repr()
            )]);
        }
    };
    let default_safety = Value::Str("legacy".to_string());
    let safety = py::get(snapshot, "safety_policy")?.unwrap_or(&default_safety);
    let safety = match safety.as_str() {
        Some("legacy") => Safety::Legacy,
        Some(CONDITIONAL_SAFETY) => Safety::Conditional,
        Some(NO_SAFETY) => Safety::None,
        _ => {
            return Ok(vec![format!(
                "unsupported safety policy {}",
                safety.py_repr()
            )]);
        }
    };
    if safety != Safety::Legacy && contract != Contract::V3 {
        return Ok(vec![
            "the safety policy requires verdict contract v3".to_string(),
        ]);
    }
    let pinned_inventory = py::get(snapshot, "inventory")?;
    if safety == Safety::None && pinned_inventory.is_none() {
        return Ok(vec![
            "the second grader requires a pinned claim inventory".to_string(),
        ]);
    }
    let shaped = has_exactly(verdicts, &VERDICT_FIELDS)
        && field(verdicts, "reviews").as_object().is_some()
        && field(verdicts, "new_candidates").as_array().is_some()
        && field(verdicts, "link_disputes").as_array().is_some();
    if !shaped {
        return Ok(vec![
            "verdicts.json needs exactly a reviews object, a new_candidates list and a link_disputes list"
                .to_string(),
        ]);
    }
    let snapshot = Snapshot {
        value: snapshot,
        contract,
    };
    let reviews = field(verdicts, "reviews");
    let sources = py::getitem(snapshot.value, "reviews")?;
    if snapshot.v2()
        && let Some(inventory) = pinned_inventory
    {
        let found = inventory_problems(inventory, sources)?;
        if !found.is_empty() {
            return Ok(found);
        }
    }
    let given = py::set(reviews)?;
    let mut problems: Vec<String> = py::sorted(py::difference(&py::set(sources)?, &given))?
        .iter()
        .map(|token| format!("{}: no verdicts for this review", token.py_str()))
        .collect();
    let known = py::set(sources)?;
    problems.extend(
        py::sorted(py::difference(&given, &known))?
            .iter()
            .map(|token| format!("{}: not a review the grader was given", token.py_str())),
    );
    let (found, disputed) = dispute_problems(
        field(verdicts, "link_disputes")
            .as_array()
            .unwrap_or_default(),
        snapshot.value,
    )?;
    problems.extend(found);
    let mut naming = Naming::default();
    let mut pre_existing_names: Option<Vec<String>> = snapshot.v3().then(Vec::new);
    let mut ordinary_names: Vec<String> = Vec::new();
    let shared: Vec<Value> = py::sorted(
        given
            .iter()
            .filter(|token| known.iter().any(|other| py::same(other, token)))
            .cloned()
            .collect(),
    )?;
    for token in &shared {
        let token = token.as_str().unwrap_or_default();
        let review = field(reviews, token);
        if !has_exactly(review, &REVIEW_FIELDS) {
            problems.push(format!(
                "{token}: needs exactly {}",
                sorted_names(&REVIEW_FIELDS)
            ));
            continue;
        }
        let items = py::getitem(py::getitem(sources, token)?, "items")?;
        let (found, claims) =
            claim_problems(token, review, items, &snapshot, &disputed, &mut naming)?;
        let clean = found.is_empty();
        problems.extend(found);
        for (_, _, claim) in &claims {
            if let Some(name) = field(claim, "candidate").as_str()
                && !ordinary_names.iter().any(|seen| seen == name)
            {
                ordinary_names.push(name.to_string());
            }
            if let Some(names) = pre_existing_names.as_mut()
                && let Some(name) = field(claim, "pre_existing_candidate").as_str()
                && !names.iter().any(|seen| seen == name)
            {
                names.push(name.to_string());
            }
        }
        if clean {
            let items = py::getitem(py::getitem(sources, token)?, "items")?;
            problems.extend(remedy_problems(
                token,
                review,
                items,
                &claims,
                snapshot.v2(),
                safety,
            )?);
        }
    }
    if let Some(names) = &pre_existing_names
        && names.iter().any(|name| ordinary_names.contains(name))
    {
        problems.push("a candidate cannot be both ordinary and pre-existing".to_string());
    }
    problems.extend(candidate_problems(
        field(verdicts, "new_candidates")
            .as_array()
            .unwrap_or_default(),
        &naming,
        snapshot.v2(),
        pre_existing_names.as_deref(),
    ));
    Ok(problems)
}

/// `read_verdicts`: strict UTF-8 JSON that writes no object key twice.
pub fn read_verdicts(path: &[u8]) -> Result<Value, String> {
    let text = pytext::read_file_text(path).map_err(|error| error.message().to_string())?;
    pyjson::loads(&text, Duplicates::Refuse).map_err(|error| error.to_string())
}

/// The lines `bench-tools grading validate` prints: each violation, or the single refusal when an input cannot be
/// read or holds a shape the rules cannot read.
pub fn command(verdicts: &[u8], inputs: &[u8]) -> Vec<String> {
    let snapshot = pytext::read_file_text(inputs)
        .map_err(|error| error.message().to_string())
        .and_then(|text| {
            pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| error.to_string())
        });
    let outcome = snapshot.and_then(|snapshot| Ok((read_verdicts(verdicts)?, snapshot)));
    match outcome.map(|(verdicts, snapshot)| validate(&verdicts, &snapshot)) {
        Ok(Ok(problems)) => problems,
        _ => vec![MALFORMED.to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_raise_anywhere_replaces_every_violation() {
        let snapshot = pyjson::loads("[]", Duplicates::KeepLast).unwrap();
        let verdicts = pyjson::loads("{}", Duplicates::Refuse).unwrap();
        assert_eq!(
            validate(&verdicts, &snapshot).err(),
            Some(Raised::AttributeError)
        );
    }
}

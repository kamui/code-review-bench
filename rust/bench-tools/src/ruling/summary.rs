//! `ruling_summary.py`: read ruling records without changing saved decisions or granting authority.

use super::{Repo, record};
use crate::delegation;
use crate::pyjson::{Object, Value};
use crate::pytext::{self, PyPath};
use crate::pyvalue::{self, Natural, Py, Raised, get, item};

const RESERVING_CLAUSES: [&str; 14] = [
    "before-4",
    "promised-1c",
    "promised-5",
    "promised-7b",
    "promised-8b",
    "delivered-2",
    "delivered-3",
    "B4",
    "P1c",
    "P5",
    "P7b",
    "P9b",
    "D8",
    "D9",
];

/// A before-record and its after-record, with where they are and the round that holds them.
#[derive(Clone, Debug)]
pub struct Pair {
    pub before: Value,
    /// `None` (JSON null) when there is no after-record.
    pub after: Value,
    /// Relative to the root.
    pub path: String,
    pub hash: String,
    pub round: Value,
    pub opened: Value,
    pub closed: Value,
    pub opening: Value,
    /// Answers with the confidence a declared-answer supplement restores, when one does.
    pub declared: Option<Vec<Value>>,
}

fn value_error<T>(message: String) -> Py<T> {
    Err(Raised::Value(message))
}

/// `Path(path).parent`, for a relative path.
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or(".", |(head, _)| head)
}

/// `sorted(root.rglob(name pattern))`: every entry whose name matches, in subdirectories too but not through
/// symbolic links, in `PurePosixPath` order.
fn rglob(directory: &PyPath, pattern: &str) -> Py<Vec<PyPath>> {
    let pattern: Vec<u32> = pattern.chars().map(u32::from).collect();
    let mut found = Vec::new();
    let mut pending = vec![directory.clone()];
    while let Some(current) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(current.to_path_buf()) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name =
                std::os::unix::ffi::OsStrExt::as_bytes(entry.file_name().as_os_str()).to_vec();
            let path = current.join(&name);
            if pyvalue::fnmatchcase(&pytext::surrogateescape(&name), &pattern)? {
                found.push(path.clone());
            }
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                pending.push(path);
            }
        }
    }
    found.sort_by_key(|path| pytext::path_order(path.as_bytes()));
    Ok(found)
}

fn text_path(path: &PyPath) -> String {
    pytext::stderr_text(path.as_bytes())
}

fn natural_key(pair: &Pair) -> Py<Vec<Natural>> {
    pyvalue::natural(&pair.path)
}

fn compare_keys(left: &(Value, Vec<Natural>), right: &(Value, Vec<Natural>)) -> std::cmp::Ordering {
    pyvalue::compare(&left.0, &right.0)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| pyvalue::natural_order(&left.1, &right.1))
}

/// Every before-record under docs/research with its after-record, in the order rounds opened. Refuses a bad
/// record, pin or round file.
pub fn records(repo: &Repo) -> Py<Vec<Pair>> {
    let research = repo.join("docs/research");
    for round_path in rglob(&research, "round.json")? {
        let directory = PyPath::new(parent_bytes(round_path.as_bytes()));
        let problems = record::round_faults(repo, &directory)?;
        if !problems.is_empty() {
            return value_error(format!(
                "{}: {}",
                text_path(&round_path),
                problems.join("; ")
            ));
        }
    }
    let mut pairs = Vec::new();
    for path in rglob(&research, "*.before.json")? {
        let before = record::read_before(repo, &path)?;
        let mut problems = record::faults(repo, &before)?;
        let directory = parent_bytes(path.as_bytes());
        let round_path = PyPath::new(directory).join(b"round.json");
        if !pyvalue::is_file(&round_path) {
            problems.push("no round.json".into());
        }
        let name = path
            .as_bytes()
            .rsplit(|byte| *byte == b'/')
            .next()
            .unwrap_or_default();
        let after_name = replace_all(name, b".before.json", b".after.json");
        let after_path = PyPath::new(directory).join(&after_name);
        let after = if pyvalue::is_file(&after_path) {
            repo.load(&after_path)?
        } else {
            Value::Null
        };
        if pyvalue::is_file(&after_path) && problems.is_empty() {
            problems.extend(record::after_faults(
                repo, &before, &after, &path, false, None,
            )?);
        }
        if !problems.is_empty() {
            return value_error(format!("{}: {}", text_path(&path), problems.join("; ")));
        }
        let sheet = repo.load(&round_path)?;
        let relative = crate::pyfs::relative_to(path.as_bytes(), repo.root.as_bytes())
            .map_err(Raised::Value)?;
        let relative = String::from_utf8(relative).map_err(|error| {
            Raised::Unsupported(format!(
                "a ruling record's path is not UTF-8: {}",
                pytext::stderr_text(error.as_bytes())
            ))
        })?;
        let hash = repo.digest(&path)?;
        pairs.push(Pair {
            before,
            after,
            path: relative,
            hash,
            round: item(&sheet, "round")?.clone(),
            opened: item(&sheet, "opened")?.clone(),
            closed: item(&sheet, "closed")?.clone(),
            opening: get(&sheet, "opening")?.clone(),
            declared: None,
        });
    }
    let mut keyed = Vec::with_capacity(pairs.len());
    for pair in pairs {
        let key = (pair.opened.clone(), natural_key(&pair)?);
        keyed.push((key, pair));
    }
    for window in keyed.windows(2) {
        pyvalue::compare(&window[0].0.0, &window[1].0.0)?;
    }
    keyed.sort_by(|left, right| compare_keys(&left.0, &right.0));
    let pairs: Vec<Pair> = keyed.into_iter().map(|(_, pair)| pair).collect();
    let mut rounds: Vec<(Value, Vec<usize>)> = Vec::new();
    for (position, pair) in pairs.iter().enumerate() {
        pyvalue::hashable(&pair.round)?;
        let at = match rounds.iter().position(|(name, _)| name.py_eq(&pair.round)) {
            Some(at) => at,
            None => {
                rounds.push((pair.round.clone(), Vec::new()));
                rounds.len() - 1
            }
        };
        let group = &mut rounds[at].1;
        if let Some(first) = group.first()
            && parent(&pairs[*first].path) != parent(&pair.path)
        {
            return value_error(format!("duplicate round name: {}", pair.round.py_str()));
        }
        group.push(position);
    }
    let mut walk = Walk {
        repo,
        pairs: &pairs,
        rounds: &rounds,
        ordered: Vec::new(),
        visiting: Vec::new(),
        finished: Vec::new(),
    };
    for (name, _) in &rounds {
        walk.visit(name)?;
    }
    let ordered = walk.ordered;
    let mut pairs: Vec<Pair> = ordered
        .into_iter()
        .map(|position| pairs[position].clone())
        .collect();
    for position in 0..pairs.len() {
        if !pairs[position].after.truthy() {
            continue;
        }
        let pair = &pairs[position];
        let problems = delegation::settlement_faults(
            repo,
            &pair.before,
            &pair.after,
            Some(&repo.join(&pair.path)),
            Some(&pairs[..=position]),
        )?;
        if !problems.is_empty() {
            return value_error(format!("{}: {}", pair.path, problems.join("; ")));
        }
    }
    declared_answers(repo, &mut pairs)?;
    Ok(pairs)
}

fn parent_bytes(path: &[u8]) -> &[u8] {
    match path.iter().rposition(|byte| *byte == b'/') {
        Some(0) => b"/",
        Some(position) => &path[..position],
        None => b".",
    }
}

fn replace_all(text: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        if text[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(text[at]);
            at += 1;
        }
    }
    out
}

/// The walk that orders rounds after the closed rounds their opening pins.
struct Walk<'a> {
    repo: &'a Repo,
    pairs: &'a [Pair],
    rounds: &'a [(Value, Vec<usize>)],
    ordered: Vec<usize>,
    visiting: Vec<Value>,
    finished: Vec<Value>,
}

impl Walk<'_> {
    fn visit(&mut self, name: &Value) -> Py<()> {
        pyvalue::hashable(name)?;
        if self.finished.iter().any(|done| done.py_eq(name)) {
            return Ok(());
        }
        let Some((_, group)) = self.rounds.iter().find(|(known, _)| known.py_eq(name)) else {
            return Ok(());
        };
        if self.visiting.iter().any(|open| open.py_eq(name)) {
            return value_error("round openings contain a cycle".into());
        }
        self.visiting.push(name.clone());
        let pin = &self.pairs[group[0]].opening;
        if pin.truthy() {
            if !record::pinned(self.repo, pin)? {
                return value_error("round opening pin changed".into());
            }
            let opening = self.repo.load(&self.repo.path(item(pin, "path")?)?)?;
            for prior in pyvalue::iterate(item(&opening, "closed_rounds")?)? {
                if !record::pinned(self.repo, &prior)? {
                    return value_error("closed round pin changed".into());
                }
                let sheet = self.repo.load(&self.repo.path(item(&prior, "path")?)?)?;
                self.visit(item(&sheet, "round")?)?;
            }
        }
        let mut members: Vec<(Vec<Natural>, usize)> = Vec::new();
        for position in group {
            members.push((natural_key(&self.pairs[*position])?, *position));
        }
        members.sort_by(|left, right| pyvalue::natural_order(&left.0, &right.0));
        self.ordered
            .extend(members.into_iter().map(|(_, position)| position));
        self.visiting.retain(|open| !open.py_eq(name));
        self.finished.push(name.clone());
        Ok(())
    }
}

fn supplement_error<T>(supplement: &PyPath, what: &str) -> Py<T> {
    value_error(format!("{}: {what}", text_path(supplement)))
}

fn declared_answers(repo: &Repo, pairs: &mut [Pair]) -> Py<()> {
    for supplement in rglob(&repo.join("docs/research"), "declared-answers.v1.json")? {
        let saved = repo.load(&supplement)?;
        let valid = matches!(saved, Value::Object(_))
            && get(&saved, "schema_version")?.py_eq(&Value::Int("1".into()))
            && matches!(get(&saved, "records")?, Value::Array(_));
        if !valid {
            return supplement_error(&supplement, "invalid declared-answer supplement");
        }
        for record_item in pyvalue::iterate(item(&saved, "records")?)? {
            let valid = matches!(record_item, Value::Object(_))
                && matches!(get(&record_item, "before")?, Value::Object(_))
                && matches!(get(&record_item, "answers")?, Value::Array(_));
            if !valid {
                return supplement_error(&supplement, "invalid declared-answer record");
            }
            let empty = Value::Object(Object::default());
            let pin = pyvalue::get_or(&record_item, "before", &empty)?;
            let named = get(pin, "path")?;
            pyvalue::hashable(named)?;
            let position = pairs
                .iter()
                .position(|pair| pyvalue::text(&pair.path).py_eq(named));
            let usable = match position {
                None => false,
                Some(position) => {
                    get(pin, "sha256")?.py_eq(&pyvalue::text(&pairs[position].hash))
                        && pairs[position].declared.is_none()
                }
            };
            let Some(position) = position.filter(|_| usable) else {
                return supplement_error(&supplement, "missing, changed or repeated before-record");
            };
            let mut answers: Vec<(Value, Value)> = Vec::new();
            for answer in pyvalue::iterate(item(&pairs[position].before, "answers")?)? {
                let by = item(&answer, "by")?.clone();
                pyvalue::hashable(&by)?;
                let copy = match &answer {
                    Value::Object(_) => answer.clone(),
                    other => {
                        return Err(Raised::Type(format!(
                            "'{}' object is not iterable",
                            other.type_name()
                        )));
                    }
                };
                match answers.iter_mut().find(|(key, _)| key.py_eq(&by)) {
                    Some(slot) => slot.1 = copy,
                    None => answers.push((by, copy)),
                }
            }
            let mut seen: Vec<Value> = Vec::new();
            for declaration in pyvalue::iterate(item(&record_item, "answers")?)? {
                let mut complete = matches!(declaration, Value::Object(_));
                if complete {
                    for field in ["by", "source", "case"] {
                        if !pyvalue::has(&declaration, field)? {
                            complete = false;
                            break;
                        }
                    }
                }
                if !complete {
                    return supplement_error(&supplement, "invalid declared-answer source");
                }
                let by = item(&declaration, "by")?;
                let source = item(&declaration, "source")?;
                pyvalue::hashable(by)?;
                let refused = !answers.iter().any(|(key, _)| key.py_eq(by))
                    || seen.iter().any(|done| done.py_eq(by))
                    || !record::pinned(repo, source)?
                    || !record::kept(repo, source)?;
                if refused {
                    return supplement_error(
                        &supplement,
                        "missing, changed or repeated answer source",
                    );
                }
                seen.push(by.clone());
                let original = repo.load(&repo.path(item(source, "path")?)?)?;
                let listed = match &original {
                    Value::Array(answers) => answers
                        .iter()
                        .all(|answer| matches!(answer, Value::Object(_))),
                    _ => false,
                };
                if !listed {
                    return supplement_error(
                        &supplement,
                        "declared-answer source must be a list of answers",
                    );
                }
                let case = item(&declaration, "case")?;
                let matches: Vec<&Value> = original
                    .as_array()
                    .unwrap_or_default()
                    .iter()
                    .filter(|answer| get(answer, "case").is_ok_and(|named| named.py_eq(case)))
                    .collect();
                if matches.len() != 1 {
                    return supplement_error(
                        &supplement,
                        "declared answer must select exactly one case",
                    );
                }
                let confidence = get(matches[0], "confidence")?.clone();
                let settle = get(matches[0], "would_settle")?.clone();
                let high = pyvalue::is(&confidence, "high");
                let claimed = pyvalue::one_of(&confidence, &record::LEVELS)
                    && matches!(settle, Value::Bool(flag) if flag == high);
                if !claimed {
                    return supplement_error(
                        &supplement,
                        "invalid declared confidence or settlement claim",
                    );
                }
                if let Some((_, Value::Object(answer))) =
                    answers.iter_mut().find(|(key, _)| key.py_eq(by))
                {
                    answer.insert("confidence".into(), confidence);
                    answer.insert("would_settle".into(), settle);
                }
            }
            pairs[position].declared =
                Some(answers.into_iter().map(|(_, answer)| answer).collect());
        }
    }
    Ok(())
}

pub fn index(repo: &Repo, before: &Value) -> Py<Value> {
    Ok(record::tables(repo, before)?.1)
}

/// The ruling a record decides, by its name in the index.
pub fn ruling(repo: &Repo, before: &Value) -> Py<Value> {
    let named = item(before, "ruling")?;
    let canonical = record::canonical(named, &index(repo, before)?)?;
    Ok(if canonical.truthy() {
        canonical
    } else {
        item(before, "ruling")?.clone()
    })
}

/// The later records that review this pair's ruling, by position.
fn successors(repo: &Repo, at: usize, pairs: &[Pair]) -> Py<Vec<usize>> {
    let pair = &pairs[at];
    let name = ruling(repo, &pair.before)?;
    let mut found = Vec::new();
    for (position, later) in pairs.iter().enumerate() {
        let before = &later.before;
        let reviewed = get(before, "reviews")?;
        if position == at || !reviewed.truthy() || !later.after.truthy() || position <= at {
            continue;
        }
        let same_type = item(before, "decision_type")?.py_eq(item(&pair.before, "decision_type")?);
        if !same_type {
            continue;
        }
        let named = item(reviewed, "ruling")?;
        if !record::canonical(named, &index(repo, before)?)?.py_eq(&name) {
            continue;
        }
        let group = item(before, "group")?;
        let reviewed_group = item(&pair.before, "group")?;
        let same_group = group.py_eq(reviewed_group) || {
            let Value::Str(text) = reviewed_group else {
                return Err(Raised::Type(format!(
                    "expected string or bytes-like object, got '{}'",
                    reviewed_group.type_name()
                )));
            };
            pyvalue::split_separators(text)
                .into_iter()
                .any(|part| pyvalue::text(&part).py_eq(group))
        };
        if same_group {
            found.push(position);
        }
    }
    Ok(found)
}

/// The decisions that stand for a pair: its own, or those of the reviews that replaced it.
pub fn final_decisions(repo: &Repo, at: usize, pairs: &[Pair]) -> Py<Vec<Value>> {
    let later = successors(repo, at, pairs)?;
    if later.is_empty() {
        return Ok(if pairs[at].after.truthy() {
            vec![pairs[at].after.clone()]
        } else {
            Vec::new()
        });
    }
    let mut decisions = Vec::new();
    for child in later {
        decisions.extend(final_decisions(repo, child, pairs)?);
    }
    Ok(decisions)
}

/// `outcome(answer)`: the outcome, the problem it names and its sorted facts.
struct Outcome {
    outcome: Value,
    same: Value,
    facts: Vec<(String, Value)>,
}

impl Outcome {
    fn eq(&self, other: &Self) -> bool {
        self.outcome.py_eq(&other.outcome)
            && self.same.py_eq(&other.same)
            && self.facts.len() == other.facts.len()
            && self
                .facts
                .iter()
                .zip(&other.facts)
                .all(|(left, right)| left.0 == right.0 && left.1.py_eq(&right.1))
    }
}

fn outcome(answer: &Value) -> Py<Outcome> {
    let picked = item(answer, "outcome")?.clone();
    let same = if pyvalue::one_of(item(answer, "outcome")?, &record::NAMES_A_PROBLEM) {
        get(answer, "same_fault_as")?.clone()
    } else {
        Value::Null
    };
    let empty = Value::Object(Object::default());
    let facts = match pyvalue::get_or(answer, "facts", &empty)? {
        Value::Object(facts) => {
            let mut items: Vec<(String, Value)> = facts
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            items.sort_by(|left, right| left.0.cmp(&right.0));
            items
        }
        other => {
            return Err(Raised::Attribute(format!(
                "'{}' object has no attribute 'items'",
                other.type_name()
            )));
        }
    };
    Ok(Outcome {
        outcome: picked,
        same,
        facts,
    })
}

/// Whether an answer agrees with every decision that stands.
pub fn agrees(answer: &Value, decisions: &[Value]) -> Py<bool> {
    if decisions.is_empty() {
        return Ok(false);
    }
    for decision in decisions {
        let agreed = item(answer, "outcome")?.py_eq(item(decision, "outcome")?)
            && (!get(answer, "same_fault_as")?.truthy()
                || !get(decision, "same_fault_as")?.truthy()
                || item(answer, "same_fault_as")?.py_eq(item(decision, "same_fault_as")?))
            && (!get(answer, "facts")?.truthy()
                || !get(decision, "facts")?.truthy()
                || item(answer, "facts")?.py_eq(item(decision, "facts")?));
        if !agreed {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Whether a blind answer reached its outcome without a clause written from the ruling it decides.
fn independent(repo: &Repo, answer: &Value, before: &Value) -> Py<bool> {
    let (clauses, rulings) = record::tables(repo, before)?;
    let name = if get(before, "reviews")?.truthy() {
        record::canonical(item(item(before, "reviews")?, "ruling")?, &rulings)?
    } else {
        ruling(repo, before)?
    };
    if !item(answer, "blind")?.truthy() || !clauses.truthy() {
        return Ok(true);
    }
    for clause in pyvalue::iterate(item(answer, "clauses")?)? {
        if pyvalue::contains(item(pyvalue::subscript(&clauses, &clause)?, "from")?, &name)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn blind_answers(answers: &[Value]) -> Py<Vec<Value>> {
    let mut blind = Vec::new();
    for answer in answers {
        if item(answer, "blind")?.truthy() {
            blind.push(answer.clone());
        }
    }
    Ok(blind)
}

/// Two independent blind answers at high confidence that would settle, and a medium-or-high recommender, all
/// with one outcome.
pub fn consensus(repo: &Repo, before: &Value) -> Py<bool> {
    let blind = blind_answers(&pyvalue::iterate(item(before, "answers")?)?)?;
    let (_, first) = record::recommender(before)?;
    if blind.len() != 2 {
        return Ok(false);
    }
    for answer in &blind {
        let high = pyvalue::is(item(answer, "confidence")?, "high")
            && item(answer, "would_settle")?.truthy()
            && independent(repo, answer, before)?;
        if !high {
            return Ok(false);
        }
    }
    Ok(outcome(&blind[0])?.eq(&outcome(&blind[1])?)
        && outcome(&first)?.eq(&outcome(&blind[0])?)
        && pyvalue::one_of(item(&first, "confidence")?, &["medium", "high"]))
}

/// A contract 2 record whose first answers were all written before the question.
pub fn eligible(before: &Value) -> Py<bool> {
    if !get(before, "contract")?.py_eq(&Value::Int("2".into()))
        || item(before, "reconstructed")?.truthy()
    {
        return Ok(false);
    }
    for answer in pyvalue::iterate(item(before, "answers")?)? {
        if !pyvalue::is(get(&answer, "written")?, "before-question") {
            return Ok(false);
        }
    }
    Ok(true)
}

fn level(count: u64, targets: usize) -> u64 {
    if count >= 30 && targets >= 5 {
        2
    } else if count >= 15 && targets >= 4 {
        1
    } else {
        0
    }
}

/// `earned()`: the decisions of a type counted toward delegation since the last reset, and the resets.
pub struct Earned {
    pub count: u64,
    pub pull_requests: Vec<Value>,
    pub level: u64,
    pub resets: Vec<(String, &'static str)>,
}

impl Earned {
    pub fn to_value(&self) -> Value {
        let resets = self
            .resets
            .iter()
            .map(|(path, reason)| {
                Value::Object(
                    [
                        ("path".to_string(), pyvalue::text(path)),
                        ("reason".to_string(), pyvalue::text(reason)),
                    ]
                    .into_iter()
                    .collect(),
                )
            })
            .collect();
        Value::Object(
            [
                ("count".to_string(), Value::Int(self.count.to_string())),
                (
                    "pull_requests".to_string(),
                    Value::Array(self.pull_requests.clone()),
                ),
                ("level".to_string(), Value::Int(self.level.to_string())),
                ("resets".to_string(), Value::Array(resets)),
            ]
            .into_iter()
            .collect(),
        )
    }
}

/// `earned(pairs, decision_type, root, before_round, closed_rounds)`.
pub fn earned(
    repo: &Repo,
    pairs: &[Pair],
    kind: &Value,
    before_round: Option<&Value>,
    closed_rounds: Option<&[Value]>,
) -> Py<Earned> {
    let mut count = 0;
    let mut targets: Vec<Value> = Vec::new();
    let mut models: Option<(Value, Vec<Value>)> = None;
    let mut resets = Vec::new();
    let mut watched: Vec<usize> = Vec::new();
    let before_round = before_round.filter(|round| round.truthy());
    let cutoff =
        before_round.and_then(|round| pairs.iter().position(|pair| pair.round.py_eq(round)));
    for (position, pair) in pairs.iter().enumerate() {
        let before = &pair.before;
        if !item(before, "decision_type")?.py_eq(kind) {
            continue;
        }
        if before_round.is_some() {
            let Some(cutoff) = cutoff else {
                return Err(Raised::Type(
                    "'>=' not supported between instances of 'int' and 'NoneType'".into(),
                ));
            };
            if position >= cutoff {
                continue;
            }
        }
        let mut changed = Vec::new();
        for prior in &watched {
            let decisions = final_decisions(repo, *prior, &pairs[..=position])?;
            if !agrees(&pairs[*prior].after, &decisions)? {
                changed.push(*prior);
            }
        }
        if !changed.is_empty() {
            count = 0;
            targets.clear();
            watched.retain(|prior| !changed.contains(prior));
            resets.push((pair.path.clone(), "counted or settled decision changed"));
        }
        if !eligible(before)? {
            continue;
        }
        let setup = delegation::setup(before)?;
        if let Some(previous) = &models
            && !delegation::same_setup(&setup, previous)
        {
            count = 0;
            targets.clear();
            resets.push((pair.path.clone(), "model changed"));
        }
        models = Some(setup);
        if !pair.after.truthy() {
            continue;
        }
        if pyvalue::is(get(&pair.after, "settled_by")?, "agents") {
            watched.push(position);
        }
        if consensus(repo, before)? {
            let (_, first) = record::recommender(before)?;
            if agrees(&first, std::slice::from_ref(&pair.after))? {
                let by_owner = pyvalue::is(
                    pyvalue::get_or(&pair.after, "settled_by", &pyvalue::text("owner"))?,
                    "owner",
                );
                let counted = by_owner
                    && (before_round.is_none()
                        || match closed_rounds {
                            Some(closed) => {
                                pyvalue::hashable(&pair.round)?;
                                closed.iter().any(|round| round.py_eq(&pair.round))
                            }
                            None => pair.closed.truthy(),
                        });
                if counted {
                    count += 1;
                    let target = item(before, "target")?;
                    pyvalue::hashable(target)?;
                    if !targets.iter().any(|known| known.py_eq(target)) {
                        targets.push(target.clone());
                    }
                    watched.push(position);
                }
            } else {
                count = 0;
                targets.clear();
                resets.push((pair.path.clone(), "agreed answer was not the decision"));
            }
        }
    }
    let level = level(count, targets.len());
    Ok(Earned {
        count,
        pull_requests: pyvalue::sorted(targets)?,
        level,
        resets,
    })
}

/// A count keyed by any value, in first-counted order.
#[derive(Default)]
struct Counter(Vec<(Value, u64)>);

impl Counter {
    fn add(&mut self, key: Value) -> Py<()> {
        pyvalue::hashable(&key)?;
        match self.0.iter_mut().find(|(known, _)| known.py_eq(&key)) {
            Some(slot) => slot.1 += 1,
            None => self.0.push((key, 1)),
        }
        Ok(())
    }

    /// `dict(sorted(counter.items()))`.
    fn sorted(self) -> Py<Vec<(Value, u64)>> {
        let keys = pyvalue::sorted(self.0.iter().map(|(key, _)| key.clone()).collect())?;
        Ok(keys
            .into_iter()
            .filter_map(|key| self.0.iter().find(|(known, _)| known.py_eq(&key)).cloned())
            .collect())
    }

    /// `dict(counter.most_common())`: by count, ties in first-counted order.
    fn most_common(mut self) -> Vec<(Value, u64)> {
        self.0.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        self.0
    }
}

#[derive(Clone, Copy, Default)]
struct Counts {
    answers: u64,
    matches: u64,
    excluded: u64,
}

/// One decision type's summary.
pub struct Entry {
    decisions: usize,
    before_question: usize,
    pull_requests: Vec<Value>,
    parties: Vec<(Value, Vec<(Value, Counts)>)>,
    blind_high: u64,
    unanimous: u64,
    ask_reasons: Vec<(Value, u64)>,
    misses: Vec<(Value, u64)>,
    lessons: Vec<Value>,
    digest: Vec<Value>,
    contract_2: Option<Trust>,
}

struct Trust {
    earned: Earned,
    round_level: Option<u64>,
    effective_round_level: Option<u64>,
    audit_calls: Option<Vec<String>>,
}

pub struct Summary {
    adopted: bool,
    round: Option<String>,
    sections: [Vec<(&'static str, Entry)>; 2],
    citations: Vec<(Value, u64)>,
    pending: Vec<String>,
    reservations: Vec<Value>,
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

/// `summarize(pairs, root, round_name)`.
pub fn summarize(repo: &Repo, pairs: &[Pair], round_name: Option<&str>) -> Py<Summary> {
    let named = round_name.filter(|name| !name.is_empty());
    if let Some(name) = named
        && !pairs.iter().any(|pair| pyvalue::is(&pair.round, name))
    {
        return value_error(format!("unknown round: {name}"));
    }
    let selected: Vec<usize> = (0..pairs.len())
        .filter(|position| named.is_none_or(|name| pyvalue::is(&pairs[*position].round, name)))
        .collect();
    let adopted = delegation::policy(repo, &repo.join(delegation::POLICY))?.is_some();
    let pending = selected
        .iter()
        .filter(|position| !pairs[**position].after.truthy())
        .map(|position| pairs[*position].path.clone())
        .collect();
    let mut reservations = Vec::new();
    for position in &selected {
        let pair = &pairs[*position];
        for answer in pyvalue::iterate(item(&pair.before, "answers")?)? {
            let reserving = item(&answer, "blind")?.truthy()
                && pyvalue::is(item(&answer, "outcome")?, "cannot-tell")
                && item(&answer, "clauses")?.truthy()
                && pyvalue::distinct(&pyvalue::iterate(item(&answer, "clauses")?)?)?
                    .iter()
                    .all(|clause| pyvalue::one_of(clause, &RESERVING_CLAUSES));
            if reserving {
                reservations.push(object(vec![
                    ("path", pyvalue::text(&pair.path)),
                    ("ruling", item(&pair.before, "ruling")?.clone()),
                    ("group", item(&pair.before, "group")?.clone()),
                    ("by", item(&answer, "by")?.clone()),
                    ("clauses", item(&answer, "clauses")?.clone()),
                ]));
            }
        }
    }
    let mut citations = Counter::default();
    for position in &selected {
        let before = &pairs[*position].before;
        let rulings = index(repo, before)?;
        for answer in pyvalue::iterate(item(before, "answers")?)? {
            for name in pyvalue::iterate(item(&answer, "nearest")?)? {
                let canonical = record::canonical(&name, &rulings)?;
                if pyvalue::is(
                    item(pyvalue::subscript(&rulings, &canonical)?, "reading")?,
                    "earlier",
                ) {
                    citations.add(record::canonical(&name, &rulings)?)?;
                }
            }
        }
    }
    let citations = citations.most_common();
    let mut sections: [Vec<(&'static str, Entry)>; 2] = [Vec::new(), Vec::new()];
    for (section, contract) in [(0, 1), (1, 2)] {
        for (kind, _, _) in record::DECISION_TYPES {
            if let Some(entry) = entry(repo, pairs, &selected, contract, kind, adopted, named)? {
                sections[section].push((kind, entry));
            }
        }
    }
    Ok(Summary {
        adopted,
        round: round_name.map(str::to_string),
        sections,
        citations,
        pending,
        reservations,
    })
}

fn entry(
    repo: &Repo,
    pairs: &[Pair],
    selected: &[usize],
    contract: i64,
    kind: &str,
    adopted: bool,
    round_name: Option<&str>,
) -> Py<Option<Entry>> {
    let kind_value = pyvalue::text(kind);
    let mut selected_pairs = Vec::new();
    for position in selected {
        let before = &pairs[*position].before;
        if pyvalue::get_or(before, "contract", &int(1))?.py_eq(&int(contract))
            && pyvalue::is(item(before, "decision_type")?, kind)
        {
            selected_pairs.push(*position);
        }
    }
    let group: Vec<usize> = selected_pairs
        .iter()
        .copied()
        .filter(|position| pairs[*position].after.truthy())
        .collect();
    if selected_pairs.is_empty() {
        return Ok(None);
    }
    let mut parties: Vec<(Value, Vec<(Value, Counts)>)> = Vec::new();
    let (mut asks, mut misses) = (Counter::default(), Counter::default());
    let (mut lessons, mut settled) = (Vec::new(), Vec::new());
    let (mut blind_high, mut unanimous) = (0, 0);
    let mut previous_fixes: Vec<(Value, String)> = Vec::new();
    let mut recurrence: Vec<(String, String)> = Vec::new();
    for earlier in pairs {
        if !pyvalue::is(item(&earlier.before, "decision_type")?, kind) || !earlier.after.truthy() {
            continue;
        }
        let saved = &earlier.after;
        if get(saved, "miss")?.truthy() {
            let cause = item(item(saved, "miss")?, "cause")?;
            pyvalue::hashable(cause)?;
            if let Some((_, fixed)) = previous_fixes.iter().find(|(known, _)| known.py_eq(cause)) {
                recurrence.retain(|(path, _)| *path != earlier.path);
                recurrence.push((earlier.path.clone(), fixed.clone()));
            }
        }
        if get(saved, "lesson")?.truthy() {
            let cause = item(item(saved, "miss")?, "cause")?.clone();
            pyvalue::hashable(&cause)?;
            match previous_fixes
                .iter_mut()
                .find(|(known, _)| known.py_eq(&cause))
            {
                Some(slot) => slot.1 = earlier.path.clone(),
                None => previous_fixes.push((cause, earlier.path.clone())),
            }
        }
    }
    for position in &group {
        let pair = &pairs[*position];
        let (before, after) = (&pair.before, &pair.after);
        let decisions = final_decisions(repo, *position, pairs)?;
        let answers = item(before, "answers")?;
        let calibration = match &pair.declared {
            Some(declared) => declared.clone(),
            None => pyvalue::iterate(answers)?,
        };
        for answer in &calibration {
            let by = item(answer, "by")?.clone();
            pyvalue::hashable(&by)?;
            let confidence = item(answer, "confidence")?;
            let confidence = if confidence.truthy() {
                confidence.clone()
            } else {
                pyvalue::text("not recorded")
            };
            pyvalue::hashable(&confidence)?;
            let at = match parties.iter().position(|(known, _)| known.py_eq(&by)) {
                Some(at) => at,
                None => {
                    parties.push((by, Vec::new()));
                    parties.len() - 1
                }
            };
            let rows = &mut parties[at].1;
            let row = match rows.iter().position(|(known, _)| known.py_eq(&confidence)) {
                Some(row) => row,
                None => {
                    rows.push((confidence, Counts::default()));
                    rows.len() - 1
                }
            };
            parties[at].1[row].1.answers += 1;
            if !independent(repo, answer, before)? {
                parties[at].1[row].1.excluded += 1;
            } else if agrees(answer, &decisions)? {
                parties[at].1[row].1.matches += 1;
            }
        }
        let blind = blind_answers(&calibration)?;
        let mut high = blind.len() == 2;
        if high {
            for answer in &blind {
                if !(pyvalue::is(item(answer, "confidence")?, "high")
                    && item(answer, "would_settle")?.truthy()
                    && independent(repo, answer, before)?)
                {
                    high = false;
                    break;
                }
            }
        }
        let high = high && outcome(&blind[0])?.eq(&outcome(&blind[1])?);
        if high && agrees(&blind[0], &decisions)? {
            blind_high += 1;
        }
        if high && agrees(&blind[0], &decisions)? {
            let (_, first) = record::recommender(before)?;
            if agrees(&first, &decisions)? {
                unanimous += 1;
            }
        }
        if item(after, "asked")?.truthy() {
            let routed = delegation::route(
                repo,
                before,
                &repo.join(delegation::POLICY),
                Some(&pairs[..=*position]),
                Some(&repo.join(&pair.path)),
            )?;
            for reason in routed.reasons {
                asks.add(pyvalue::text(&reason))?;
            }
        }
        let miss = get(after, "miss")?;
        if miss.truthy() {
            misses.add(item(miss, "cause")?.clone())?;
            if let Some((_, returned)) = recurrence.iter().find(|(path, _)| *path == pair.path) {
                lessons.push(object(vec![
                    ("path", pyvalue::text(&pair.path)),
                    ("cause_returned_after", pyvalue::text(returned)),
                ]));
            }
        }
        if get(after, "lesson")?.truthy() {
            let Value::Object(lesson) = item(after, "lesson")? else {
                return Err(Raised::Type(format!(
                    "'{}' object is not a mapping",
                    item(after, "lesson")?.type_name()
                )));
            };
            let mut merged = Object::default();
            merged.insert("path".into(), pyvalue::text(&pair.path));
            for (key, value) in lesson.iter() {
                merged.insert(key.clone(), value.clone());
            }
            lessons.push(Value::Object(merged));
        }
        if pyvalue::is(get(after, "settled_by")?, "agents") {
            let mut outcomes = Vec::new();
            for decision in &decisions {
                outcomes.push(item(decision, "outcome")?.clone());
            }
            settled.push(object(vec![
                ("path", pyvalue::text(&pair.path)),
                ("ruling", item(before, "ruling")?.clone()),
                ("group", item(before, "group")?.clone()),
                ("outcome", item(after, "outcome")?.clone()),
                ("final_outcomes", Value::Array(outcomes)),
                ("hash", pyvalue::text(&pair.hash)),
            ]));
        }
    }
    let mut before_question = 0;
    let mut targets = Vec::new();
    for position in &group {
        if !item(&pairs[*position].before, "reconstructed")?.truthy() {
            before_question += 1;
        }
    }
    for position in &group {
        let target = item(&pairs[*position].before, "target")?;
        pyvalue::hashable(target)?;
        if !targets.iter().any(|known: &Value| known.py_eq(target)) {
            targets.push(target.clone());
        }
    }
    let pull_requests = pyvalue::sorted(targets)?;
    let ask_reasons = asks.sorted()?;
    let misses = misses.sorted()?;
    let mut trust = None;
    if contract == 2 {
        let earned_now = earned(repo, pairs, &kind_value, None, None)?;
        let opening_pair = selected.iter().copied().find(|position| {
            pyvalue::is(
                item(&pairs[*position].before, "decision_type").unwrap_or(&Value::Null),
                kind,
            )
        });
        let closed = match (adopted, round_name, opening_pair) {
            (true, Some(_), Some(opening)) if pairs[opening].opening.truthy() => {
                Some(delegation::opening_rounds(repo, opening, pairs)?)
            }
            _ => None,
        };
        let round_level = match round_name {
            Some(name) => Some(
                earned(
                    repo,
                    pairs,
                    &kind_value,
                    Some(&pyvalue::text(name)),
                    closed.as_deref(),
                )?
                .level,
            ),
            None => None,
        };
        let selected_paths: Vec<&str> = selected_pairs
            .iter()
            .map(|position| pairs[*position].path.as_str())
            .collect();
        let reset_in_round = earned_now
            .resets
            .iter()
            .any(|(path, _)| selected_paths.contains(&path.as_str()));
        let effective_round_level = if reset_in_round { Some(0) } else { round_level };
        let mut order: Vec<usize> = (0..settled.len()).collect();
        order.sort_by(|left, right| {
            item(&settled[*left], "hash")
                .ok()
                .and_then(Value::as_str)
                .cmp(&item(&settled[*right], "hash").ok().and_then(Value::as_str))
        });
        for (number, at) in order.into_iter().enumerate() {
            if let Value::Object(members) = &mut settled[at] {
                members.insert(
                    "sampled".into(),
                    Value::Bool(effective_round_level == Some(2) && number % 5 == 0),
                );
            }
        }
        let audit_calls = (kind == "reconciliation").then(|| {
            let mut ordered = group.clone();
            ordered.sort_by(|left, right| pairs[*left].hash.cmp(&pairs[*right].hash));
            ordered
                .into_iter()
                .take(5)
                .map(|position| pairs[position].path.clone())
                .collect()
        });
        trust = Some(Trust {
            earned: earned_now,
            round_level,
            effective_round_level,
            audit_calls,
        });
    }
    Ok(Some(Entry {
        decisions: group.len(),
        before_question,
        pull_requests,
        parties,
        blind_high,
        unanimous,
        ask_reasons,
        misses,
        lessons,
        digest: settled,
        contract_2: trust,
    }))
}

/// A dict key as `json.dumps` writes it.
fn json_key(key: &Value) -> Py<String> {
    match key {
        Value::Str(text) => Ok(text.clone()),
        Value::Int(digits) => Ok(digits.clone()),
        Value::Float(number) => Ok(crate::pyjson::float_repr(*number)),
        Value::Bool(true) => Ok("true".into()),
        Value::Bool(false) => Ok("false".into()),
        Value::Null => Ok("null".into()),
        other => Err(Raised::Type(format!(
            "keys must be str, int, float, bool or None, not {}",
            other.type_name()
        ))),
    }
}

fn keyed_object(members: impl IntoIterator<Item = (Value, Value)>) -> Py<Value> {
    let mut object = Object::default();
    for (key, value) in members {
        let key = json_key(&key)?;
        if object.get(&key).is_some() {
            return Err(Raised::Unsupported(format!(
                "two keys of one summary object are both written as {}",
                pytext::repr_str(&key)
            )));
        }
        object.insert(key, value);
    }
    Ok(Value::Object(object))
}

fn counted(counts: &[(Value, u64)]) -> Py<Value> {
    keyed_object(counts.iter().map(|(key, count)| (key.clone(), int(count))))
}

impl Entry {
    fn to_value(&self) -> Py<Value> {
        let mut parties = Vec::new();
        for (party, rows) in &self.parties {
            let rows = keyed_object(rows.iter().map(|(confidence, counts)| {
                (
                    confidence.clone(),
                    object(vec![
                        ("answers", int(counts.answers)),
                        ("matches", int(counts.matches)),
                        ("excluded", int(counts.excluded)),
                    ]),
                )
            }))?;
            parties.push((party.clone(), rows));
        }
        let mut members = vec![
            ("decisions", int(self.decisions)),
            ("before_question", int(self.before_question)),
            ("pull_requests", Value::Array(self.pull_requests.clone())),
            ("parties", keyed_object(parties)?),
            ("blind_high_matches", int(self.blind_high)),
            ("unanimous_matches", int(self.unanimous)),
            ("ask_reasons", counted(&self.ask_reasons)?),
            ("misses", counted(&self.misses)?),
            ("lessons", Value::Array(self.lessons.clone())),
            ("digest", Value::Array(self.digest.clone())),
        ];
        if let Some(trust) = &self.contract_2 {
            let level = |value: Option<u64>| value.map_or(Value::Null, int);
            members.push(("earned", trust.earned.to_value()));
            members.push(("round_level", level(trust.round_level)));
            members.push(("effective_round_level", level(trust.effective_round_level)));
            if let Some(calls) = &trust.audit_calls {
                members.push((
                    "audit_calls",
                    Value::Array(calls.iter().map(|path| pyvalue::text(path)).collect()),
                ));
            }
        }
        Ok(object(members))
    }
}

impl Summary {
    fn policy(&self) -> &'static str {
        if self.adopted {
            "policy v2 adopted"
        } else {
            "not adopted; counts and levels grant no authority"
        }
    }

    /// The summary as `--json` prints it.
    pub fn to_value(&self) -> Py<Value> {
        let section = |entries: &[(&'static str, Entry)]| -> Py<Value> {
            let mut object = Object::default();
            for (kind, entry) in entries {
                object.insert((*kind).to_string(), entry.to_value()?);
            }
            Ok(Value::Object(object))
        };
        let baseline = section(&self.sections[0])?;
        let contract_2 = section(&self.sections[1])?;
        Ok(object(vec![
            ("policy", pyvalue::text(self.policy())),
            (
                "round",
                self.round.as_deref().map_or(Value::Null, pyvalue::text),
            ),
            ("baseline", baseline),
            ("contract_2", contract_2),
            ("earlier_reading_citations", counted(&self.citations)?),
            (
                "pending",
                Value::Array(
                    self.pending
                        .iter()
                        .map(|path| pyvalue::text(path))
                        .collect(),
                ),
            ),
            (
                "reservation_only_cannot_tell",
                Value::Array(self.reservations.clone()),
            ),
        ]))
    }

    /// The summary as text.
    pub fn render(&self) -> Py<String> {
        let mut lines = vec![format!("Policy: {}", self.policy()), String::new()];
        for (section, title) in self
            .sections
            .iter()
            .zip(["Contract 1 baseline", "Contract 2"])
        {
            lines.extend([format!("## {title}"), String::new()]);
            for (kind, entry) in section {
                lines.extend([
                    format!("### {kind}"),
                    String::new(),
                    format!(
                        "{} decisions on {} pull requests; {} written before the question.",
                        entry.decisions,
                        entry.pull_requests.len(),
                        entry.before_question
                    ),
                    String::new(),
                    "| Party | Confidence | Matches / answers | Excluded |".into(),
                    "| --- | --- | --- | --- |".into(),
                ]);
                let parties = pyvalue::sorted(
                    entry
                        .parties
                        .iter()
                        .map(|(party, _)| party.clone())
                        .collect(),
                )?;
                for party in parties {
                    let Some((_, rows)) =
                        entry.parties.iter().find(|(known, _)| known.py_eq(&party))
                    else {
                        continue;
                    };
                    let confidences = pyvalue::sorted(
                        rows.iter()
                            .map(|(confidence, _)| confidence.clone())
                            .collect(),
                    )?;
                    for confidence in confidences {
                        if let Some((_, counts)) =
                            rows.iter().find(|(known, _)| known.py_eq(&confidence))
                        {
                            lines.push(format!(
                                "| {} | {} | {} / {} | {} |",
                                party.py_str(),
                                confidence.py_str(),
                                counts.matches,
                                counts.answers,
                                counts.excluded
                            ));
                        }
                    }
                }
                lines.extend([
                    String::new(),
                    format!(
                        "Both blind assessors agreed at high confidence and matched: {}. The recommender also matched: {}.",
                        entry.blind_high, entry.unanimous
                    ),
                ]);
                if let Some(trust) = &entry.contract_2 {
                    let opening = trust.round_level.map_or_else(
                        || "no round selected".to_string(),
                        |level| level.to_string(),
                    );
                    lines.push(format!(
                        "Count since the last miss or model change: {} on {} pull requests; level {}. Round opening level: {opening}.",
                        trust.earned.count,
                        trust.earned.pull_requests.len(),
                        trust.earned.level
                    ));
                    if let Some(level) = trust.effective_round_level {
                        lines.push(format!("Effective round level: {level}."));
                        if level == 0 && !entry.digest.is_empty() {
                            lines
                                .push("Read every settled decision in this round's digest.".into());
                        }
                    }
                }
                lines.extend([String::new(), "Reasons for asking:".into()]);
                lines.extend(
                    entry
                        .ask_reasons
                        .iter()
                        .map(|(reason, count)| format!("- {}: {count}", reason.py_str())),
                );
                lines.extend([String::new(), "Misses by cause:".into()]);
                if entry.misses.is_empty() {
                    lines.push("- none".into());
                }
                lines.extend(
                    entry
                        .misses
                        .iter()
                        .map(|(cause, count)| format!("- {}: {count}", cause.py_str())),
                );
                lines.extend([String::new(), "Lessons:".into()]);
                for lesson in &entry.lessons {
                    let tail = if pyvalue::has(lesson, "cause_returned_after")? {
                        format!(
                            "cause returned after {}",
                            item(lesson, "cause_returned_after")?.py_str()
                        )
                    } else {
                        format!(
                            "{} Changed file: {}",
                            item(lesson, "says")?.py_str(),
                            item(lesson, "goes_to")?.py_str()
                        )
                    };
                    lines.push(format!("- {}: {tail}", item(lesson, "path")?.py_str()));
                }
                lines.extend([String::new(), "Settled decisions:".into()]);
                for settled in &entry.digest {
                    let sample = if get(settled, "sampled")?.truthy() {
                        " [sample]"
                    } else {
                        ""
                    };
                    lines.push(format!(
                        "- {} {}: {}. {}{sample}",
                        item(settled, "ruling")?.py_str(),
                        item(settled, "group")?.py_str(),
                        item(settled, "outcome")?.py_str(),
                        item(settled, "path")?.py_str()
                    ));
                }
                if let Some(calls) = entry
                    .contract_2
                    .as_ref()
                    .and_then(|trust| trust.audit_calls.as_ref())
                {
                    lines.extend([String::new(), "Five audit calls:".into()]);
                    lines.extend(calls.iter().map(|path| format!("- {path}")));
                }
                lines.push(String::new());
            }
        }
        lines.push("Earlier-reading rulings cited:".into());
        lines.extend(
            self.citations
                .iter()
                .map(|(name, count)| format!("- {}: {count}", name.py_str())),
        );
        lines.push(String::new());
        lines.push(format!(
            "Blind cannot-tell answers citing only reserving clauses: {}.",
            self.reservations.len()
        ));
        for reservation in &self.reservations {
            let clauses = pyvalue::join(", ", &pyvalue::iterate(item(reservation, "clauses")?)?)?;
            lines.push(format!(
                "- {} {}, {}: {clauses}. {}",
                item(reservation, "ruling")?.py_str(),
                item(reservation, "group")?.py_str(),
                item(reservation, "by")?.py_str(),
                item(reservation, "path")?.py_str()
            ));
        }
        lines.push(String::new());
        if !self.pending.is_empty() {
            lines.push("Pending decisions:".into());
            lines.extend(self.pending.iter().map(|path| format!("- {path}")));
        }
        Ok(lines.join("\n"))
    }
}

/// `precedents(pairs, decision_type, exclude, root)`: the sheet of current-reading owner rulings of a type, without
/// the excluded ruling and the reviews that descend from it.
pub fn precedents(repo: &Repo, pairs: &[Pair], kind: &str, exclude: &str) -> Py<String> {
    let exclude = pyvalue::text(exclude);
    let mut excluded: Vec<Value> = Vec::new();
    for pair in pairs {
        let canonical = record::canonical(&exclude, &index(repo, &pair.before)?)?;
        let name = if canonical.truthy() {
            canonical
        } else {
            exclude.clone()
        };
        pyvalue::hashable(&name)?;
        if !excluded.iter().any(|known| known.py_eq(&name)) {
            excluded.push(name);
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for pair in pairs {
            let reviewed = get(&pair.before, "reviews")?;
            if !reviewed.truthy() {
                continue;
            }
            let canonical =
                record::canonical(item(reviewed, "ruling")?, &index(repo, &pair.before)?)?;
            let reviewed_name = if canonical.truthy() {
                canonical
            } else {
                item(reviewed, "ruling")?.clone()
            };
            pyvalue::hashable(&reviewed_name)?;
            if !excluded.iter().any(|known| known.py_eq(&reviewed_name)) {
                continue;
            }
            let name = ruling(repo, &pair.before)?;
            pyvalue::hashable(&name)?;
            if !excluded.iter().any(|known| known.py_eq(&name)) {
                excluded.push(name);
                changed = true;
            }
        }
    }
    let mut lines = vec![
        pyvalue::text(&format!("# Earlier rulings: {kind}")),
        pyvalue::text(""),
    ];
    for (position, pair) in pairs.iter().enumerate() {
        let (before, after) = (&pair.before, &pair.after);
        let name = ruling(repo, before)?;
        let rulings = index(repo, before)?;
        let skip = !after.truthy()
            || !pyvalue::is(item(before, "decision_type")?, kind)
            || {
                pyvalue::hashable(&name)?;
                excluded.iter().any(|known| known.py_eq(&name))
            }
            || !pyvalue::contains(&rulings, &name)?
            || !pyvalue::is(
                item(pyvalue::subscript(&rulings, &name)?, "reading")?,
                "current",
            )
            || !successors(repo, position, pairs)?.is_empty();
        if skip {
            continue;
        }
        let mut ruling_path = repo.join(&pair.path.replace(".before.json", ".md"));
        if !pyvalue::is_file(&ruling_path) {
            let entry = pyvalue::subscript(&rulings, &name)?;
            let file = item(pyvalue::subscript(&rulings, &name)?, "file")?;
            let chosen = pyvalue::get_or(entry, "reviewed_in", file)?;
            ruling_path = repo.path(chosen)?;
        }
        let text = pyvalue::read_text(&ruling_path)?;
        let title = pytext::splitlines(&text)
            .into_iter()
            .find(|line| line.starts_with('#'))
            .map_or_else(
                || name.py_str(),
                |line| line.trim_start_matches(['#', ' ']).to_string(),
            );
        lines.extend([
            pyvalue::text(&format!("## {title}")),
            pyvalue::text(""),
            pyvalue::text(&format!(
                "Ruling: {}. Decision: {}.",
                name.py_str(),
                item(before, "group")?.py_str()
            )),
            pyvalue::text(&format!("Outcome: {}.", record::pick(after)?)),
            pyvalue::text(""),
        ]);
        let ground = item(after, "ground")?;
        lines.push(if matches!(ground, Value::Null) {
            pyvalue::text("no reason given")
        } else {
            ground.clone()
        });
        lines.push(pyvalue::text(""));
        if get(after, "lesson")?.truthy() {
            lines.push(item(item(after, "lesson")?, "says")?.clone());
            lines.push(pyvalue::text(""));
        }
    }
    pyvalue::join("\n", &lines)
}

//! List the benchmarks the roster has and the ones still missing.
//!
//! `bench/roster.json` is the hand-edited roster: the clients, models and efforts that benchmarks run
//! against by default. It is an object keyed by client, and each client maps the models it runs to the
//! list of efforts to benchmark them at.
//!
//! Review methods are not listed. A method counts as running on a client once any entry in
//! `bench/scoreboard.current.json` has benchmarked it there, the client being that of the source arm's
//! kind. For every suite, every such method and every listed model on one of the method's clients, one
//! tab-separated line is printed per effort: suite, method, client, model, effort, then the id of the
//! entry whose source arm runs that model at that effort on that client, or `missing` when the suite has
//! none. Nothing is written and no reviewer or grader is started.

use crate::pyjson::{Duplicates, Object, Value, loads};
use crate::pytext::{self, PyPath};

pub const ROSTER: &str = "bench/roster.json";

pub const USAGE: &str = "usage: bench-tools roster [-h] [--root ROOT]";

pub fn client_of_kind(kind: &str) -> Option<&'static str> {
    match kind {
        "claude-builtin" | "claude-skill" | "review-code" => Some("claude-code"),
        "codex" | "codex-skill" => Some("codex"),
        _ => None,
    }
}

/// An input cannot be read, or has a shape the roster cannot use; exit code 2.
#[derive(Debug, PartialEq, Eq)]
pub struct InputError(pub String);

#[derive(Debug, PartialEq, Eq)]
pub enum Report {
    /// Exit code 0: one line per combination.
    Lines(Vec<String>),
    /// Exit code 1: one line per violation.
    Violations(Vec<String>),
}

pub struct Listed {
    pub client: String,
    pub model: String,
    pub effort: String,
}

fn read_json(path: &PyPath, duplicates: Duplicates) -> Result<Value, InputError> {
    let cannot = |detail: &str| InputError(format!("cannot read {}: {detail}", path.display()));
    let text = pytext::read_text(path).map_err(|error| cannot(error.message()))?;
    loads(&text, duplicates).map_err(|error| cannot(&error.to_string()))
}

fn shape(path: &PyPath, detail: &str) -> InputError {
    InputError(format!("{}: {detail}", path.display()))
}

fn object<'a>(value: &'a Value, path: &PyPath, what: &str) -> Result<&'a Object, InputError> {
    value
        .as_object()
        .ok_or_else(|| shape(path, &format!("{what} is not an object")))
}

/// The items a Python `for` loop visits: a list's elements, or nothing for an empty object or text.
/// A non-empty object or text yields keys or characters that no caller here can index, so it is refused.
fn iterate<'a>(value: &'a Value, path: &PyPath, what: &str) -> Result<&'a [Value], InputError> {
    match value {
        Value::Array(values) => Ok(values),
        Value::Object(_) | Value::Str(_) if !value.truthy() => Ok(&[]),
        _ => Err(shape(path, &format!("{what} is not a list"))),
    }
}

fn field<'a>(
    object: &'a Object,
    key: &str,
    path: &PyPath,
    what: &str,
) -> Result<&'a Value, InputError> {
    object
        .get(key)
        .ok_or_else(|| shape(path, &format!("{what} has no {}", pytext::repr_str(key))))
}

/// The listed models, one per effort, and the roster's violations.
pub fn read_roster(root: &PyPath) -> Result<(Vec<Listed>, Vec<String>), InputError> {
    let roster = root.join(ROSTER.as_bytes());
    let listed = read_json(&roster, Duplicates::Refuse)?;
    let Value::Object(listed) = listed else {
        return Ok((
            Vec::new(),
            vec![format!("{ROSTER}: needs an object keyed by client")],
        ));
    };
    let priced = priced_models(root)?;
    let mut models = Vec::new();
    let mut problems = Vec::new();
    for (client, efforts_of) in listed.iter() {
        let harness = root
            .join(b"bench/harness")
            .join(format!("{client}.json").as_bytes());
        if !harness.is_file() {
            problems.push(format!(
                "{ROSTER}: client {client} has no bench/harness/{client}.json"
            ));
        }
        let Value::Object(efforts_of) = efforts_of else {
            problems.push(format!("{ROSTER}: {client} needs an object of models"));
            continue;
        };
        for (model, efforts) in efforts_of.iter() {
            if !priced.iter().any(|priced| priced == model) {
                problems.push(format!(
                    "{ROSTER}: model {model} has no entry in bench/rates.current.json"
                ));
            }
            let named: Option<Vec<&str>> = efforts
                .as_array()
                .filter(|efforts| !efforts.is_empty())
                .and_then(|efforts| {
                    efforts
                        .iter()
                        .map(|effort| effort.as_str().filter(|effort| !effort.is_empty()))
                        .collect()
                });
            let distinct = named.filter(|efforts| {
                efforts
                    .iter()
                    .enumerate()
                    .all(|(position, effort)| !efforts[..position].contains(effort))
            });
            let Some(efforts) = distinct else {
                problems.push(format!(
                    "{ROSTER}: {client} {model} needs a list of distinct efforts"
                ));
                continue;
            };
            models.extend(efforts.into_iter().map(|effort| Listed {
                client: client.clone(),
                model: model.clone(),
                effort: effort.to_string(),
            }));
        }
    }
    if models.is_empty() && problems.is_empty() {
        problems.push(format!("{ROSTER}: lists no models"));
    }
    Ok((models, problems))
}

/// The models `bench/rates.current.json` prices. Only text models can match a roster key.
fn priced_models(root: &PyPath) -> Result<Vec<String>, InputError> {
    let path = root.join(b"bench/rates.current.json");
    let rates = read_json(&path, Duplicates::KeepLast)?;
    let rates = field(
        object(&rates, &path, "the catalog")?,
        "rates",
        &path,
        "the catalog",
    )?;
    let mut priced = Vec::new();
    for rate in iterate(rates, &path, "rates")? {
        let model = field(object(rate, &path, "a rate")?, "model", &path, "a rate")?;
        if !model.hashable() {
            return Err(shape(&path, "a rate's model is not a text or number"));
        }
        if let Value::Str(model) = model {
            priced.push(model.clone());
        }
    }
    Ok(priced)
}

struct Suite {
    /// Read only where Python reads it, so a suite no line or benchmark needs may omit it.
    id: Option<Value>,
    entries: Vec<Value>,
}

impl Suite {
    fn id(&self, path: &PyPath) -> Result<&Value, InputError> {
        self.id
            .as_ref()
            .ok_or_else(|| shape(path, "a suite has no 'id'"))
    }
}

/// The suites with their entries; a `current-cohort-input/v1` registry lists configurations once and
/// places each source in a suite by its tasks.
fn suites(registry: &Value, path: &PyPath) -> Result<Vec<Suite>, InputError> {
    let registry = object(registry, path, "the registry")?;
    let listed = iterate(
        field(registry, "suites", path, "the registry")?,
        path,
        "suites",
    )?;
    let cohort = registry
        .get("contract")
        .is_some_and(|contract| contract.as_str() == Some("current-cohort-input/v1"));
    let mut suites = Vec::new();
    for suite in listed {
        let suite = object(suite, path, "a suite")?;
        let id = suite.get("id").cloned();
        let entries = if cohort {
            cohort_entries(registry, suite, path)?
        } else {
            iterate(field(suite, "entries", path, "a suite")?, path, "entries")?.to_vec()
        };
        suites.push(Suite { id, entries });
    }
    Ok(suites)
}

fn cohort_entries(
    registry: &Object,
    suite: &Object,
    path: &PyPath,
) -> Result<Vec<Value>, InputError> {
    let configurations = iterate(
        field(registry, "configurations", path, "the registry")?,
        path,
        "configurations",
    )?;
    let sources = iterate(
        field(registry, "sources", path, "the registry")?,
        path,
        "sources",
    )?;
    let mut suite_tasks = None;
    let mut entries = Vec::new();
    for wanted in iterate(
        field(suite, "configurations", path, "a suite")?,
        path,
        "a suite's configurations",
    )? {
        let mut found = None;
        for configuration in configurations {
            let configuration = object(configuration, path, "a configuration")?;
            let id = field(configuration, "id", path, "a configuration")?;
            if !id.hashable() || !wanted.hashable() {
                return Err(shape(path, "a configuration id is not a text or number"));
            }
            if id.py_eq(wanted) {
                found = Some(configuration);
            }
        }
        let configuration = found.ok_or_else(|| {
            shape(
                path,
                &format!("suite configuration {} is not listed", wanted.py_repr()),
            )
        })?;
        let mut placed = Vec::new();
        for source in sources {
            let source_object = object(source, path, "a source")?;
            if !field(source_object, "configuration", path, "a source")?.py_eq(wanted) {
                continue;
            }
            let tasks = hashable_items(
                field(source_object, "tasks", path, "a source")?,
                path,
                "a source's tasks",
            )?;
            let suite_tasks = match suite_tasks {
                Some(tasks) => tasks,
                None => *suite_tasks.insert(hashable_items(
                    field(suite, "tasks", path, "a suite")?,
                    path,
                    "a suite's tasks",
                )?),
            };
            if tasks
                .iter()
                .any(|task| suite_tasks.iter().any(|other| task.py_eq(other)))
            {
                placed.push(source.clone());
            }
        }
        let mut entry: Vec<(String, Value)> = configuration
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        entry.push(("sources".to_string(), Value::Array(placed)));
        entries.push(Value::Object(entry.into_iter().collect()));
    }
    Ok(entries)
}

fn hashable_items<'a>(
    value: &'a Value,
    path: &PyPath,
    what: &str,
) -> Result<&'a [Value], InputError> {
    let items = iterate(value, path, what)?;
    if items.iter().all(Value::hashable) {
        Ok(items)
    } else {
        Err(shape(path, &format!("{what} hold a list or object")))
    }
}

struct Benchmark {
    suite: Value,
    method: Value,
    client: &'static str,
    model: Value,
    effort: Value,
    entry: Value,
}

fn text<'a>(value: &'a Value, path: &PyPath, what: &str) -> Result<&'a str, InputError> {
    value
        .as_str()
        .ok_or_else(|| shape(path, &format!("{what} {} is not text", value.py_repr())))
}

/// One line per suite, benchmarked method, and listed model on one of that method's clients.
pub fn status(root: &PyPath, models: &[Listed]) -> Result<Vec<String>, InputError> {
    let registry_path = root.join(b"bench/scoreboard.current.json");
    let registry = read_json(&registry_path, Duplicates::KeepLast)?;
    let suites = suites(&registry, &registry_path)?;
    let mut clients: Vec<(Value, Vec<&'static str>)> = Vec::new();
    let mut benchmarked: Vec<Benchmark> = Vec::new();
    for suite in &suites {
        for entry in &suite.entries {
            let entry = object(entry, &registry_path, "an entry")?;
            let Some(method) = entry.get("method").filter(|method| method.truthy()) else {
                continue;
            };
            if !method.hashable() {
                return Err(shape(
                    &registry_path,
                    "an entry's method is a list or object",
                ));
            }
            for source in iterate(
                field(entry, "sources", &registry_path, "an entry")?,
                &registry_path,
                "sources",
            )? {
                let arm_name = field(
                    object(source, &registry_path, "a source")?,
                    "arm",
                    &registry_path,
                    "a source",
                )?;
                let path = root
                    .join(b"bench/arms")
                    .join(format!("{}.json", arm_name.py_str()).as_bytes());
                let arm = read_json(&path, Duplicates::KeepLast)?;
                let arm = object(&arm, &path, "the arm")?;
                let kind = arm.get("kind").cloned().unwrap_or(Value::Null);
                if !kind.hashable() {
                    return Err(shape(&path, "the arm's kind is a list or object"));
                }
                let Some(client) = kind.as_str().and_then(client_of_kind) else {
                    return Err(InputError(format!(
                        "{}: kind {} has no client in CLIENT_OF_KIND",
                        path.display(),
                        kind.py_repr()
                    )));
                };
                match clients.iter_mut().find(|(known, _)| known.py_eq(method)) {
                    Some((_, runs_on)) if runs_on.contains(&client) => {}
                    Some((_, runs_on)) => runs_on.push(client),
                    None => clients.push((method.clone(), vec![client])),
                }
                let model = arm.get("model").cloned().unwrap_or(Value::Null);
                let effort = arm.get("effort").cloned().unwrap_or(Value::Null);
                let suite_id = suite.id(&registry_path)?;
                let id = field(entry, "id", &registry_path, "an entry")?.clone();
                if !suite_id.hashable() || !model.hashable() || !effort.hashable() {
                    return Err(shape(
                        &path,
                        "a suite id, model or effort is a list or object",
                    ));
                }
                let known = benchmarked.iter().any(|row| {
                    row.suite.py_eq(suite_id)
                        && row.method.py_eq(method)
                        && row.client == client
                        && row.model.py_eq(&model)
                        && row.effort.py_eq(&effort)
                });
                if !known {
                    benchmarked.push(Benchmark {
                        suite: suite_id.clone(),
                        method: method.clone(),
                        client,
                        model,
                        effort,
                        entry: id,
                    });
                }
            }
        }
    }
    let mut lines = Vec::new();
    for suite in &suites {
        for (method, runs_on) in &clients {
            for listed in models {
                if !runs_on.contains(&listed.client.as_str()) {
                    continue;
                }
                let suite_id = suite.id(&registry_path)?;
                let model = Value::Str(listed.model.clone());
                let effort = Value::Str(listed.effort.clone());
                let found = benchmarked.iter().find(|row| {
                    row.suite.py_eq(suite_id)
                        && row.method.py_eq(method)
                        && row.client == listed.client
                        && row.model.py_eq(&model)
                        && row.effort.py_eq(&effort)
                });
                let entry = match found {
                    Some(row) => text(&row.entry, &registry_path, "an entry id")?,
                    None => "missing",
                };
                lines.push(
                    [
                        text(suite_id, &registry_path, "a suite id")?,
                        text(method, &registry_path, "a method")?,
                        &listed.client,
                        &listed.model,
                        &listed.effort,
                        entry,
                    ]
                    .join("\t"),
                );
            }
        }
    }
    Ok(lines)
}

/// The roster's report for the repository at `root`.
pub fn report(root: &PyPath) -> Result<Report, InputError> {
    let (models, problems) = read_roster(root)?;
    if problems.is_empty() {
        Ok(Report::Lines(status(root, &models)?))
    } else {
        Ok(Report::Violations(problems))
    }
}

//! `ruling_dossier.py`: refuse a candidate dossier that does not show where a promise comes from and the
//! searches made to find one. It reads structure only.

use super::Repo;
use crate::pyjson::{Object, Value};
use crate::pytext::PyPath;
use crate::pyvalue::{self, Py, Raised, get, has, item, one_of};

pub const PLACES: [&str; 6] = [
    "project-docs",
    "owner-docs",
    "change",
    "maintainers",
    "public-code",
    "documented-way",
];
const WAYS: [&str; 3] = ["written", "announced", "built"];
const CLASSES: [&str; 6] = [
    "public",
    "private",
    "deprecated",
    "removed",
    "other-purpose",
    "unstated",
];
const FIELDS: [&str; 5] = [
    "made_by",
    "whose_interface",
    "classification",
    "source",
    "searched",
];
const UNSORTED: [&str; 3] = ["refuted", "unproven", "outside-scope"];

/// The candidate entries `entries()` yields before it stops, and what stopped it. Python's generator raises
/// only when a reader reaches that point, so a reader that stops early never sees the error.
pub struct Entries {
    pub items: Vec<Value>,
    pub error: Option<Raised>,
}

impl Entries {
    fn failed(error: Raised) -> Self {
        Self {
            items: Vec::new(),
            error: Some(error),
        }
    }
}

fn refreshed(repo: &Repo, path: &PyPath) -> Py<Vec<(Value, Value)>> {
    let loaded = repo.load(path)?;
    let mut by_group: Vec<(Value, Value)> = Vec::new();
    for entry in pyvalue::iterate(&loaded)? {
        let group = item(&entry, "group")?.clone();
        pyvalue::hashable(&group)?;
        match by_group.iter_mut().find(|(key, _)| key.py_eq(&group)) {
            Some(slot) => slot.1 = entry,
            None => by_group.push((group, entry)),
        }
    }
    Ok(by_group)
}

/// Candidate entries as they are ready to ask: the first summary with any refreshed promise laid over it.
pub fn entries(repo: &Repo, directory: &PyPath) -> Entries {
    let refresh = directory.join(b"refresh.json");
    let refreshed = if pyvalue::exists(&refresh) {
        match refreshed(repo, &refresh) {
            Ok(refreshed) => refreshed,
            Err(error) => return Entries::failed(error),
        }
    } else {
        Vec::new()
    };
    let summary = match repo
        .load(&directory.join(b"summary.json"))
        .and_then(|summary| pyvalue::iterate(&summary))
    {
        Ok(summary) => summary,
        Err(error) => return Entries::failed(error),
    };
    let mut items = Vec::new();
    for entry in summary {
        let merged = (|| -> Py<Option<Value>> {
            if !pyvalue::is(get(&entry, "kind")?, "candidate") {
                return Ok(None);
            }
            let group = item(&entry, "group")?;
            pyvalue::hashable(group)?;
            let mut merged = entry.as_object().cloned().unwrap_or_default();
            if let Some((_, refresh)) = refreshed.iter().find(|(key, _)| key.py_eq(group)) {
                let Value::Object(members) = refresh else {
                    return Err(Raised::Type(format!(
                        "'{}' object is not a mapping",
                        refresh.type_name()
                    )));
                };
                for (key, value) in members.iter() {
                    merged.insert(key.clone(), value.clone());
                }
            }
            Ok(Some(Value::Object(merged)))
        })();
        match merged {
            Ok(Some(merged)) => items.push(merged),
            Ok(None) => {}
            Err(error) => {
                return Entries {
                    items,
                    error: Some(error),
                };
            }
        }
    }
    Entries { items, error: None }
}

/// `(value or {}).get(key)`.
fn get_or_empty<'a>(value: &'a Value, key: &str, empty: &'a Value) -> Py<&'a Value> {
    get(if value.truthy() { value } else { empty }, key)
}

/// The places whose search the entry records as blocked.
pub fn blocked(entry: &Value) -> Py<Vec<&'static str>> {
    let empty = Value::Object(Object::default());
    let searched = get_or_empty(get(entry, "promise")?, "searched", &empty)?;
    let mut places = Vec::new();
    for place in PLACES {
        if matches!(searched, Value::Object(_))
            && pyvalue::is(
                get_or_empty(get(searched, place)?, "state", &empty)?,
                "blocked",
            )
        {
            places.push(place);
        }
    }
    Ok(places)
}

fn search_faults(directory: &PyPath, group: &str, place: &str, search: &Value) -> Py<Vec<String>> {
    let state = match search {
        Value::Object(_) => get(search, "state")?,
        _ => &pyvalue::NONE,
    };
    if one_of(state, &["not-applicable", "blocked"]) {
        return Ok(if get(search, "reason")?.truthy() {
            Vec::new()
        } else {
            vec![format!(
                "{group}: searched.{place} is {} and needs the reason",
                state.py_str()
            )]
        });
    }
    if !pyvalue::is(state, "checked") {
        return Ok(vec![format!(
            "{group}: searched.{place} needs `state`: checked, not-applicable or blocked"
        )]);
    }
    let mut found = Vec::new();
    if !get(search, "query")?.truthy() {
        found.push(format!(
            "{group}: searched.{place} needs the query as it was run"
        ));
    }
    let saved = get(search, "saved")?;
    let unsaved = !saved.truthy() || {
        let path = pyvalue::join_path(directory, saved)?;
        !pyvalue::is_file(&path) || {
            let size = std::fs::metadata(path.to_path_buf())
                .map(|meta| meta.len())
                .map_err(|error| Raised::Os(crate::pytext::os_error(&error, path.as_bytes())))?;
            size == 0
        }
    };
    if unsaved {
        found.push(format!(
            "{group}: searched.{place}.saved must name a saved, non-empty response under the dossier directory"
        ));
    }
    let (hits, read) = (get(search, "hits")?, get(search, "read")?);
    let count = |value: &Value| matches!(value, Value::Int(_) | Value::Bool(_));
    if !count(hits) || !count(read) || pyvalue::compare(read, hits)?.is_gt() {
        found.push(format!(
            "{group}: searched.{place} needs `hits` and `read` as counts"
        ));
    } else if pyvalue::compare(read, hits)?.is_lt() && !get(search, "unread")?.truthy() {
        found.push(format!(
            "{group}: searched.{place} read {} of {} hits; read the rest or say in `unread` why not",
            read.py_str(),
            hits.py_str()
        ));
    }
    if !has(search, "found")? {
        found.push(format!(
            "{group}: searched.{place} needs `found`: the quotation, or null when the hits were read and silent"
        ));
    }
    Ok(found)
}

/// What the dossier at `directory` lacks.
pub fn faults(repo: &Repo, directory: &PyPath) -> Py<Vec<String>> {
    let mut found = Vec::new();
    let empty = Value::Object(Object::default());
    let entries = entries(repo, directory);
    for entry in entries.items {
        if one_of(get(&entry, "recommendation")?, &UNSORTED) {
            continue;
        }
        let group_value = item(&entry, "group")?;
        let promise = get(&entry, "promise")?;
        let group = group_value.py_str();
        let complete = match promise {
            Value::Object(_) => {
                let mut complete = true;
                for field in FIELDS {
                    if !has(promise, field)? {
                        complete = false;
                        break;
                    }
                }
                complete
            }
            _ => false,
        };
        if !complete {
            found.push(format!(
                "{group}: needs `promise` with {}, in summary.json or refresh.json",
                FIELDS.join(", ")
            ));
            continue;
        }
        let made_by = item(promise, "made_by")?;
        let promised = one_of(made_by, &WAYS);
        if !promised && !pyvalue::is(made_by, "none") {
            found.push(format!(
                "{group}: promise.made_by must be one of {} or none",
                WAYS.join(", ")
            ));
        }
        if !item(promise, "whose_interface")?.truthy() {
            found.push(format!("{group}: promise.whose_interface is empty"));
        }
        if !one_of(item(promise, "classification")?, &CLASSES) {
            found.push(format!(
                "{group}: promise.classification must be one of {}",
                CLASSES.join(", ")
            ));
        }
        if promised && !item(promise, "source")?.truthy() {
            found.push(format!("{group}: a promise needs its source quoted"));
        }
        let searched = match item(promise, "searched")? {
            searched @ Value::Object(_) => searched,
            _ => &empty,
        };
        for place in PLACES {
            found.extend(search_faults(
                directory,
                &group,
                place,
                get(searched, place)?,
            )?);
        }
        if pyvalue::is(get(&entry, "recommendation")?, "relied-on")
            && !get_or_empty(get(searched, "public-code")?, "found", &empty)?.truthy()
        {
            found.push(format!(
                "{group}: `relied-on` needs the programs found in searched.public-code"
            ));
        }
        if !promised && !blocked(&entry)?.is_empty() {
            found.push(format!(
                "{group}: a blocked search cannot support `none`; finish it or recommend unproven"
            ));
        }
        let delivered = get(&entry, "delivered")?;
        let allowed = if promised {
            one_of(delivered, &["yes", "no"])
        } else {
            matches!(delivered, Value::Null)
        };
        if !allowed {
            found.push(format!(
                "{group}: `delivered` must be yes or no when promised and null when not"
            ));
        } else {
            let follows: [&str; 2] = match (promised, delivered.as_str()) {
                (true, Some("no")) => ["problem", "duplicate"],
                (true, _) => ["minor-defect", "duplicate"],
                (false, _) => ["suggestion", "relied-on"],
            };
            if !one_of(get(&entry, "recommendation")?, &follows) {
                found.push(format!(
                    "{group}: its promise and delivery say the recommendation is one of {}; otherwise {}",
                    follows.join(", "),
                    UNSORTED.join(", ")
                ));
            }
        }
        let dossiers = directory.join(b"dossiers");
        let mut text = String::new();
        for name in [format!("{group}.md"), format!("{group}-supplement.md")] {
            let path = dossiers.join(name.as_bytes());
            if pyvalue::exists(&path) {
                text.push_str(&pyvalue::read_text(&path)?);
            }
        }
        let headings: &[&str] = if promised {
            &["## Promised?", "## Delivered?"]
        } else {
            &["## Promised?"]
        };
        for heading in headings {
            if !text.contains(heading) {
                found.push(format!(
                    "{group}: neither dossiers/{group}.md nor its supplement has a `{heading}` section"
                ));
            }
        }
    }
    match entries.error {
        Some(error) => Err(error),
        None => Ok(found),
    }
}

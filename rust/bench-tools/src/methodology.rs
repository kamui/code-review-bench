//! The current grading queue of `bench/tools/methodology.py`: the selected reviews, batches and their grading
//! state, pending candidates and shared-claim links, planned without changing a saved review or grade.
//!
//! The record is read as `current_grading.load_current(root, grades=False)` reads it, so a stale grade still
//! plans. The queue is written as `json.dumps(indent=2)` writes it, to a new file only.

use crate::current_grading::{self, Failure, R, Root};
use crate::pyjson::Value;
use crate::pytext::{self, PyPath};
use crate::pyvalue::{self, item, iterate, length};

pub const PROGRAM: &str = "bench-tools methodology";

fn text(value: &str) -> Value {
    Value::Str(value.to_string())
}

fn object(members: Vec<(&str, Value)>) -> Value {
    Value::Object(
        members
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

/// `{c["id"]: c for c in cells}`, read as `cells[key]`.
fn lookup<'a>(cells: &'a [(Value, Value)], key: &Value) -> R<&'a Value> {
    pyvalue::hashable(key)?;
    cells
        .iter()
        .rev()
        .find(|(identifier, _)| crate::pyobject::eq(identifier, key))
        .map(|(_, cell)| cell)
        .ok_or_else(|| pyvalue::key_error(key).into())
}

/// `methodology.plan(root)`.
pub fn plan(root: &Root) -> R<Value> {
    let (selected, documents) =
        current_grading::load_current(root, current_grading::CURRENT.as_bytes(), false)?;
    let mut targets = Vec::new();
    for reference in iterate(item(documents.get("reference"), "targets")?)? {
        let mut families = Vec::new();
        for family in iterate(item(&reference, "families")?)? {
            families.push(item(&family, "id")?.clone());
        }
        targets.push(object(vec![
            ("target", item(&reference, "target")?.clone()),
            ("revision", item(&reference, "revision")?.clone()),
            ("families", Value::Array(families)),
            ("control", item(&reference, "control")?.clone()),
            ("referenceAudit", text("pending")),
        ]));
    }
    let mut cells = Vec::new();
    for cell in iterate(item(&selected, "cells")?)? {
        let identifier = item(&cell, "id")?.clone();
        pyvalue::hashable(&identifier)?;
        cells.push((identifier, cell));
    }
    let mut reviews = Vec::new();
    for attempt in iterate(item(&selected, "attempts")?)? {
        let review = item(&attempt, "review")?;
        if matches!(review, Value::Null) {
            continue;
        }
        let cell = lookup(&cells, item(&attempt, "cell")?)?;
        let document = current_grading::read_json(&root.resolve_pin(review)?)?;
        reviews.push(object(vec![
            ("target", item(cell, "target")?.clone()),
            ("run", item(cell, "run")?.clone()),
            ("attempt", item(&attempt, "id")?.clone()),
            ("review", review.clone()),
            ("record", item(&attempt, "record")?.clone()),
            (
                "items",
                Value::Int(length(item(&document, "items")?)?.to_string()),
            ),
            ("parseStatus", item(&document, "parse_status")?.clone()),
            ("admission", item(&attempt, "admission")?.clone()),
            ("action", text("current-claim-assessment")),
            (
                "reason",
                text("Selected scheduled cell and pinned source identity"),
            ),
        ]));
    }
    let mut batches = Vec::new();
    for batch in iterate(item(&selected, "batches")?)? {
        let (fingerprint, state) =
            current_grading::batch_state(root, &batch, &selected, &documents)?;
        let Value::Object(members) = &batch else {
            return Err(Failure::Crash(format!(
                "TypeError: '{}' object is not a mapping",
                batch.type_name()
            )));
        };
        let mut row = members.clone();
        row.insert("inputFingerprint".to_string(), text(&fingerprint));
        row.insert("state".to_string(), text(state));
        batches.push(Value::Object(row));
    }
    let mut pending = Vec::new();
    for candidate in iterate(item(documents.get("candidate"), "candidates")?)? {
        if matches!(item(&candidate, "decision")?, Value::Null) {
            pending.push(candidate);
        }
    }
    let mut shared = Vec::new();
    for claim in iterate(item(documents.get("claim"), "claims")?)? {
        shared.push(object(vec![
            ("claim", item(&claim, "id")?.clone()),
            ("target", item(&claim, "target")?.clone()),
            ("links", item(&claim, "links")?.clone()),
            ("adjudication", item(&claim, "adjudication")?.clone()),
        ]));
    }
    Ok(object(vec![
        ("schemaVersion", Value::Int("1".to_string())),
        ("contract", text("current-reconciliation/v1")),
        ("counts", item(&selected, "counts")?.clone()),
        (
            "publication",
            text("pending current assessments, reference calibration and evaluator audit"),
        ),
        ("targets", Value::Array(targets)),
        ("reviews", Value::Array(reviews)),
        ("batches", Value::Array(batches)),
        ("pendingCandidates", Value::Array(pending)),
        ("sharedClaimItems", Value::Array(shared)),
    ]))
}

/// What `methodology.py` prints once the queue is saved.
pub fn summary(result: &Value) -> R<String> {
    let batches = iterate(item(result, "batches")?)?;
    let mut waiting = 0;
    for batch in &batches {
        if !matches!(item(batch, "state")?, Value::Str(state) if state == "current") {
            waiting += 1;
        }
    }
    Ok(format!(
        "Saved {} selected reviews, {} batches ({waiting} awaiting grading), {} shared-claim links. No grades changed.\n",
        length(item(result, "reviews")?)?,
        batches.len(),
        length(item(result, "sharedClaimItems")?)?
    ))
}

/// `out.parent.mkdir(parents=True, exist_ok=True)`, then `out.open("x")`: a file that exists is never replaced.
pub fn save(out: &[u8], result: &Value) -> R<()> {
    use std::io::Write;
    let path = PyPath::new(out);
    current_grading::make_parents(&current_grading::parent_of(&path))?;
    let failed = |error: std::io::Error| Failure::Input(pytext::os_error(&error, path.as_bytes()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.to_path_buf())
        .map_err(failed)?;
    file.write_all(format!("{}\n", current_grading::dumps(result)).as_bytes())
        .map_err(failed)
}

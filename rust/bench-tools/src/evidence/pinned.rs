//! `evidence pinned`: every large input a tracked file pins by SHA-256 must be a member of a published manifest, or
//! be listed with a reason in `bench/evidence/unpublished.json`. A listed hash that is now published, or that the
//! file no longer pins, is stale and fails too.
//!
//! The pins are the cache archive entry of each `bench/targets/*/target.json`, the `sha256` of each target in every
//! `cache-replacements*.json` under `docs/research`, and each `dependency_supplement` of
//! `bench/policies/grading-commands.v*.json`.

use super::{Error, Result, manifests, validate, violation};
use crate::pyjson::{self, Duplicates, Value};
use crate::pytext::PyPath;
use crate::schema::Validator;
use std::collections::BTreeSet;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// `bench/schema/evidence-unpublished.schema.json`, which a cargo test keeps byte-identical.
pub const SCHEMA: &str = include_str!("unpublished.schema.json");
pub const EXCEPTIONS: &str = "bench/evidence/unpublished.json";
const CACHE_ARCHIVE: &str = "cache archive (";

/// A hash that a tracked file pins; `file` is relative to the repository root.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pin {
    pub file: String,
    pub sha256: String,
}

#[derive(Debug)]
pub struct Report {
    pub problems: Vec<String>,
    pub summary: String,
}

fn load(root: &Path, file: &str) -> Result<Value> {
    let path = root.join(file);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| Error::Io(crate::pytext::os_error(&error, path.as_os_str().as_bytes())))?;
    pyjson::loads(&text, Duplicates::KeepLast)
        .map_err(|error| violation(format!("{file}: {error}")))
}

fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    names.sort();
    names
}

fn hash_of<'a>(entry: &'a Value, key: &str) -> Option<&'a str> {
    entry.as_object()?.get(key)?.as_str()
}

fn research_manifests(root: &Path, directory: &str, found: &mut Vec<String>) {
    for name in names(&root.join(directory)) {
        let relative = format!("{directory}/{name}");
        let path = root.join(&relative);
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_dir() {
            research_manifests(root, &relative, found);
        } else if metadata.is_file()
            && name.starts_with("cache-replacements")
            && name.ends_with(".json")
        {
            found.push(relative);
        }
    }
}

/// Every hash pinned under `root`, in file then hash order.
pub fn pins(root: &Path) -> Result<BTreeSet<Pin>> {
    let mut pins = BTreeSet::new();
    let mut pin = |file: &str, sha256: Option<&str>| {
        if let Some(sha256) = sha256 {
            pins.insert(Pin {
                file: file.to_string(),
                sha256: sha256.to_string(),
            });
        }
    };
    for name in names(&root.join("bench/targets")) {
        let file = format!("bench/targets/{name}/target.json");
        if !root.join(&file).is_file() {
            continue;
        }
        let target = load(root, &file)?;
        let identities = target
            .as_object()
            .and_then(|target| {
                target
                    .get("provisioning")?
                    .as_object()?
                    .get("dependency_identity")
            })
            .and_then(Value::as_array)
            .unwrap_or_default();
        for entry in identities {
            if hash_of(entry, "name").is_some_and(|name| name.starts_with(CACHE_ARCHIVE)) {
                pin(&file, hash_of(entry, "sha256"));
            }
        }
    }
    let mut replacements = Vec::new();
    research_manifests(root, "docs/research", &mut replacements);
    for file in replacements {
        let manifest = load(root, &file)?;
        let targets = manifest
            .as_object()
            .and_then(|manifest| manifest.get("targets")?.as_array())
            .unwrap_or_default();
        for entry in targets {
            pin(&file, hash_of(entry, "sha256"));
        }
    }
    for name in names(&root.join("bench/policies")) {
        if !(name.starts_with("grading-commands.v") && name.ends_with(".json")) {
            continue;
        }
        let file = format!("bench/policies/{name}");
        let policy = load(root, &file)?;
        let targets = policy
            .as_object()
            .and_then(|policy| policy.get("targets")?.as_object());
        for (_, target) in targets.into_iter().flat_map(|targets| targets.iter()) {
            let supplement = target
                .as_object()
                .and_then(|target| target.get("dependency_supplement"));
            pin(
                &file,
                supplement.and_then(|supplement| hash_of(supplement, "sha256")),
            );
        }
    }
    Ok(pins)
}

fn published(root: &Path) -> Result<BTreeSet<String>> {
    let mut hashes = BTreeSet::new();
    for (_, document) in manifests(&PyPath::new(root.as_os_str().as_bytes()))? {
        for package in validate(&document, true)?.packages {
            hashes.extend(package.members.into_iter().map(|member| member.sha256));
        }
    }
    Ok(hashes)
}

fn exceptions(root: &Path) -> Result<BTreeSet<Pin>> {
    let document = load(root, EXCEPTIONS)?;
    let schema = pyjson::loads(SCHEMA, Duplicates::KeepLast)
        .map_err(|error| Error::Io(format!("unpublished schema: {error}")))?;
    let errors = Validator::default()
        .validate(&schema, &document)
        .map_err(|error| violation(error.0))?;
    if !errors.is_empty() {
        return Err(violation(format!("{EXCEPTIONS}: {}", errors.join("; "))));
    }
    let entries = document
        .as_object()
        .and_then(|document| document.get("unpublished")?.as_array())
        .unwrap_or_default();
    let mut listed = BTreeSet::new();
    for entry in entries {
        let pin = Pin {
            file: hash_of(entry, "file").unwrap_or_default().to_string(),
            sha256: hash_of(entry, "sha256").unwrap_or_default().to_string(),
        };
        if !listed.insert(pin.clone()) {
            return Err(violation(format!(
                "{EXCEPTIONS}: duplicate entry for {} in {}",
                pin.sha256, pin.file
            )));
        }
    }
    Ok(listed)
}

pub fn check(root: &Path) -> Result<Report> {
    let pinned = pins(root)?;
    let published = published(root)?;
    let listed = exceptions(root)?;
    let unpublished: BTreeSet<&Pin> = pinned
        .iter()
        .filter(|pin| !published.contains(&pin.sha256))
        .collect();
    let mut problems = Vec::new();
    for pin in &unpublished {
        if !listed.contains(*pin) {
            problems.push(format!(
                "unpublished: {} pinned by {}; publish it, or list it in {EXCEPTIONS} with a reason",
                pin.sha256, pin.file
            ));
        }
    }
    for pin in &listed {
        if unpublished.contains(pin) {
            continue;
        }
        let why = if published.contains(&pin.sha256) {
            "is now published"
        } else {
            "is no longer pinned there"
        };
        problems.push(format!(
            "stale exception: {} for {} {why}; remove it from {EXCEPTIONS}",
            pin.sha256, pin.file
        ));
    }
    let hashes: BTreeSet<&str> = pinned.iter().map(|pin| pin.sha256.as_str()).collect();
    let summary = format!(
        "pinned: {} hashes in {} files; {} pins unpublished, {} listed in {EXCEPTIONS}",
        hashes.len(),
        pinned
            .iter()
            .map(|pin| pin.file.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        unpublished.len(),
        listed.len(),
    );
    Ok(Report { problems, summary })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_schema_is_the_repository_schema() {
        assert_eq!(
            SCHEMA,
            include_str!("../../../../bench/schema/evidence-unpublished.schema.json")
        );
    }
}

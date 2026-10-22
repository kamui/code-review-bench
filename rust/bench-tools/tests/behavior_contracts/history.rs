//! Published decision versions never change, each family's chain from its root is unbroken, and every corpus
//! uses its family's latest version.

use crate::corpus::{field, load_decisions, string};
use crate::json;
use bench_tools::pyjson::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

const DIRECTORY: &str = "tools/behavior_contracts/";

/// The family of a decision file name, as `(?P<family>.+)-decisions\.v[^/]*\.json` reads it.
fn family(name: &str) -> Option<&str> {
    let stem = name.strip_suffix(".json")?;
    let at = stem.rfind("-decisions.v")?;
    (at > 0 && !stem[at..].contains('/')).then(|| &name[..at])
}

struct Git<'a>(&'a Path);

impl Git<'_> {
    fn run(&self, args: &[&str]) -> Result<Vec<u8>, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(self.0)
            .args(args)
            .output()
            .map_err(|error| format!("git: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "git {args:?}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        Ok(output.stdout)
    }

    fn lines(&self, args: &[&str]) -> Result<Vec<String>, String> {
        let out = self.run(args)?;
        Ok(String::from_utf8_lossy(&out)
            .lines()
            .map(str::to_string)
            .collect())
    }

    fn first_added(&self, path: &str) -> Result<Option<String>, String> {
        Ok(self
            .lines(&[
                "log",
                "--reverse",
                "--format=%H",
                "--diff-filter=A",
                "HEAD",
                "--",
                path,
            ])?
            .into_iter()
            .find(|line| !line.trim().is_empty()))
    }
}

fn read(path: &Path) -> Result<Value, String> {
    let raw = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    json::value(&raw)
}

fn basename(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// `{family}-decisions.v*.json`, matched against a path's last component.
fn in_family(path: &str, family: &str) -> bool {
    basename(path)
        .strip_prefix(&format!("{family}-decisions.v"))
        .is_some_and(|rest| rest.ends_with(".json"))
}

pub fn verify(repo: &Path) -> Result<(), String> {
    let git = Git(repo);
    if git
        .run(&["rev-parse", "--is-shallow-repository"])?
        .trim_ascii()
        != b"false"
    {
        return Err("decision history requires a full git checkout (fetch-depth: 0)".to_string());
    }
    let mut names: BTreeSet<String> = git
        .lines(&[
            "log",
            "--format=",
            "--name-only",
            "--diff-filter=A",
            "HEAD",
            "--",
            DIRECTORY,
        ])?
        .into_iter()
        .filter(|path| path.ends_with(".json"))
        .map(|path| basename(&path).to_string())
        .collect();
    for entry in std::fs::read_dir(repo.join(DIRECTORY)).map_err(|error| error.to_string())? {
        let name = entry.map_err(|error| error.to_string())?.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".json") {
            names.insert(name.into_owned());
        }
    }
    let mut corpora: BTreeMap<String, Value> = BTreeMap::new();
    for name in names.iter().filter(|name| *name != "digest-vectors.json") {
        let path = repo.join(DIRECTORY).join(name);
        if family(name).is_none() && path.is_file() {
            let corpus = read(&path)?;
            if let Some(decisions) = corpus
                .as_object()
                .and_then(|object| object.get("decisions"))
            {
                corpora.insert(name.clone(), decisions.clone());
            }
        }
    }
    let corpus_family = |decisions: &Value| decisions.as_str().and_then(family).map(str::to_string);
    let families: BTreeSet<&str> = names.iter().filter_map(|name| family(name)).collect();
    for family in &families {
        let members: Vec<&String> = corpora
            .iter()
            .filter(|(_, decisions)| corpus_family(decisions).as_deref() == Some(family))
            .map(|(name, _)| name)
            .collect();
        verify_family(repo, &git, family, &members)?;
    }
    let unknown: Vec<&str> = corpora
        .iter()
        .filter(|(_, decisions)| {
            !corpus_family(decisions).is_some_and(|name| families.contains(name.as_str()))
        })
        .map(|(name, _)| name.as_str())
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "{}: corpus must use the latest published decision version",
            unknown.join(", ")
        ));
    }
    Ok(())
}

/// A family whose v1 is the legacy decisions/1 map anchors every version at the commit that adopted v2; every
/// other version, including a decisions/2 root, is anchored at its own first addition.
fn verify_family(repo: &Path, git: &Git, family: &str, corpora: &[&String]) -> Result<(), String> {
    let pattern = format!("{DIRECTORY}{family}-decisions.v*.json");
    let root = format!("{DIRECTORY}{family}-decisions.v1.json");
    let root_commit = git
        .first_added(&root)?
        .ok_or_else(|| format!("{root}: decision history has no committed first version"))?;
    let first = json::value(&git.run(&["show", &format!("{root_commit}:{root}")])?)?;
    let legacy = !first
        .as_object()
        .is_some_and(|object| object.get("records").is_some());
    let anchor = if legacy {
        git.first_added(&format!("{DIRECTORY}{family}-decisions.v2.json"))?
    } else {
        Some(root_commit)
    }
    .ok_or("decision history has no committed v2 adoption")?;
    let mut releases: BTreeMap<String, String> = git
        .lines(&["ls-tree", "-r", "--name-only", &anchor, "--", DIRECTORY])?
        .into_iter()
        .filter(|path| in_family(path, family))
        .map(|path| (path, anchor.clone()))
        .collect();
    let range = format!("{anchor}..HEAD");
    for commit in git.lines(&[
        "log",
        "--reverse",
        "--format=%H",
        "--diff-filter=A",
        &range,
        "--",
        &pattern,
    ])? {
        let added = git.lines(&[
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--name-only",
            "-r",
            "--diff-filter=A",
            &commit,
            "--",
            &pattern,
        ])?;
        for path in added {
            releases.entry(path).or_insert_with(|| commit.clone());
        }
    }
    for (path, commit) in &releases {
        let current = repo.join(path);
        let published = git.run(&["show", &format!("{commit}:{path}")])?;
        if !current.is_file()
            || std::fs::read(&current).ok().as_deref() != Some(published.as_slice())
        {
            return Err(format!(
                "{path}: published decision changed; add a new version superseding it"
            ));
        }
    }
    let versions: Vec<String> = (1..=releases.len())
        .map(|number| format!("{DIRECTORY}{family}-decisions.v{number}.json"))
        .collect();
    if versions.iter().collect::<BTreeSet<_>>() != releases.keys().collect::<BTreeSet<_>>() {
        return Err("decision history versions must be contiguous".to_string());
    }
    let latest = versions
        .last()
        .ok_or("decision history versions must be contiguous")?;
    if corpora.is_empty() {
        return Err(format!("{family}: no corpus uses this decision history"));
    }
    for corpus in corpora {
        let decisions = field(&read(&repo.join(DIRECTORY).join(corpus))?, "decisions")?.clone();
        if decisions.as_str() != Some(basename(latest)) {
            return Err(format!(
                "{corpus}: corpus must use the latest published decision version"
            ));
        }
    }
    if !legacy
        && read(&repo.join(&versions[0]))?
            .as_object()
            .is_some_and(|object| object.get("previous").is_some())
    {
        return Err(format!(
            "{}: a first decision version has no predecessor",
            versions[0]
        ));
    }
    for (index, path) in versions.iter().enumerate().skip(1) {
        let number = index + 1;
        let value = read(&repo.join(path))?;
        let previous = value.as_object().and_then(|object| object.get("previous"));
        let format = if legacy && number == 2 {
            "decisions/1"
        } else {
            "decisions/2"
        };
        let retained = previous.is_some_and(|previous| {
            string(previous, "path").ok() == Some(basename(&versions[index - 1]))
                && string(previous, "format").ok() == Some(format)
        });
        if !retained {
            return Err(format!(
                "{path}: decision history must retain its immediate predecessor"
            ));
        }
    }
    load_decisions(&repo.join(latest)).map(|_| ())
}

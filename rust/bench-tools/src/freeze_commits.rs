//! `bench-tools fetch-freeze-commits`: fetch the freeze commit of every run that pins re-cut packets and is missing
//! from this clone.
//!
//! Packet selection reads such a run's manifest at its `freeze_commit`. A squash merge leaves that commit off
//! `main`, and a shallow checkout lacks it either way. The forge still serves it by its id, through the pull request
//! that carried it. A shallow clone fetches at depth 1 so that it stays shallow.
//!
//! Exit codes: 0 every such commit is present afterwards; 1 one could not be fetched, named on stderr; 2 a manifest
//! or the repository cannot be read.

use crate::pyjson::{self, Duplicates};
use std::collections::BTreeSet;
use std::path::Path;
use std::process::{Command, Stdio};

fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).stdin(Stdio::null());
    command
}

fn present(root: &Path, commit: &str) -> bool {
    git(root)
        .args(["cat-file", "-e", &format!("{commit}^{{commit}}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The freeze commits of the runs under `<root>/bench/runs` whose manifests pin packet replacements.
fn pinned(root: &Path) -> Result<BTreeSet<String>, String> {
    let runs = root.join("bench/runs");
    let mut manifests: Vec<_> = std::fs::read_dir(&runs)
        .map_err(|error| format!("{}: {error}", runs.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("manifest.json"))
        .filter(|path| path.is_file())
        .collect();
    manifests.sort();
    let mut commits = BTreeSet::new();
    for path in manifests {
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let manifest = pyjson::loads(&text, Duplicates::KeepLast)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let Some(manifest) = manifest.as_object() else {
            continue;
        };
        if manifest.get("packet_replacements").is_some()
            && let Some(commit) = manifest
                .get("freeze_commit")
                .filter(|commit| commit.truthy())
        {
            commits.insert(
                commit
                    .as_str()
                    .map_or_else(|| commit.py_str(), str::to_string),
            );
        }
    }
    Ok(commits)
}

/// Fetch the missing freeze commits; the summary line, and the commits still absent.
pub fn fetch(root: &Path) -> Result<(String, Vec<String>), String> {
    let commits = pinned(root)?;
    let missing: Vec<&String> = commits
        .iter()
        .filter(|commit| !present(root, commit))
        .collect();
    if !missing.is_empty() {
        let shallow = git(root)
            .args(["rev-parse", "--is-shallow-repository"])
            .output()
            .map_err(|error| format!("git: {error}"))?;
        let mut command = git(root);
        command.args(["fetch", "--no-tags"]);
        if String::from_utf8_lossy(&shallow.stdout).trim() == "true" {
            command.arg("--depth=1");
        }
        // A failed fetch is reported through the commits it left absent.
        let _ = command
            .arg("origin")
            .args(missing.iter().map(|commit| commit.as_str()))
            .stdout(Stdio::inherit())
            .status();
    }
    let absent: Vec<String> = missing
        .iter()
        .filter(|commit| !present(root, commit))
        .map(|commit| (*commit).clone())
        .collect();
    Ok((
        format!(
            "{} freeze commits, {} fetched, {} absent\n",
            commits.len(),
            missing.len() - absent.len(),
            absent.len()
        ),
        absent,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn run(directory: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.invalid",
            ])
            .args(args)
            .output()
            .expect("git");
        assert!(output.status.success(), "{args:?}: {output:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn a_missing_freeze_commit_is_fetched_and_the_clone_keeps_its_depth() {
        for shallow in [false, true] {
            let root: PathBuf =
                crate::test_scratch("freeze-commit-tests").join(crate::evidence::random_name("f-"));
            let source = root.join("source");
            std::fs::create_dir_all(&source).expect("mkdir");
            run(&source, &["init", "-q", "--initial-branch=main"]);
            std::fs::write(source.join("main-only"), "main history").expect("write");
            run(&source, &["add", "."]);
            run(&source, &["commit", "-qm", "main history"]);
            run(&source, &["checkout", "-qb", "freeze"]);
            std::fs::write(source.join("freeze-only"), "pinned runtime").expect("write");
            run(&source, &["add", "."]);
            run(&source, &["commit", "-qm", "an off-main freeze commit"]);
            let commit = run(&source, &["rev-parse", "HEAD"]);
            run(&source, &["checkout", "-q", "main"]);
            let clone = root.join("clone");
            let url = format!("file://{}", source.display());
            let mut args = vec!["clone", "-q", "--single-branch", "--branch=main"];
            if shallow {
                args.push("--depth=1");
            }
            args.extend([url.as_str(), clone.to_str().expect("utf-8")]);
            run(&root, &args);
            let manifest = clone.join("bench/runs/example/manifest.json");
            std::fs::create_dir_all(manifest.parent().expect("a parent")).expect("mkdir");
            std::fs::write(
                &manifest,
                format!(r#"{{"freeze_commit": "{commit}", "packet_replacements": {{}}}}"#),
            )
            .expect("write");
            std::fs::create_dir_all(clone.join("bench/runs/unpinned")).expect("mkdir");
            std::fs::write(
                clone.join("bench/runs/unpinned/manifest.json"),
                r#"{"freeze_commit": "0000000000000000000000000000000000000000"}"#,
            )
            .expect("write");
            assert!(!present(&clone, &commit));
            assert_eq!(
                fetch(&clone).expect("fetch"),
                (
                    "1 freeze commits, 1 fetched, 0 absent\n".to_string(),
                    vec![]
                )
            );
            assert!(present(&clone, &commit));
            assert_eq!(
                fetch(&clone).expect("fetch").0,
                "1 freeze commits, 0 fetched, 0 absent\n"
            );
            assert_eq!(
                run(&clone, &["rev-parse", "--is-shallow-repository"]),
                shallow.to_string()
            );
            std::fs::remove_dir_all(&root).expect("cleanup");
        }
    }

    #[test]
    fn a_commit_the_forge_does_not_serve_is_reported_absent() {
        let root =
            crate::test_scratch("freeze-commit-tests").join(crate::evidence::random_name("a-"));
        let source = root.join("source");
        std::fs::create_dir_all(&source).expect("mkdir");
        run(&source, &["init", "-q", "--initial-branch=main"]);
        run(&source, &["commit", "-q", "--allow-empty", "-m", "main"]);
        let clone = root.join("clone");
        let url = format!("file://{}", source.display());
        run(
            &root,
            &["clone", "-q", url.as_str(), clone.to_str().expect("utf-8")],
        );
        let gone = "1".repeat(40);
        let manifest = clone.join("bench/runs/example/manifest.json");
        std::fs::create_dir_all(manifest.parent().expect("a parent")).expect("mkdir");
        std::fs::write(
            &manifest,
            format!(r#"{{"freeze_commit": "{gone}", "packet_replacements": {{}}}}"#),
        )
        .expect("write");
        assert_eq!(
            fetch(&clone).expect("fetch"),
            (
                "1 freeze commits, 0 fetched, 1 absent\n".to_string(),
                vec![gone]
            )
        );
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}

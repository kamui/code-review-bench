//! `provision fetch`: place a target's published dependency archive and git mirror in a cache root, by hash.
//!
//! The archive is the one `prepare` would restore, so a `--cache-replacements` manifest changes which hash is
//! wanted. It lands at `<cache-root>/archives/<id>.<sha256>.tar.gz`, and the mirror, when a manifest holds its
//! bundle, at `<cache-root>/mirrors/<id>.git` with its receipt beside it. Nothing that exists is overwritten.
//!
//! Exit codes: 0 done; 1 the archive is unpublished or a file in the cache root differs; 2 an input cannot be
//! read, release storage failed or git failed.

use super::{Cache, Options, Outcome, P, git, load_target, os_failure, recorded_archives};
use crate::evidence::{self, Store};
use crate::pyjson::Value;
use crate::pytext::PyPath;
use std::ffi::OsStr;
use std::io::ErrorKind;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

const ARCHIVES: &str = "artifacts/reproducibility/archives";
const MIRRORS: &str = "artifacts/reproducibility/mirrors";

type Member = (String, String);

enum Existing {
    Absent,
    Same,
    Differs(String),
}

fn failure(error: evidence::Error) -> String {
    error.message().to_string()
}

fn py(path: &Path) -> PyPath {
    PyPath::new(path.as_os_str().as_bytes())
}

fn published(root: &Path) -> P<Vec<Member>> {
    let mut members = Vec::new();
    for (_, document) in evidence::manifests(&py(root)).map_err(failure)? {
        for package in evidence::validate(&document, true)
            .map_err(failure)?
            .packages
        {
            members.extend(
                package
                    .members
                    .into_iter()
                    .map(|member| (member.path, member.sha256)),
            );
        }
    }
    Ok(members)
}

fn existing(path: &Path, sha256: &str) -> P<Existing> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Existing::Absent),
        Err(error) => Err(os_failure(path)(error)),
        Ok(metadata) if !metadata.is_file() => {
            Ok(Existing::Differs("not a regular file".to_string()))
        }
        Ok(_) => {
            let found = evidence::digest(&py(path)).map_err(failure)?;
            Ok(if found == sha256 {
                Existing::Same
            } else {
                Existing::Differs(format!("SHA-256 {found}"))
            })
        }
    }
}

/// Restore `logical` from release storage and put it at `destination`: a hard link when the filesystem allows
/// one, otherwise a copy. The inner error is a line to report and leaves `destination` untouched.
fn install(
    store: &mut Store<'_>,
    root: &Path,
    logical: &str,
    sha256: &str,
    destination: &Path,
) -> P<Result<String, String>> {
    match existing(destination, sha256)? {
        Existing::Same => return Ok(Ok(format!("{} is already present", destination.display()))),
        Existing::Differs(found) => {
            return Ok(Err(format!(
                "{} holds {found}, not {sha256}; move it aside, nothing was overwritten",
                destination.display()
            )));
        }
        Existing::Absent => {}
    }
    let source = evidence::resolve(store, &py(root), logical, Some(sha256), false)
        .map_err(failure)?
        .to_path_buf();
    let directory = destination.parent().unwrap_or(Path::new("/"));
    std::fs::create_dir_all(directory).map_err(os_failure(directory))?;
    let how = match store.disk.link(&source, destination) {
        Ok(()) => {
            store
                .disk
                .sync_dir(directory)
                .map_err(os_failure(directory))?;
            "linked"
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Err(format!(
                "{} appeared during the fetch; nothing was overwritten",
                destination.display()
            ));
        }
        Err(_) => {
            evidence::create_new(store.disk, &py(destination), &mut |file| {
                std::io::copy(&mut std::fs::File::open(&source)?, file).map(|_| ())
            })
            .map_err(failure)?;
            "copied"
        }
    };
    Ok(Ok(format!(
        "{how} {} to {}",
        logical,
        destination.display()
    )))
}

fn find<'a>(members: &'a [Member], paths: &[String], sha256: &str) -> Option<&'a str> {
    paths.iter().find_map(|wanted| {
        members
            .iter()
            .find(|(path, sha)| path == wanted && sha == sha256)
            .map(|(path, _)| path.as_str())
    })
}

fn archive(
    store: &mut Store<'_>,
    root: &Path,
    cache_root: &Path,
    id: &str,
    sha256: &str,
    members: &[Member],
) -> P<Result<String, String>> {
    let keyed = format!("{ARCHIVES}/{id}.{sha256}.tar.gz");
    let plain = format!("{ARCHIVES}/{id}.tar.gz");
    let Some(logical) = find(members, &[keyed, plain], sha256) else {
        return Ok(Err(format!(
            "archive: no published member has SHA-256 {sha256} for {id}"
        )));
    };
    let destination = cache_root
        .join("archives")
        .join(format!("{id}.{sha256}.tar.gz"));
    Ok(install(store, root, logical, sha256, &destination)?
        .map(|line| format!("archive: {line}"))
        .map_err(|line| format!("archive: {line}")))
}

fn mirror(
    store: &mut Store<'_>,
    root: &Path,
    cache_root: &Path,
    id: &str,
    members: &[Member],
) -> P<Result<String, String>> {
    let directory = cache_root.join("mirrors");
    let path = directory.join(format!("{id}.git"));
    if std::fs::symlink_metadata(&path).is_ok() {
        return Ok(Ok(format!("mirror: {} is already present", path.display())));
    }
    let sha_of = |logical: &str| {
        members
            .iter()
            .find(|(path, _)| path == logical)
            .map(|(_, sha)| sha.as_str())
    };
    let (bundle, receipt) = (
        format!("{MIRRORS}/{id}.bundle"),
        format!("{MIRRORS}/{id}.json"),
    );
    let Some(bundle_sha) = sha_of(&bundle) else {
        return Ok(Ok(format!(
            "mirror: no published bundle for {id}; build it with provision mirror"
        )));
    };
    let Some(receipt_sha) = sha_of(&receipt) else {
        return Ok(Err(format!(
            "mirror: {bundle} is published without {receipt}"
        )));
    };
    let receipt_line = match install(
        store,
        root,
        &receipt,
        receipt_sha,
        &directory.join(format!("{id}.json")),
    )? {
        Ok(line) => line,
        Err(line) => return Ok(Err(format!("mirror: {line}"))),
    };
    let bundle = evidence::resolve(store, &py(root), &bundle, Some(bundle_sha), false)
        .map_err(failure)?
        .to_path_buf();
    let staging = directory.join(format!(".{id}.git.partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&staging);
    let restored = git(&[
        OsStr::new("clone"),
        OsStr::new("--quiet"),
        OsStr::new("--mirror"),
        bundle.as_os_str(),
        staging.as_os_str(),
    ])
    .and_then(|_| {
        git(&[
            OsStr::new("-C"),
            staging.as_os_str(),
            OsStr::new("remote"),
            OsStr::new("remove"),
            OsStr::new("origin"),
        ])
    })
    .and_then(|_| std::fs::rename(&staging, &path).map_err(os_failure(&path)));
    if let Err(error) = restored {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(Ok(format!(
        "mirror: restored {} from {}; receipt {receipt_line}",
        path.display(),
        bundle.display()
    )))
}

pub fn run(store: &mut Store<'_>, root: &Path, options: &Options) -> P<Outcome> {
    let cache_root = options.cache_root.as_path();
    if !cache_root.is_absolute() {
        return Err(format!(
            "--cache-root {} must be an absolute path",
            cache_root.display()
        ));
    }
    let target = load_target(&options.target, options.cache_replacements.as_deref())?;
    let config = Cache::of(&target)?;
    let members = published(root)?;
    let mut lines = Vec::new();
    let mut code = 0;
    let mut report = |lines: &mut Vec<String>, outcome: Result<String, String>| match outcome {
        Ok(line) => lines.push(line),
        Err(line) => {
            code = 1;
            lines.push(line);
        }
    };
    match recorded_archives(&target, &config).as_slice() {
        [] => lines.push(format!("archive: {} records no cache archive", target.id)),
        [Value::Str(sha256)] => {
            let outcome = archive(store, root, cache_root, &target.id, sha256, &members)?;
            report(&mut lines, outcome);
        }
        _ => {
            return Err(format!(
                "{}: target.json records no single cache archive hash",
                target.id
            ));
        }
    }
    let outcome = mirror(store, root, cache_root, &target.id, &members)?;
    report(&mut lines, outcome);
    Ok(Outcome::lines(&lines, code))
}

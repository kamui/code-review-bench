//! The diff identity of a base..head pair: the changed-file manifest and its SHA-256.
//!
//! `git diff-tree -r --no-renames -z <base> <head>` gives one record per changed path with the base and
//! head blob ids. Records are sorted by path and rendered as `<status>\t<path>\t<base-blob>\t<head-blob>\n`,
//! and the SHA-256 is over those bytes. An empty diff has the hash of the empty string.
//!
//! Paths stay bytes. They sort as `bench/tools/diff_identity.py` sorts them: by code point after
//! `surrogateescape` decoding, so a byte that is not part of valid UTF-8 sorts as U+DC80..U+DCFF rather
//! than by its byte value. Frozen `diff_manifest_sha256` values depend on this order.

use crate::pytext;
use crate::sha256;
use std::ffi::OsStr;
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub status: String,
    pub path: Vec<u8>,
    pub base_blob: String,
    pub head_blob: String,
}

/// A git command failed or printed a record this cannot read; exit code 2.
#[derive(Debug, PartialEq, Eq)]
pub struct RepoError(pub String);

fn diff_tree(repo: &OsStr, base: &OsStr, head: &OsStr) -> Result<Vec<u8>, RepoError> {
    let arguments: [&OsStr; 8] = [
        OsStr::new("-C"),
        repo,
        OsStr::new("diff-tree"),
        OsStr::new("-r"),
        OsStr::new("--no-renames"),
        OsStr::new("-z"),
        base,
        head,
    ];
    let done = Command::new("git")
        .args(arguments)
        .output()
        .map_err(|error| {
            RepoError(format!(
                "cannot run git: {}",
                pytext::os_error(&error, b"git")
            ))
        })?;
    if !done.status.success() {
        let command: Vec<String> = std::iter::once("git".to_string())
            .chain(
                arguments
                    .iter()
                    .map(|part| pytext::stderr_text(pytext::os_bytes(part))),
            )
            .collect();
        let stderr = String::from_utf8_lossy(&done.stderr);
        return Err(RepoError(format!(
            "command failed: {}\n{}",
            command.join(" "),
            pytext::strip(&stderr)
        )));
    }
    Ok(done.stdout)
}

/// The changed paths of `base..head` in manifest order.
pub fn manifest(repo: &OsStr, base: &OsStr, head: &OsStr) -> Result<Vec<Row>, RepoError> {
    let raw = diff_tree(repo, base, head)?;
    let mut fields = raw.split(|byte| *byte == 0);
    let mut rows = Vec::new();
    while let Some(meta) = fields.next().filter(|meta| !meta.is_empty()) {
        let unexpected = || {
            RepoError(format!(
                "unexpected diff-tree record {}",
                pytext::repr(&pytext::surrogateescape(meta))
            ))
        };
        let text = std::str::from_utf8(meta).map_err(|_| unexpected())?;
        let parts: Vec<&str> = text
            .strip_prefix(':')
            .ok_or_else(unexpected)?
            .split(' ')
            .collect();
        let [_, _, base_blob, head_blob, status] = parts[..] else {
            return Err(unexpected());
        };
        let path = fields.next().ok_or_else(unexpected)?;
        rows.push(Row {
            status: status.to_string(),
            path: path.to_vec(),
            base_blob: base_blob.to_string(),
            head_blob: head_blob.to_string(),
        });
    }
    rows.sort_by_cached_key(|row| pytext::surrogateescape(&row.path));
    Ok(rows)
}

pub fn render(rows: &[Row]) -> Vec<u8> {
    let mut text = Vec::new();
    for row in rows {
        for (part, end) in [
            (row.status.as_bytes(), b'\t'),
            (&row.path, b'\t'),
            (row.base_blob.as_bytes(), b'\t'),
            (row.head_blob.as_bytes(), b'\n'),
        ] {
            text.extend_from_slice(part);
            text.push(end);
        }
    }
    text
}

/// The manifest rows and the SHA-256 of their rendering.
pub fn identity(repo: &OsStr, base: &OsStr, head: &OsStr) -> Result<(Vec<Row>, String), RepoError> {
    let rows = manifest(repo, base, head)?;
    let digest = sha256::hex_digest(&render(&rows));
    Ok((rows, digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn git(repo: &Path, arguments: &[&str], input: Option<&[u8]>) -> String {
        use std::io::Write as _;
        let mut child = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(arguments)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.unwrap_or_default())
            .unwrap();
        let done = child.wait_with_output().unwrap();
        assert!(done.status.success(), "git {arguments:?}");
        String::from_utf8(done.stdout).unwrap().trim().to_string()
    }

    fn tree(repo: &Path, entries: &[(&str, &[u8], &[u8])]) -> String {
        let mut listing = Vec::new();
        for (mode, name, content) in entries {
            let blob = git(repo, &["hash-object", "-w", "--stdin"], Some(content));
            listing.extend_from_slice(format!("{mode} blob {blob}\t").as_bytes());
            listing.extend_from_slice(name);
            listing.push(0);
        }
        git(repo, &["mktree", "-z"], Some(&listing))
    }

    #[test]
    fn sorts_escaped_bytes_by_code_point_and_hashes_the_manifest() {
        let scratch = std::env::temp_dir().join(format!("bench-tools-diff-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
        std::fs::create_dir_all(&scratch).unwrap();
        git(&scratch, &["init", "-q", "--bare", "repo.git"], None);
        let repo = scratch.join("repo.git");
        let base = tree(&repo, &[("100644", b"keep", b"same\n")]);
        let head = tree(
            &repo,
            &[
                ("100644", b"keep", b"same\n"),
                ("100644", b"\xff", b"ff\n"),
                ("100644", "\u{e000}".as_bytes(), b"private\n"),
                ("100644", "é".as_bytes(), b"e\n"),
            ],
        );
        let (rows, digest) =
            identity(repo.as_os_str(), OsStr::new(&base), OsStr::new(&head)).unwrap();
        let paths: Vec<&[u8]> = rows.iter().map(|row| row.path.as_slice()).collect();
        assert_eq!(paths, [&b"\xc3\xa9"[..], b"\xff", b"\xee\x80\x80"]);
        assert_eq!(digest, sha256::hex_digest(&render(&rows)));
        let (empty, same) =
            identity(repo.as_os_str(), OsStr::new(&head), OsStr::new(&head)).unwrap();
        assert!(empty.is_empty());
        assert_eq!(
            same,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let error = identity(
            repo.as_os_str(),
            OsStr::new(&base),
            OsStr::new("nonexistent"),
        )
        .unwrap_err();
        assert!(error.0.starts_with(&format!(
            "command failed: git -C {} diff-tree -r --no-renames -z {base} nonexistent\n",
            repo.display()
        )));
        std::fs::remove_dir_all(&scratch).unwrap();
    }
}

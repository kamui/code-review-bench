//! `file_attempt.archive_transcripts`: archive an attempt's transcripts, read the archive back against them, and
//! name it by the path its record keeps.
//!
//! Unlike the Python, the archive is written to a synced temporary file and linked to its name, so an interrupted
//! call leaves no partial archive, and an existing archive is never replaced: one whose members are the transcripts'
//! names, sizes, modes and bytes is kept and recorded by its own hash, and any other is refused.
//!
//! Each member holds its transcript as [`redaction`] leaves it, and the members and the restoration check are
//! compared with that form; the transcripts themselves are not changed. An existing archive whose members are the
//! transcripts as they are, filed before archives were redacted, is kept too.

use super::{Disk, Error, make_dirs, os_failure, parents, random_name, sync_dirs, violation};
use crate::archive::{self, Limits, PackageWriter};
use crate::pyfs::normpath;
use crate::pytext::{self, PyPath};
use crate::redaction::{self, Finding, Rules};
use crate::sha256::{self, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;

type Result<T> = std::result::Result<T, Error>;

/// What `archive_transcripts` returns: the recorded path, the archive's SHA-256 and the restoration check, and the
/// members redaction changed or could not rewrite, which the record does not show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Archived {
    pub path: Vec<u8>,
    pub sha256: String,
    pub passed: bool,
    pub redacted: Vec<Vec<u8>>,
    pub unredactable: Vec<Vec<u8>>,
}

impl Archived {
    /// `json.dumps` of the dict `file_attempt.archive_transcripts` returns.
    pub fn json(&self) -> String {
        format!(
            "{{\"path\": {}, \"sha256\": \"{}\", \"restoration_check\": \"{}\"}}",
            crate::pyjson::ascii_string(&pytext::surrogateescape(&self.path)),
            self.sha256,
            if self.passed { "passed" } else { "failed" }
        )
    }
}

fn current_dir() -> Result<Vec<u8>> {
    std::env::current_dir()
        .map(|path| path.as_os_str().as_bytes().to_vec())
        .map_err(|error| Error::Io(pytext::os_error(&error, b".")))
}

/// `os.path.abspath`.
fn abspath(path: &[u8]) -> Result<Vec<u8>> {
    if path.starts_with(b"/") {
        return Ok(normpath(path));
    }
    Ok(normpath(&[current_dir()?.as_slice(), b"/", path].concat()))
}

/// `os.path.relpath(path, start)`.
fn relpath(path: &[u8], start: &[u8]) -> Result<Vec<u8>> {
    let components = |value: Vec<u8>| -> Vec<Vec<u8>> {
        value
            .split(|byte| *byte == b'/')
            .filter(|part| !part.is_empty())
            .map(<[u8]>::to_vec)
            .collect()
    };
    let start = components(abspath(start)?);
    let path = components(abspath(path)?);
    let common = start.iter().zip(&path).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<Vec<u8>> = vec![b"..".to_vec(); start.len() - common];
    parts.extend(path[common..].iter().cloned());
    if parts.is_empty() {
        return Ok(b".".to_vec());
    }
    Ok(parts.join(&b'/'))
}

/// `os.path.dirname`.
fn dirname(path: &[u8]) -> Vec<u8> {
    let at = path
        .iter()
        .rposition(|byte| *byte == b'/')
        .map_or(0, |at| at + 1);
    let head = &path[..at];
    if !head.is_empty() && head.iter().any(|byte| *byte != b'/') {
        let end = head
            .iter()
            .rposition(|byte| *byte != b'/')
            .map_or(0, |at| at + 1);
        return head[..end].to_vec();
    }
    head.to_vec()
}

/// `PurePosixPath(path).parts`.
fn parts(path: &[u8]) -> Vec<Vec<u8>> {
    let mut found = Vec::new();
    if path.starts_with(b"//") && !path.starts_with(b"///") {
        found.push(b"//".to_vec());
    } else if path.starts_with(b"/") {
        found.push(b"/".to_vec());
    }
    found.extend(
        path.split(|byte| *byte == b'/')
            .filter(|part| !part.is_empty() && *part != b".")
            .map(<[u8]>::to_vec),
    );
    found
}

/// `os.path.expanduser('~')`: `HOME` without trailing slashes, else the user database's home, else nothing.
pub fn python_home() -> Option<Vec<u8>> {
    let home = match std::env::var_os("HOME") {
        Some(home) => home.as_bytes().to_vec(),
        None => std::env::home_dir()?.as_os_str().as_bytes().to_vec(),
    };
    let end = home
        .iter()
        .rposition(|byte| *byte != b'/')
        .map_or(0, |at| at + 1);
    Some(if end == 0 {
        b"/".to_vec()
    } else {
        home[..end].to_vec()
    })
}

/// `file_attempt.recorded_archive_path`: relative to the checkout that holds both `out` (below its `bench/runs`)
/// and the archive, so it survives a move of that checkout; home-relative otherwise.
pub fn recorded_path(dest: &[u8], out: &[u8], home: Option<&[u8]>) -> Vec<u8> {
    let out = parts(out);
    let dest_parts = parts(dest);
    for index in (1..out.len().saturating_sub(1)).rev() {
        if out[index] == b"bench"
            && out[index + 1] == b"runs"
            && dest_parts.starts_with(&out[..index])
        {
            let rest = &dest_parts[index..];
            return if rest.is_empty() {
                b".".to_vec()
            } else {
                rest.join(&b'/')
            };
        }
    }
    match home {
        Some(home) if dest.starts_with(home) => [b"~", &dest[home.len()..]].concat(),
        _ => dest.to_vec(),
    }
}

/// A member as the archive holds it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Member {
    name: Vec<u8>,
    size: u64,
    mode: u32,
    sha256: String,
}

fn shown(path: &[u8]) -> String {
    pytext::stderr_text(path)
}

fn hash_file(path: &[u8]) -> Result<String> {
    let os = PyPath::new(path).to_path_buf();
    let mut file = File::open(&os).map_err(|error| Error::Io(pytext::os_error(&error, path)))?;
    hash(&mut file).map_err(|error| Error::Io(pytext::os_error(&error, path)))
}

fn read_file(path: &[u8]) -> Result<Vec<u8>> {
    std::fs::read(PyPath::new(path).to_path_buf())
        .map_err(|error| Error::Io(pytext::os_error(&error, path)))
}

/// A transcript's bytes as it is and as redaction leaves it, and whether it holds something redaction cannot
/// rewrite.
struct Forms {
    raw: Vec<u8>,
    clean: Option<Vec<u8>>,
    unredactable: bool,
}

impl Forms {
    fn read(path: &[u8], rules: &Rules) -> Result<Self> {
        let raw = read_file(path)?;
        let redacted = redaction::redact(&raw, rules);
        let unredactable = redacted.findings.iter().any(Finding::unredactable);
        Ok(Self {
            raw,
            clean: redacted.data,
            unredactable,
        })
    }

    fn clean(&self) -> &[u8] {
        self.clean.as_deref().unwrap_or(&self.raw)
    }
}

fn hash(reader: &mut dyn Read) -> io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            return Ok(sha256::hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

/// The archive's members, read through `bench_tools::archive` within what the transcripts allow.
fn members(path: &[u8], limits: &Limits) -> std::result::Result<Vec<Member>, String> {
    let file = File::open(PyPath::new(path).to_path_buf())
        .map_err(|error| pytext::os_error(&error, path))?;
    let mut found = Vec::new();
    archive::read_tar_gz(file, limits, |member, data| {
        found.push(Member {
            name: member.name.clone(),
            size: member.size,
            mode: member.mode,
            sha256: hash(data)?,
        });
        Ok(())
    })
    .map_err(|error| error.to_string())?;
    Ok(found)
}

/// Archive `paths` to `dest` with names relative to `attempt_dir`, check it, and describe it for a record in `out`,
/// under the redaction rules of [`Rules::load`].
pub fn archive_transcripts(
    disk: &mut dyn Disk,
    paths: &[Vec<u8>],
    attempt_dir: &[u8],
    dest: &[u8],
    out: &[u8],
    home: Option<&[u8]>,
) -> Result<Archived> {
    let rules = Rules::load().map_err(|error| violation(error.0))?;
    archive_transcripts_with(disk, paths, attempt_dir, dest, out, home, &rules)
}

/// As [`archive_transcripts`], under `rules`.
pub fn archive_transcripts_with(
    disk: &mut dyn Disk,
    paths: &[Vec<u8>],
    attempt_dir: &[u8],
    dest: &[u8],
    out: &[u8],
    home: Option<&[u8]>,
    rules: &Rules,
) -> Result<Archived> {
    let dest = abspath(dest)?;
    let directory = PyPath::new(&dirname(&dest));
    let created = make_dirs(&directory)?;
    let mut inputs: Vec<&Vec<u8>> = paths.iter().collect::<BTreeSet<_>>().into_iter().collect();
    inputs.sort_by_key(|path| pytext::surrogateescape(path));
    let mut expected = Vec::with_capacity(inputs.len());
    let mut unredacted = Vec::with_capacity(inputs.len());
    let mut redacted = Vec::new();
    let mut unredactable = Vec::new();
    let mut named: HashMap<Vec<u8>, &[u8]> = HashMap::new();
    for path in &inputs {
        let name = relpath(path, attempt_dir)?;
        let metadata = std::fs::symlink_metadata(PyPath::new(path).to_path_buf())
            .map_err(|error| Error::Io(pytext::os_error(&error, path)))?;
        if !metadata.file_type().is_file() {
            return Err(violation(format!(
                "transcript is not a regular file: {}",
                shown(path)
            )));
        }
        if let Some(other) = named.insert(name.clone(), path) {
            return Err(violation(format!(
                "transcripts {} and {} are both archived as {}",
                shown(other),
                shown(path),
                shown(&name)
            )));
        }
        let forms = Forms::read(path, rules)?;
        let mode = metadata.mode() & 0o7777;
        if forms.clean.is_some() {
            redacted.push(name.clone());
        }
        if forms.unredactable {
            unredactable.push(name.clone());
        }
        unredacted.push(Member {
            name: name.clone(),
            size: forms.raw.len() as u64,
            mode,
            sha256: sha256::hex_digest(&forms.raw),
        });
        expected.push(Member {
            name,
            size: forms.clean().len() as u64,
            mode,
            sha256: sha256::hex_digest(forms.clean()),
        });
    }
    let limits = Limits {
        members: expected.len() as u64,
        payload: expected
            .iter()
            .chain(&unredacted)
            .map(|member| member.size)
            .fold(0, u64::saturating_add),
    };
    let target = PyPath::new(&dest);
    if std::fs::symlink_metadata(target.to_path_buf()).is_err() {
        write(disk, &inputs, &expected, &directory, &target, rules)?;
    }
    let mut sorted = expected.clone();
    sorted.sort();
    let mut legacy_sorted = unredacted;
    legacy_sorted.sort();
    let found = match std::fs::symlink_metadata(target.to_path_buf()) {
        Ok(metadata) if metadata.file_type().is_file() => members(&dest, &limits),
        _ => Err("it is not a regular file".to_string()),
    };
    let (found, legacy) = match found {
        Ok(mut found) => {
            found.sort();
            let legacy = found != sorted;
            if legacy && found != legacy_sorted {
                return Err(violation(format!(
                    "refusing to replace {}: its members differ from the transcripts",
                    shown(&dest)
                )));
            }
            (found, legacy)
        }
        Err(reason) => {
            return Err(violation(format!(
                "refusing to replace {}: {reason}",
                shown(&dest)
            )));
        }
    };
    let reaching: BTreeSet<Vec<u8>> = std::iter::once(directory.clone())
        .chain(parents(&directory))
        .chain(created.iter().flat_map(parents))
        .map(|path| path.as_bytes().to_vec())
        .collect();
    sync_dirs(disk, &reaching)?;
    let mut passed = found.len() == inputs.len();
    for member in &found {
        if member.name.starts_with(b"/") || member.name.starts_with(b"..") {
            passed = false;
            continue;
        }
        let path = PyPath::new(attempt_dir).join(&member.name);
        let forms = Forms::read(path.as_bytes(), rules)?;
        let live = if legacy { &forms.raw } else { forms.clean() };
        if sha256::hex_digest(live) != member.sha256 {
            passed = false;
        }
    }
    Ok(Archived {
        path: recorded_path(&dest, out, home),
        sha256: hash_file(&dest)?,
        passed,
        redacted: if legacy { Vec::new() } else { redacted },
        unredactable,
    })
}

/// Write the archive to a synced temporary file beside `target`, then link it there.
fn write(
    disk: &mut dyn Disk,
    inputs: &[&Vec<u8>],
    expected: &[Member],
    directory: &PyPath,
    target: &PyPath,
    rules: &Rules,
) -> Result<()> {
    let name = target
        .as_bytes()
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default();
    let temporary =
        directory.join(&[b".", name, b".", random_name("").as_bytes(), b".tmp"].concat());
    let os_temporary = temporary.to_path_buf();
    let written = (|| -> Result<()> {
        let file = disk
            .create(&os_temporary)
            .map_err(|error| os_failure(&error, temporary.as_bytes()))?;
        let mut writer = PackageWriter::new(&file);
        for (path, member) in inputs.iter().zip(expected) {
            let forms = Forms::read(path, rules)?;
            writer
                .append(&member.name, member.size, member.mode, &mut forms.clean())
                .map_err(|error| match error {
                    archive::ArchiveError::Io(error) => os_failure(&error, path),
                    refused => violation(refused.to_string()),
                })?;
        }
        writer
            .finish()
            .map_err(|error| Error::Io(format!("{}: {error}", shown(temporary.as_bytes()))))?;
        disk.sync(&file, &os_temporary)
            .map_err(|error| os_failure(&error, temporary.as_bytes()))?;
        match disk.link(&os_temporary, &target.to_path_buf()) {
            Err(error) if error.kind() != io::ErrorKind::AlreadyExists => {
                Err(os_failure(&error, target.as_bytes()))
            }
            _ => Ok(()),
        }
    })();
    let _ = std::fs::remove_file(&os_temporary);
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_follow_posixpath() {
        assert_eq!(normpath(b"/a/./b/../c//"), b"/a/c");
        assert_eq!(normpath(b"//a/../.."), b"//");
        assert_eq!(normpath(b"///a"), b"/a");
        assert_eq!(normpath(b"../x/.."), b"..");
        assert_eq!(dirname(b"/a/b/c.tar.gz"), b"/a/b");
        assert_eq!(dirname(b"/c.tar.gz"), b"/");
        assert_eq!(dirname(b"//c"), b"//");
        assert_eq!(relpath(b"/a/b/c", b"/a/d").unwrap(), b"../b/c");
        assert_eq!(relpath(b"/a", b"/a").unwrap(), b".");
    }

    #[test]
    fn recorded_paths_follow_file_attempt() {
        let out = b"/w/checkout/bench/runs/r/attempts/att-001";
        assert_eq!(
            recorded_path(
                b"/w/checkout/artifacts/t/r/att-001.tar.gz",
                out,
                Some(b"/home/u")
            ),
            b"artifacts/t/r/att-001.tar.gz"
        );
        assert_eq!(
            recorded_path(b"/home/u/cache/r/att-001.tar.gz", out, Some(b"/home/u")),
            b"~/cache/r/att-001.tar.gz"
        );
        // Python compares text, not path components.
        assert_eq!(
            recorded_path(b"/home/user2/x", out, Some(b"/home/u")),
            b"~ser2/x"
        );
        assert_eq!(recorded_path(b"/elsewhere/x", out, None), b"/elsewhere/x");
        assert_eq!(
            recorded_path(b"/w/x.tar.gz", b"bench/runs/r/attempts/a", Some(b"/home/u")),
            b"/w/x.tar.gz"
        );
    }

    struct Attempt {
        base: std::path::PathBuf,
        transcript: Vec<u8>,
    }

    impl Attempt {
        fn new(group: &str, content: &[u8]) -> Self {
            let base = crate::test_scratch("transcript-redaction")
                .join(crate::evidence::random_name(group));
            std::fs::create_dir_all(base.join("attempt/home")).unwrap();
            let transcript = base.join("attempt/home/s1.jsonl");
            std::fs::write(&transcript, content).unwrap();
            Self {
                transcript: transcript.as_os_str().as_bytes().to_vec(),
                base,
            }
        }

        fn archive(&self, rules: &Rules) -> Result<Archived> {
            archive_transcripts_with(
                &mut crate::evidence::System,
                std::slice::from_ref(&self.transcript),
                self.base.join("attempt").as_os_str().as_bytes(),
                self.base.join("archive/a.tar.gz").as_os_str().as_bytes(),
                self.base.join("out").as_os_str().as_bytes(),
                None,
                rules,
            )
        }

        fn member(&self) -> Vec<u8> {
            let file = File::open(self.base.join("archive/a.tar.gz")).unwrap();
            let limits = Limits {
                members: 1,
                payload: 1 << 20,
            };
            let mut found = Vec::new();
            archive::read_tar_gz(file, &limits, |_, data| {
                data.read_to_end(&mut found)?;
                Ok(())
            })
            .unwrap();
            found
        }
    }

    fn plan(value: &str) -> String {
        format!("{{\"plan_type\":\"{value}\"}}\n")
    }

    #[test]
    fn members_hold_the_redacted_transcripts_and_the_live_files_stay() {
        let attempt = Attempt::new("r-", plan("plus").as_bytes());
        let archived = attempt.archive(&Rules::fields()).unwrap();
        assert!(archived.passed);
        assert_eq!(archived.redacted, [b"home/s1.jsonl".to_vec()]);
        assert_eq!(attempt.member(), plan("[redacted-plan]").as_bytes());
        assert_eq!(
            std::fs::read(PyPath::new(&attempt.transcript).to_path_buf()).unwrap(),
            plan("plus").as_bytes()
        );
        let again = attempt.archive(&Rules::fields()).unwrap();
        assert_eq!(again.sha256, archived.sha256);
        assert!(again.passed);
    }

    #[test]
    fn a_transcript_redaction_cannot_rewrite_is_archived_as_it_is_and_named() {
        let content = b"SQLite format 3\0 mail person@example.com".to_vec();
        let attempt = Attempt::new("u-", &content);
        let rules =
            Rules::from_values(br#"{"schema_version": 1, "email": ["person@example.com"]}"#)
                .unwrap();
        let archived = attempt.archive(&rules).unwrap();
        assert!(archived.passed);
        assert_eq!(archived.unredactable, [b"home/s1.jsonl".to_vec()]);
        assert_eq!(archived.redacted, Vec::<Vec<u8>>::new());
        assert_eq!(attempt.member(), content);
        let clean = Attempt::new("v-", b"{\"type\": \"user\"}\n");
        assert_eq!(
            clean.archive(&rules).unwrap().unredactable,
            Vec::<Vec<u8>>::new()
        );
    }

    #[test]
    fn clean_transcripts_archive_as_before() {
        let content = b"{\"type\": \"user\"}\n";
        let attempt = Attempt::new("c-", content);
        let archived = attempt.archive(&Rules::fields()).unwrap();
        assert!(archived.passed && archived.redacted.is_empty());
        let mode = std::fs::metadata(PyPath::new(&attempt.transcript).to_path_buf())
            .unwrap()
            .mode()
            & 0o7777;
        let mut writer = PackageWriter::new(Vec::new());
        writer
            .append(
                b"home/s1.jsonl",
                content.len() as u64,
                mode,
                &mut &content[..],
            )
            .unwrap();
        let before = writer.finish().unwrap();
        assert_eq!(archived.sha256, sha256::hex_digest(&before));
    }

    #[test]
    fn an_archive_filed_before_redaction_is_kept() {
        let attempt = Attempt::new("l-", b"user@examplehost:~$\n");
        let before = attempt.archive(&Rules::fields()).unwrap();
        assert_eq!(before.redacted, Vec::<Vec<u8>>::new());
        let hosts = Rules::from_values(
            br#"{"schema_version": 1, "hosts": {"examplehost": "host-01-xyz"}}"#,
        )
        .unwrap();
        let again = attempt.archive(&hosts).unwrap();
        assert_eq!(again.sha256, before.sha256);
        assert!(again.passed && again.redacted.is_empty());
        std::fs::write(PyPath::new(&attempt.transcript).to_path_buf(), b"changed\n").unwrap();
        assert!(
            attempt
                .archive(&hosts)
                .unwrap_err()
                .message()
                .starts_with("refusing to replace ")
        );
    }
}

//! The release storage that published evidence lives in, and its `gh` implementation.

use super::Error;
use crate::pyjson::{self, Duplicates, Value};
use crate::pytext;
use crate::pytext::PyPath;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, ExitStatus, Stdio};

/// How an upload command ended, once it ran.
#[derive(Debug)]
pub enum Upload {
    Done,
    /// The command failed, so the server may still have committed the asset; the error is its `CalledProcessError`.
    Failed(Error),
}

/// What `evidence_store.GitHub` does. Cargo tests substitute a fake; the binary always runs `gh`.
pub trait Remote {
    /// The release a tag names, with the assets of every page joined under `assets`.
    fn release(&mut self, repository: &str, tag: &str) -> Result<Value, Error>;
    /// Stream one asset into `destination`, refusing more than `limit` bytes.
    fn download(
        &mut self,
        repository: &str,
        asset_id: &str,
        destination: &mut dyn Write,
        limit: u64,
    ) -> Result<(), Error>;
    /// Add `archive` to the release under its file name, never replacing an asset. `Err` means it never ran.
    fn upload(&mut self, repository: &str, tag: &str, archive: &PyPath) -> Result<Upload, Error>;
}

/// `gh`, with `evidence_store.GitHub`'s argument lists.
pub struct GitHub;

/// `urllib.parse.quote(text, safe="")`.
pub fn quote(text: &str) -> String {
    let mut quoted = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b'~') {
            quoted.push(char::from(byte));
        } else {
            quoted.push_str(&format!("%{byte:02X}"));
        }
    }
    quoted
}

const SIGNALS: [(i32, &str); 11] = [
    (1, "SIGHUP"),
    (2, "SIGINT"),
    (3, "SIGQUIT"),
    (4, "SIGILL"),
    (5, "SIGTRAP"),
    (6, "SIGABRT"),
    (9, "SIGKILL"),
    (11, "SIGSEGV"),
    (13, "SIGPIPE"),
    (14, "SIGALRM"),
    (15, "SIGTERM"),
];

/// `str(subprocess.CalledProcessError(status, command))`.
pub fn called_process_error(command: &[&str], status: ExitStatus) -> String {
    let listed: Vec<String> = command.iter().map(|part| pytext::repr_str(part)).collect();
    process_error(&listed, status)
}

/// `str(subprocess.CalledProcessError(status, command))` for a command whose parts are already quoted.
fn process_error(listed: &[String], status: ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    let command = format!("[{}]", listed.join(", "));
    match (status.code(), status.signal()) {
        (Some(code), _) => format!("Command '{command}' returned non-zero exit status {code}."),
        (None, Some(signal)) => match SIGNALS.iter().find(|(number, _)| *number == signal) {
            Some((number, name)) => {
                format!("Command '{command}' died with <Signals.{name}: {number}>.")
            }
            None => format!("Command '{command}' died with unknown signal {signal}."),
        },
        (None, None) => format!("Command '{command}' returned non-zero exit status -1."),
    }
}

fn spawn_error(error: &std::io::Error) -> Error {
    Error::Io(pytext::os_error(error, b"gh"))
}

/// `json.loads(subprocess.check_output(command, text=True, encoding='utf-8'))`.
fn api(arguments: &[&str]) -> Result<Value, Error> {
    let output = Command::new("gh")
        .args(arguments)
        .stdin(Stdio::inherit())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| spawn_error(&error))?;
    if !output.status.success() {
        let command: Vec<&str> = std::iter::once("gh")
            .chain(arguments.iter().copied())
            .collect();
        return Err(Error::Io(called_process_error(&command, output.status)));
    }
    let text = pytext::decode_utf8(&output.stdout).map_err(Error::Violation)?;
    pyjson::loads(&pytext::universal_newlines(&text), Duplicates::KeepLast)
        .map_err(|error| Error::Violation(error.to_string()))
}

fn key<'a>(value: &'a Value, name: &str) -> Result<&'a Value, Error> {
    match value {
        Value::Object(object) => object
            .get(name)
            .ok_or_else(|| Error::Violation(format!("KeyError: {}", pytext::repr_str(name)))),
        other => Err(Error::Violation(format!(
            "TypeError: {} indices must be integers or slices, not str",
            other.type_name()
        ))),
    }
}

impl Remote for GitHub {
    fn release(&mut self, repository: &str, tag: &str) -> Result<Value, Error> {
        let route = format!("repos/{repository}/releases/tags/{}", quote(tag));
        let mut release = api(&["api", &route])?;
        let assets_route = format!(
            "repos/{repository}/releases/{}/assets?per_page=100",
            key(&release, "id")?.py_str()
        );
        let pages = api(&["api", "--paginate", "--slurp", &assets_route])?;
        let mut assets = Vec::new();
        for page in pages.as_array().ok_or_else(|| {
            Error::Violation(format!(
                "TypeError: '{}' object is not iterable",
                pages.type_name()
            ))
        })? {
            assets.extend(page.as_array().map(<[Value]>::to_vec).ok_or_else(|| {
                Error::Violation(format!(
                    "TypeError: '{}' object is not iterable",
                    page.type_name()
                ))
            })?);
        }
        match &mut release {
            Value::Object(object) => object.insert("assets".to_string(), Value::Array(assets)),
            _ => unreachable!("key() refused a release that is not an object"),
        }
        Ok(release)
    }

    fn download(
        &mut self,
        repository: &str,
        asset_id: &str,
        destination: &mut dyn Write,
        limit: u64,
    ) -> Result<(), Error> {
        let route = format!("repos/{repository}/releases/assets/{asset_id}");
        let arguments = [
            "api",
            route.as_str(),
            "-H",
            "Accept: application/octet-stream",
        ];
        let mut child = Command::new("gh")
            .args(arguments)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|error| spawn_error(&error))?;
        let streamed = stream(child.stdout.take(), destination, limit);
        if streamed.is_err() {
            let _ = child.kill();
        }
        let status = child.wait().map_err(|error| Error::Io(error.to_string()));
        streamed?;
        let status = status?;
        if !status.success() {
            let command: Vec<&str> = std::iter::once("gh").chain(arguments).collect();
            return Err(Error::Io(called_process_error(&command, status)));
        }
        Ok(())
    }

    fn upload(&mut self, repository: &str, tag: &str, archive: &PyPath) -> Result<Upload, Error> {
        let status = Command::new("gh")
            .args(["release", "upload", tag])
            .arg(std::ffi::OsStr::from_bytes(archive.as_bytes()))
            .args(["--repo", repository])
            .status()
            .map_err(|error| spawn_error(&error))?;
        if status.success() {
            return Ok(Upload::Done);
        }
        let listed = [
            pytext::repr_str("gh"),
            pytext::repr_str("release"),
            pytext::repr_str("upload"),
            pytext::repr_str(tag),
            archive.repr(),
            pytext::repr_str("--repo"),
            pytext::repr_str(repository),
        ];
        Ok(Upload::Failed(Error::Io(process_error(&listed, status))))
    }
}

fn stream(source: Option<impl Read>, destination: &mut dyn Write, limit: u64) -> Result<(), Error> {
    let Some(mut source) = source else {
        return Ok(());
    };
    let mut remaining = limit;
    let mut buffer = vec![0u8; 1024 * 1024];
    loop {
        let wanted = buffer
            .len()
            .min(usize::try_from(remaining.saturating_add(1)).unwrap_or(usize::MAX));
        let read = source
            .read(&mut buffer[..wanted])
            .map_err(|error| Error::Io(error.to_string()))?;
        if read == 0 {
            return Ok(());
        }
        if read as u64 > remaining {
            return Err(Error::Violation(
                "download exceeds pinned archive size".to_string(),
            ));
        }
        remaining -= read as u64;
        destination
            .write_all(&buffer[..read])
            .map_err(|error| Error::Io(error.to_string()))?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn quote_matches_urllib_with_no_safe_characters() {
        assert_eq!(quote("evidence-a/b c~é"), "evidence-a%2Fb%20c~%C3%A9");
    }

    #[test]
    fn called_process_error_matches_python() {
        assert_eq!(
            called_process_error(&["gh", "api", "x'y"], ExitStatus::from_raw(1 << 8)),
            "Command '['gh', 'api', \"x'y\"]' returned non-zero exit status 1."
        );
        assert_eq!(
            called_process_error(&["gh"], ExitStatus::from_raw(9)),
            "Command '['gh']' died with <Signals.SIGKILL: 9>."
        );
    }

    #[test]
    fn stream_refuses_more_than_the_limit() {
        let mut out = Vec::new();
        assert!(stream(Some(&b"12345"[..]), &mut out, 5).is_ok());
        assert_eq!(out, b"12345");
        let mut out = Vec::new();
        assert_eq!(
            stream(Some(&b"123456"[..]), &mut out, 5),
            Err(Error::Violation(
                "download exceeds pinned archive size".to_string()
            ))
        );
    }
}

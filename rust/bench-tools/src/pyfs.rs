//! Filesystem steps that diagnostics and outputs quote, done as Python's `pathlib` does them on POSIX.
//!
//! Paths are bytes. `resolve` is `os.path.realpath(strict=False)` of Python 3.13 and later: missing parts and
//! symbolic-link loops are kept as written rather than refused.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

fn os(path: &[u8]) -> &Path {
    Path::new(OsStr::from_bytes(path))
}

/// `Path(path).resolve()`. A relative path is taken from the current directory.
pub fn resolve(path: &[u8]) -> Vec<u8> {
    enum Part {
        Name(Vec<u8>),
        /// A symbolic link whose target parts precede this marker on the stack has been resolved.
        Resolved(Vec<u8>),
    }
    let mut rest: Vec<Part> = path
        .split(|byte| *byte == b'/')
        .rev()
        .map(|part| Part::Name(part.to_vec()))
        .collect();
    let mut current: Vec<u8> = if path.starts_with(b"/") {
        b"/".to_vec()
    } else {
        std::env::current_dir()
            .map(|directory| directory.as_os_str().as_bytes().to_vec())
            .unwrap_or_else(|_| b"/".to_vec())
    };
    let mut seen: std::collections::HashMap<Vec<u8>, Option<Vec<u8>>> = Default::default();
    while let Some(part) = rest.pop() {
        let name = match part {
            Part::Resolved(link) => {
                seen.insert(link, Some(current.clone()));
                continue;
            }
            Part::Name(name) => name,
        };
        if name.is_empty() || name == b"." {
            continue;
        }
        if name == b".." {
            let cut = current.iter().rposition(|byte| *byte == b'/').unwrap_or(0);
            current.truncate(cut.max(1));
            continue;
        }
        let mut next = current.clone();
        if next != b"/" {
            next.push(b'/');
        }
        next.extend_from_slice(&name);
        let is_link =
            std::fs::symlink_metadata(os(&next)).is_ok_and(|meta| meta.file_type().is_symlink());
        if !is_link {
            current = next;
            continue;
        }
        if let Some(known) = seen.get(&next) {
            current = known.clone().unwrap_or_else(|| next.clone());
            continue;
        }
        let Ok(target) = std::fs::read_link(os(&next)) else {
            current = next;
            continue;
        };
        let target = target.as_os_str().as_bytes().to_vec();
        if target.starts_with(b"/") {
            current = b"/".to_vec();
        }
        seen.insert(next.clone(), None);
        rest.push(Part::Resolved(next));
        rest.extend(
            target
                .split(|byte| *byte == b'/')
                .rev()
                .map(|part| Part::Name(part.to_vec())),
        );
    }
    current
}

fn parts(path: &[u8]) -> Vec<&[u8]> {
    path.split(|byte| *byte == b'/')
        .filter(|part| !part.is_empty() && *part != b".")
        .collect()
}

/// The parts of `path` below `base`, for two absolute paths, or `None` when `path` is not inside `base`.
pub fn relative_parts<'a>(path: &'a [u8], base: &[u8]) -> Option<Vec<&'a [u8]>> {
    let inner = parts(path);
    let outer = parts(base);
    (inner.len() >= outer.len() && inner[..outer.len()] == outer[..])
        .then(|| inner[outer.len()..].to_vec())
}

/// `path.relative_to(base)` as text, with the `ValueError` message for a path outside `base`.
pub fn relative_to(path: &[u8], base: &[u8]) -> Result<Vec<u8>, String> {
    relative_parts(path, base)
        .map(|parts| {
            if parts.is_empty() {
                b".".to_vec()
            } else {
                parts.join(&b'/')
            }
        })
        .ok_or_else(|| {
            let text = |bytes: &[u8]| crate::pytext::repr(&crate::pytext::surrogateescape(bytes));
            format!("{} is not in the subpath of {}", text(path), text(base))
        })
}

/// The names in a directory, in the order the system lists them; nothing when it cannot be listed, as a
/// `pathlib` glob ignores the error.
pub fn names(directory: &[u8]) -> Vec<Vec<u8>> {
    let Ok(entries) = std::fs::read_dir(os(directory)) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().as_bytes().to_vec())
        .collect()
}

pub fn is_dir(path: &[u8]) -> bool {
    os(path).is_dir()
}

/// Whether the path names an entry, without following a final symbolic link.
pub fn lexists(path: &[u8]) -> bool {
    std::fs::symlink_metadata(os(path)).is_ok()
}

/// `posixpath.normpath`.
pub fn normpath(path: &[u8]) -> Vec<u8> {
    if path.is_empty() {
        return b".".to_vec();
    }
    let initial: &[u8] = if path.starts_with(b"//") && !path.starts_with(b"///") {
        b"//"
    } else if path.starts_with(b"/") {
        b"/"
    } else {
        b""
    };
    let mut parts: Vec<&[u8]> = Vec::new();
    for part in path.split(|byte| *byte == b'/') {
        match part {
            b"" | b"." => {}
            b".." if !initial.is_empty() && parts.is_empty() => {}
            b".." if parts.last().is_some_and(|last| *last != b"..") => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    let joined = [initial, &parts.join(&b'/')].concat();
    if joined.is_empty() {
        b".".to_vec()
    } else {
        joined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_follows_links_and_keeps_missing_parts() {
        let base = std::env::temp_dir().join(format!("bench-tools-pyfs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("real/inner")).unwrap();
        std::os::unix::fs::symlink("real", base.join("link")).unwrap();
        std::os::unix::fs::symlink("loop", base.join("loop")).unwrap();
        let root = resolve(base.as_os_str().as_bytes());
        let join = |tail: &str| [root.as_slice(), tail.as_bytes()].concat();
        assert_eq!(resolve(&join("/link/inner")), join("/real/inner"));
        assert_eq!(resolve(&join("/link/missing/../x")), join("/real/x"));
        assert_eq!(resolve(&join("/loop/x")), join("/loop/x"));
        assert_eq!(resolve(&join("/./real//inner/")), join("/real/inner"));
        assert_eq!(resolve(b"/.."), b"/".to_vec());
        assert_eq!(
            relative_to(&join("/real/inner"), &root),
            Ok(b"real/inner".to_vec())
        );
        assert!(relative_to(b"/elsewhere", &root).is_err());
        std::fs::remove_dir_all(&base).unwrap();
    }
}

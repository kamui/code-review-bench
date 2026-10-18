//! Python text handling that byte-pinned outputs and saved diagnostics depend on.
//!
//! Strings that may hold Python's `surrogateescape` surrogates are sequences of code points (`u32`),
//! because a Rust `char` cannot hold a surrogate.

use std::fmt::Write as _;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;

/// `bytes.decode("utf-8", "surrogateescape")`: each byte outside a valid sequence becomes U+DC80..U+DCFF.
pub fn surrogateescape(bytes: &[u8]) -> Vec<u32> {
    let mut points = Vec::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        points.extend(chunk.valid().chars().map(u32::from));
        points.extend(chunk.invalid().iter().map(|byte| 0xDC00 + u32::from(*byte)));
    }
    points
}

/// How Python writes a `surrogateescape`-decoded string to stderr, whose error handler is `backslashreplace`.
pub fn stderr_text(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len());
    for chunk in bytes.utf8_chunks() {
        text.push_str(chunk.valid());
        for byte in chunk.invalid() {
            let _ = write!(text, "\\udc{byte:02x}");
        }
    }
    text
}

const WHITESPACE: [(u32, u32); 10] = [
    (0x09, 0x0D),
    (0x1C, 0x20),
    (0x85, 0x85),
    (0xA0, 0xA0),
    (0x1680, 0x1680),
    (0x2000, 0x200A),
    (0x2028, 0x2029),
    (0x202F, 0x202F),
    (0x205F, 0x205F),
    (0x3000, 0x3000),
];

/// `str.isspace()`, which `str.strip()` and `str.split()` use with no argument.
pub fn is_space(c: char) -> bool {
    let point = u32::from(c);
    WHITESPACE
        .iter()
        .any(|(low, high)| (*low..=*high).contains(&point))
}

/// `str.strip()` with no argument.
pub fn strip(text: &str) -> &str {
    text.trim_matches(is_space)
}

/// Code points `str.isprintable()` rejects outside ASCII: separators other than space, controls, format
/// characters, surrogates, private use and noncharacters. Other unassigned code points are printable here,
/// though Python, whose Unicode tables this crate does not carry, escapes them.
const NONPRINTABLE: [(u32, u32); 36] = [
    (0x80, 0xA0),
    (0xAD, 0xAD),
    (0x600, 0x605),
    (0x61C, 0x61C),
    (0x6DD, 0x6DD),
    (0x70F, 0x70F),
    (0x890, 0x891),
    (0x8E2, 0x8E2),
    (0x1680, 0x1680),
    (0x180E, 0x180E),
    (0x2000, 0x200F),
    (0x2028, 0x202F),
    (0x205F, 0x206F),
    (0x3000, 0x3000),
    (0xD800, 0xF8FF),
    (0xFDD0, 0xFDEF),
    (0xFEFF, 0xFEFF),
    (0xFFF9, 0xFFFB),
    (0xFFFE, 0xFFFF),
    (0x110BD, 0x110BD),
    (0x110CD, 0x110CD),
    (0x13430, 0x1343F),
    (0x1BCA0, 0x1BCA3),
    (0x1D173, 0x1D17A),
    (0x1FFFE, 0x1FFFF),
    (0x2FFFE, 0x2FFFF),
    (0x3FFFE, 0x3FFFF),
    (0xE0001, 0xE0001),
    (0xE0020, 0xE007F),
    (0xEFFFE, 0xEFFFF),
    (0xF0000, 0x10FFFF),
    (0x4FFFE, 0x4FFFF),
    (0x5FFFE, 0x5FFFF),
    (0x6FFFE, 0x6FFFF),
    (0x7FFFE, 0x7FFFF),
    (0xDFFFE, 0xDFFFF),
];

fn printable(point: u32) -> bool {
    if point < 0x80 {
        return (0x20..0x7F).contains(&point);
    }
    !NONPRINTABLE
        .iter()
        .any(|(low, high)| (*low..=*high).contains(&point))
}

/// `repr(str)`.
pub fn repr(points: &[u32]) -> String {
    let single = u32::from(b'\'');
    let double = u32::from(b'"');
    let quote = if points.contains(&single) && !points.contains(&double) {
        '"'
    } else {
        '\''
    };
    let mut text = String::with_capacity(points.len() + 2);
    text.push(quote);
    for point in points {
        match char::from_u32(*point) {
            Some(c) if c == quote || c == '\\' => {
                text.push('\\');
                text.push(c);
            }
            Some('\t') => text.push_str("\\t"),
            Some('\n') => text.push_str("\\n"),
            Some('\r') => text.push_str("\\r"),
            Some(c) if printable(*point) => text.push(c),
            _ if *point <= 0xFF => {
                let _ = write!(text, "\\x{point:02x}");
            }
            _ if *point <= 0xFFFF => {
                let _ = write!(text, "\\u{point:04x}");
            }
            _ => {
                let _ = write!(text, "\\U{point:08x}");
            }
        }
    }
    text.push(quote);
    text
}

pub fn repr_str(text: &str) -> String {
    repr(&text.chars().map(u32::from).collect::<Vec<_>>())
}

/// A filesystem path built as `pathlib.PurePosixPath` builds it, so its text matches Python's `str(path)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyPath {
    bytes: Vec<u8>,
}

impl PyPath {
    pub fn new(raw: &[u8]) -> Self {
        Self::default_root().join(raw)
    }

    fn default_root() -> Self {
        Self { bytes: Vec::new() }
    }

    /// `self / segment`: an absolute segment replaces the path; empty and `.` parts are dropped.
    pub fn join(&self, segment: &[u8]) -> Self {
        let (mut root, mut parts): (&[u8], Vec<&[u8]>) = self.split();
        if segment.starts_with(b"/") {
            root = if segment.starts_with(b"//") && !segment.starts_with(b"///") {
                b"//"
            } else {
                b"/"
            };
            parts.clear();
        }
        parts.extend(
            segment
                .split(|byte| *byte == b'/')
                .filter(|part| !part.is_empty() && *part != b"."),
        );
        let mut bytes = root.to_vec();
        bytes.extend(parts.join(&b'/'));
        Self { bytes }
    }

    fn split(&self) -> (&[u8], Vec<&[u8]>) {
        let root: &[u8] = if self.bytes.starts_with(b"//") {
            b"//"
        } else if self.bytes.starts_with(b"/") {
            b"/"
        } else {
            b""
        };
        let parts = self.bytes[root.len()..]
            .split(|byte| *byte == b'/')
            .filter(|part| !part.is_empty())
            .collect();
        (root, parts)
    }

    pub fn as_bytes(&self) -> &[u8] {
        if self.bytes.is_empty() {
            b"."
        } else {
            &self.bytes
        }
    }

    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(std::ffi::OsString::from_vec(self.as_bytes().to_vec()))
    }

    /// `str(path)` written to stderr.
    pub fn display(&self) -> String {
        stderr_text(self.as_bytes())
    }

    /// `repr(str(path))`, as an `OSError` message quotes its filename.
    pub fn repr(&self) -> String {
        repr(&surrogateescape(self.as_bytes()))
    }

    pub fn is_file(&self) -> bool {
        self.to_path_buf().is_file()
    }
}

/// `str(OSError)` for an error that names a file, such as `[Errno 2] No such file or directory: 'x'`.
pub fn os_error(error: &io::Error, filename: &[u8]) -> String {
    let quoted = repr(&surrogateescape(filename));
    match error.raw_os_error() {
        Some(code) => format!("[Errno {code}] {}: {quoted}", strerror(error)),
        None => format!("{error}: {quoted}"),
    }
}

fn strerror(error: &io::Error) -> String {
    let text = error.to_string();
    match text.rfind(" (os error ") {
        Some(end) => text[..end].to_string(),
        None => text,
    }
}

pub enum ReadError {
    Os(String),
    Decode(String),
}

impl ReadError {
    pub fn message(&self) -> &str {
        match self {
            Self::Os(text) | Self::Decode(text) => text,
        }
    }
}

/// `Path.read_text(encoding="utf-8")`: strict UTF-8, then universal newlines.
pub fn read_text(path: &PyPath) -> Result<String, ReadError> {
    let bytes = std::fs::read(path.to_path_buf())
        .map_err(|error| ReadError::Os(os_error(&error, path.as_bytes())))?;
    let text = decode_utf8(&bytes).map_err(ReadError::Decode)?;
    Ok(universal_newlines(&text))
}

/// `bytes.decode("utf-8")`, with the message of the `UnicodeDecodeError` it raises.
pub fn decode_utf8(bytes: &[u8]) -> Result<String, String> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(text.to_string()),
        Err(error) => {
            let start = error.valid_up_to();
            let (length, reason) = match error.error_len() {
                None => (bytes.len() - start, "unexpected end of data"),
                Some(length) if (0xC2..=0xF4).contains(&bytes[start]) => {
                    (length, "invalid continuation byte")
                }
                Some(length) => (length, "invalid start byte"),
            };
            Err(if length == 1 {
                format!(
                    "'utf-8' codec can't decode byte 0x{:02x} in position {start}: {reason}",
                    bytes[start]
                )
            } else {
                format!(
                    "'utf-8' codec can't decode bytes in position {start}-{}: {reason}",
                    start + length - 1
                )
            })
        }
    }
}

pub fn universal_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn os_bytes(value: &std::ffi::OsStr) -> &[u8] {
    value.as_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surrogate_escape_orders_invalid_bytes_by_code_point() {
        assert_eq!(surrogateescape(b"\xff"), vec![0xDCFF]);
        assert_eq!(
            surrogateescape(b"\xed\xa0\x80"),
            vec![0xDCED, 0xDCA0, 0xDC80]
        );
        assert_eq!(surrogateescape(b"\xe2\x82A"), vec![0xDCE2, 0xDC82, 0x41]);
        assert!(surrogateescape("\u{e000}".as_bytes()) > surrogateescape(b"\xff"));
        assert!(surrogateescape("\u{20ac}".as_bytes()) < surrogateescape(b"\xe2\x82"));
        assert_eq!(stderr_text(b"a\xffb"), "a\\udcffb");
    }

    #[test]
    fn repr_matches_python() {
        assert_eq!(repr_str("gemini-skill"), "'gemini-skill'");
        assert_eq!(repr_str("it's"), "\"it's\"");
        assert_eq!(repr_str("a'b\""), "'a\\'b\"'");
        assert_eq!(repr_str("t\tn\nr\r\\"), "'t\\tn\\nr\\r\\\\'");
        assert_eq!(
            repr_str("\u{7f}\u{a0}\u{200b}\u{e000}\u{1f600}é"),
            "'\\x7f\\xa0\\u200b\\ue000\u{1f600}é'"
        );
        assert_eq!(repr_str("\u{10fffd}"), "'\\U0010fffd'");
        assert_eq!(repr(&surrogateescape(b"x\xff")), "'x\\udcff'");
    }

    #[test]
    fn strip_uses_python_whitespace() {
        assert_eq!(strip("\u{1c}\u{85}\u{a0} fatal \u{3000}\n"), "fatal");
        assert_eq!(strip("\u{200b}x"), "\u{200b}x");
    }

    #[test]
    fn paths_join_like_pathlib() {
        assert_eq!(
            PyPath::new(b".").join(b"bench/roster.json").display(),
            "bench/roster.json"
        );
        assert_eq!(PyPath::new(b"a//b/./c/").join(b"/abs").display(), "/abs");
        assert_eq!(PyPath::new(b"//x/y").display(), "//x/y");
        assert_eq!(PyPath::new(b"///x").display(), "/x");
        assert_eq!(PyPath::new(b"").display(), ".");
        assert_eq!(PyPath::new(b"r").join(b"../a/./b").display(), "r/../a/b");
    }

    #[test]
    fn decode_errors_match_python() {
        let cases: [(&[u8], &str); 5] = [
            (
                b"\xff",
                "'utf-8' codec can't decode byte 0xff in position 0: invalid start byte",
            ),
            (
                b"ab\xe2\x82",
                "'utf-8' codec can't decode bytes in position 2-3: unexpected end of data",
            ),
            (
                b"\xe2A",
                "'utf-8' codec can't decode byte 0xe2 in position 0: invalid continuation byte",
            ),
            (
                b"\xe2\x82A",
                "'utf-8' codec can't decode bytes in position 0-1: invalid continuation byte",
            ),
            (
                b"\xed\xa0\x80",
                "'utf-8' codec can't decode byte 0xed in position 0: invalid continuation byte",
            ),
        ];
        for (bytes, message) in cases {
            assert_eq!(decode_utf8(bytes).err().as_deref(), Some(message));
        }
        assert_eq!(universal_newlines("a\r\nb\rc\n"), "a\nb\nc\n");
    }

    #[test]
    fn os_errors_match_python() {
        let missing = io::Error::from_raw_os_error(2);
        assert_eq!(
            os_error(&missing, b"bench/arms/x.json"),
            "[Errno 2] No such file or directory: 'bench/arms/x.json'"
        );
    }
}

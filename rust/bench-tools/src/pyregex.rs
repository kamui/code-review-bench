//! `re.search` for the pattern syntax the schemas under `bench/schema` use.
//!
//! Supported: literals, `.`, `^`, `$`, groups without `?`, alternation, character classes with ranges and
//! negation, `\d`, `\D`, `\s`, `\S`, `\w`, `\W`, `\b`, `\B`, the escapes `\n`, `\t`, `\r`, `\f`, `\v`,
//! escaped punctuation, the quantifiers `*`, `+`, `?` and `{m,n}`, greedy or lazy, with counts up to 1,000,
//! and the global flags `(?i)`, `(?m)`, `(?s)`, `(?x)` and `(?u)` at the start. Matching follows Python
//! 3.14's `str` patterns: `\d` is Unicode category Nd, `\s` is `str.isspace`, `\w` is `str.isalnum` or `_`,
//! `\B` matches an empty text, `.` excludes a newline and `$` also matches before a final newline unless a
//! flag changes them, and `(?i)` folds case with `re`'s tables. Anything else is refused rather than read
//! some other way.
//!
//! The `regex` crate matches a translation of the parsed pattern, so it answers whether a match exists, which is
//! all a schema `pattern` asks; it reports no groups or positions. A pattern whose translation exceeds the
//! crate's default compiled size limit, or nests more than 2,000 levels of the crate's syntax, is refused. The
//! recursive parser runs first, so a pattern nested near 100,000 groups still exhausts the stack.

use std::fmt::Write;
use std::sync::OnceLock;

use crate::{pytext, pyunicode};

const MAX_REPEAT: u32 = 1_000;
const NEST_LIMIT: u32 = 2_000;

#[derive(Debug, PartialEq, Eq)]
pub struct Unsupported(pub String);

#[derive(Clone, Debug)]
enum ClassItem {
    Range(char, char),
    Digit(bool),
    Space(bool),
    Word(bool),
    /// A range `(?i)` reads outside the Basic Multilingual Plane, where `re` also tries the uppercase.
    FoldedRange(char, char),
}

#[derive(Clone, Copy, Debug, Default)]
struct Flags {
    ignore_case: bool,
    multiline: bool,
    dot_all: bool,
    verbose: bool,
}

#[derive(Clone, Debug)]
enum Atom {
    Literal(char),
    Any,
    Class {
        items: Vec<ClassItem>,
        negated: bool,
        /// Whether the items test the character's lowercase, as `re` does under `(?i)`.
        lowered: bool,
    },
}

#[derive(Clone, Debug)]
enum Node {
    Atom(Atom),
    Start,
    End,
    Boundary(bool),
    Concat(Vec<Node>),
    Alternation(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: u32,
        max: Option<u32>,
    },
}

#[derive(Clone, Debug)]
pub struct Regex {
    matcher: regex::Regex,
    multiline: bool,
}

struct Parser {
    chars: Vec<char>,
    position: usize,
    flags: Flags,
}

impl Parser {
    fn refuse(&self, what: &str) -> Unsupported {
        Unsupported(format!("{what} at position {}", self.position))
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn alternation(&mut self, top: bool) -> Result<Node, Unsupported> {
        let mut branches = vec![self.sequence(top)?];
        while self.peek() == Some('|') {
            self.position += 1;
            branches.push(self.sequence(false)?);
        }
        Ok(if branches.len() == 1 {
            branches.remove(0)
        } else {
            Node::Alternation(branches)
        })
    }

    /// One branch. Global flags are read only in the pattern's first branch before any item, as Python
    /// refuses them anywhere else.
    fn sequence(&mut self, first: bool) -> Result<Node, Unsupported> {
        let mut items: Vec<Node> = Vec::new();
        // Whether the last item can take a quantifier: Python refuses one after nothing, an anchor or another
        // quantifier.
        let mut repeatable = false;
        while let Some(c) = self.peek() {
            if self.flags.verbose && " \t\n\r\u{b}\u{c}".contains(c) {
                self.position += 1;
                continue;
            }
            if self.flags.verbose && c == '#' {
                while self.peek().is_some_and(|c| c != '\n') {
                    self.position += 1;
                }
                self.position = (self.position + 1).min(self.chars.len());
                continue;
            }
            match c {
                '|' | ')' => break,
                '*' | '+' | '?' => {
                    self.position += 1;
                    let (min, max) = match c {
                        '*' => (0, None),
                        '+' => (1, None),
                        _ => (0, Some(1)),
                    };
                    self.repeat(&mut items, &mut repeatable, min, max)?;
                }
                '{' => match self.counted()? {
                    Some((min, max)) => self.repeat(&mut items, &mut repeatable, min, max)?,
                    None => {
                        self.position += 1;
                        items.push(Node::Atom(Atom::Literal('{')));
                        repeatable = true;
                    }
                },
                '(' => {
                    self.position += 1;
                    if self.peek() == Some('?') {
                        if first && items.is_empty() && self.global_flags() {
                            continue;
                        }
                        return Err(self.refuse("unsupported group syntax"));
                    }
                    let inner = self.alternation(false)?;
                    if self.peek() != Some(')') {
                        return Err(self.refuse("missing )"));
                    }
                    self.position += 1;
                    items.push(inner);
                    repeatable = true;
                }
                '^' | '$' => {
                    self.position += 1;
                    items.push(if c == '^' { Node::Start } else { Node::End });
                    repeatable = false;
                }
                '.' => {
                    self.position += 1;
                    items.push(Node::Atom(Atom::Any));
                    repeatable = true;
                }
                '[' => {
                    self.position += 1;
                    items.push(Node::Atom(self.class()?));
                    repeatable = true;
                }
                '\\' if matches!(self.chars.get(self.position + 1), Some('b' | 'B')) => {
                    items.push(Node::Boundary(self.chars[self.position + 1] == 'B'));
                    self.position += 2;
                    repeatable = false;
                }
                '\\' => {
                    self.position += 1;
                    items.push(Node::Atom(self.escape()?));
                    repeatable = true;
                }
                _ => {
                    self.position += 1;
                    items.push(Node::Atom(Atom::Literal(c)));
                    repeatable = true;
                }
            }
        }
        Ok(Node::Concat(items))
    }

    /// Reads `(?flags)` after its `(`, at the `?`, when every flag is one this matcher supports.
    fn global_flags(&mut self) -> bool {
        let mut index = self.position + 1;
        let mut flags = self.flags;
        while let Some(c) = self.chars.get(index) {
            match c {
                'i' => flags.ignore_case = true,
                'm' => flags.multiline = true,
                's' => flags.dot_all = true,
                'x' => flags.verbose = true,
                'u' => {}
                ')' if index > self.position + 1 => {
                    self.position = index + 1;
                    self.flags = flags;
                    return true;
                }
                _ => return false,
            }
            index += 1;
        }
        false
    }

    fn repeat(
        &mut self,
        items: &mut Vec<Node>,
        repeatable: &mut bool,
        min: u32,
        max: Option<u32>,
    ) -> Result<(), Unsupported> {
        if !*repeatable {
            return Err(self.refuse("nothing to repeat"));
        }
        match self.peek() {
            Some('?') => self.position += 1,
            Some('+') => return Err(self.refuse("unsupported possessive quantifier")),
            _ => {}
        }
        let node = items
            .pop()
            .ok_or_else(|| self.refuse("nothing to repeat"))?;
        items.push(Node::Repeat {
            node: Box::new(node),
            min,
            max,
        });
        *repeatable = false;
        Ok(())
    }

    /// A `{m}`, `{m,}`, `{,n}` or `{m,n}` quantifier at the current `{`, or `None` where Python reads the
    /// brace as a literal.
    fn counted(&mut self) -> Result<Option<(u32, Option<u32>)>, Unsupported> {
        let start = self.position;
        let mut index = start + 1;
        let digits = |index: &mut usize, chars: &[char]| {
            let from = *index;
            while chars.get(*index).is_some_and(char::is_ascii_digit) {
                *index += 1;
            }
            chars[from..*index].iter().collect::<String>()
        };
        if self.chars.get(index) == Some(&'}') {
            return Ok(None);
        }
        let low = digits(&mut index, &self.chars);
        let high = if self.chars.get(index) == Some(&',') {
            index += 1;
            Some(digits(&mut index, &self.chars))
        } else {
            None
        };
        if self.chars.get(index) != Some(&'}') {
            return Ok(None);
        }
        self.position = index + 1;
        let number = |text: &str| -> Result<u32, Unsupported> {
            text.parse::<u32>()
                .ok()
                .filter(|count| *count <= MAX_REPEAT)
                .ok_or_else(|| {
                    Unsupported(format!(
                        "repetition count above {MAX_REPEAT} at position {start}"
                    ))
                })
        };
        let min = if low.is_empty() { 0 } else { number(&low)? };
        let max = match &high {
            None => Some(min),
            Some(text) if text.is_empty() => None,
            Some(text) => Some(number(text)?),
        };
        if max.is_some_and(|max| max < min) {
            return Err(Unsupported(format!(
                "min repeat greater than max repeat at position {start}"
            )));
        }
        Ok(Some((min, max)))
    }

    /// The escape after a backslash. `\d`, `\s`, `\w` and their negations become one-item classes; `\b` is a
    /// backspace, as it is inside a class.
    fn escape(&mut self) -> Result<Atom, Unsupported> {
        let Some(c) = self.peek() else {
            return Err(self.refuse("bad escape (end of pattern)"));
        };
        self.position += 1;
        let item = match c {
            'd' => ClassItem::Digit(false),
            'D' => ClassItem::Digit(true),
            's' => ClassItem::Space(false),
            'S' => ClassItem::Space(true),
            'w' => ClassItem::Word(false),
            'W' => ClassItem::Word(true),
            'b' => return Ok(Atom::Literal('\u{8}')),
            'n' => return Ok(Atom::Literal('\n')),
            't' => return Ok(Atom::Literal('\t')),
            'r' => return Ok(Atom::Literal('\r')),
            'f' => return Ok(Atom::Literal('\u{c}')),
            'v' => return Ok(Atom::Literal('\u{b}')),
            c if c.is_ascii_alphanumeric() => {
                self.position -= 1;
                return Err(self.refuse(&format!("unsupported escape \\{c}")));
            }
            c => return Ok(Atom::Literal(c)),
        };
        Ok(Atom::Class {
            items: vec![item],
            negated: false,
            lowered: false,
        })
    }

    fn class(&mut self) -> Result<Atom, Unsupported> {
        let negated = self.peek() == Some('^');
        if negated {
            self.position += 1;
        }
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let Some(c) = self.peek() else {
                return Err(self.refuse("unterminated character set"));
            };
            if c == ']' && !first {
                self.position += 1;
                break;
            }
            first = false;
            let low = self.class_member()?;
            let ranged = self.peek() == Some('-')
                && self
                    .chars
                    .get(self.position + 1)
                    .is_some_and(|next| *next != ']');
            if !ranged {
                items.push(low);
                continue;
            }
            self.position += 1;
            let high = self.class_member()?;
            match (low, high) {
                (ClassItem::Range(low, _), ClassItem::Range(high, _)) if low <= high => {
                    items.push(ClassItem::Range(low, high));
                }
                (ClassItem::Range(..), ClassItem::Range(..)) => {
                    return Err(self.refuse("bad character range"));
                }
                _ => return Err(self.refuse("bad character range")),
            }
        }
        Ok(Atom::Class {
            items,
            negated,
            lowered: false,
        })
    }

    fn class_member(&mut self) -> Result<ClassItem, Unsupported> {
        let c = self
            .peek()
            .ok_or_else(|| self.refuse("unterminated character set"))?;
        self.position += 1;
        if c != '\\' {
            return Ok(ClassItem::Range(c, c));
        }
        match self.escape()? {
            Atom::Literal(c) => Ok(ClassItem::Range(c, c)),
            Atom::Class { mut items, .. } => Ok(items.remove(0)),
            Atom::Any => Err(self.refuse("unsupported escape")),
        }
    }
}

const BMP_END: u32 = 0xFFFF;

/// `re`'s `(?i)` form of an atom: a cased literal or a class with a cased member tests the character's
/// lowercase against the lowercases of its members and their [`pyunicode::case_fixes`].
fn fold(atom: &Atom) -> Atom {
    let members = match atom {
        Atom::Literal(c) if pyunicode::is_cased(*c) => vec![ClassItem::Range(*c, *c)],
        Atom::Class {
            items,
            lowered: false,
            ..
        } if items.iter().any(|item| match item {
            ClassItem::Range(low, high) => {
                u32::from(*high) > BMP_END || (*low..=*high).any(pyunicode::is_cased)
            }
            _ => false,
        }) =>
        {
            items.clone()
        }
        _ => return atom.clone(),
    };
    let mut basic = vec![false; BMP_END as usize + 1];
    let mut mark = |c: char| {
        let lower = pyunicode::to_lower(c);
        basic[u32::from(lower) as usize] = true;
        for fix in pyunicode::case_fixes(lower) {
            basic[*fix as usize] = true;
        }
    };
    let mut items = Vec::new();
    for item in members {
        match item {
            ClassItem::Range(low, high) => {
                (low..=high)
                    .take_while(|c| u32::from(*c) <= BMP_END)
                    .for_each(&mut mark);
                if u32::from(high) > BMP_END {
                    items.push(ClassItem::FoldedRange(low, high));
                }
            }
            other => items.push(other),
        }
    }
    let mut start = None;
    for (point, marked) in basic.iter().chain([&false]).enumerate() {
        match (marked, start) {
            (true, None) => start = Some(point),
            (false, Some(first)) => {
                let range = |point: usize| {
                    u32::try_from(point)
                        .ok()
                        .and_then(char::from_u32)
                        .unwrap_or('\0')
                };
                items.push(ClassItem::Range(range(first), range(point - 1)));
                start = None;
            }
            _ => {}
        }
    }
    Atom::Class {
        items,
        negated: matches!(atom, Atom::Class { negated: true, .. }),
        lowered: true,
    }
}

/// Matches one encoded character's marker; see [`Regex::search`].
const MARK: &str = r"[\n a]";

fn write_char(out: &mut String, c: char) {
    let _ = write!(out, "\\x{{{:X}}}", u32::from(c));
}

fn write_range(out: &mut String, low: char, high: char) {
    write_char(out, low);
    if high != low {
        out.push('-');
        write_char(out, high);
    }
}

/// Ascending characters as the ranges of a class body.
fn ranges(chars: impl IntoIterator<Item = char>) -> String {
    let mut out = String::new();
    let mut run: Option<(char, char)> = None;
    for c in chars {
        run = match run {
            Some((low, high)) if u32::from(high) + 1 == u32::from(c) => Some((low, c)),
            Some((low, high)) => {
                write_range(&mut out, low, high);
                Some((c, c))
            }
            None => Some((c, c)),
        };
    }
    if let Some((low, high)) = run {
        write_range(&mut out, low, high);
    }
    out
}

/// The class body of `pytext`'s table behind `\d`, `\s` or `\w`, built once from the table itself so the crate's
/// own Unicode tables, whose version and definitions differ from Python's, are never consulted.
fn table(cell: &'static OnceLock<String>, test: fn(char) -> bool) -> &'static str {
    cell.get_or_init(|| ranges(('\0'..=char::MAX).filter(|c| test(*c))))
}

static DIGITS: OnceLock<String> = OnceLock::new();
static SPACES: OnceLock<String> = OnceLock::new();
static WORDS: OnceLock<String> = OnceLock::new();
static CASE_CHANGING: OnceLock<(String, Vec<char>)> = OnceLock::new();

/// The characters a `(?i)` atom can treat differently from its plain form: those `to_lower` or
/// `to_upper_astral` changes. Every other character is its own lowercase, so the plain class decides it.
fn case_changing() -> &'static (String, Vec<char>) {
    CASE_CHANGING.get_or_init(|| {
        let changes = |c: char| pyunicode::to_lower(c) != c || pyunicode::to_upper_astral(c) != c;
        (
            ranges(('\0'..=char::MAX).filter(|c| changes(*c))),
            ('\0'..=char::MAX).filter(|c| changes(*c)).collect(),
        )
    })
}

fn write_item(out: &mut String, item: &ClassItem) {
    let (body, negated) = match item {
        ClassItem::Range(low, high) | ClassItem::FoldedRange(low, high) => {
            return write_range(out, *low, *high);
        }
        ClassItem::Digit(negated) => (
            DIGITS
                .get_or_init(|| ranges(pytext::decimal_digits()))
                .as_str(),
            negated,
        ),
        ClassItem::Space(negated) => (table(&SPACES, pytext::is_space), negated),
        ClassItem::Word(negated) => (table(&WORDS, pytext::is_word), negated),
    };
    out.push_str(if *negated { "[^" } else { "[" });
    out.push_str(body);
    out.push(']');
}

/// The crate class matching the characters [`Atom::matches`] accepts.
fn write_atom(out: &mut String, atom: &Atom) {
    match atom {
        Atom::Literal(c) => write_char(out, *c),
        Atom::Any => out.push_str(r"[^\n]"),
        Atom::Class {
            items,
            negated,
            lowered: false,
        } => {
            out.push_str(if *negated { "[^" } else { "[" });
            for item in items {
                write_item(out, item);
            }
            out.push(']');
        }
        Atom::Class {
            items,
            negated,
            lowered: true,
        } => {
            let (changing, changing_chars) = case_changing();
            out.push_str("[[");
            write_atom(
                out,
                &Atom::Class {
                    items: items.clone(),
                    negated: *negated,
                    lowered: false,
                },
            );
            let _ = write!(out, "--[{changing}]]");
            for c in changing_chars.iter().filter(|c| atom.matches(**c)) {
                write_char(out, *c);
            }
            out.push(']');
        }
    }
}

fn write_node(out: &mut String, node: &Node, flags: Flags) {
    match node {
        Node::Atom(atom) => {
            out.push_str(MARK);
            match atom {
                Atom::Any if flags.dot_all => out.push_str("(?s:.)"),
                atom if flags.ignore_case => write_atom(out, &fold(atom)),
                atom => write_atom(out, atom),
            }
            out.push_str(MARK);
        }
        Node::Start if flags.multiline => out.push_str("(?m:^)"),
        Node::Start => out.push_str(r"\A"),
        Node::End => out.push_str("(?m:$)"),
        Node::Boundary(false) => out.push_str(r"(?-u:\b)"),
        Node::Boundary(true) => out.push_str(r"(?-u:\B)"),
        Node::Concat(items) => {
            for item in items {
                write_node(out, item, flags);
            }
        }
        Node::Alternation(branches) => {
            out.push_str("(?:");
            for (index, branch) in branches.iter().enumerate() {
                if index > 0 {
                    out.push('|');
                }
                write_node(out, branch, flags);
            }
            out.push(')');
        }
        Node::Repeat { node, min, max } => {
            out.push_str("(?:");
            write_node(out, node, flags);
            match max {
                Some(max) => {
                    let _ = write!(out, "){{{min},{max}}}");
                }
                None => {
                    let _ = write!(out, "){{{min},}}");
                }
            }
        }
    }
}

impl ClassItem {
    fn matches(&self, c: char) -> bool {
        match self {
            Self::Range(low, high) => (*low..=*high).contains(&c),
            Self::Digit(negated) => pytext::decimal_value(c).is_some() != *negated,
            Self::Space(negated) => pytext::is_space(c) != *negated,
            Self::Word(negated) => pytext::is_word(c) != *negated,
            Self::FoldedRange(low, high) => {
                (*low..=*high).contains(&c)
                    || (*low..=*high).contains(&pyunicode::to_upper_astral(c))
            }
        }
    }
}

impl Atom {
    fn matches(&self, c: char) -> bool {
        match self {
            Self::Literal(literal) => *literal == c,
            Self::Any => c != '\n',
            Self::Class {
                items,
                negated,
                lowered,
            } => {
                let c = if *lowered { pyunicode::to_lower(c) } else { c };
                items.iter().any(|item| item.matches(c)) != *negated
            }
        }
    }
}

pub fn compile(pattern: &str) -> Result<Regex, Unsupported> {
    compile_with(pattern, false)
}

/// `re.compile(pattern, re.IGNORECASE)`.
pub fn compile_ignore_case(pattern: &str) -> Result<Regex, Unsupported> {
    compile_with(pattern, true)
}

fn compile_with(pattern: &str, ignore_case: bool) -> Result<Regex, Unsupported> {
    let mut parser = Parser {
        chars: pattern.chars().collect(),
        position: 0,
        flags: Flags {
            ignore_case,
            ..Flags::default()
        },
    };
    let node = parser.alternation(true)?;
    if parser.position != parser.chars.len() {
        return Err(parser.refuse("unbalanced parenthesis"));
    }
    let flags = parser.flags;
    let mut syntax = format!(r"\A(?:{MARK}(?s:.){MARK})*?(?:");
    write_node(&mut syntax, &node, flags);
    syntax.push(')');
    // A repeated group whose alternation branch is a concatenation costs five levels of this syntax, and Python
    // reaches its recursion limit past 331 of them, 1,659 levels. The crate's compiler stays within a 1 MiB thread
    // stack up to 2,000.
    let matcher = regex::RegexBuilder::new(&syntax)
        .nest_limit(NEST_LIMIT)
        .build()
        .map_err(|error| match error {
            regex::Error::CompiledTooBig(_) => Unsupported("pattern too large".to_string()),
            _ => Unsupported("pattern nested too deeply".to_string()),
        })?;
    Ok(Regex {
        matcher,
        multiline: flags.multiline,
    })
}

impl Regex {
    /// Whether `re.search(pattern, text)` finds a match.
    ///
    /// The crate has no lookaround and its own word characters, so the text is encoded to let its ASCII
    /// assertions decide Python's: every character is wrapped in two ASCII markers, `a` beside a Python word
    /// character, `\n` beside a newline that ends a line for `^` and `$`, and a space otherwise. Between two
    /// characters the crate's ASCII `\b` then sees exactly Python's word boundary, and `(?m:^)` and `(?m:$)`
    /// see Python's line ends: every newline under `(?m)`, otherwise only a final one, which `$` stops before.
    /// The pattern consumes whole wrapped characters from the start, so it is only tried between them.
    pub fn search(&self, text: &str) -> bool {
        let mut encoded = String::with_capacity(text.len() * 3);
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            let word = pytext::is_word(c);
            let line_end = c == '\n' && (self.multiline || chars.peek().is_none());
            encoded.push(if word {
                'a'
            } else if line_end {
                '\n'
            } else {
                ' '
            });
            encoded.push(c);
            encoded.push(if word {
                'a'
            } else if c == '\n' && self.multiline {
                '\n'
            } else {
                ' '
            });
        }
        self.matcher.is_match(&encoded)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(pattern: &str, text: &str) -> bool {
        compile(pattern).unwrap().search(text)
    }

    #[test]
    fn schema_patterns_match_as_python_does() {
        let hex = "^[a-f0-9]{64}$";
        let digest = "a".repeat(64);
        assert!(search(hex, &digest));
        assert!(search(hex, &format!("{digest}\n")));
        assert!(!search(hex, &format!("{digest}\n\n")));
        assert!(!search(hex, &format!("{digest}0")));
        assert!(!search(hex, &"A".repeat(64)));
        assert!(search(r"\S", " x "));
        assert!(!search(r"\S", " \t\u{3000}\u{85}"));
        assert!(search(".+", "x"));
        assert!(!search(".+", "\n"));
        assert!(search(r"^\d{4}-\d{2}-\d{2}$", "2026-01-01"));
        assert!(search(
            r"^\d{4}-\d{2}-\d{2}$",
            "\u{0662}\u{0660}\u{0662}\u{0666}-01-01"
        ));
        assert!(!search(r"^\d{4}-\d{2}-\d{2}$", "2026-1-01"));
        assert!(search("^att-[0-9]{3,}$", "att-0001"));
        assert!(!search("^att-[0-9]{3,}$", "att-01"));
        assert!(!search("^[0-9]$", "\u{0663}"));
        assert!(search(
            "^(fresh|resumed from att-[0-9]{3,})$",
            "resumed from att-123"
        ));
        assert!(!search("^(fresh|resumed from att-[0-9]{3,})$", "freshly"));
        assert!(search("^(/[^/]+)+$", "/a/b"));
        assert!(!search("^(/[^/]+)+$", "/a//b"));
        assert!(search(r"^[^/\s]+/[^/\s]+$", "owner/repo"));
        assert!(!search(r"^[^/\s]+/[^/\s]+$", "owner/re po"));
        assert!(search("^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$", "a.b-c/d_e"));
        assert!(search(r"\.enc$", "x.json.enc"));
        assert!(!search(r"\.enc$", "x.jsonXenc"));
        assert!(search("^GT-[a-z0-9]+-?[0-9]+$", "GT-abc12"));
        assert!(search(
            r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})$",
            "2026-01-01T00:00:00.5+05:30"
        ));
        assert!(search("^all$", "all"));
        assert!(search("a{", "a{"));
        assert!(search("a{x}", "a{x}"));
        assert!(search("^a{,2}$", "aa"));
        assert!(!search("^a{,2}$", "aaa"));
        assert!(search("^[]a]$", "]"));
        assert!(search("^(a|)$", ""));
        assert!(search("^(a*)*$", "aaa"));
        assert!(search("^a+?$", "aa"));
    }

    #[test]
    fn word_classes_and_boundaries_match_as_python_does() {
        assert!(search(r"^\w+$", "abc_1\u{e9}\u{661}\u{b2}"));
        assert!(!search(r"^\w+$", "a-b"));
        assert!(search(r"^\W$", "-"));
        assert!(!search(r"\W", "x"));
        assert!(search(r"^[\w-]+$", "a-b"));
        assert!(search(r"\bcat\b", "a cat."));
        assert!(!search(r"\bcat\b", "scatter"));
        assert!(search(r"a\Bb", "ab"));
        assert!(!search(r"a\Bb", "a b"));
        assert!(search(r"\B", ""));
        assert!(!search(r"\b", ""));
        assert!(search(r"^[\b]$", "\u{8}"));
    }

    #[test]
    fn global_flags_match_as_python_does() {
        assert!(search("(?i)x", "X"));
        assert!(search("(?i)^[a-z]+$", "AbC"));
        assert!(search("(?i)k", "\u{212a}"));
        assert!(search("(?i)\u{131}", "I"));
        assert!(search("(?i)[^k]", "x"));
        assert!(!search("(?i)[^k]", "\u{212a}"));
        assert!(search("(?i)\u{10400}", "\u{10428}"));
        assert!(search("(?i)[\u{10400}-\u{10410}]", "\u{10428}"));
        assert!(!search("(?i)1", "x"));
        assert!(search("(?m)^b$", "a\nb\nc"));
        assert!(!search("^b$", "a\nb\nc"));
        assert!(search("(?s)a.b", "a\nb"));
        assert!(!search("a.b", "a\nb"));
        assert!(search("(?x) a  b # comment\n c", "abc"));
        assert!(search("(?x)a\\ b", "a b"));
        assert!(search("(?x)[ ]", " "));
        assert!(search("(?x)a *$", "aaa"));
        assert!(search("(?i)(?m)^A", "x\na"));
        assert!(search("(?ui)A", "a"));
    }

    #[test]
    fn anchors_around_newlines_match_as_python_does() {
        assert!(search("a$\n", "a\n"));
        assert!(search("^$", "\n"));
        assert!(!search("^$", "\n\n"));
        assert!(!search("a$", "a\n\n"));
        assert!(search("$\n$", "\n"));
        assert!(!search("a$b", "a\nb"));
        assert!(search("(?m)a$\nb", "a\nb"));
        assert!(search("(?m)^$", "a\n"));
        assert!(search("(?m)\n^", "\n"));
        assert!(!search("^\n", ""));
        assert!(search(r"\bb", "a\nb"));
        assert!(search("(?s).$", "\n"));
    }

    #[test]
    fn digit_class_lists_every_decimal_digit() {
        assert_eq!(
            ranges(pytext::decimal_digits()),
            ranges(('\0'..=char::MAX).filter(|c| pytext::decimal_value(*c).is_some()))
        );
    }

    #[test]
    fn translations_beyond_the_crate_limits_are_refused() {
        let message = |pattern: &str| compile(pattern).err().map(|Unsupported(reason)| reason);
        assert!(compile(r"^\w{504}$").is_ok());
        assert_eq!(message(r"^\w{1000}$").as_deref(), Some("pattern too large"));
        let nested = |depth: usize| format!("{}a{}", "(".repeat(depth), "|b)*".repeat(depth));
        // An unoptimized build's frames are several times a release build's.
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || {
                assert!(compile(&nested(498)).is_ok());
                assert_eq!(
                    message(&nested(499)).as_deref(),
                    Some("pattern nested too deeply")
                );
                let concatenated =
                    |depth: usize| format!("{}a{}", "(a|".repeat(depth), "b)*".repeat(depth));
                assert!(compile(&concatenated(399)).is_ok());
                assert_eq!(
                    message(&concatenated(400)).as_deref(),
                    Some("pattern nested too deeply")
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn unsupported_syntax_is_refused() {
        for pattern in [
            r"\A", r"\Z", r"\1", "*a", "a**", "a*+", "^*", r"\b*", "(a", "a)", "[a", r"[\B]",
            "[z-a]", r"[a-\d]", "a{2000}", "a{3,1}", r"a\", "(?=a)", "(?<=a)b", "(?:a)",
            "(?P<n>a)", "(?i:a)", "(?a)a", "(?L)a", "a(?i)", "a|(?i)b", "((?i)a)", " (?i)a",
            "(?i)*",
        ] {
            assert!(compile(pattern).is_err(), "{pattern:?}");
        }
    }
}

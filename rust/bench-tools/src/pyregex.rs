//! `re.search` for the pattern syntax the schemas under `bench/schema` use.
//!
//! Supported: literals, `.`, `^`, `$`, groups without `?` flags, alternation, character classes with ranges
//! and negation, `\d`, `\D`, `\s`, `\S`, the escapes `\n`, `\t`, `\r`, `\f`, `\v`, escaped punctuation, and
//! the quantifiers `*`, `+`, `?` and `{m,n}`, greedy or lazy, with counts up to 1,000. Matching follows
//! Python's `str` patterns: `\d` is Unicode category Nd, `\s` is `str.isspace`, `.` excludes a newline and
//! `$` also matches before a final newline. Anything else is refused rather than read some other way.
//!
//! The matcher simulates the pattern's automaton, so it answers whether a match exists, which is all a
//! schema `pattern` asks; it reports no groups or positions.

use crate::{pytext, pyunicode};

const MAX_REPEAT: u32 = 1_000;
const MAX_PROGRAM: usize = 100_000;

#[derive(Debug, PartialEq, Eq)]
pub struct Unsupported(pub String);

#[derive(Clone, Debug)]
enum ClassItem {
    Range(char, char),
    Digit(bool),
    Space(bool),
}

#[derive(Clone, Debug)]
enum Atom {
    Literal(char),
    Any,
    Class {
        items: Vec<ClassItem>,
        negated: bool,
    },
}

#[derive(Clone, Debug)]
enum Node {
    Atom(Atom),
    Start,
    End,
    Concat(Vec<Node>),
    Alternation(Vec<Node>),
    Repeat {
        node: Box<Node>,
        min: u32,
        max: Option<u32>,
    },
}

#[derive(Clone, Debug)]
enum Inst {
    Atom(Atom),
    Split(usize, usize),
    Jump(usize),
    Start,
    End,
    Match,
}

#[derive(Clone, Debug)]
pub struct Regex {
    program: Vec<Inst>,
}

struct Parser {
    chars: Vec<char>,
    position: usize,
}

impl Parser {
    fn refuse(&self, what: &str) -> Unsupported {
        Unsupported(format!("{what} at position {}", self.position))
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.position).copied()
    }

    fn alternation(&mut self) -> Result<Node, Unsupported> {
        let mut branches = vec![self.sequence()?];
        while self.peek() == Some('|') {
            self.position += 1;
            branches.push(self.sequence()?);
        }
        Ok(if branches.len() == 1 {
            branches.remove(0)
        } else {
            Node::Alternation(branches)
        })
    }

    fn sequence(&mut self) -> Result<Node, Unsupported> {
        let mut items: Vec<Node> = Vec::new();
        // Whether the last item can take a quantifier: Python refuses one after nothing, an anchor or another
        // quantifier.
        let mut repeatable = false;
        while let Some(c) = self.peek() {
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
                        return Err(self.refuse("unsupported group syntax"));
                    }
                    let inner = self.alternation()?;
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

    /// The escape after a backslash. `\d`, `\s` and their negations become one-item classes.
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
        Ok(Atom::Class { items, negated })
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

fn emit(node: &Node, program: &mut Vec<Inst>) -> Result<(), Unsupported> {
    if program.len() > MAX_PROGRAM {
        return Err(Unsupported("pattern too large".to_string()));
    }
    match node {
        Node::Atom(atom) => program.push(Inst::Atom(atom.clone())),
        Node::Start => program.push(Inst::Start),
        Node::End => program.push(Inst::End),
        Node::Concat(items) => {
            for item in items {
                emit(item, program)?;
            }
        }
        Node::Alternation(branches) => {
            let mut jumps = Vec::new();
            for (index, branch) in branches.iter().enumerate() {
                if index + 1 == branches.len() {
                    emit(branch, program)?;
                } else {
                    let split = program.len();
                    program.push(Inst::Split(split + 1, 0));
                    emit(branch, program)?;
                    jumps.push(program.len());
                    program.push(Inst::Jump(0));
                    let next = program.len();
                    program[split] = Inst::Split(split + 1, next);
                }
            }
            let end = program.len();
            for jump in jumps {
                program[jump] = Inst::Jump(end);
            }
        }
        Node::Repeat { node, min, max } => {
            for _ in 0..*min {
                emit(node, program)?;
            }
            match max {
                None => {
                    let split = program.len();
                    program.push(Inst::Split(split + 1, 0));
                    emit(node, program)?;
                    program.push(Inst::Jump(split));
                    let end = program.len();
                    program[split] = Inst::Split(split + 1, end);
                }
                Some(max) => {
                    let mut splits = Vec::new();
                    for _ in *min..*max {
                        splits.push(program.len());
                        program.push(Inst::Split(0, 0));
                        emit(node, program)?;
                    }
                    let end = program.len();
                    for split in splits {
                        program[split] = Inst::Split(split + 1, end);
                    }
                }
            }
        }
    }
    Ok(())
}

impl ClassItem {
    fn matches(&self, c: char) -> bool {
        match self {
            Self::Range(low, high) => (*low..=*high).contains(&c),
            Self::Digit(negated) => pyunicode::decimal(c).is_some() != *negated,
            Self::Space(negated) => pytext::is_space(c) != *negated,
        }
    }
}

impl Atom {
    fn matches(&self, c: char) -> bool {
        match self {
            Self::Literal(literal) => *literal == c,
            Self::Any => c != '\n',
            Self::Class { items, negated } => items.iter().any(|item| item.matches(c)) != *negated,
        }
    }
}

pub fn compile(pattern: &str) -> Result<Regex, Unsupported> {
    let mut parser = Parser {
        chars: pattern.chars().collect(),
        position: 0,
    };
    let node = parser.alternation()?;
    if parser.position != parser.chars.len() {
        return Err(parser.refuse("unbalanced parenthesis"));
    }
    let mut program = Vec::new();
    emit(&node, &mut program)?;
    program.push(Inst::Match);
    Ok(Regex { program })
}

impl Regex {
    /// Whether `re.search(pattern, text)` finds a match.
    pub fn search(&self, text: &str) -> bool {
        let chars: Vec<char> = text.chars().collect();
        let size = self.program.len();
        let mut current: Vec<usize> = Vec::new();
        let mut seen = vec![usize::MAX; size];
        for position in 0..=chars.len() {
            if self.add(0, position, &chars, &mut current, &mut seen) {
                return true;
            }
            let Some(c) = chars.get(position) else {
                break;
            };
            let mut next = Vec::new();
            for pc in std::mem::take(&mut current) {
                if let Inst::Atom(atom) = &self.program[pc]
                    && atom.matches(*c)
                    && self.add(pc + 1, position + 1, &chars, &mut next, &mut seen)
                {
                    return true;
                }
            }
            current = next;
        }
        false
    }

    /// Follows the instructions that consume nothing from `pc`, queueing those that consume a character.
    /// Returns whether a match is reached.
    fn add(
        &self,
        pc: usize,
        position: usize,
        chars: &[char],
        list: &mut Vec<usize>,
        seen: &mut [usize],
    ) -> bool {
        let mut stack = vec![pc];
        while let Some(pc) = stack.pop() {
            if seen[pc] == position {
                continue;
            }
            seen[pc] = position;
            match &self.program[pc] {
                Inst::Atom(_) => list.push(pc),
                Inst::Split(first, second) => {
                    stack.push(*second);
                    stack.push(*first);
                }
                Inst::Jump(target) => stack.push(*target),
                Inst::Start if position == 0 => stack.push(pc + 1),
                Inst::End
                    if position == chars.len()
                        || (position + 1 == chars.len() && chars[position] == '\n') =>
                {
                    stack.push(pc + 1);
                }
                Inst::Start | Inst::End => {}
                Inst::Match => return true,
            }
        }
        false
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
    fn unsupported_syntax_is_refused() {
        for pattern in [
            "(?i)a", r"\w", r"\b", r"\A", r"\1", "*a", "a**", "a*+", "^*", "(a", "a)", "[a",
            r"[\w]", "[z-a]", r"[a-\d]", "a{2000}", "a{3,1}", r"a\",
        ] {
            assert!(compile(pattern).is_err(), "{pattern:?}");
        }
    }
}

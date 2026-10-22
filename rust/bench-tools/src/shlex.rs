//! POSIX shell words, as Python's `shlex` quotes, joins and splits them.

fn safe(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c)
}

pub fn quote(word: &str) -> String {
    if word.is_empty() {
        return "''".to_string();
    }
    if word.chars().all(safe) {
        return word.to_string();
    }
    format!("'{}'", word.replace('\'', "'\"'\"'"))
}

pub fn join<S: AsRef<str>>(words: &[S]) -> String {
    words
        .iter()
        .map(|word| quote(word.as_ref()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `ValueError` `shlex` raises on unfinished input.
#[derive(Debug, PartialEq, Eq)]
pub enum LexError {
    NoClosingQuotation,
    NoEscapedCharacter,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NoClosingQuotation => "No closing quotation",
            Self::NoEscapedCharacter => "No escaped character",
        })
    }
}

/// `shlex.split(text)`.
pub fn split(text: &str) -> Result<Vec<String>, LexError> {
    words(text, true, "")
}

/// `shlex.split(text, comments=True)`: a `#` outside quotes drops the rest of its line.
pub fn split_commented(text: &str) -> Result<Vec<String>, LexError> {
    let mut kept = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut comment = false;
    for c in text.chars() {
        if comment {
            if c == '\n' {
                comment = false;
                kept.push(c);
            }
            continue;
        }
        if escaped {
            escaped = false;
        } else if c == '\\' && quote != Some('\'') {
            escaped = true;
        } else if let Some(open) = quote {
            if c == open {
                quote = None;
            }
        } else if is_quote(c) {
            quote = Some(c);
        } else if c == '#' {
            comment = true;
            kept.push(' ');
            continue;
        }
        kept.push(c);
    }
    split(&kept)
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Space,
    Word,
    Punctuation,
    Quote(char),
    /// After a backslash; holds the state the escaped character returns to.
    Escape(Option<char>),
}

fn is_space(c: char) -> bool {
    " \t\r\n".contains(c)
}

fn is_quote(c: char) -> bool {
    c == '\'' || c == '"'
}

/// The tokens of `shlex.shlex(text, posix=posix, punctuation_chars=punctuation)` with `whitespace_split = True`
/// and `commenters = ""`. Outside POSIX mode quotes stay in the tokens and a backslash is an ordinary character.
pub fn words(text: &str, posix: bool, punctuation: &str) -> Result<Vec<String>, LexError> {
    let is_punctuation = |c: char| punctuation.contains(c);
    let mut chars = text.chars();
    let mut pushed: Option<char> = None;
    let mut tokens = Vec::new();
    loop {
        let mut state = State::Space;
        let mut token = String::new();
        let mut quoted = false;
        loop {
            let next = pushed.take().or_else(|| chars.next());
            match state {
                State::Space => match next {
                    None => break,
                    Some(c) if is_space(c) => {
                        if !token.is_empty() || (posix && quoted) {
                            break;
                        }
                    }
                    Some('\\') if posix => state = State::Escape(None),
                    Some(c) if is_punctuation(c) => {
                        token.push(c);
                        state = State::Punctuation;
                    }
                    Some(c) if is_quote(c) => {
                        if !posix {
                            token.push(c);
                        }
                        state = State::Quote(c);
                    }
                    Some(c) => {
                        token.push(c);
                        state = State::Word;
                    }
                },
                State::Quote(quote) => {
                    quoted = true;
                    match next {
                        None => return Err(LexError::NoClosingQuotation),
                        Some(c) if c == quote => {
                            if !posix {
                                token.push(c);
                                break;
                            }
                            state = State::Word;
                        }
                        Some('\\') if posix && quote == '"' => state = State::Escape(Some(quote)),
                        Some(c) => token.push(c),
                    }
                }
                State::Escape(within) => {
                    let Some(c) = next else {
                        return Err(LexError::NoEscapedCharacter);
                    };
                    if within.is_some_and(|quote| c != '\\' && c != quote) {
                        token.push('\\');
                    }
                    token.push(c);
                    state = within.map_or(State::Word, State::Quote);
                }
                State::Word | State::Punctuation => match next {
                    None => break,
                    Some(c) if is_space(c) => {
                        state = State::Space;
                        if !token.is_empty() || (posix && quoted) {
                            break;
                        }
                    }
                    Some(c) if state == State::Punctuation => {
                        if is_punctuation(c) {
                            token.push(c);
                        } else {
                            pushed = Some(c);
                            break;
                        }
                    }
                    Some(c) if posix && is_quote(c) => state = State::Quote(c),
                    Some('\\') if posix => state = State::Escape(None),
                    Some(c) if !is_punctuation(c) => token.push(c),
                    Some(c) => {
                        pushed = Some(c);
                        state = State::Space;
                        if !token.is_empty() || (posix && quoted) {
                            break;
                        }
                    }
                },
            }
        }
        if token.is_empty() && !(posix && quoted) {
            return Ok(tokens);
        }
        tokens.push(token);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_only_what_the_shell_would_split() {
        assert_eq!(quote(""), "''");
        assert_eq!(quote("/usr/bin/a-b_c.d"), "/usr/bin/a-b_c.d");
        assert_eq!(quote("it's here"), "'it'\"'\"'s here'");
        assert_eq!(join(&["a b", "c"]), "'a b' c");
    }

    #[test]
    fn splits_as_posix_shlex() {
        assert_eq!(
            split(r#"a 'b c' "d\"e" f\ g"#).unwrap(),
            ["a", "b c", "d\"e", "f g"]
        );
        assert_eq!(split(r#"x'' '' "\$a\q""#).unwrap(), ["x", "", r"\$a\q"]);
        assert_eq!(split("  a#b  ").unwrap(), ["a#b"]);
        assert_eq!(split(" \t\n").unwrap(), [] as [String; 0]);
        assert_eq!(split("'open"), Err(LexError::NoClosingQuotation));
        assert_eq!(split("end\\"), Err(LexError::NoEscapedCharacter));
        assert_eq!(
            LexError::NoClosingQuotation.to_string(),
            "No closing quotation"
        );
    }

    #[test]
    fn splits_punctuation_into_its_own_tokens() {
        assert_eq!(
            words("cd a&&ls 'x;y'|wc ;; b", true, ";&|").unwrap(),
            ["cd", "a", "&&", "ls", "x;y", "|", "wc", ";;", "b"]
        );
        assert_eq!(
            words(r#"cat "a b"c 'd'|x"#, false, ";&|").unwrap(),
            ["cat", "\"a b\"", "c", "'d'", "|", "x"]
        );
        assert_eq!(words(r#"a"b c""#, false, ";&|").unwrap(), ["a\"b", "c\""]);
        assert_eq!(words(r"\$x", false, "").unwrap(), [r"\$x"]);
    }
}

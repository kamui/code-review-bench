//! POSIX shell quoting, as Python's `shlex.quote` and `shlex.join` write it.

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
}

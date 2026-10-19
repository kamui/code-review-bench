//! Python's decimal digits and `int(str)`.
//!
//! `re`'s `\d` and `int()` accept every character of Unicode category Nd. These are the Unicode 16.0 digits
//! of Python 3.14; Python 3.12 uses Unicode 15.0, which lacks the digit blocks 16.0 added.

/// The zero of each run of ten decimal digits.
const DIGIT_ZEROS: [u32; 76] = [
    0x30, 0x660, 0x6F0, 0x7C0, 0x966, 0x9E6, 0xA66, 0xAE6, 0xB66, 0xBE6, 0xC66, 0xCE6, 0xD66,
    0xDE6, 0xE50, 0xED0, 0xF20, 0x1040, 0x1090, 0x17E0, 0x1810, 0x1946, 0x19D0, 0x1A80, 0x1A90,
    0x1B50, 0x1BB0, 0x1C40, 0x1C50, 0xA620, 0xA8D0, 0xA900, 0xA9D0, 0xA9F0, 0xAA50, 0xABF0, 0xFF10,
    0x104A0, 0x10D30, 0x10D40, 0x11066, 0x110F0, 0x11136, 0x111D0, 0x112F0, 0x11450, 0x114D0,
    0x11650, 0x116C0, 0x116D0, 0x116DA, 0x11730, 0x118E0, 0x11950, 0x11BF0, 0x11C50, 0x11D50,
    0x11DA0, 0x11F50, 0x16130, 0x16A60, 0x16AC0, 0x16B50, 0x16D70, 0x1CCF0, 0x1D7CE, 0x1D7D8,
    0x1D7E2, 0x1D7EC, 0x1D7F6, 0x1E140, 0x1E2F0, 0x1E4F0, 0x1E5F1, 0x1E950, 0x1FBF0,
];

/// The value of a decimal digit, as `unicodedata.decimal` gives it.
pub fn decimal(c: char) -> Option<u32> {
    let point = u32::from(c);
    let index = DIGIT_ZEROS.partition_point(|zero| *zero <= point);
    let zero = *DIGIT_ZEROS.get(index.checked_sub(1)?)?;
    (point - zero < 10).then_some(point - zero)
}

const INT_MAX_STR_DIGITS: usize = 4300;

/// `int(text)`: surrounding whitespace, a sign, and single underscores between digits are allowed. The
/// result is decimal digits with an optional `-`, never `-0`; the error is the `ValueError` message.
pub fn parse_int(text: &str) -> Result<String, String> {
    let invalid = || {
        format!(
            "invalid literal for int() with base 10: {}",
            crate::pytext::repr_str(text)
        )
    };
    let body = crate::pytext::strip(text);
    let (negative, body) = match body.chars().next() {
        Some('-') => (true, &body[1..]),
        Some('+') => (false, &body[1..]),
        _ => (false, body),
    };
    let mut digits = String::new();
    let mut previous_underscore = true;
    for c in body.chars() {
        if c == '_' {
            if previous_underscore {
                return Err(invalid());
            }
            previous_underscore = true;
        } else {
            let value = decimal(c).ok_or_else(invalid)?;
            digits.push(char::from_digit(value, 10).ok_or_else(invalid)?);
            previous_underscore = false;
        }
    }
    if digits.is_empty() || previous_underscore {
        return Err(invalid());
    }
    if digits.len() > INT_MAX_STR_DIGITS {
        return Err(format!(
            "Exceeds the limit ({INT_MAX_STR_DIGITS} digits) for integer string conversion: value has {} digits; use sys.set_int_max_str_digits() to increase the limit",
            digits.len()
        ));
    }
    let trimmed = digits.trim_start_matches('0');
    Ok(match (trimmed.is_empty(), negative) {
        (true, _) => "0".to_string(),
        (false, true) => format!("-{trimmed}"),
        (false, false) => trimmed.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_are_category_nd() {
        assert_eq!(decimal('7'), Some(7));
        assert_eq!(decimal('\u{0665}'), Some(5));
        assert_eq!(decimal('\u{1FBF9}'), Some(9));
        assert_eq!(decimal('\u{1D7CE}'), Some(0));
        assert_eq!(decimal('\u{1D7D7}'), Some(9));
        assert_eq!(decimal('\u{1D7D8}'), Some(0));
        assert_eq!(decimal('a'), None);
        assert_eq!(decimal('/'), None);
        assert_eq!(decimal(':'), None);
        assert_eq!(decimal('\u{00B2}'), None);
        assert_eq!(decimal('\u{2160}'), None);
    }

    #[test]
    fn int_matches_python() {
        for (text, value) in [
            ("7", "7"),
            (" 7\n", "7"),
            ("+07", "7"),
            ("-0", "0"),
            ("-12", "-12"),
            ("1_000", "1000"),
            ("\u{0663}", "3"),
            ("00", "0"),
        ] {
            assert_eq!(parse_int(text).as_deref(), Ok(value), "{text:?}");
        }
        for text in ["", " ", "_1", "1_", "1__0", "1.0", "0x1", "--1", "+", "1 2"] {
            assert_eq!(
                parse_int(text),
                Err(format!(
                    "invalid literal for int() with base 10: {}",
                    crate::pytext::repr_str(text)
                )),
                "{text:?}"
            );
        }
    }
}

//! Billed-usage meters: Claude transcripts (`usage claude`) and Codex rollouts (`usage codex`), priced at dollars per
//! million tokens.

pub mod claude;
pub mod codex;

use crate::pyjson::Value;
use std::fmt::Write as _;

/// What one meter run prints and exits with.
pub struct Outcome {
    pub code: u8,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Outcome {
    fn success(stdout: Vec<u8>) -> Self {
        Self {
            code: 0,
            stdout,
            stderr: Vec::new(),
        }
    }

    fn failure(code: u8, message: &str) -> Self {
        Self {
            code,
            stdout: Vec::new(),
            stderr: format!("{message}\n").into_bytes(),
        }
    }
}

pub const COLUMNS: [&str; 14] = [
    "Run / agent",
    "Model",
    "Turns",
    "Tool calls",
    "Text-only turns",
    "Input",
    "Cache write",
    "Cache read",
    "Output",
    "Thinking",
    "Wall",
    "Billed cost ($)",
    "Report output (est.)",
    "Production-shaped ($)",
];

pub fn table_row<S: AsRef<str>>(cells: impl IntoIterator<Item = S>) -> String {
    let cells: Vec<String> = cells
        .into_iter()
        .map(|cell| cell.as_ref().to_string())
        .collect();
    format!("| {} |", cells.join(" | "))
}

/// The Markdown header both meters print with `--header`, matching their `--row`.
pub fn header() -> String {
    format!(
        "{}\n{}\n",
        table_row(COLUMNS),
        table_row(COLUMNS.map(|_| "---"))
    )
}

/// A token count as JSON holds it: an integer of any size, or absent.
#[derive(Clone, Copy, Default)]
pub enum Count {
    #[default]
    Absent,
    Known(u64),
    Invalid,
}

pub fn count(value: Option<&Value>) -> Count {
    match value {
        None | Some(Value::Null) => Count::Absent,
        Some(Value::Int(digits)) => digits.parse().map_or(Count::Invalid, Count::Known),
        Some(_) => Count::Invalid,
    }
}

pub fn checked_sum(a: u64, b: u64) -> Result<u64, String> {
    a.checked_add(b)
        .ok_or_else(|| "token count exceeds 2^64 - 1".to_string())
}

/// `f"{n:,}"`.
pub fn thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `f"{x:g}"`.
pub fn general(value: f64) -> String {
    if !value.is_finite() {
        return if value.is_nan() {
            "nan".to_string()
        } else if value > 0.0 {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    let scientific = format!("{value:.5e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    if (-4..6).contains(&exponent) {
        let decimals = usize::try_from(5 - exponent).unwrap_or(0);
        strip_zeros(&format!("{value:.decimals$}")).to_string()
    } else {
        let sign = if exponent < 0 { '-' } else { '+' };
        format!("{}e{sign}{:02}", strip_zeros(mantissa), exponent.abs())
    }
}

fn strip_zeros(text: &str) -> &str {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}

/// `round(x, digits)`.
pub fn round_to(value: f64, digits: usize) -> f64 {
    format!("{value:.digits$}").parse().unwrap_or(value)
}

/// A timestamp as `datetime.fromisoformat` reads the common ISO 8601 forms, in microseconds since the epoch. A
/// timestamp without an offset is read as UTC.
pub fn timestamp_micros(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = bytes.get(range)?;
        part.iter()
            .all(u8::is_ascii_digit)
            .then(|| std::str::from_utf8(part).ok()?.parse().ok())?
    };
    if bytes.get(4) != Some(&b'-') || bytes.get(7) != Some(&b'-') {
        return None;
    }
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) || year < 1 {
        return None;
    }
    let mut micros = days_from_civil(year, month, day) * 86_400_000_000;
    let mut at = 10;
    if bytes.len() > at {
        if !matches!(bytes[at], b'T' | b' ') {
            return None;
        }
        let hour = number(at + 1..at + 3)?;
        let mut minute = 0;
        let mut second = 0;
        let mut fraction = 0;
        at += 3;
        if bytes.get(at) == Some(&b':') {
            minute = number(at + 1..at + 3)?;
            at += 3;
            if bytes.get(at) == Some(&b':') {
                second = number(at + 1..at + 3)?;
                at += 3;
                if matches!(bytes.get(at), Some(b'.' | b',')) {
                    let start = at + 1;
                    at = start;
                    while bytes.get(at).is_some_and(u8::is_ascii_digit) {
                        at += 1;
                    }
                    if at == start {
                        return None;
                    }
                    let digits = &text[start..at.min(start + 6)];
                    fraction = format!("{digits:0<6}").parse().ok()?;
                }
            }
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        micros += ((hour * 60 + minute) * 60 + second) * 1_000_000 + fraction;
        let offset = &text[at..];
        micros -= match offset {
            "" | "Z" => 0,
            _ => offset_micros(offset)?,
        };
    }
    Some(micros)
}

fn offset_micros(text: &str) -> Option<i64> {
    let sign = match text.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let fields: Vec<&str> = if text[1..].contains(':') {
        text[1..].split(':').collect()
    } else {
        text.as_bytes()[1..]
            .chunks(2)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
            .collect()
    };
    if fields.is_empty() || fields.len() > 3 {
        return None;
    }
    let mut seconds = 0;
    for (index, field) in fields.iter().enumerate() {
        if field.len() != 2 || !field.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let value: i64 = field.parse().ok()?;
        if value > if index == 0 { 23 } else { 59 } {
            return None;
        }
        seconds = seconds * 60 + value;
    }
    seconds *= 60_i64.pow(3 - u32::try_from(fields.len()).ok()?);
    Some(sign * seconds * 1_000_000)
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days since 1970-01-01 of a proleptic Gregorian date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let of_era = year - era * 400;
    let of_year = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let of_cycle = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    era * 146_097 + of_cycle - 719_468
}

/// `datetime.isoformat()` of a UTC instant: `YYYY-MM-DDTHH:MM:SS[.ffffff]+00:00`.
pub fn utc_isoformat(micros: i64) -> String {
    let days = micros.div_euclid(86_400_000_000);
    let rest = micros.rem_euclid(86_400_000_000);
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let of_era = shifted - era * 146_097;
    let of_cycle = (of_era - of_era / 1460 + of_era / 36_524 - of_era / 146_096) / 365;
    let of_year = of_era - (365 * of_cycle + of_cycle / 4 - of_cycle / 100);
    let shifted_month = (5 * of_year + 2) / 153;
    let day = of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = shifted_month + if shifted_month < 10 { 3 } else { -9 };
    let year = of_cycle + era * 400 + i64::from(month <= 2);
    let seconds = rest / 1_000_000;
    let fraction = rest % 1_000_000;
    let mut out = format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}",
        seconds / 3600,
        seconds % 3600 / 60,
        seconds % 60
    );
    if fraction != 0 {
        let _ = write!(out, ".{fraction:06}");
    }
    out.push_str("+00:00");
    out
}

/// `float(text)`, or `None` where Python raises `ValueError`.
pub fn float(text: &str) -> Option<f64> {
    crate::pysemantics::parse_float(text).ok()
}

pub fn int(value: u64) -> Value {
    Value::Int(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_matches_python() {
        let cases = [
            (2.0, "2"),
            (1.25, "1.25"),
            (0.1, "0.1"),
            (0.00001, "1e-05"),
            (123_456_789.0, "1.23457e+08"),
            (1e16, "1e+16"),
            (0.0001, "0.0001"),
            (0.0, "0"),
        ];
        for (value, text) in cases {
            assert_eq!(general(value), text, "{value}");
        }
    }

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(9_007_199_254_740_993), "9,007,199,254,740,993");
    }

    #[test]
    fn timestamps_read_offsets_and_fractions() {
        let utc = timestamp_micros("2026-01-01T00:00:01.999Z").unwrap();
        let offset = timestamp_micros("2026-01-01T01:00:00.250+01:00").unwrap();
        assert_eq!(utc - offset, 1_749_000);
        assert_eq!(
            timestamp_micros("2026-01-01T00:00:00"),
            timestamp_micros("2026-01-01T00:00:00Z")
        );
        assert_eq!(timestamp_micros("1970-01-01"), Some(0));
        assert!(timestamp_micros("2026-02-30T00:00:00Z").is_none());
        assert!(timestamp_micros("yesterday").is_none());
        assert_eq!(utc_isoformat(utc), "2026-01-01T00:00:01.999000+00:00");
        assert_eq!(utc_isoformat(0), "1970-01-01T00:00:00+00:00");
        let leap = timestamp_micros("2024-02-29T12:00:00-04:00").unwrap();
        assert_eq!(utc_isoformat(leap), "2024-02-29T16:00:00+00:00");
    }
}

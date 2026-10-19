//! Check the dated rate catalog against the providers' current standard short-context prices, as
//! the `check` command of `bench/tools/rates.py` does. Nothing is written.
//!
//! The catalog follows `bench/schema/rates.schema.json`. Each selected model's latest dated entry is compared
//! with the price its provider's Markdown pricing page lists today. Providers are found by model prefix:
//! `gpt-` is OpenAI and `claude-` is Anthropic. `rates.py refresh`, which appends changed prices, stays in
//! Python.

use crate::pyjson::{self, Duplicates, Object, Value};
use crate::pytext::{self, PyPath};
use crate::schema::{SchemaError, Validator};
use crate::sha256;

pub const USAGE: &str = "usage: bench-tools rates check [-h] [--rates RATES] [--model MODEL] [--json] [--source PROVIDER=URL] [--root ROOT]";

pub const FIELDS: [&str; 5] = [
    "input",
    "output",
    "cache_read",
    "cache_write_5m",
    "cache_write_1h",
];

pub const PROVIDERS: [(&str, &str); 2] = [
    (
        "openai",
        "https://developers.openai.com/api/docs/pricing.md",
    ),
    (
        "anthropic",
        "https://docs.anthropic.com/en/docs/about-claude/pricing.md",
    ),
];

/// Why a check stopped.
#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    /// `rates.RateError`: prices or the catalog disagree with what the check needs; stdout, exit 1.
    Rate(String),
    /// An input or source cannot be read; stderr, exit 2.
    Input(String),
}

/// Reads a provider's pricing page. The error is the message `rates.py` prints after `rates.py: `.
pub trait Fetch {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// `file:` URLs, in any letter case, read a local file, as `urllib` does; `http:` and `https:` URLs are fetched with
/// `ureq`, and other schemes are refused.
pub struct Network;

fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let hex = |byte: u8| char::from(byte).to_digit(16);
        if bytes[index] == b'%'
            && let (Some(high), Some(low)) = (
                bytes.get(index + 1).and_then(|b| hex(*b)),
                bytes.get(index + 2).and_then(|b| hex(*b)),
            )
        {
            out.push(u8::try_from(high * 16 + low).unwrap_or(0));
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned().into_bytes()
}

impl Fetch for Network {
    fn fetch(&self, url: &str) -> Result<Vec<u8>, String> {
        if let Some(rest) = url
            .get(..5)
            .filter(|scheme| scheme.eq_ignore_ascii_case("file:"))
            .map(|_| &url[5..])
        {
            let local = match rest.strip_prefix("//") {
                Some(authority) => {
                    let (host, path) =
                        authority.split_at(authority.find('/').unwrap_or(authority.len()));
                    if !host.is_empty() && host != "localhost" {
                        return Err(
                            "<urlopen error file:// scheme is supported only on localhost>"
                                .to_string(),
                        );
                    }
                    path
                }
                None => rest,
            };
            let path = percent_decode(local);
            return std::fs::read(PyPath::new(&path).to_path_buf())
                .map_err(|error| format!("<urlopen error {}>", pytext::os_error(&error, &path)));
        }
        let Some(scheme) = url
            .split_once(':')
            .map(|(scheme, _)| scheme)
            .filter(|scheme| {
                ["http", "https"]
                    .iter()
                    .any(|known| scheme.eq_ignore_ascii_case(known))
            })
        else {
            return Err(
                "<urlopen error unsupported url type: only file, http and https are read>"
                    .to_string(),
            );
        };
        let url = format!("{}{}", scheme.to_ascii_lowercase(), &url[scheme.len()..]);
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(20)))
            .user_agent("code-review-bench-rates/1")
            .http_status_as_error(false)
            .build()
            .into();
        let mut response = agent.get(&url).call().map_err(urlopen_error)?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!(
                "HTTP Error {}: {}",
                status.as_u16(),
                status.canonical_reason().unwrap_or_default()
            ));
        }
        response
            .body_mut()
            .with_config()
            .limit(u64::MAX)
            .read_to_vec()
            .map_err(urlopen_error)
    }
}

/// `URLError`'s text: an operating-system error as Python's `OSError` prints it, a timeout as `socket.timeout` does,
/// and anything else in `ureq`'s words.
fn urlopen_error(error: ureq::Error) -> String {
    let reason = match &error {
        ureq::Error::Io(io) => match io.raw_os_error() {
            Some(code) => {
                let text = io.to_string();
                let strerror = text
                    .rfind(" (os error ")
                    .map_or(text.as_str(), |end| &text[..end]);
                format!("[Errno {code}] {strerror}")
            }
            None => io.to_string(),
        },
        ureq::Error::Timeout(_) => "timed out".to_string(),
        other => other.to_string(),
    };
    format!("<urlopen error {reason}>")
}

fn schema_failure(SchemaError(message): SchemaError) -> Failure {
    Failure::Input(message)
}

fn read_json(path: &PyPath) -> Result<Value, Failure> {
    let text =
        pytext::read_text(path).map_err(|error| Failure::Input(error.message().to_string()))?;
    pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| Failure::Input(error.to_string()))
}

fn number_is_finite(value: &Value) -> Result<bool, Failure> {
    match value {
        Value::Float(number) => Ok(number.is_finite()),
        Value::Int(digits) => {
            let magnitude = digits.trim_start_matches('-');
            // 2**1024 has 309 digits: `math.isfinite` cannot convert an integer that large to a float.
            if magnitude.len() > 309 || magnitude.parse::<f64>().is_ok_and(f64::is_infinite) {
                Err(Failure::Input(
                    "int too large to convert to float".to_string(),
                ))
            } else {
                Ok(true)
            }
        }
        other => Err(Failure::Input(format!(
            "must be real number, not {}",
            other.type_name()
        ))),
    }
}

/// `datetime.strptime(text, "%Y-%m-%d")`, which accepts any decimal digits and a one-digit day.
fn check_date(text: &str) -> Result<(), Failure> {
    let chars: Vec<char> = text.chars().collect();
    let mismatch = || {
        Failure::Input(format!(
            "time data {} does not match format '%Y-%m-%d'",
            pytext::repr_str(text)
        ))
    };
    let digit = |index: usize| chars.get(index).and_then(|c| pytext::decimal_value(*c));
    let ascii = |index: usize, set: &str| chars.get(index).is_some_and(|c| set.contains(*c));
    let year: Vec<u32> = (0..4)
        .map(digit)
        .collect::<Option<_>>()
        .ok_or_else(mismatch)?;
    if chars.get(4) != Some(&'-') {
        return Err(mismatch());
    }
    let month = if ascii(5, "1") && ascii(6, "012") || ascii(5, "0") && ascii(6, "123456789") {
        7
    } else if ascii(5, "123456789") {
        6
    } else {
        return Err(mismatch());
    };
    if chars.get(month) != Some(&'-') {
        return Err(mismatch());
    }
    let day = month + 1;
    let end = if ascii(day, "3") && ascii(day + 1, "01")
        || ascii(day, "12") && digit(day + 1).is_some()
        || ascii(day, "0") && ascii(day + 1, "123456789")
    {
        day + 2
    } else if ascii(day, "123456789") {
        day + 1
    } else if ascii(day, " ") && ascii(day + 1, "123456789") {
        day + 2
    } else {
        return Err(mismatch());
    };
    if end < chars.len() {
        let rest: String = chars[end..].iter().collect();
        return Err(Failure::Input(format!("unconverted data remains: {rest}")));
    }
    let value = |range: std::ops::Range<usize>| {
        range.fold(0_u32, |total, index| {
            total * 10
                + chars[index]
                    .to_digit(10)
                    .or_else(|| pytext::decimal_value(chars[index]))
                    .unwrap_or(0)
        })
    };
    let year = year.iter().fold(0_u32, |total, digit| total * 10 + digit);
    let month_number = value(5..month);
    let day_start = if chars[day] == ' ' { day + 1 } else { day };
    let day_number = value(day_start..end);
    if year == 0 {
        return Err(Failure::Input("year must be in 1..9999, not 0".to_string()));
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month_number {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    if day_number > days {
        return Err(Failure::Input(format!(
            "day {day_number} must be in range 1..{days} for month {month_number} in year {year}"
        )));
    }
    Ok(())
}

fn object_field<'a>(row: &'a Object, key: &str) -> Result<&'a Value, Failure> {
    row.get(key)
        .ok_or_else(|| Failure::Input(pytext::repr_str(key)))
}

/// `rates.read_catalog`: the catalog after its schema and the checks the schema cannot express.
pub fn read_catalog(root: &PyPath, path: &PyPath) -> Result<Value, Failure> {
    let catalog = read_json(path)?;
    let schema = read_json(&root.join(b"bench/schema/rates.schema.json"))?;
    let mut problems = Validator::default()
        .validate(&schema, &catalog)
        .map_err(schema_failure)?;
    if !problems.is_empty() {
        return Err(Failure::Rate(problems.join("; ")));
    }
    let rows = catalog
        .as_object()
        .and_then(|catalog| catalog.get("rates"))
        .and_then(Value::as_array)
        .unwrap_or(&[]);
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| Failure::Input("a rate entry is not an object".to_string()))?;
        let text = |key: &str| -> Result<&str, Failure> {
            object_field(row, key)?
                .as_str()
                .ok_or_else(|| Failure::Input(format!("{key} is not text")))
        };
        let key = (text("model")?, text("as_of")?);
        if seen.contains(&key) {
            problems.push(format!(
                "duplicate dated rate: ({}, {})",
                pytext::repr_str(key.0),
                pytext::repr_str(key.1)
            ));
        }
        seen.push(key);
        for field in FIELDS {
            if !number_is_finite(object_field(row, field)?)? {
                problems.push(format!("non-finite {field} for {}", key.0));
            }
        }
        check_date(key.1)?;
    }
    if !problems.is_empty() {
        return Err(Failure::Rate(problems.join("; ")));
    }
    Ok(catalog)
}

/// `rates.latest`: each selected model's latest dated entry, in selection order.
pub fn latest<'a>(
    catalog: &'a Value,
    models: Option<&[String]>,
) -> Result<Vec<(String, &'a Object)>, Failure> {
    let mut rows: Vec<&Object> = catalog
        .as_object()
        .and_then(|catalog| catalog.get("rates"))
        .and_then(Value::as_array)
        .unwrap_or(&[])
        .iter()
        .filter_map(Value::as_object)
        .collect();
    let as_of = |row: &Object| {
        row.get("as_of")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    rows.sort_by_key(|row| as_of(row));
    let mut by_model: Vec<(String, &Object)> = Vec::new();
    for row in rows {
        let model = row
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        match by_model.iter_mut().find(|(name, _)| *name == model) {
            Some(entry) => entry.1 = row,
            None => by_model.push((model, row)),
        }
    }
    let selected: Vec<String> = match models {
        Some(models) if !models.is_empty() => models.to_vec(),
        _ => {
            let mut names: Vec<String> = by_model.iter().map(|(name, _)| name.clone()).collect();
            names.sort();
            names
        }
    };
    let mut missing: Vec<&String> = selected
        .iter()
        .filter(|model| !by_model.iter().any(|(name, _)| name == *model))
        .collect();
    missing.sort();
    missing.dedup();
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|name| name.as_str()).collect();
        return Err(Failure::Rate(format!(
            "no catalog entry for {}; add a dated price entry first",
            names.join(", ")
        )));
    }
    if selected.is_empty() {
        return Err(Failure::Rate("the rate catalog is empty".to_string()));
    }
    let mut result: Vec<(String, &Object)> = Vec::new();
    for model in selected {
        if !result.iter().any(|(name, _)| *name == model) {
            let row = by_model
                .iter()
                .find(|(name, _)| *name == model)
                .map(|(_, row)| *row)
                .ok_or_else(|| Failure::Rate(format!("no catalog entry for {model}")))?;
            result.push((model, row));
        }
    }
    Ok(result)
}

const LINE_BREAKS: [char; 10] = [
    '\n', '\u{b}', '\u{c}', '\r', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}', '\u{2029}',
];

/// `str.splitlines()`.
fn splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if LINE_BREAKS.contains(&c) {
            lines.push(&text[start..index]);
            let mut end = index + c.len_utf8();
            if c == '\r' && chars.peek().map(|(_, next)| *next) == Some('\n') {
                chars.next();
                end += 1;
            }
            start = end;
        }
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

/// `re.split(r"(?m)^#{1,6} ", markdown)`.
fn sections(markdown: &str) -> Vec<&str> {
    let bytes = markdown.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut index = 0;
    while index < bytes.len() {
        if index == 0 || bytes[index - 1] == b'\n' {
            let hashes = bytes[index..]
                .iter()
                .take_while(|byte| **byte == b'#')
                .count();
            if (1..=6).contains(&hashes) && bytes.get(index + hashes) == Some(&b' ') {
                parts.push(&markdown[start..index]);
                index += hashes + 1;
                start = index;
                continue;
            }
        }
        index += 1;
    }
    parts.push(&markdown[start..]);
    parts
}

/// `rates.table_rows`: the data rows of the first table in the section with this heading.
fn table_rows(markdown: &str, heading: &str, header: &[&str]) -> Result<Vec<Vec<String>>, Failure> {
    let section = sections(markdown)
        .into_iter()
        .find(|part| {
            !part.is_empty()
                && splitlines(part).first().map(|line| pytext::strip(line)) == Some(heading)
        })
        .ok_or_else(|| Failure::Rate(format!("provider pricing section is missing: {heading}")))?;
    let rows: Vec<Vec<String>> = splitlines(section)
        .into_iter()
        .filter(|line| pytext::strip(line).starts_with('|'))
        .map(|line| {
            pytext::strip(line)
                .trim_matches('|')
                .split('|')
                .map(|cell| pytext::strip(cell).to_string())
                .collect()
        })
        .collect();
    if rows
        .first()
        .is_none_or(|first| first.iter().map(String::as_str).ne(header.iter().copied()))
    {
        return Err(Failure::Rate(format!(
            "provider pricing columns changed: {heading}"
        )));
    }
    Ok(rows.into_iter().skip(2).collect())
}

/// `rates.price`: a cell such as `$2.50`, `$4 / MTok` or `$0.20 / MTok<sup>2</sup>`.
fn price(cell: &str) -> Result<f64, String> {
    let unsupported = || format!("unsupported provider price: {}", pytext::repr_str(cell));
    let rest = cell.strip_prefix('$').ok_or_else(unsupported)?;
    let whole = rest.bytes().take_while(u8::is_ascii_digit).count();
    if whole == 0 {
        return Err(unsupported());
    }
    let mut end = whole;
    if rest[end..].starts_with('.') {
        let fraction = rest[end + 1..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        if fraction == 0 {
            return Err(unsupported());
        }
        end += 1 + fraction;
    }
    let number = &rest[..end];
    let mut tail = &rest[end..];
    if let Some(after) = tail.strip_prefix(" / MTok") {
        tail = after;
    }
    if let Some(after) = tail.strip_prefix("<sup>") {
        let digits: usize = after
            .chars()
            .take_while(|c| pytext::decimal_value(*c).is_some())
            .map(char::len_utf8)
            .sum();
        tail = after[digits..]
            .strip_prefix("</sup>")
            .filter(|_| digits > 0)
            .ok_or_else(unsupported)?;
    }
    if !tail.is_empty() {
        return Err(unsupported());
    }
    number.parse::<f64>().map_err(|_| unsupported())
}

/// Whether `re.search(r"\(for prompts over [\d,]+ tokens\)", name)` matches.
fn long_prompt_row(name: &str) -> bool {
    const OPEN: &str = "(for prompts over ";
    name.match_indices(OPEN).any(|(index, _)| {
        let after = &name[index + OPEN.len()..];
        let count: usize = after
            .chars()
            .take_while(|c| *c == ',' || pytext::decimal_value(*c).is_some())
            .map(char::len_utf8)
            .sum();
        count > 0 && after[count..].starts_with(" tokens)")
    })
}

/// `re.sub(r"\s+\(.*", "", name)`: the name up to the first whitespace run followed by `(`.
fn before_parenthesis(name: &str) -> &str {
    let mut run_start = None;
    for (index, c) in name.char_indices() {
        if pytext::is_space(c) {
            run_start.get_or_insert(index);
        } else {
            if c == '('
                && let Some(start) = run_start
            {
                return &name[..start];
            }
            run_start = None;
        }
    }
    name
}

/// Prices by model in table order, and the selected models skipped with their reasons.
pub type Parsed = (Vec<(String, [f64; 5])>, Vec<(String, String)>);

/// `rates.parse_prices`.
pub fn parse_prices(provider: &str, markdown: &str, models: &[String]) -> Result<Parsed, Failure> {
    let (header, heading, positions): (&[&str], &str, [usize; 5]) = if provider == "openai" {
        (
            &[
                "Model",
                "Short context input",
                "Short context cached input",
                "Short context cache writes",
                "Short context output",
                "Long context input",
                "Long context cached input",
                "Long context cache writes",
                "Long context output",
            ],
            "Standard pricing data",
            [1, 4, 2, 3, 3],
        )
    } else {
        (
            &[
                "Model",
                "Base input tokens",
                "5m cache writes",
                "1h cache writes",
                "Cache hits and refreshes",
                "Output tokens",
            ],
            "Model pricing",
            [1, 5, 4, 2, 3],
        )
    };
    let rows = table_rows(markdown, heading, header)?;
    let mut prices: Vec<(String, [f64; 5])> = Vec::new();
    let mut unsupported: Vec<(String, String)> = Vec::new();
    for row in rows {
        let mut name = row[0].clone();
        if provider == "anthropic" {
            if long_prompt_row(&name) {
                continue;
            }
            name = before_parenthesis(&name)
                .to_lowercase()
                .replace(['.', ' '], "-");
        }
        if !models.contains(&name) {
            continue;
        }
        if row.len() != header.len()
            || prices.iter().any(|(model, _)| *model == name)
            || unsupported.iter().any(|(model, _)| *model == name)
        {
            return Err(Failure::Rate(format!(
                "ambiguous provider pricing row for {name}"
            )));
        }
        let parsed: Result<Vec<f64>, String> = positions
            .iter()
            .map(|position| price(&row[*position]))
            .collect();
        match parsed {
            Ok(values) => {
                let mut fields = [0.0; 5];
                fields.copy_from_slice(&values);
                prices.push((name, fields));
            }
            Err(reason) => unsupported.push((name, reason)),
        }
    }
    let mut missing: Vec<&String> = models
        .iter()
        .filter(|model| {
            !prices.iter().any(|(name, _)| name == *model)
                && !unsupported.iter().any(|(name, _)| name == *model)
        })
        .collect();
    missing.sort();
    missing.dedup();
    for model in missing {
        unsupported.push((
            model.clone(),
            "no standard short-context row in the provider table".to_string(),
        ));
    }
    Ok((prices, unsupported))
}

fn object(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

/// `datetime.now(timezone.utc).isoformat(timespec="seconds")` for seconds since the epoch.
pub fn utc_timestamp(seconds: u64) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let within = seconds % 86_400;
    // Howard Hinnant's days-to-civil conversion.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}+00:00",
        within / 3_600,
        within / 60 % 60,
        within % 60
    )
}

/// `rates.inspect`: the check receipt for the selected models.
pub fn inspect(
    catalog: &Value,
    models: Option<&[String]>,
    sources: &[(String, String)],
    fetch: &dyn Fetch,
    checked_at: &str,
) -> Result<Value, Failure> {
    let saved = latest(catalog, models)?;
    let mut grouped: Vec<(&str, Vec<String>)> = PROVIDERS
        .iter()
        .map(|(provider, _)| (*provider, Vec::new()))
        .collect();
    for (model, _) in &saved {
        let provider = if model.starts_with("gpt-") {
            "openai"
        } else if model.starts_with("claude-") {
            "anthropic"
        } else {
            return Err(Failure::Rate(format!(
                "unsupported rate provider for {model}"
            )));
        };
        if let Some((_, selected)) = grouped.iter_mut().find(|(name, _)| *name == provider) {
            selected.push(model.clone());
        }
    }
    let explicit = models.is_some_and(|models| !models.is_empty());
    let mut observed: Vec<(String, [f64; 5])> = Vec::new();
    let mut source_entries: Vec<(String, Value)> = Vec::new();
    let mut unsupported: Vec<(String, String)> = Vec::new();
    for (provider, selected) in &grouped {
        if selected.is_empty() {
            continue;
        }
        let url = sources
            .iter()
            .find(|(name, _)| name == provider)
            .map(|(_, url)| url.clone())
            .unwrap_or_default();
        let raw = fetch.fetch(&url).map_err(Failure::Input)?;
        let markdown = pytext::decode_utf8(&raw).map_err(Failure::Input)?;
        source_entries.push((
            (*provider).to_string(),
            object(vec![
                ("url", Value::Str(url.clone())),
                (
                    "sha256",
                    Value::Str(sha256::hex_digest(markdown.as_bytes())),
                ),
            ]),
        ));
        let (prices, skipped) = parse_prices(provider, &markdown, selected)?;
        if !skipped.is_empty() && (explicit || prices.is_empty()) {
            let reasons: Vec<String> = skipped
                .iter()
                .map(|(model, reason)| format!("{model} ({reason})"))
                .collect();
            return Err(Failure::Rate(format!(
                "provider has no supported standard short-context prices: {}",
                reasons.join("; ")
            )));
        }
        for (model, fields) in prices {
            match observed.iter_mut().find(|(name, _)| *name == model) {
                Some(entry) => entry.1 = fields,
                None => observed.push((model, fields)),
            }
        }
        for (model, reason) in skipped {
            match unsupported.iter_mut().find(|(name, _)| *name == model) {
                Some(entry) => entry.1 = reason,
                None => unsupported.push((model, reason)),
            }
        }
    }
    let mut changes = Vec::new();
    for (model, row) in &saved {
        if unsupported.iter().any(|(name, _)| name == model) {
            continue;
        }
        let current = observed
            .iter()
            .find(|(name, _)| name == model)
            .map(|(_, fields)| fields)
            .ok_or_else(|| Failure::Input(pytext::repr_str(model)))?;
        let mut fields: Vec<(String, Value)> = Vec::new();
        for (index, field) in FIELDS.iter().enumerate() {
            let saved_value = row.get(field).cloned().unwrap_or(Value::Null);
            let current_value = Value::Float(current[index]);
            if !saved_value.py_eq(&current_value) {
                fields.push((
                    (*field).to_string(),
                    object(vec![("saved", saved_value), ("current", current_value)]),
                ));
            }
        }
        if !fields.is_empty() {
            changes.push(object(vec![
                ("model", Value::Str(model.clone())),
                ("fields", Value::Object(fields.into_iter().collect())),
            ]));
        }
    }
    let checked_models: Vec<Value> = saved
        .iter()
        .filter(|(model, _)| observed.iter().any(|(name, _)| name == model))
        .map(|(model, _)| Value::Str(model.clone()))
        .collect();
    let prices: Object = observed
        .iter()
        .map(|(model, fields)| {
            (
                model.clone(),
                Value::Object(
                    FIELDS
                        .iter()
                        .zip(fields)
                        .map(|(field, value)| ((*field).to_string(), Value::Float(*value)))
                        .collect(),
                ),
            )
        })
        .collect();
    Ok(object(vec![
        ("checked_at", Value::Str(checked_at.to_string())),
        (
            "sources",
            Value::Object(source_entries.into_iter().collect()),
        ),
        ("models", Value::Array(checked_models)),
        ("prices", Value::Object(prices)),
        ("changes", Value::Array(changes)),
        (
            "unsupported",
            Value::Object(
                unsupported
                    .into_iter()
                    .map(|(model, reason)| (model, Value::Str(reason)))
                    .collect(),
            ),
        ),
    ]))
}

/// The lines the `check` command of `rates.py` prints without `--json`.
pub fn render(report: &Value) -> String {
    let field = |key: &str| report.as_object().and_then(|report| report.get(key));
    let mut out = String::new();
    for change in field("changes").and_then(Value::as_array).unwrap_or(&[]) {
        let model = change
            .as_object()
            .and_then(|change| change.get("model"))
            .map(Value::py_str)
            .unwrap_or_default();
        let fields = change.as_object().and_then(|change| change.get("fields"));
        for (name, values) in fields
            .and_then(Value::as_object)
            .into_iter()
            .flat_map(Object::iter)
        {
            let part = |key: &str| {
                values
                    .as_object()
                    .and_then(|values| values.get(key))
                    .map(Value::py_str)
                    .unwrap_or_default()
            };
            out.push_str(&format!(
                "{model} {name}: {} -> {} USD per million tokens\n",
                part("saved"),
                part("current")
            ));
        }
    }
    for (model, reason) in field("unsupported")
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(Object::iter)
    {
        out.push_str(&format!("{model} skipped: {}\n", reason.py_str()));
    }
    let count = |key: &str| {
        field(key)
            .and_then(Value::as_array)
            .map_or(0, <[Value]>::len)
    };
    out.push_str(&format!(
        "{} models checked; {} changed\n",
        count("models"),
        count("changes")
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const OPENAI: &str = "### Standard pricing data
| Model | Short context input | Short context cached input | Short context cache writes | Short context output | Long context input | Long context cached input | Long context cache writes | Long context output |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| gpt-6.1-sol | $2.00 | $0.10 | $2.50 | $10.00 | $4.00 | $0.20 | $5.00 | $15.00 |
### Batch pricing data
| Model | Short context input | Short context cached input | Short context cache writes | Short context output | Long context input | Long context cached input | Long context cache writes | Long context output |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| gpt-6.1-sol | $1.00 | $0.05 | $1.25 | $5.00 | $2.00 | $0.10 | $2.50 | $7.50 |
";
    const ANTHROPIC: &str = "## Model pricing
| Model | Base input tokens | 5m cache writes | 1h cache writes | Cache hits and refreshes | Output tokens |
| --- | --- | --- | --- | --- | --- |
| Claude Opus 5.5 | $4 / MTok | $5 / MTok | $8 / MTok | $0.20 / MTok<sup>2</sup> | $20 / MTok |
";

    #[test]
    fn provider_tables_read_as_python_reads_them() {
        let models = vec!["gpt-6.1-sol".to_string()];
        let (prices, unsupported) = parse_prices("openai", OPENAI, &models).unwrap();
        assert_eq!(
            prices,
            vec![("gpt-6.1-sol".to_string(), [2.0, 10.0, 0.1, 2.5, 2.5])]
        );
        assert!(unsupported.is_empty());
        let models = vec!["claude-opus-5-5".to_string()];
        let (prices, _) = parse_prices("anthropic", ANTHROPIC, &models).unwrap();
        assert_eq!(
            prices,
            vec![("claude-opus-5-5".to_string(), [4.0, 20.0, 0.2, 5.0, 8.0])]
        );
        let tiered = format!(
            "{ANTHROPIC}| Claude Haiku 5.5 (for prompts up to 100,000 tokens) | $0.10 / MTok | $0.125 / MTok | $0.20 / MTok | $0.01 / MTok | $0.50 / MTok |\n| Claude Haiku 5.5 (for prompts over 100,000 tokens) | $0.50 / MTok | $0.625 / MTok | $1 / MTok | $0.05 / MTok | $2.50 / MTok |\n"
        );
        let models = vec!["claude-haiku-5-5".to_string()];
        let (prices, unsupported) = parse_prices("anthropic", &tiered, &models).unwrap();
        assert_eq!(
            prices,
            vec![("claude-haiku-5-5".to_string(), [0.1, 0.5, 0.01, 0.125, 0.2])]
        );
        assert!(unsupported.is_empty());
        let duplicated = format!("{ANTHROPIC}{}\n", ANTHROPIC.lines().last().unwrap());
        let models = vec!["claude-opus-5-5".to_string()];
        assert_eq!(
            parse_prices("anthropic", &duplicated, &models),
            Err(Failure::Rate(
                "ambiguous provider pricing row for claude-opus-5-5".to_string()
            ))
        );
        assert_eq!(
            parse_prices(
                "openai",
                &OPENAI.replace("Short context input", "Input"),
                &["gpt-6.1-sol".to_string()]
            ),
            Err(Failure::Rate(
                "provider pricing columns changed: Standard pricing data".to_string()
            ))
        );
    }

    #[test]
    fn prices_follow_the_python_pattern() {
        assert_eq!(price("$2.50"), Ok(2.5));
        assert_eq!(price("$4 / MTok"), Ok(4.0));
        assert_eq!(price("$0.20 / MTok<sup>2</sup>"), Ok(0.2));
        assert_eq!(price("$1<sup>\u{0663}</sup>"), Ok(1.0));
        for cell in [
            "-",
            "$",
            "$.5",
            "$5.",
            "$5 / MTok ",
            "$5<sup></sup>",
            "5",
            "$5/MTok",
            "$\u{0665}",
        ] {
            assert!(price(cell).is_err(), "{cell:?}");
        }
        assert_eq!(
            before_parenthesis("Claude Opus 5.5 (deprecated)"),
            "Claude Opus 5.5"
        );
        assert_eq!(before_parenthesis("A (b) (c)"), "A");
        assert_eq!(before_parenthesis("A b(c)"), "A b(c)");
        assert!(long_prompt_row("x (for prompts over 1,000 tokens)"));
        assert!(!long_prompt_row("x (for prompts over tokens)"));
    }

    #[test]
    fn splitting_matches_python() {
        assert_eq!(splitlines("a\r\nb\rc\u{2028}d\n"), vec!["a", "b", "c", "d"]);
        assert_eq!(splitlines(""), Vec::<&str>::new());
        assert_eq!(splitlines("\n"), vec![""]);
        assert_eq!(
            sections("x\n## A\n####### B\n# C"),
            vec!["x\n", "A\n####### B\n", "C"]
        );
        assert_eq!(sections("## A"), vec!["", "A"]);
    }

    #[test]
    fn dates_follow_strptime() {
        assert_eq!(check_date("2026-01-01"), Ok(()));
        assert_eq!(check_date("2024-02-29"), Ok(()));
        assert_eq!(check_date("2026-01-1\u{0665}"), Ok(()));
        assert_eq!(
            check_date("2026-00-01"),
            Err(Failure::Input(
                "time data '2026-00-01' does not match format '%Y-%m-%d'".to_string()
            ))
        );
        assert_eq!(
            check_date("2026-01-35"),
            Err(Failure::Input("unconverted data remains: 5".to_string()))
        );
        assert_eq!(
            check_date("2026-02-30"),
            Err(Failure::Input(
                "day 30 must be in range 1..28 for month 2 in year 2026".to_string()
            ))
        );
        assert_eq!(
            check_date("0000-01-01"),
            Err(Failure::Input("year must be in 1..9999, not 0".to_string()))
        );
    }

    #[test]
    fn timestamps_are_utc_iso_seconds() {
        assert_eq!(utc_timestamp(0), "1970-01-01T00:00:00+00:00");
        assert_eq!(utc_timestamp(1_792_281_600), "2026-10-18T00:00:00+00:00");
        assert_eq!(
            utc_timestamp(951_782_400 + 3_661),
            "2000-02-29T01:01:01+00:00"
        );
    }
}

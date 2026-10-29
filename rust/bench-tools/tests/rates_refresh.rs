use bench_tools::pyjson::{self, Duplicates, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const OPENAI: &str = include_str!("fixtures/rates/openai-pricing.md");
const MODEL: &str = "gpt-6.1-sol";

/// A root holding the rates schema, a catalog with one row per model and a saved OpenAI pricing page.
struct Fixture {
    root: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn row(model: &str, as_of: &str, output: u32) -> String {
    format!(
        r#"{{"model": "{model}", "as_of": "{as_of}", "input": 2, "output": {output}, "cache_read": 0.1, "cache_write_5m": 2.5, "cache_write_1h": 2.5, "source": "original evidence", "billing": "list-price-equivalent"}}"#
    )
}

impl Fixture {
    fn new(rows: &[String]) -> Self {
        let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join("rates-refresh-tests")
            .join(format!(
                "r-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        fs::create_dir_all(root.join("bench/schema")).unwrap();
        fs::copy(
            repository.join("bench/schema/rates.schema.json"),
            root.join("bench/schema/rates.schema.json"),
        )
        .unwrap();
        fs::write(root.join("openai.md"), OPENAI).unwrap();
        let fixture = Self { root };
        fixture.catalog(rows);
        fixture
    }

    fn catalog(&self, rows: &[String]) {
        fs::write(
            self.root.join("rates.json"),
            format!(r#"{{"schema_version": 1, "rates": [{}]}}"#, rows.join(", ")),
        )
        .unwrap();
    }

    fn bytes(&self) -> Vec<u8> {
        fs::read(self.root.join("rates.json")).unwrap()
    }

    fn rows(&self) -> Vec<Value> {
        let text = String::from_utf8(self.bytes()).unwrap();
        let catalog = pyjson::loads(&text, Duplicates::KeepLast).unwrap();
        get(&catalog, "rates").as_array().unwrap().to_vec()
    }

    fn rates_at(&self, catalog: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_bench-tools"))
            .arg("rates")
            .args(args)
            .arg("--rates")
            .arg(catalog)
            .arg("--root")
            .arg(&self.root)
            .arg("--source")
            .arg(format!(
                "openai=file:{}",
                self.root.join("openai.md").display()
            ))
            .output()
            .unwrap()
    }

    fn rates(&self, args: &[&str]) -> Output {
        self.rates_at(&self.root.join("rates.json"), args)
    }
}

fn get<'a>(value: &'a Value, key: &str) -> &'a Value {
    value
        .as_object()
        .and_then(|object| object.get(key))
        .unwrap()
}

fn report(output: &Output) -> Value {
    pyjson::loads(
        &String::from_utf8_lossy(&output.stdout),
        Duplicates::KeepLast,
    )
    .unwrap()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn an_unchanged_check_or_refresh_keeps_the_catalog_bytes() {
    let fx = Fixture::new(&[row(MODEL, "2026-01-01", 10)]);
    let original = fx.bytes();
    for action in ["check", "refresh"] {
        let output = fx.rates(&[action, "--json"]);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let report = report(&output);
        assert!(get(&report, "changes").as_array().unwrap().is_empty());
        assert_eq!(
            get(get(get(&report, "prices"), MODEL), "output").py_str(),
            "10.0"
        );
        assert_eq!(
            get(get(get(&report, "sources"), "openai"), "sha256")
                .py_str()
                .len(),
            64
        );
        assert_eq!(fx.bytes(), original);
    }
}

#[test]
fn a_changed_price_is_appended_once_and_check_reports_it_without_writing() {
    let fx = Fixture::new(&[row(MODEL, "2026-01-01", 9)]);
    let original = fx.bytes();
    let output = fx.rates(&["check", "--json"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let fields = get(
        &get(&report(&output), "changes").as_array().unwrap()[0],
        "fields",
    )
    .clone();
    assert_eq!(get(get(&fields, "output"), "saved").py_str(), "9");
    assert_eq!(get(get(&fields, "output"), "current").py_str(), "10.0");
    assert_eq!(fx.bytes(), original);
    let output = fx.rates(&["refresh"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        stdout(&output),
        format!(
            "{MODEL} output: 9 -> 10.0 USD per million tokens\n1 models checked; 1 changed; catalog refreshed\n"
        )
    );
    let rows = fx.rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(get(&rows[0], "output").py_str(), "9");
    assert_eq!(get(&rows[1], "output").py_str(), "10.0");
    assert_eq!(get(&rows[1], "billing").py_str(), "list-price-equivalent");
    assert!(get(&rows[1], "source").py_str().contains("; sha256 "));
    let refreshed = fx.bytes();
    assert!(refreshed.ends_with(b"}\n"));
    let again = fx.rates(&["refresh"]);
    assert_eq!(again.status.code(), Some(0), "{again:?}");
    assert_eq!(fx.bytes(), refreshed, "a second refresh changes nothing");
}

#[test]
fn an_unreadable_source_or_changed_columns_or_unknown_model_write_nothing() {
    let fx = Fixture::new(&[row(MODEL, "2026-01-01", 9)]);
    let original = fx.bytes();
    let unknown = fx.rates(&["refresh", "--model", "gpt-unknown"]);
    assert_eq!(unknown.status.code(), Some(1), "{unknown:?}");
    fs::write(
        fx.root.join("openai.md"),
        OPENAI.replace("Short context input", "Input"),
    )
    .unwrap();
    let changed = fx.rates(&["refresh"]);
    assert_eq!(changed.status.code(), Some(1), "{changed:?}");
    assert!(stdout(&changed).contains("columns changed"));
    fs::remove_file(fx.root.join("openai.md")).unwrap();
    let missing = fx.rates(&["refresh"]);
    assert_eq!(missing.status.code(), Some(2), "{missing:?}");
    assert!(String::from_utf8_lossy(&missing.stderr).contains("No such file"));
    assert_eq!(fx.bytes(), original);
}

#[test]
fn a_full_refresh_skips_unsupported_models_and_naming_one_is_refused() {
    let rows = [
        row(MODEL, "2026-01-01", 9),
        row("gpt-retired", "2026-01-01", 9),
        row("gpt-unpriced", "2026-01-01", 9),
    ];
    let fx = Fixture::new(&rows);
    let unpriced = "| gpt-unpriced | $2.00 | $9.00 | - | $2.50 | $5.00 | $4.00 | $15.00 | $0.20 | $5.00 | $10.00 |\n### Batch";
    fs::write(
        fx.root.join("openai.md"),
        OPENAI.replacen("### Batch", unpriced, 1),
    )
    .unwrap();
    let output = fx.rates(&["refresh", "--json"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let report = report(&output);
    assert_eq!(get(&report, "models").py_str(), "['gpt-6.1-sol']");
    let unsupported: Vec<String> = get(&report, "unsupported")
        .as_object()
        .unwrap()
        .iter()
        .map(|(model, _)| model.clone())
        .collect();
    assert_eq!(unsupported, ["gpt-unpriced", "gpt-retired"]);
    let saved = fx.rows();
    assert_eq!(saved.len(), 4);
    assert_eq!(get(&saved[3], "model").py_str(), MODEL);
    assert_eq!(get(&saved[3], "output").py_str(), "10.0");
    for model in ["gpt-retired", "gpt-unpriced"] {
        let refused = fx.rates(&["check", "--model", MODEL, "--model", model]);
        assert_eq!(refused.status.code(), Some(1), "{refused:?}");
        assert!(stdout(&refused).contains(model));
    }
}

#[test]
fn a_provider_listing_no_cataloged_model_is_refused() {
    let fx = Fixture::new(&[row("gpt-retired", "2026-01-01", 9)]);
    let original = fx.bytes();
    let output = fx.rates(&["refresh"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stdout(&output).contains("gpt-retired"));
    assert_eq!(fx.bytes(), original);
}

#[test]
fn a_same_day_change_never_overwrites_history() {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let today = &bench_tools::rates::utc_timestamp(seconds)[..10];
    let fx = Fixture::new(&[row(MODEL, today, 9)]);
    let original = fx.bytes();
    let output = fx.rates(&["refresh"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stdout(&output).contains("same-day"));
    assert_eq!(fx.bytes(), original);
}

#[test]
fn historical_catalogs_and_frozen_runs_are_never_refreshed() {
    let fx = Fixture::new(&[row(MODEL, "2026-01-01", 9)]);
    for path in [
        "bench/rates.json",
        "bench/runs/test/rates.json",
        "artifacts/x/rates.json",
    ] {
        let destination = fx.root.join(path);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(fx.root.join("rates.json"), &destination).unwrap();
        let output = fx.rates_at(&destination, &["refresh"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(stdout(&output).contains("immutable"), "{output:?}");
        assert_eq!(fs::read(&destination).unwrap(), fx.bytes());
    }
}

#[test]
fn an_invalid_catalog_is_refused() {
    let fx = Fixture::new(&[]);
    fs::write(
        fx.root.join("rates.json"),
        r#"{"schema_version": 1, "rates": [{"model": "bad"}]}"#,
    )
    .unwrap();
    let output = fx.rates(&["refresh"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(stdout(&output).contains("missing required"));
}

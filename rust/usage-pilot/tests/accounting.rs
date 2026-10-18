use std::io::Cursor;
use usage_pilot::{ErrorKind, Rates, read_rollout, summarize};

fn evaluate(input: &str) -> Result<serde_json::Value, usage_pilot::UsageError> {
    let rollout = read_rollout(Cursor::new(input), "fixture")?;
    let summary = summarize(&rollout, "fixture", Rates::new(10.0, 50.0, 0.1, 1.25)?)?;
    Ok(serde_json::to_value(summary).expect("serializable summary"))
}

fn record(id: &str, input: &str, cached: &str, write: &str, output: &str) -> String {
    format!(
        r#"{{"type":"token_usage_record","payload":{{"response_id":"{id}","usage":{{"input_tokens":{input},"cached_input_tokens":{cached},"cache_write_input_tokens":{write},"output_tokens":{output},"reasoning_output_tokens":0}}}}}}"#
    )
}

#[test]
fn known_zero_and_unknown_are_distinct() {
    assert_eq!(
        evaluate(&record("r", "0", "0", "0", "0")).unwrap()["input"],
        0
    );
    let err = evaluate(&record("r", "null", "0", "0", "0")).unwrap_err();
    assert!(matches!(err.kind, ErrorKind::MissingCount));
    assert_eq!(err.detail, "fixture: request 1 missing input_tokens");
}

#[test]
fn release_arithmetic_rejects_overflow_and_invalid_counts() {
    let maximum = u64::MAX.to_string();
    assert_eq!(
        evaluate(&record("r", &maximum, "0", "0", "0")).unwrap()["input"],
        u64::MAX
    );
    for bad in ["-1", "true", "1.0", "18446744073709551616", "\"2\""] {
        assert!(matches!(
            evaluate(&record("r", bad, "0", "0", "0")).unwrap_err().kind,
            ErrorKind::InvalidInput
        ));
    }
    let overflow = format!(
        "{}\n{}",
        record("a", &maximum, "0", "0", "0"),
        record("b", "1", "0", "0", "0")
    );
    assert!(matches!(
        evaluate(&overflow).unwrap_err().kind,
        ErrorKind::Overflow
    ));
    assert!(matches!(
        evaluate(&record("r", &maximum, &maximum, "1", "0"))
            .unwrap_err()
            .kind,
        ErrorKind::Overflow
    ));
    assert!(matches!(
        evaluate(&record("r", "10", "8", "8", "0"))
            .unwrap_err()
            .kind,
        ErrorKind::CacheBounds
    ));
}

#[test]
fn duplicate_uses_last_record_and_first_tool_attribution() {
    let input = format!(
        "{}\n{}\n{}\n{}\n{}",
        r#"{"type":"response_item","payload":{"type":"function_call"}}"#,
        record("a", "100", "20", "10", "4"),
        r#"{"type":"response_item","payload":{"type":"function_call"}}"#,
        record("a", "200", "30", "20", "6"),
        record("b", "5", "0", "0", "0")
    );
    let result = evaluate(&input).unwrap();
    assert_eq!(result["turns"], 2);
    assert_eq!(result["input"], 155);
    assert_eq!(result["output"], 6);
    assert_eq!(result["tool_calls"], 2);
    assert_eq!(result["text_only_turns"], 0);
}

#[test]
fn malformed_tail_never_returns_a_partial_summary() {
    assert!(matches!(
        evaluate(&format!(
            "{}\n{{\"type\":",
            record("r", "10", "0", "0", "0")
        ))
        .unwrap_err()
        .kind,
        ErrorKind::MalformedJson
    ));
}

#[test]
fn identities_cannot_collide_with_ordinal_fallback() {
    let anonymous = r#"{"type":"token_usage_record","payload":{"usage":{"input_tokens":1,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0}}}"#;
    let result = evaluate(&format!(
        "{anonymous}\n{}",
        record("ordinal-None", "2", "0", "0", "0")
    ))
    .unwrap();
    assert_eq!(result["turns"], 2);
    assert_eq!(result["input"], 3);
}

#[test]
fn timestamps_models_and_large_integers_survive() {
    let first = record("a", "9007199254740993", "0", "0", "0").replacen(
        '{',
        "{\"timestamp\":\"2026-01-01T01:00:00.250+01:00\",",
        1,
    );
    let last = record("b", "0", "0", "0", "0").replacen(
        '{',
        "{\"timestamp\":\"2026-01-01T00:00:01.999Z\",",
        1,
    );
    let result = evaluate(&format!("{first}\n{last}")).unwrap();
    assert_eq!(result["input"], 9007199254740993_u64);
    assert_eq!(result["wall_seconds"], 1);
    assert_eq!(
        evaluate(&record("r", "0", "0", "0", "0")).unwrap()["wall_seconds"],
        serde_json::Value::Null
    );
}

#[test]
fn rate_validation_and_cost_overflow_are_errors() {
    for invalid in [-1.0, f64::INFINITY, f64::NAN] {
        assert!(Rates::new(invalid, 1.0, 0.1, 1.25).is_err());
    }
    let rollout = read_rollout(Cursor::new(record("r", "100", "0", "0", "0")), "fixture").unwrap();
    assert!(matches!(
        summarize(
            &rollout,
            "fixture",
            Rates::new(f64::MAX, 1.0, 0.1, 1.25).unwrap()
        )
        .unwrap_err()
        .kind,
        ErrorKind::Overflow
    ));
}

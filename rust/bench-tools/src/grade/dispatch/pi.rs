//! A Pi grader session through OpenRouter: the fresh home and agent directory, the copied provider credential, the
//! session and its record.

use super::{
    Dispatch, Outcome, SignIn, already_dispatched, append_charge, check_effort, checked_batch,
    client, client_preflight, enforcement, exclusive, exit_value, field_models, get, hold,
    identities, inconsistent, indented, input, insert, json, named, now, object, path_text,
    problems_after, rate_for, read_bytes, record_logs, released, resolved_work, session, shown,
    start_directory, strings, text, write_file,
};
use crate::current_grading::R;
use crate::grade::Runner;
use crate::pyjson::Value;
use crate::{clean_context, interrupt, pytext, review_isolation, sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

/// The Pi package the session would run, whose directory Pi names to the model.
fn installed_package() -> R<PathBuf> {
    let cli = review_isolation::which(client::pi::NAME)
        .and_then(|found| found.canonicalize().ok())
        .ok_or_else(|| input("cannot run pi: not found on PATH".to_string()))?;
    client::pi::package_of(&cli).map_err(input)
}

/// The session's evidence, or a record that it is unavailable and so cannot prove a zero charge.
fn result(work: &Path, session_id: &str, rate: &Value, exit_code: Option<i32>) -> Value {
    let sessions = client::pi::sessions(&work.join("home"));
    let mut result = client::pi::evidence(&sessions, session_id, rate).unwrap_or_else(|error| {
        object(vec![
            ("session_id", Value::Null),
            ("models_observed", Value::Array(Vec::new())),
            ("subagents", Value::Int("0".to_string())),
            (
                "audit_violations",
                strings([format!("Pi session evidence unavailable: {error}").as_str()]),
            ),
            (
                "usage",
                object(vec![
                    ("priced_total_usd", Value::Null),
                    ("low", Value::Null),
                    ("high", Value::Null),
                    ("billing", text("api-dollars")),
                ]),
            ),
        ])
    });
    if exit_code != Some(0)
        && let Value::Object(entries) = &mut result
        && let Some(usage) = entries.get_mut("usage")
    {
        insert(usage, "high", Value::Null);
    }
    insert(&mut result, "exit_code", exit_value(exit_code));
    insert(&mut result, "budget_policy", text("pi-unbounded"));
    insert(&mut result, "provider_call_possible", Value::Bool(true));
    result
}

/// Run the session from `start` with only the model's provider credential copied into the fresh agent directory;
/// the caller's `SignIn` removes it however the session ends.
fn run(
    runner: &Runner,
    held: &interrupt::Held,
    work: &Path,
    options: &Dispatch,
    start: &Path,
    session_id: &str,
) -> R<(String, Option<i32>)> {
    let home = work.join("home");
    let agent = client::pi::agent_directory(&home);
    let user_home = client::user_home();
    let (provider, _) = client::pi::split(&options.client.model).map_err(input)?;
    clean_context::prepare(work).map_err(input)?;
    let mut arguments = vec![
        "grading".to_string(),
        "tools".to_string(),
        "--work".to_string(),
        work.to_string_lossy().into_owned(),
        "--policy".to_string(),
        work.join("command-policy.json")
            .to_string_lossy()
            .into_owned(),
    ];
    for protected in [&options.key, &user_home] {
        arguments.push("--protected".to_string());
        arguments.push(
            review_isolation::resolve(protected)
                .to_string_lossy()
                .into_owned(),
        );
    }
    arguments.push("--bench-tools".to_string());
    arguments.push(runner.executable.to_string_lossy().into_owned());
    client::pi::configure(&agent, &runner.executable, &arguments).map_err(input)?;
    let tmp = work.join("tmp");
    if let Err(error) = std::fs::create_dir(&tmp)
        && !tmp.is_dir()
    {
        return Err(input(shown(&error, &tmp)));
    }
    let source = crate::current_grading::read_json(&pytext::PyPath::new(pytext::os_bytes(
        client::pi::agent_directory(&user_home)
            .join("auth.json")
            .as_os_str(),
    )))?;
    let entry = get(&source, provider).cloned().unwrap_or(Value::Null);
    let credentials = agent.join("auth.json");
    exclusive(&credentials, 0o600)
        .and_then(|mut file| file.write_all(json(&object(vec![(provider, entry)])).as_bytes()))
        .map_err(|error| {
            input(format!(
                "cannot copy the Pi {provider} credential: {}",
                shown(&error, &credentials)
            ))
        })?;
    let dispatched_at = now();
    write_file(
        &work.join("timing.json"),
        format!(
            "{}\n",
            indented(&object(vec![("root_dispatched_at", text(&dispatched_at))]))
        ),
    )?;
    let created = |name: &str| {
        exclusive(&work.join(name), 0o666).map_err(|error| input(shown(&error, &work.join(name))))
    };
    let (stdout, stderr) = (created("stdout.jsonl")?, created("stderr.txt")?);
    let mut command = released(held, client::pi::NAME)?;
    command
        .args(client::pi::session_arguments(
            &options.client.model,
            Some(&options.effort),
            &client::pi::sessions(&home),
            session_id,
        ))
        .current_dir(start);
    client::pi::environment(&mut command, &home, &tmp);
    let exit = session(
        held,
        client::pi::NAME,
        &mut command,
        &work.join("prompt.md"),
        stdout,
        stderr,
        options.timeout,
    )?;
    Ok((dispatched_at, exit))
}

pub(super) fn dispatch(runner: &Runner, options: &Dispatch) -> R<Outcome> {
    let model = options.client.model.as_str();
    let work = resolved_work(&options.work);
    let prompt = read_bytes(&work.join("prompt.md"))?;
    already_dispatched(&work)?;
    if options.max_budget_usd.is_some() {
        return inconsistent(
            "Pi cannot enforce --max-budget-usd; use explicitly authorized unbounded mode",
        );
    }
    let key = checked_batch(runner, &work, &options.key)?;
    let version = client_preflight(runner, &options.client)?;
    check_effort(runner, &options.client, &options.effort)?;
    let package = installed_package()?;
    let shown_package: Vec<String> = named(&identities(&key)?, &package)
        .iter()
        .map(|identity| {
            format!(
                "the Pi package directory {} names {}; Pi shows its path to the model",
                package.display(),
                pytext::repr_str(identity)
            )
        })
        .collect();
    if !shown_package.is_empty() {
        return inconsistent(shown_package.join("\n"));
    }
    let mut enforcement = enforcement(runner, &work, &options.key, model)?;
    let rate = rate_for(runner, model)?.unwrap_or(Value::Null);
    let session_id = clean_context::uuid4().map_err(input)?;
    let held = hold(runner)?;
    let start = start_directory(&key)?;
    let sign_in = SignIn {
        credentials: client::pi::agent_directory(&work.join("home")).join("auth.json"),
        start: Some(start.clone()),
    };
    let ran = run(runner, &held, &work, options, &start, &session_id);
    drop(sign_in);
    let (dispatched_at, exit_code) = ran?;

    let mut result = result(&work, &session_id, &rate, exit_code);
    let problems = problems_after(runner, &work, &key);
    if let Value::Object(entries) = &mut result
        && let Some(Value::Array(violations)) = entries.get_mut("audit_violations")
    {
        violations.extend(problems.iter().map(|problem| text(problem)));
    }
    let completed_at = now();
    record_logs(&mut enforcement, &work, &key)?;
    insert(&mut enforcement, "client_package", path_text(&package));
    let verdicts_present = work.join("verdicts.json").is_file();
    let mut record = result.clone();
    for (name, value) in [
        ("cli_version", text(&version)),
        ("model", text(model)),
        ("effort", text(&options.effort)),
        ("prompt_sha256", text(&sha256::hex_digest(&prompt))),
        ("dispatched_at", text(&dispatched_at)),
        ("completed_at", text(&completed_at)),
        ("verdicts_present", Value::Bool(verdicts_present)),
        ("enforcement", enforcement),
    ] {
        insert(&mut record, name, value);
    }
    write_file(
        &work.join("dispatch.json"),
        format!("{}\n", indented(&record)),
    )?;

    let field = |name: &str| get(&result, name).cloned().unwrap_or(Value::Null);
    let usage = field("usage");
    let priced = get(&usage, "priced_total_usd")
        .cloned()
        .unwrap_or(Value::Null);
    let mut reasons = Vec::new();
    if exit_code != Some(0) {
        let shown_exit = exit_code.map_or_else(|| "None".to_string(), |code| code.to_string());
        reasons.push(format!(
            "Pi session exit {shown_exit}; timeout limit {} s",
            options.timeout
        ));
    }
    reasons.extend(
        field("audit_violations")
            .as_array()
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string),
    );
    let alone = field_models(&result) == [Some(model)] && !field("subagents").truthy();
    if !alone {
        reasons.push("Pi grader did not run the pinned model alone".to_string());
    }
    if matches!(priced, Value::Null) {
        reasons.push("Pi usage was not priced".to_string());
    } else if exit_code == Some(0) && matches!(get(&usage, "high"), Some(Value::Null)) {
        reasons.push(format!(
            "OpenRouter reported no cost for {} billed response(s); the charge's upper bound is unknown",
            get(&usage, "unreported_requests").map_or_else(String::new, Value::py_str)
        ));
    }
    if !verdicts_present {
        reasons.push("no verdicts.json written".to_string());
    }
    if let Some((run, step)) = &options.charge
        && !matches!(priced, Value::Null)
    {
        append_charge(
            run,
            &object(vec![
                ("at", text(&completed_at)),
                ("step", text(step)),
                ("usd", priced.clone()),
                ("model", text(model)),
                ("billing", text("api-dollars")),
                ("session", text(&session_id[..8])),
            ]),
        )?;
    }
    drop(held);
    if !reasons.is_empty() {
        return Ok(Err(reasons));
    }
    Ok(Ok(format!(
        "graded in {}: Pi session {}, ${}",
        work.display(),
        &session_id[..8],
        json(&priced)
    )))
}

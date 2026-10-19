#![forbid(unsafe_code)]

use bench_tools::normalize_review::{self, Arm};
use bench_tools::pyjson::{self, Duplicates};
use bench_tools::pytext::{self, PyPath};
use bench_tools::schema::{SchemaError, Validator};
use bench_tools::{
    archive, build_packet, claims, derive_packet, diff_identity, evidence, export_explorer,
    normalize_thermo, pyfs, pyunicode, rates, roster, upstream,
};
use clap::builder::PossibleValuesParser;
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::process::ExitCode;

/// Deterministic benchmark tools.
#[derive(Parser)]
#[command(
    name = "bench-tools",
    bin_name = "bench-tools",
    args_override_self = true,
    disable_help_subcommand = true
)]
enum Cli {
    /// List the benchmarks the roster has and the ones still missing.
    #[command(infer_long_args = true)]
    Roster(RosterArgs),
    /// Compute the diff identity of a base..head pair: the changed-file manifest and its SHA-256.
    #[command(infer_long_args = true)]
    DiffIdentity(DiffIdentityArgs),
    /// Normalize one reviewer's native output into the benchmark's common finding schema.
    #[command(infer_long_args = true)]
    NormalizeReview(NormalizeReviewArgs),
    /// Validate the thermo skill's verbatim finding index and map it to the shared schema.
    #[command(infer_long_args = true)]
    NormalizeThermo(NormalizeThermoArgs),
    /// Derive a factual packet from a legacy #137-style packet by removing its run policy.
    #[command(infer_long_args = true)]
    DerivePacket(DerivePacketArgs),
    /// Build a phase-1 review packet for one pull-request target, frozen at a cutoff instant.
    #[command(infer_long_args = true)]
    BuildPacket(Box<BuildPacketArgs>),
    /// Validate a bench manifest against its schema, as check_manifest.py does.
    #[command(infer_long_args = true)]
    CheckManifest(CheckManifestArgs),
    /// Check dated standard, short-context prices against official provider Markdown.
    #[command(infer_long_args = true)]
    Rates(RatesArgs),
    /// Print the maintainer audit's shadow report over the claim registry.
    #[command(infer_long_args = true)]
    Upstream(UpstreamArgs),
    /// List, stage or publish the explorer export from the current facts tools/export_explorer.py writes.
    #[command(infer_long_args = true)]
    ExportExplorer(ExportArgs),
    /// Check cached evidence packages or native scratch archives through bench_tools::archive.
    #[command(subcommand, infer_long_args = true)]
    Archive(ArchiveCommand),
    /// Restore, resolve, verify and recover published evidence, as evidence_store.py, recover_evidence.py and
    /// restore-local.py did.
    #[command(subcommand, infer_long_args = true)]
    Evidence(EvidenceCommand),
}

#[derive(Args)]
struct RosterArgs {
    #[arg(long, default_value = ".", allow_negative_numbers = true)]
    root: OsString,
}

#[derive(Args)]
struct DiffIdentityArgs {
    #[arg(value_names = ["REPO", "BASE", "HEAD"], allow_negative_numbers = true)]
    positional: Vec<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    expect: Option<OsString>,
    #[arg(long)]
    quiet: bool,
}

#[derive(Args)]
struct NormalizeReviewArgs {
    #[arg(long, value_parser = PossibleValuesParser::new(Arm::NAMES))]
    arm: Option<String>,
    #[arg(long, allow_negative_numbers = true)]
    composition: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    payload: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    stdout: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    native_review: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    sessions_dir: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    clone: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    out: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    render: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    timing: Option<OsString>,
}

#[derive(Args)]
struct NormalizeThermoArgs {
    #[arg(long, allow_negative_numbers = true)]
    artifact_root: OsString,
    #[arg(long, allow_negative_numbers = true)]
    clone: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    out: OsString,
    #[arg(
        long,
        default_value = "codex-skill",
        value_parser = PossibleValuesParser::new(["codex-skill", "claude-skill"])
    )]
    arm: String,
}

#[derive(Args)]
struct DerivePacketArgs {
    #[arg(allow_negative_numbers = true)]
    legacy: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    out: Option<OsString>,
    #[arg(long)]
    no_newline_translation: bool,
}

/// `int()` of a pull-request number, in decimal.
fn python_int(value: &str) -> Result<String, String> {
    pyunicode::parse_int(value).map_err(|_| "invalid int value".to_string())
}

#[derive(Args)]
struct BuildPacketArgs {
    #[arg(long, allow_negative_numbers = true)]
    repo: OsString,
    #[arg(long, allow_negative_numbers = true, value_parser = python_int)]
    pr: String,
    #[arg(long, allow_negative_numbers = true)]
    head: OsString,
    #[arg(long, allow_negative_numbers = true)]
    merge_base: OsString,
    #[arg(long, allow_negative_numbers = true)]
    base_sha: OsString,
    #[arg(long, allow_negative_numbers = true)]
    staging: OsString,
    #[arg(long, allow_negative_numbers = true)]
    target: OsString,
    #[arg(
        long,
        allow_negative_numbers = true,
        default_value = build_packet::DEFAULT_EXECUTION_NOTE
    )]
    execution_note: OsString,
    #[arg(long, allow_negative_numbers = true)]
    extra_section: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    truncation_newest: Option<OsString>,
    #[arg(long, allow_negative_numbers = true, value_parser = python_int)]
    ref_pr: Option<String>,
    #[arg(long, allow_negative_numbers = true)]
    spec_issue: Option<OsString>,
    #[arg(
        long,
        allow_negative_numbers = true,
        default_value = build_packet::DEFAULT_EXPERIMENT_LABEL
    )]
    experiment_label: OsString,
    #[arg(
        long,
        allow_negative_numbers = true,
        default_value = build_packet::DEFAULT_SUBAGENT_MODEL
    )]
    subagent_model: OsString,
    #[arg(long)]
    publish_to_fork: bool,
    #[arg(
        long,
        allow_negative_numbers = true,
        default_value = build_packet::DEFAULT_UPSTREAM_REPO
    )]
    upstream_repo: OsString,
    #[arg(
        long,
        allow_negative_numbers = true,
        default_value = build_packet::DEFAULT_ORIGINAL_AUTHOR
    )]
    original_author: OsString,
    #[arg(long, allow_negative_numbers = true)]
    cutoff: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    pushed_at: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    pushed_at_source: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    record: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    replay: Option<OsString>,
    #[arg(long)]
    factual: bool,
    #[arg(long, allow_negative_numbers = true)]
    out: OsString,
}

#[derive(Args)]
struct CheckManifestArgs {
    #[arg(value_names = ["SCHEMA", "MANIFESTS"], allow_negative_numbers = true)]
    names: Vec<OsString>,
}

fn source(value: &str) -> Result<(String, String), String> {
    value
        .split_once('=')
        .filter(|(provider, _)| rates::PROVIDERS.iter().any(|(known, _)| known == provider))
        .map(|(provider, url)| (provider.to_string(), url.to_string()))
        .ok_or_else(|| "expected openai=URL or anthropic=URL".to_string())
}

#[derive(Args)]
struct RatesArgs {
    command: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    rates: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    model: Vec<String>,
    #[arg(long)]
    json: bool,
    #[arg(long, value_name = "PROVIDER=URL", allow_negative_numbers = true, value_parser = source)]
    source: Vec<(String, String)>,
    #[arg(long, default_value = ".", allow_negative_numbers = true)]
    root: OsString,
}

#[derive(Args)]
struct UpstreamArgs {
    command: Vec<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    registry: Option<OsString>,
    #[arg(long, default_value = ".", allow_negative_numbers = true)]
    root: OsString,
}

#[derive(Args)]
struct ExportArgs {
    #[arg(value_name = "{evidence,stage,publish}")]
    command: Vec<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    root: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    facts: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    base_path: Option<String>,
    #[arg(long, allow_negative_numbers = true)]
    stage: Option<OsString>,
    #[arg(long, allow_negative_numbers = true)]
    public: Option<OsString>,
    /// Hidden because the export-explorer corpus pins this command's usage text and only
    /// tools/export_explorer.py passes it.
    #[arg(long, hide = true)]
    restore: bool,
}

#[derive(Subcommand)]
enum ArchiveCommand {
    /// Verify each package the manifests index against its cached archive.
    #[command(infer_long_args = true)]
    CheckPackages {
        #[arg(long, allow_negative_numbers = true)]
        cache: OsString,
        #[arg(required = true, allow_negative_numbers = true)]
        manifests: Vec<OsString>,
    },
    /// Check that each entry of a native scratch archive hashes to its name.
    #[command(infer_long_args = true)]
    CheckScratch {
        #[arg(required = true, allow_negative_numbers = true)]
        archives: Vec<OsString>,
    },
}

#[derive(Subcommand)]
enum EvidenceCommand {
    /// Restore a published manifest's members under ROOT, through the verified cache in ROOT/.cache/evidence.
    #[command(infer_long_args = true)]
    Fetch {
        #[arg(long, allow_negative_numbers = true)]
        manifest: OsString,
        #[arg(long, allow_negative_numbers = true)]
        root: Option<OsString>,
        #[arg(long, allow_negative_numbers = true)]
        path: Vec<OsString>,
        #[arg(long)]
        offline: bool,
    },
    /// Download and restore every package of a published manifest in an empty temporary directory; --receipt
    /// writes a new cold restoration receipt.
    #[command(infer_long_args = true)]
    Verify {
        #[arg(long, allow_negative_numbers = true)]
        manifest: OsString,
        #[arg(long, allow_negative_numbers = true)]
        receipt: Option<OsString>,
    },
    /// Restore one logical path from the manifests in ROOT/bench/evidence/manifests when it is absent, and check it.
    #[command(infer_long_args = true)]
    Resolve {
        #[arg(long, allow_negative_numbers = true)]
        sha256: Option<OsString>,
        #[arg(long, allow_negative_numbers = true)]
        root: Option<OsString>,
        #[arg(long)]
        offline: bool,
        #[arg(allow_negative_numbers = true)]
        path: OsString,
    },
    /// Restore an exact package copy from the recorded recovery release.
    #[command(infer_long_args = true)]
    Recover {
        #[arg(long, allow_negative_numbers = true)]
        source: OsString,
        #[arg(long, allow_negative_numbers = true)]
        root: Option<OsString>,
        #[arg(long, allow_negative_numbers = true)]
        path: Vec<OsString>,
        #[arg(long)]
        offline: bool,
    },
    /// Restore a captured local path into NEW_FILE, as restore-local.py does, from the repository.
    #[command(infer_long_args = true)]
    RestoreLocal {
        #[arg(value_name = "ROOT_NAME", allow_negative_numbers = true)]
        label: OsString,
        #[arg(value_name = "ORIGINAL_RELATIVE_PATH", allow_negative_numbers = true)]
        original: OsString,
        #[arg(value_name = "NEW_FILE", allow_negative_numbers = true)]
        output: OsString,
    },
}

/// Prints a parse error or the help clap renders, and exits as it does: 2 for misuse, 0 for help.
fn report(error: &clap::Error) -> ExitCode {
    if error.print().is_err() {
        return ExitCode::from(2);
    }
    ExitCode::from(u8::try_from(error.exit_code()).unwrap_or(2))
}

/// A usage error found after parsing, reported with the subcommand's usage, exit 2.
fn misuse(command: &str, kind: ErrorKind, message: impl Display) -> ExitCode {
    let mut cli = Cli::command();
    cli.build();
    let error = match cli.find_subcommand_mut(command) {
        Some(subcommand) => subcommand.error(kind, message),
        None => cli.error(kind, message),
    };
    report(&error)
}

fn bytes(value: OsString) -> Vec<u8> {
    value.into_vec()
}

fn write_stdout(bytes: &[u8]) -> Result<(), ExitCode> {
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(bytes)
        .and_then(|()| stdout.flush())
        .map_err(|error| {
            eprintln!("bench-tools: cannot write output: {error}");
            ExitCode::from(2)
        })
}

fn run_roster(arguments: RosterArgs) -> Result<ExitCode, ExitCode> {
    let (lines, code) = match roster::report(&PyPath::new(pytext::os_bytes(&arguments.root))) {
        Ok(roster::Report::Lines(lines)) => (lines, 0),
        Ok(roster::Report::Violations(lines)) => (lines, 1),
        Err(roster::InputError(message)) => {
            eprintln!("roster.py: {message}");
            return Err(ExitCode::from(2));
        }
    };
    let mut output = String::new();
    for line in lines {
        output.push_str(&line);
        output.push('\n');
    }
    write_stdout(output.as_bytes())?;
    Ok(ExitCode::from(code))
}

fn run_diff_identity(arguments: DiffIdentityArgs) -> Result<ExitCode, ExitCode> {
    let [repo, base, head] = &arguments.positional[..] else {
        let (kind, message) = if arguments.positional.len() > 3 {
            (
                ErrorKind::UnknownArgument,
                format!(
                    "unrecognized arguments: {}",
                    pytext::stderr_text(pytext::os_bytes(&arguments.positional[3]))
                ),
            )
        } else {
            (
                ErrorKind::MissingRequiredArgument,
                "give a repository, a base revision and a head revision".to_string(),
            )
        };
        return Err(misuse("diff-identity", kind, message));
    };
    let (rows, digest) = diff_identity::identity(repo, base, head).map_err(|error| {
        eprintln!("diff_identity.py: {}", error.0);
        ExitCode::from(2)
    })?;
    let mut output = if arguments.quiet {
        Vec::new()
    } else {
        diff_identity::render(&rows)
    };
    output.extend_from_slice(
        if arguments.quiet {
            format!("{digest}\n")
        } else {
            format!("sha256 {digest}\n")
        }
        .as_bytes(),
    );
    let expected = arguments
        .expect
        .as_ref()
        .map(|value| pytext::os_bytes(value))
        .filter(|value| !value.is_empty());
    let mismatch = expected.filter(|value| *value != digest.as_bytes());
    if let Some(value) = mismatch {
        output.extend_from_slice(b"diff identity mismatch: expected ");
        output.extend_from_slice(value);
        output.extend_from_slice(format!(", computed {digest}\n").as_bytes());
    }
    write_stdout(&output)?;
    Ok(ExitCode::from(if mismatch.is_some() { 1 } else { 0 }))
}

/// Writes stderr first: Python writes it as it goes and stdout when it exits, so a caller that sends
/// both to one file, as `dispatch.sh` does, sees that order.
fn finish(outcome: &normalize_review::Outcome) -> Result<ExitCode, ExitCode> {
    let mut stderr = std::io::stderr().lock();
    let _ = stderr
        .write_all(&outcome.stderr)
        .and_then(|()| stderr.flush());
    write_stdout(&outcome.stdout)?;
    Ok(ExitCode::from(outcome.code))
}

fn run_normalize_review(arguments: NormalizeReviewArgs) -> Result<ExitCode, ExitCode> {
    let arm = arguments
        .arm
        .as_deref()
        .and_then(|name| Arm::parse(name.as_bytes()));
    let parsed = normalize_review::Arguments {
        arm,
        composition: arguments.composition.map(bytes),
        payload: arguments.payload.map(bytes),
        stdout: arguments.stdout.map(bytes),
        native_review: arguments.native_review.map(bytes),
        sessions_dir: arguments.sessions_dir.map(bytes),
        clone: arguments.clone.map(bytes),
        out: arguments.out.map(bytes),
        render: arguments.render.map(bytes),
        timing: arguments.timing.map(bytes),
    };
    let given = |value: &Option<Vec<u8>>| value.as_ref().is_some_and(|value| !value.is_empty());
    if !given(&parsed.render) {
        let required = match arm {
            None => {
                return Err(misuse(
                    "normalize-review",
                    ErrorKind::MissingRequiredArgument,
                    "--arm is required",
                ));
            }
            Some(Arm::ReviewCode) => (&parsed.composition, "--composition"),
            Some(Arm::ClaudeBuiltin) => (&parsed.payload, "--payload"),
            Some(Arm::Codex) => (&parsed.stdout, "--stdout"),
            Some(Arm::CodexSkill | Arm::ClaudeSkill) => (&parsed.native_review, "--native-review"),
        };
        if !given(required.0) {
            let arm = arm.map_or("", Arm::name);
            return Err(misuse(
                "normalize-review",
                ErrorKind::MissingRequiredArgument,
                format!("{} is required for {arm}", required.1),
            ));
        }
    }
    finish(&normalize_review::command(
        &parsed,
        &normalize_review::utc_now,
    ))
}

fn run_normalize_thermo(arguments: NormalizeThermoArgs) -> Result<ExitCode, ExitCode> {
    let Some(arm) = normalize_thermo::arm(arguments.arm.as_bytes()) else {
        return Err(ExitCode::from(2));
    };
    finish(&normalize_thermo::command(&normalize_thermo::Arguments {
        artifact_root: bytes(arguments.artifact_root),
        clone: arguments.clone.map(bytes),
        out: bytes(arguments.out),
        arm,
    }))
}

fn run_derive_packet(arguments: DerivePacketArgs) -> Result<ExitCode, ExitCode> {
    let (Some(legacy), Some(out)) = (arguments.legacy, arguments.out) else {
        return Err(misuse(
            "derive-packet",
            ErrorKind::MissingRequiredArgument,
            "give the legacy packet and --out",
        ));
    };
    if legacy.is_empty() || out.is_empty() {
        return Err(misuse(
            "derive-packet",
            ErrorKind::MissingRequiredArgument,
            "give the legacy packet and --out",
        ));
    }
    let newlines = if arguments.no_newline_translation {
        derive_packet::Newlines::Keep
    } else {
        derive_packet::Newlines::Translate
    };
    let legacy = PyPath::new(pytext::os_bytes(&legacy));
    let out_path = PyPath::new(pytext::os_bytes(&out));
    match derive_packet::derive_file(&legacy, &out_path, newlines) {
        Ok(digest) => {
            let mut line = bytes(out);
            line.extend_from_slice(format!(" sha256 {digest}\n").as_bytes());
            write_stdout(&line)?;
            Ok(ExitCode::SUCCESS)
        }
        Err(derive_packet::Failure::Structure(error)) => {
            write_stdout(format!("{}\n", error.0).as_bytes())?;
            Ok(ExitCode::from(1))
        }
        Err(derive_packet::Failure::Input(error)) => {
            eprintln!("derive_packet.py: {}", error.0);
            Err(ExitCode::from(2))
        }
    }
}

fn read_json_document(path: &[u8]) -> Result<pyjson::Value, String> {
    let text =
        pytext::read_text(&PyPath::new(path)).map_err(|error| error.message().to_string())?;
    pyjson::loads(&text, Duplicates::KeepLast).map_err(|error| error.to_string())
}

fn run_check_manifest(arguments: CheckManifestArgs) -> Result<ExitCode, ExitCode> {
    let names = arguments.names;
    if names.len() < 2 || names[0].is_empty() {
        return Err(misuse(
            "check-manifest",
            ErrorKind::MissingRequiredArgument,
            "give a schema and at least one manifest",
        ));
    }
    let schema_name = pytext::os_bytes(&names[0]);
    let schema = read_json_document(schema_name).map_err(|error| {
        eprintln!(
            "check_manifest.py: {}: {error}",
            pytext::stderr_text(schema_name)
        );
        ExitCode::from(2)
    })?;
    let mut validator = Validator::default();
    let mut output = Vec::new();
    let mut failed = false;
    for name in &names[1..] {
        let name = pytext::os_bytes(name);
        let problems = match read_json_document(name) {
            Ok(value) => validator
                .validate(&schema, &value)
                .map_err(|SchemaError(error)| (pytext::stderr_text(schema_name), error)),
            Err(error) => Err((pytext::stderr_text(name), error)),
        };
        let problems = match problems {
            Ok(problems) => problems,
            Err((file, error)) => {
                write_stdout(&output)?;
                eprintln!("check_manifest.py: {file}: {error}");
                return Err(ExitCode::from(2));
            }
        };
        for problem in &problems {
            output.extend_from_slice(name);
            output.push(b' ');
            output.extend_from_slice(problem.as_bytes());
            output.push(b'\n');
        }
        if problems.is_empty() {
            output.extend_from_slice(name);
            output.extend_from_slice(b": ok\n");
        }
        failed |= !problems.is_empty();
    }
    write_stdout(&output)?;
    Ok(ExitCode::from(u8::from(failed)))
}

fn run_rates(arguments: RatesArgs) -> Result<ExitCode, ExitCode> {
    match arguments.command.as_deref().map(pytext::os_bytes) {
        Some(b"check") => {}
        Some(b"refresh") => {
            return Err(misuse(
                "rates",
                ErrorKind::InvalidValue,
                "refresh writes the catalog and stays in Python: python3 bench/tools/rates.py refresh",
            ));
        }
        None => {
            return Err(misuse(
                "rates",
                ErrorKind::MissingRequiredArgument,
                "give the command check",
            ));
        }
        Some(other) => {
            return Err(misuse(
                "rates",
                ErrorKind::InvalidValue,
                format!(
                    "argument command: invalid choice: {} (choose from 'check')",
                    pytext::repr(&pytext::surrogateescape(other))
                ),
            ));
        }
    }
    let mut sources: Vec<(String, String)> = rates::PROVIDERS
        .iter()
        .map(|(provider, url)| ((*provider).to_string(), (*url).to_string()))
        .collect();
    for (provider, url) in arguments.source {
        for entry in &mut sources {
            if entry.0 == provider {
                entry.1.clone_from(&url);
            }
        }
    }
    let root = PyPath::new(&pyfs::resolve(pytext::os_bytes(&arguments.root)));
    let catalog_path = match &arguments.rates {
        Some(path) => PyPath::new(pytext::os_bytes(path)),
        None => root.join(b"bench/rates.current.json"),
    };
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let models = (!arguments.model.is_empty()).then_some(arguments.model.as_slice());
    let report = rates::read_catalog(&root, &catalog_path).and_then(|catalog| {
        rates::inspect(
            &catalog,
            models,
            &sources,
            &rates::Network,
            &rates::utc_timestamp(seconds),
        )
    });
    let report = match report {
        Ok(report) => report,
        Err(rates::Failure::Rate(message)) => {
            write_stdout(format!("rates.py: {message}\n").as_bytes())?;
            return Ok(ExitCode::from(1));
        }
        Err(rates::Failure::Input(message)) => {
            eprintln!("rates.py: {message}");
            return Err(ExitCode::from(2));
        }
    };
    let output = if arguments.json {
        let options = pyjson::Dumps {
            indent: Some(2),
            ..pyjson::Dumps::default()
        };
        format!("{}\n", pyjson::dumps(&report, &options))
    } else {
        rates::render(&report)
    };
    write_stdout(output.as_bytes())?;
    let changed = report
        .as_object()
        .and_then(|report| report.get("changes"))
        .and_then(pyjson::Value::as_array)
        .is_some_and(|changes| !changes.is_empty());
    Ok(ExitCode::from(u8::from(changed)))
}

fn run_upstream(arguments: UpstreamArgs) -> Result<ExitCode, ExitCode> {
    match arguments
        .command
        .iter()
        .map(|value| pytext::os_bytes(value))
        .collect::<Vec<_>>()[..]
    {
        [b"report"] => {}
        [b"collect", ..] => {
            return Err(misuse(
                "upstream",
                ErrorKind::InvalidValue,
                "collect fetches snapshots and stays in Python: python3 bench/tools/upstream.py collect",
            ));
        }
        _ => {
            return Err(misuse(
                "upstream",
                ErrorKind::MissingRequiredArgument,
                "give the command report",
            ));
        }
    }
    let mut repository = claims::Repository::new(pytext::os_bytes(&arguments.root));
    let registry = match &arguments.registry {
        Some(path) => pytext::os_bytes(path).to_vec(),
        None => repository.path(b"bench/claims/registry.json"),
    };
    let report = repository
        .load_registry(&registry)
        .and_then(|(references, cases)| upstream::report(&mut repository, &cases, &references));
    match report {
        Ok(report) => {
            let options = pyjson::Dumps {
                indent: Some(2),
                ..pyjson::Dumps::default()
            };
            write_stdout(format!("{}\n", pyjson::dumps(&report, &options)).as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        Err(claims::Failure(message)) => {
            eprintln!("upstream.py: {message}");
            Err(ExitCode::from(1))
        }
    }
}

fn utf8_option(option: &str, value: OsString) -> Result<String, ExitCode> {
    value.into_string().map_err(|value| {
        misuse(
            "build-packet",
            ErrorKind::InvalidUtf8,
            format!(
                "argument {option}: not UTF-8 text: {}",
                pytext::repr(&pytext::surrogateescape(pytext::os_bytes(&value)))
            ),
        )
    })
}

fn optional_utf8(option: &str, value: Option<OsString>) -> Result<Option<String>, ExitCode> {
    value.map(|value| utf8_option(option, value)).transpose()
}

fn build_packet_arguments(arguments: BuildPacketArgs) -> Result<build_packet::Arguments, ExitCode> {
    Ok(build_packet::Arguments {
        repo: utf8_option("--repo", arguments.repo)?,
        pr: arguments.pr,
        head: utf8_option("--head", arguments.head)?,
        merge_base: utf8_option("--merge-base", arguments.merge_base)?,
        base_sha: utf8_option("--base-sha", arguments.base_sha)?,
        staging: bytes(arguments.staging),
        target: utf8_option("--target", arguments.target)?,
        execution_note: utf8_option("--execution-note", arguments.execution_note)?,
        extra_section: arguments.extra_section.map(bytes),
        truncation_newest: optional_utf8("--truncation-newest", arguments.truncation_newest)?,
        ref_pr: arguments.ref_pr,
        spec_issue: optional_utf8("--spec-issue", arguments.spec_issue)?,
        experiment_label: utf8_option("--experiment-label", arguments.experiment_label)?,
        subagent_model: utf8_option("--subagent-model", arguments.subagent_model)?,
        publish_to_fork: arguments.publish_to_fork,
        upstream_repo: utf8_option("--upstream-repo", arguments.upstream_repo)?,
        original_author: utf8_option("--original-author", arguments.original_author)?,
        cutoff: optional_utf8("--cutoff", arguments.cutoff)?,
        pushed_at: optional_utf8("--pushed-at", arguments.pushed_at)?,
        pushed_at_source: optional_utf8("--pushed-at-source", arguments.pushed_at_source)?,
        record: arguments.record.map(bytes),
        replay: arguments.replay.map(bytes),
        factual: arguments.factual,
        out: bytes(arguments.out),
    })
}

fn run_build_packet(arguments: BuildPacketArgs) -> Result<ExitCode, ExitCode> {
    match build_packet::build(&build_packet_arguments(arguments)?) {
        Ok(report) => {
            write_stdout(&report)?;
            Ok(ExitCode::SUCCESS)
        }
        Err(build_packet::Failure::Refused(text)) => {
            write_stdout(format!("{text}\n").as_bytes())?;
            Ok(ExitCode::from(1))
        }
        Err(build_packet::Failure::Input(message)) => {
            eprintln!("build_packet: {message}");
            Err(ExitCode::from(2))
        }
        Err(build_packet::Failure::Unsupported(exception)) => {
            eprintln!("build_packet: unsupported input: {exception}");
            Err(ExitCode::from(2))
        }
    }
}

fn run_export_explorer(arguments: ExportArgs) -> Result<ExitCode, ExitCode> {
    let [command] = arguments.command.as_slice() else {
        return Err(misuse(
            "export-explorer",
            ErrorKind::MissingRequiredArgument,
            "give one command: evidence, stage or publish",
        ));
    };
    let command = pytext::os_bytes(command);
    let required: &[(&str, bool)] = match command {
        b"evidence" => &[("--facts", arguments.facts.is_some())],
        b"stage" => &[
            ("--facts", arguments.facts.is_some()),
            ("--stage", arguments.stage.is_some()),
        ],
        b"publish" => &[
            ("--stage", arguments.stage.is_some()),
            ("--public", arguments.public.is_some()),
        ],
        other => {
            return Err(misuse(
                "export-explorer",
                ErrorKind::InvalidValue,
                format!(
                    "argument command: invalid choice: {} (choose from 'evidence', 'stage', 'publish')",
                    pytext::repr(&pytext::surrogateescape(other))
                ),
            ));
        }
    };
    if arguments.restore && command != b"evidence" {
        return Err(misuse(
            "export-explorer",
            ErrorKind::ArgumentConflict,
            "--restore applies only to evidence",
        ));
    }
    let missing: Vec<&str> = required
        .iter()
        .filter(|(_, given)| !given)
        .map(|(name, _)| *name)
        .collect();
    if !missing.is_empty() {
        return Err(misuse(
            "export-explorer",
            ErrorKind::MissingRequiredArgument,
            format!(
                "the following arguments are required: {}",
                missing.join(", ")
            ),
        ));
    }
    let root = pyfs::resolve(
        arguments
            .root
            .as_deref()
            .map_or(b".".as_slice(), pytext::os_bytes),
    );
    let base_path = arguments.base_path.as_deref().unwrap_or("/");
    let path = |value: &Option<OsString>| {
        value
            .as_deref()
            .map(pytext::os_bytes)
            .unwrap_or_default()
            .to_vec()
    };
    let outcome = match command {
        b"evidence" => export_explorer::Facts::read(&path(&arguments.facts))
            .and_then(|facts| export_explorer::evidence(&root, &facts, base_path))
            .and_then(|listed| {
                if arguments.restore {
                    let mut store = evidence::Store {
                        remote: &mut evidence::GitHub,
                        disk: &mut evidence::System,
                        now: &evidence::now_isoformat,
                        temporary: evidence::system_temp(),
                    };
                    export_explorer::restore(&root, &listed, &mut store)?;
                }
                Ok(listed)
            })
            .map(|listed| format!("{}\n", pyjson::dumps(&listed, &pyjson::Dumps::default()))),
        b"stage" => export_explorer::stage(
            &root,
            &path(&arguments.facts),
            base_path,
            &path(&arguments.stage),
        )
        .map(|summary| format!("{summary}\n")),
        _ => export_explorer::publish(
            &path(&arguments.stage),
            &path(&arguments.public),
            &mut export_explorer::System,
        )
        .map(|()| String::new()),
    };
    match outcome {
        Ok(output) => {
            write_stdout(output.as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        Err(failure) => {
            eprintln!("export_explorer.py: {}", failure.line());
            Err(ExitCode::from(1))
        }
    }
}

fn archive_report(rows: Vec<pyjson::Value>, key: &str, totals: Vec<(&str, u64)>) -> Vec<u8> {
    let mut report = pyjson::Object::default();
    report.insert(key.to_string(), pyjson::Value::Array(rows));
    let mut sums = pyjson::Object::default();
    for (name, value) in totals {
        sums.insert(name.to_string(), pyjson::Value::Int(value.to_string()));
    }
    report.insert("totals".to_string(), pyjson::Value::Object(sums));
    let options = pyjson::Dumps {
        indent: Some(2),
        ..pyjson::Dumps::default()
    };
    format!(
        "{}\n",
        pyjson::dumps(&pyjson::Value::Object(report), &options)
    )
    .into_bytes()
}

fn archive_row(entries: Vec<(&str, pyjson::Value)>) -> pyjson::Value {
    let mut row = pyjson::Object::default();
    for (key, value) in entries {
        row.insert(key.to_string(), value);
    }
    pyjson::Value::Object(row)
}

fn finish_archive(output: &[u8], failed: u64) -> Result<ExitCode, ExitCode> {
    write_stdout(output)?;
    Ok(ExitCode::from(u8::from(failed > 0)))
}

fn check_packages(cache: &OsStr, names: &[OsString]) -> Result<ExitCode, ExitCode> {
    let mut manifests = Vec::new();
    let (mut packages, mut members, mut bytes, mut failed) = (0, 0, 0, 0);
    for name in names {
        let shown = pytext::stderr_text(pytext::os_bytes(name));
        let indexed = read_json_document(pytext::os_bytes(name))
            .and_then(|manifest| archive::indexed_packages(&manifest))
            .map_err(|error| {
                eprintln!("archive: {shown}: {error}");
                ExitCode::from(2)
            })?;
        let mut rows = Vec::new();
        for package in &indexed {
            let result = archive::check_package(std::path::Path::new(&cache), package);
            packages += 1;
            members += package.members.len() as u64;
            bytes = u64::saturating_add(bytes, package.bytes);
            failed += u64::from(result.is_err());
            rows.push(archive_row(vec![
                ("sha256", pyjson::Value::Str(package.sha256.clone())),
                (
                    "members",
                    pyjson::Value::Int(package.members.len().to_string()),
                ),
                ("bytes", pyjson::Value::Int(package.bytes.to_string())),
                (
                    "result",
                    pyjson::Value::Str(result.err().unwrap_or_else(|| "verified".to_string())),
                ),
            ]));
        }
        manifests.push(archive_row(vec![
            ("manifest", pyjson::Value::Str(shown)),
            ("packages", pyjson::Value::Array(rows)),
        ]));
    }
    let totals = vec![
        ("manifests", names.len() as u64),
        ("packages", packages),
        ("members", members),
        ("bytes", bytes),
        ("verified", packages - failed),
        ("failed", failed),
    ];
    finish_archive(&archive_report(manifests, "manifests", totals), failed)
}

fn check_scratch(names: &[OsString]) -> Result<ExitCode, ExitCode> {
    let (mut rows, mut entries, mut failed) = (Vec::new(), 0, 0);
    for name in names {
        let result = archive::check_scratch(std::path::Path::new(name));
        let count = *result.as_ref().unwrap_or(&0);
        entries += count;
        failed += u64::from(result.is_err());
        rows.push(archive_row(vec![
            (
                "archive",
                pyjson::Value::Str(pytext::stderr_text(pytext::os_bytes(name))),
            ),
            ("entries", pyjson::Value::Int(count.to_string())),
            (
                "result",
                pyjson::Value::Str(result.err().unwrap_or_else(|| "verified".to_string())),
            ),
        ]));
    }
    let totals = vec![
        ("archives", names.len() as u64),
        ("entries", entries),
        ("verified", names.len() as u64 - failed),
        ("failed", failed),
    ];
    finish_archive(&archive_report(rows, "archives", totals), failed)
}

/// `--root ROOT` as `Path(ROOT)`, or `Path.cwd()`.
fn evidence_root(root: Option<&OsStr>) -> Result<PyPath, ExitCode> {
    match root {
        Some(root) => Ok(PyPath::new(pytext::os_bytes(root))),
        None => std::env::current_dir()
            .map(|cwd| PyPath::new(pytext::os_bytes(cwd.as_os_str())))
            .map_err(|error| {
                eprintln!("evidence: {}", pytext::os_error(&error, b"."));
                ExitCode::from(2)
            }),
    }
}

fn lossy(value: &OsStr) -> String {
    String::from_utf8_lossy(pytext::os_bytes(value)).into_owned()
}

fn lossy_paths(paths: &[OsString]) -> Option<Vec<String>> {
    (!paths.is_empty()).then(|| paths.iter().map(|path| lossy(path)).collect())
}

/// How `evidence_store.main` reports an outcome: violations on stdout with exit 1, I/O on stderr with exit 2.
fn evidence_outcome(outcome: Result<String, evidence::Error>) -> Result<ExitCode, ExitCode> {
    match outcome {
        Ok(line) => {
            write_stdout(format!("{line}\n").as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        Err(evidence::Error::Violation(message)) => {
            write_stdout(format!("evidence: {message}\n").as_bytes())?;
            Err(ExitCode::from(1))
        }
        Err(evidence::Error::Io(message)) => {
            eprintln!("evidence: {message}");
            Err(ExitCode::from(2))
        }
    }
}

fn run_evidence(command: EvidenceCommand) -> Result<ExitCode, ExitCode> {
    if let EvidenceCommand::RestoreLocal {
        label,
        original,
        output,
    } = command
    {
        return run_restore_local(&label, &original, &output);
    }
    let mut remote = evidence::GitHub;
    let mut disk = evidence::System;
    let mut store = evidence::Store {
        remote: &mut remote,
        disk: &mut disk,
        now: &evidence::now_isoformat,
        temporary: evidence::system_temp(),
    };
    let outcome = match command {
        EvidenceCommand::Fetch {
            manifest,
            root,
            path,
            offline,
        } => {
            let root = evidence_root(root.as_deref())?;
            evidence::read(&PyPath::new(pytext::os_bytes(&manifest)))
                .and_then(|document| {
                    evidence::materialize(
                        &mut store,
                        &document,
                        &root,
                        lossy_paths(&path).as_deref(),
                        offline,
                    )
                })
                .map(|_| "fetch: verified".to_string())
        }
        EvidenceCommand::Verify { manifest, receipt } => {
            let manifest = PyPath::new(pytext::os_bytes(&manifest));
            match receipt {
                Some(receipt) => evidence::restoration_receipt(
                    &mut store,
                    &manifest,
                    &PyPath::new(pytext::os_bytes(&receipt)),
                )
                .map(|_| "verify: verified".to_string()),
                None => evidence::read(&manifest)
                    .and_then(|document| evidence::verify_remote(&mut store, &document))
                    .map(|()| "verify: verified".to_string()),
            }
        }
        EvidenceCommand::Resolve {
            sha256,
            root,
            offline,
            path,
        } => {
            let root = evidence_root(root.as_deref())?;
            evidence::resolve(
                &mut store,
                &root,
                &lossy(&path),
                sha256.as_deref().map(lossy).as_deref(),
                offline,
            )
            .map(|_| "resolve: verified".to_string())
        }
        EvidenceCommand::Recover {
            source,
            root,
            path,
            offline,
        } => {
            let root = evidence_root(root.as_deref())?;
            evidence::recovery::recover(
                &mut store,
                &root,
                &lossy(&source),
                lossy_paths(&path).as_deref(),
                offline,
            )
            .map(|()| "Recovery copy verified and restored.".to_string())
        }
        EvidenceCommand::RestoreLocal { .. } => unreachable!("handled above"),
    };
    evidence_outcome(outcome)
}

/// `restore-local.py ROOT_NAME ORIGINAL_RELATIVE_PATH NEW_FILE`, run in the repository: every refusal is its message
/// alone on stderr, with exit 1.
fn run_restore_local(
    label: &OsStr,
    original: &OsStr,
    output: &OsStr,
) -> Result<ExitCode, ExitCode> {
    let root = std::env::current_dir()
        .map(|cwd| PyPath::new(pytext::os_bytes(cwd.as_os_str())))
        .map_err(|error| {
            eprintln!("{}", pytext::os_error(&error, b"."));
            ExitCode::from(1)
        })?;
    let mut remote = evidence::GitHub;
    let mut disk = evidence::System;
    let mut store = evidence::Store {
        remote: &mut remote,
        disk: &mut disk,
        now: &evidence::now_isoformat,
        temporary: evidence::system_temp(),
    };
    match evidence::recovery::restore_local(
        &mut store,
        &root,
        &lossy(label),
        &lossy(original),
        pytext::os_bytes(output),
    ) {
        Ok(line) => {
            write_stdout(format!("{line}\n").as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            eprintln!("{}", error.message());
            Err(ExitCode::from(1))
        }
    }
}

/// argparse reads a dash-led argument as an option's value when it names no option and reads as a negative
/// number or holds a space; `build-packet-option-values-with-leading-dash` pins that. clap takes only numbers,
/// so build-packet's arguments attach such a value to its option with `=` before clap parses them.
fn attach_dash_led_values(arguments: Vec<OsString>) -> Vec<OsString> {
    if arguments.get(1).map(|name| pytext::os_bytes(name)) != Some(b"build-packet") {
        return arguments;
    }
    let mut command = Cli::command();
    command.build();
    let Some(build_packet) = command.find_subcommand("build-packet") else {
        return arguments;
    };
    let options: Vec<(String, bool)> = build_packet
        .get_arguments()
        .filter_map(|argument| {
            let takes_value = argument.get_action().takes_values();
            argument
                .get_long()
                .map(|long| (format!("--{long}"), takes_value))
        })
        .collect();
    let takes_value = |argument: &[u8]| {
        if !argument.starts_with(b"--") || argument.contains(&b'=') {
            return false;
        }
        let matches: Vec<bool> = match options.iter().find(|(name, _)| name.as_bytes() == argument)
        {
            Some((_, takes)) => vec![*takes],
            None => options
                .iter()
                .filter(|(name, _)| name.as_bytes().starts_with(argument))
                .map(|(_, takes)| *takes)
                .collect(),
        };
        matches == [true]
    };
    let names_option = |argument: &[u8]| {
        let prefix = argument
            .split(|byte| *byte == b'=')
            .next()
            .unwrap_or_default();
        argument.starts_with(b"-h")
            || options.iter().any(|(name, _)| {
                name.as_bytes() == prefix
                    || (argument.starts_with(b"--") && name.as_bytes().starts_with(prefix))
            })
    };
    let dash_led_value = |argument: &[u8]| {
        let text = String::from_utf8_lossy(argument);
        let mut rest = text.chars().skip(1).peekable();
        if rest.peek() == Some(&'.') {
            rest.next();
        }
        let negative_number = rest
            .next()
            .is_some_and(|c| pytext::decimal_value(c).is_some());
        argument.len() > 1
            && argument[0] == b'-'
            && !names_option(argument)
            && (negative_number || argument.contains(&b' '))
    };
    let mut attached = Vec::with_capacity(arguments.len());
    let mut rest = arguments.into_iter().peekable();
    while let Some(argument) = rest.next() {
        let bytes = pytext::os_bytes(&argument);
        if bytes == b"--" {
            attached.push(argument);
            attached.extend(rest);
            break;
        }
        match rest.next_if(|value| takes_value(bytes) && dash_led_value(pytext::os_bytes(value))) {
            Some(value) => {
                let mut joined = argument.into_vec();
                joined.push(b'=');
                joined.extend_from_slice(pytext::os_bytes(&value));
                attached.push(OsString::from_vec(joined));
            }
            None => attached.push(argument),
        }
    }
    attached
}

/// argparse refuses `-h`, or a run of `h`s such as `-hh`, with an attached argument that starts with `=` or `-`,
/// such as `-h=1`, `-hh=1` or `-h-x`, exit 2, where clap prints help. The first `-h` before `--` decides, as argparse acts on the first help it reads.
fn refuse_help_with_explicit_argument(arguments: &[OsString]) -> Option<ExitCode> {
    let command = arguments
        .get(1)?
        .to_str()
        .filter(|name| !name.starts_with('-'))?;
    let help = arguments
        .iter()
        .skip(2)
        .map(|argument| pytext::os_bytes(argument))
        .take_while(|argument| *argument != b"--")
        .find(|argument| argument.starts_with(b"-h"))?;
    let attached = &help[2..];
    let attached = &attached[attached.iter().take_while(|byte| **byte == b'h').count()..];
    let explicit = match attached.first()? {
        b'=' => &attached[1..],
        b'-' => attached,
        _ => return None,
    };
    Some(misuse(
        command,
        ErrorKind::UnknownArgument,
        format!(
            "argument -h/--help: ignored explicit argument {}",
            pytext::repr(&pytext::surrogateescape(explicit))
        ),
    ))
}

fn main() -> ExitCode {
    let arguments: Vec<OsString> = std::env::args_os().collect();
    // clap reads `-hx` before a subcommand as `-h` and prints help; the top level has no Python
    // counterpart, so it keeps the former parser's refusal of an unknown command.
    if let Some(command) = arguments.get(1).map(|name| pytext::os_bytes(name))
        && command.len() > 2
        && command.starts_with(b"-h")
    {
        return misuse(
            "",
            ErrorKind::InvalidSubcommand,
            format!("unknown command {}", pytext::stderr_text(command)),
        );
    }
    let cli = match Cli::try_parse_from(attach_dash_led_values(arguments.clone())) {
        Ok(cli) => cli,
        Err(error) if error.kind() == ErrorKind::DisplayHelp => {
            return refuse_help_with_explicit_argument(&arguments)
                .unwrap_or_else(|| report(&error));
        }
        Err(error) => return report(&error),
    };
    let outcome = match cli {
        Cli::Roster(arguments) => run_roster(arguments),
        Cli::DiffIdentity(arguments) => run_diff_identity(arguments),
        Cli::NormalizeReview(arguments) => run_normalize_review(arguments),
        Cli::NormalizeThermo(arguments) => run_normalize_thermo(arguments),
        Cli::DerivePacket(arguments) => run_derive_packet(arguments),
        Cli::BuildPacket(arguments) => run_build_packet(*arguments),
        Cli::CheckManifest(arguments) => run_check_manifest(arguments),
        Cli::Rates(arguments) => run_rates(arguments),
        Cli::Upstream(arguments) => run_upstream(arguments),
        Cli::ExportExplorer(arguments) => run_export_explorer(arguments),
        Cli::Archive(ArchiveCommand::CheckPackages { cache, manifests }) => {
            check_packages(&cache, &manifests)
        }
        Cli::Archive(ArchiveCommand::CheckScratch { archives }) => check_scratch(&archives),
        Cli::Evidence(command) => run_evidence(command),
    };
    outcome.unwrap_or_else(|code| code)
}

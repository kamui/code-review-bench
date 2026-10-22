//! Running one case: a disposable root populated from the case, each step's command against the built
//! bench-tools, and a snapshot of every entry the command left.

use crate::corpus::{self, field, has, relative, string};
use crate::normalize::{self, Entry, Kind, Observed};
use crate::{b64, fixture_git};
use bench_tools::pyjson::Value;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, Instant};

const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

/// Writers of executable files take this exclusively, and every spawn shares it, so no child forked by another
/// worker holds a file open for writing while a case executes it (ETXTBSY).
static SPAWN: RwLock<()> = RwLock::new(());

pub enum Failure {
    /// The host cannot provide what the case needs; reported as a skip with its reason.
    Unavailable(String),
    Error(String),
}

impl From<String> for Failure {
    fn from(error: String) -> Self {
        Self::Error(error)
    }
}

impl From<&str> for Failure {
    fn from(error: &str) -> Self {
        Self::Error(error.to_string())
    }
}

/// The directory that holds the `gh` stub, the fault library and the bench-tools store entry for the whole run.
pub struct Scratch {
    pub tools: PathBuf,
    pub executable: PathBuf,
}

impl Scratch {
    pub fn create() -> Result<Self, String> {
        let tools =
            std::env::temp_dir().join(format!("behavior-contract-bin-{}", std::process::id()));
        make_private_dir(&tools)?;
        let quoted = bench_tools::shlex::quote(
            &std::env::current_exe()
                .map_err(|error| format!("current executable: {error}"))?
                .to_string_lossy(),
        );
        let stub = tools.join("gh");
        std::fs::write(
            &stub,
            format!("#!/bin/sh\nexec {quoted} --contract-gh-stub \"$@\"\n"),
        )
        .map_err(|error| format!("gh stub: {error}"))?;
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
            .map_err(|error| format!("gh stub: {error}"))?;
        let executable = install(&tools)?;
        Ok(Self { tools, executable })
    }
}

/// Lays the built bench-tools out as an installed store entry, `<store>/bench-tools/<sha256>/bench-tools` beside
/// its `receipt.json`, because grading pins the runtime it runs from and refuses an executable outside a store.
fn install(tools: &Path) -> Result<PathBuf, String> {
    let built = Path::new(env!("CARGO_BIN_EXE_bench-tools"));
    let fail = |error: std::io::Error| format!("bench-tools store entry: {error}");
    let body = std::fs::read(built).map_err(fail)?;
    let sha = bench_tools::sha256::hex_digest(&body);
    let entry = tools.join("store/bench-tools").join(&sha);
    std::fs::create_dir_all(&entry).map_err(fail)?;
    let executable = entry.join("bench-tools");
    if std::fs::hard_link(built, &executable).is_err() {
        std::fs::write(&executable, &body).map_err(fail)?;
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755))
            .map_err(fail)?;
    }
    let commit = "0".repeat(40);
    let zero = "0".repeat(64);
    let receipt = format!(
        r#"{{"schema": "rust-runtime-receipt/1",
 "crate": {{"name": "bench-tools", "bin": "bench-tools", "path": "rust/bench-tools"}},
 "source": {{"revision": "{commit}", "tree": "{commit}", "cargo_toml_sha256": "{zero}",
  "cargo_lock_sha256": "{zero}", "toolchain_file_sha256": "{zero}"}},
 "toolchain": {{"rustc": {{"release": "contract-runner"}}}}, "target": "{target}",
 "build": {{"command": ["cargo", "test"]}},
 "artifact": {{"sha256": "{sha}", "file": "bench-tools", "size": {size}}}}}"#,
        target = std::env::consts::ARCH,
        size = body.len()
    );
    std::fs::write(entry.join("receipt.json"), receipt).map_err(fail)?;
    Ok(executable)
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = remove_tree(&self.tools);
    }
}

fn make_private_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir(path).map_err(|error| format!("{}: {error}", path.display()))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn remove_tree(path: &Path) -> Result<(), String> {
    if std::fs::remove_dir_all(path).is_ok() || !path.exists() {
        return Ok(());
    }
    fn writable(path: &Path) {
        if let Ok(info) = std::fs::symlink_metadata(path)
            && info.is_dir()
        {
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.flatten() {
                    writable(&entry.path());
                }
            }
        }
    }
    writable(path);
    std::fs::remove_dir_all(path).map_err(|error| format!("{}: {error}", path.display()))
}

struct Root(PathBuf);

impl Root {
    fn create() -> Result<Self, String> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "behavior-contract-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        make_private_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        if let Err(error) = remove_tree(&self.0) {
            eprintln!("behavior_contracts: could not remove {error}");
        }
    }
}

fn os(bytes: &[u8]) -> &OsStr {
    OsStr::from_bytes(bytes)
}

fn require_capabilities(case: &Value, root: &Path) -> Result<(), Failure> {
    for capability in corpus::list(case, "requires")? {
        if !capability.py_eq(&Value::Str("non-utf8-paths".to_string())) {
            return Err(Failure::Error(format!(
                "unknown fixture capability {}",
                capability.py_str()
            )));
        }
        let probe = root.join(os(b"probe-\xff"));
        if let Err(error) = std::fs::write(&probe, b"") {
            return Err(match error.raw_os_error() {
                Some(code) if code == nix::libc::EILSEQ || code == nix::libc::EINVAL => {
                    Failure::Unavailable(format!("non-utf8-paths: {error}"))
                }
                _ => Failure::Error(format!("non-utf8-paths: {error}")),
            });
        }
        let listed = std::fs::read_dir(root)
            .map_err(|error| error.to_string())?
            .flatten()
            .any(|entry| entry.file_name().as_bytes() == b"probe-\xff");
        std::fs::remove_file(&probe).map_err(|error| error.to_string())?;
        if !listed {
            return Err(Failure::Unavailable(
                "non-utf8-paths: filesystem changed the filename bytes".to_string(),
            ));
        }
    }
    Ok(())
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|info| info.file_type().is_symlink())
}

fn populate(root: &Path, entries: &[Value]) -> Result<(), String> {
    let _writing = SPAWN.write().map_err(|error| error.to_string())?;
    let mut modes = Vec::new();
    for entry in entries {
        let name = relative(field(entry, "path_b64")?)?;
        let path = root.join(os(&name));
        let shown = path.display().to_string();
        let fail = |error: std::io::Error| format!("{shown}: {error}");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(fail)?;
        }
        if is_link(&path) || std::fs::metadata(&path).is_ok_and(|info| !info.is_dir()) {
            std::fs::remove_file(&path).map_err(fail)?;
        }
        let kind = string(entry, "kind")?;
        match kind {
            "directory" => match std::fs::create_dir(&path) {
                Err(error)
                    if !(error.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir()) =>
                {
                    return Err(fail(error));
                }
                _ => {}
            },
            "file" => {
                std::fs::write(&path, b64::decode(string(entry, "bytes_b64")?)?).map_err(fail)?
            }
            "symlink" => {
                std::os::unix::fs::symlink(os(&b64::decode(string(entry, "target_b64")?)?), &path)
                    .map_err(fail)?
            }
            "special" => {
                nix::unistd::mkfifo(&path, nix::sys::stat::Mode::from_bits_truncate(0o666))
                    .map_err(|error| format!("{shown}: {error}"))?
            }
            _ => {
                return Err(
                    "initial entries support file, directory, symlink or special".to_string(),
                );
            }
        }
        if has(entry, "mode") {
            if kind == "symlink" {
                return Err("a symlink has no mode of its own".to_string());
            }
            let mode = match field(entry, "mode")? {
                Value::Int(digits) => digits
                    .parse::<u32>()
                    .map_err(|error| format!("mode: {error}"))?,
                other => return Err(format!("mode {} is not an integer", other.py_repr())),
            };
            modes.push((path, mode, name.split(|byte| *byte == b'/').count()));
        }
    }
    modes.sort_by_key(|(_, _, depth)| std::cmp::Reverse(*depth));
    for (path, mode, _) in modes {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    Ok(())
}

fn remove(root: &Path, paths: &[Value]) -> Result<(), String> {
    for value in paths {
        let path = root.join(os(&relative(value)?));
        let fail = |error: std::io::Error| format!("{}: {error}", path.display());
        if is_link(&path) || std::fs::metadata(&path).is_ok_and(|info| !info.is_dir()) {
            std::fs::remove_file(&path).map_err(fail)?;
        } else if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(fail)?;
        }
    }
    Ok(())
}

/// Every entry below root, as `os.walk` finds them: symlinked directories are not followed and an unreadable
/// directory's contents are skipped. A special file is recorded by kind and mode without being opened.
fn snapshot(root: &Path, modes: bool) -> Result<Vec<Entry>, String> {
    fn walk(
        root: &Path,
        directory: &Path,
        modes: bool,
        out: &mut Vec<Entry>,
    ) -> Result<(), String> {
        let Ok(listing) = std::fs::read_dir(directory) else {
            return Ok(());
        };
        for item in listing.flatten() {
            let path = item.path();
            let info = std::fs::symlink_metadata(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let name = path
                .strip_prefix(root)
                .map_err(|error| error.to_string())?
                .as_os_str()
                .as_bytes()
                .to_vec();
            let file_type = info.file_type();
            let mode = info.mode() & 0o7777;
            let kind = if file_type.is_symlink() {
                Kind::Symlink(
                    std::fs::read_link(&path)
                        .map_err(|error| format!("{}: {error}", path.display()))?
                        .into_os_string()
                        .into_vec(),
                )
            } else if file_type.is_dir() {
                Kind::Directory
            } else if file_type.is_file() {
                Kind::File(
                    std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?,
                )
            } else {
                Kind::Special(mode)
            };
            let recorded =
                (modes && matches!(kind, Kind::File(_) | Kind::Directory)).then_some(mode);
            let descend = matches!(kind, Kind::Directory);
            out.push(Entry {
                path: name,
                kind,
                mode: recorded,
            });
            if descend {
                walk(root, &path, modes, out)?;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, root, modes, &mut out)?;
    out.sort_by_cached_key(|entry| b64::encode(&entry.path));
    Ok(out)
}

pub struct Output {
    pub exit: i64,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Runs a prepared command to completion, feeding it `stdin`, and kills it at the deadline.
fn run_child(command: &mut Command, stdin: Vec<u8>, timeout: Duration) -> Result<Output, String> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = {
        let _spawning = SPAWN.read().map_err(|error| error.to_string())?;
        command
            .spawn()
            .map_err(|error| format!("{:?}: {error}", command.get_program()))?
    };
    let mut input = child.stdin.take().ok_or("no stdin pipe")?;
    let writer = std::thread::spawn(move || {
        let _ = input.write_all(&stdin);
    });
    let mut stdout = child.stdout.take().ok_or("no stdout pipe")?;
    let mut stderr = child.stderr.take().ok_or("no stderr pipe")?;
    let out_reader = std::thread::spawn(move || {
        let mut data = Vec::new();
        let _ = stdout.read_to_end(&mut data);
        data
    });
    let err_reader = std::thread::spawn(move || {
        let mut data = Vec::new();
        let _ = stderr.read_to_end(&mut data);
        data
    });
    let deadline = Instant::now() + timeout;
    let mut status = None;
    loop {
        if status.is_none() {
            status = child.try_wait().map_err(|error| error.to_string())?;
        }
        if status.is_some() && out_reader.is_finished() && err_reader.is_finished() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "{:?} timed out after {} seconds",
                command.get_program(),
                timeout.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let _ = writer.join();
    let status = status.ok_or("no exit status")?;
    Ok(Output {
        exit: status
            .code()
            .map(i64::from)
            .or_else(|| status.signal().map(|signal| -i64::from(signal)))
            .ok_or("no exit status")?,
        stdout: out_reader.join().map_err(|_| "stdout reader panicked")?,
        stderr: err_reader.join().map_err(|_| "stderr reader panicked")?,
    })
}

/// The command line that runs a corpus program against the built bench-tools.
fn command_for(program: &str, exe: &Path) -> Option<Vec<OsString>> {
    let words: &[&str] = match program {
        "codex" => &["usage", "codex"],
        "transcript" => &["usage", "claude"],
        "evidence" | "evidence-store" => &["evidence"],
        "recover-evidence" => &["evidence", "recover"],
        "restore-local" => &["evidence", "restore-local"],
        "collect-run" => &["evidence", "collect"],
        "reconcile-evidence" => &["evidence", "reconcile"],
        "archive-transcripts" => &["evidence", "archive-transcripts"],
        "evidence-inventory" => &["evidence", "inventory"],
        "regrade-status" => &["regrade", "status", "--"],
        "regrade-archive-attempt" => &["regrade", "archive-attempt"],
        "second-assessment" => &["evaluator-audit", "second"],
        "python-native-artifacts" => &["native-artifacts"],
        "roster" | "diff-identity" | "normalize-review" | "normalize-thermo" | "derive-packet"
        | "build-packet" | "export-explorer" | "archive" | "import-benchmark" | "audit-log"
        | "check-manifest" | "grade" | "upstream" | "ruling" | "claims" | "grading"
        | "current-grading" | "methodology" | "calibration" | "evaluator-audit"
        | "native-artifacts" | "rates" => {
            return Some(vec![exe.as_os_str().to_owned(), OsString::from(program)]);
        }
        _ => return None,
    };
    let mut command = vec![exe.as_os_str().to_owned()];
    command.extend(words.iter().map(OsString::from));
    Some(command)
}

fn which(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|directory| directory.join(program))
            .find(|candidate| {
                std::fs::metadata(candidate)
                    .is_ok_and(|info| info.is_file() && info.mode() & 0o111 != 0)
            })
    })
}

const FAIL_WRITE_C: &str = include_str!("fail_write.c");

/// The variable that preloads the interposer failing a step's `fail_write_b64` path, and the library.
fn fail_write_library(tools: &Path) -> Result<(&'static str, PathBuf), Failure> {
    type Built = Result<(&'static str, PathBuf), (bool, String)>;
    static BUILT: OnceLock<Built> = OnceLock::new();
    let built = BUILT.get_or_init(|| {
        let variable = if cfg!(target_os = "linux") {
            "LD_PRELOAD"
        } else if cfg!(target_os = "macos") {
            "DYLD_INSERT_LIBRARIES"
        } else {
            return Err((
                true,
                format!(
                    "fail_write_b64: needs LD_PRELOAD or DYLD_INSERT_LIBRARIES, not {}",
                    std::env::consts::OS
                ),
            ));
        };
        let compiler = which("cc").ok_or((
            true,
            "fail_write_b64: needs a C compiler on PATH".to_string(),
        ))?;
        let source = tools.join("fail-write.c");
        let library = tools.join("fail-write.so");
        std::fs::write(&source, FAIL_WRITE_C).map_err(|error| (false, error.to_string()))?;
        let mut compile = Command::new(compiler);
        compile
            .args(["-shared", "-fPIC", "-o"])
            .arg(&library)
            .arg(&source);
        if variable == "LD_PRELOAD" {
            compile.arg("-ldl");
        }
        let output = run_child(&mut compile, Vec::new(), Duration::from_secs(60))
            .map_err(|error| (false, error))?;
        if output.exit != 0 {
            return Err((
                false,
                format!(
                    "fail_write_b64: cc failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            ));
        }
        let probe = std::fs::canonicalize(tools)
            .map_err(|error| (false, error.to_string()))?
            .join("fail-write-probe");
        let mut refused =
            Command::new(std::env::current_exe().map_err(|error| (false, error.to_string()))?);
        refused
            .arg("--contract-fail-write-probe")
            .arg(&probe)
            .env(variable, &library)
            .env("BEHAVIOR_CONTRACT_FAIL_WRITE", &probe);
        let output =
            run_child(&mut refused, Vec::new(), COMMAND_TIMEOUT).map_err(|error| (false, error))?;
        if output.exit != i64::from(nix::libc::ENOSPC) {
            return Err((
                true,
                format!("fail_write_b64: {variable} did not take effect"),
            ));
        }
        Ok((variable, library))
    });
    match built {
        Ok((variable, library)) => Ok((variable, library.clone())),
        Err((true, reason)) => Err(Failure::Unavailable(reason.clone())),
        Err((false, error)) => Err(Failure::Error(error.clone())),
    }
}

/// Runs a case's command, or each of its steps on one shared root, and observes each command's result. A program
/// named in `programs` runs that command line in place of its bench-tools command.
pub fn execute(
    case: &Value,
    options: &corpus::Corpus,
    scratch: &Scratch,
    programs: &[(&str, Vec<OsString>)],
) -> Result<Vec<Observed>, Failure> {
    let modes = options.snapshot_modes();
    let root_dir = Root::create()?;
    let root = root_dir.0.as_path();
    require_capabilities(case, root)?;
    populate(root, corpus::required_list(case, "initial")?)?;
    let home = root.join(".home");
    std::fs::create_dir(&home).map_err(|error| format!("{}: {error}", home.display()))?;
    let mut environment: Vec<(String, String)> = vec![
        ("PATH".into(), "/bin:/usr/bin".into()),
        ("HOME".into(), home.to_string_lossy().into_owned()),
        ("LC_ALL".into(), "C.UTF-8".into()),
        ("TZ".into(), "UTC".into()),
        ("PYTHONDONTWRITEBYTECODE".into(), "1".into()),
        ("PYTHONUTF8".into(), "1".into()),
    ];
    if has(&options.options, "gh_stub_b64") {
        let state = relative(field(&options.options, "gh_stub_b64")?)?;
        environment[0].1 = format!("{}:/bin:/usr/bin", scratch.tools.display());
        environment.push((
            "CONTRACT_GH_STATE".into(),
            root.join(os(&state)).to_string_lossy().into_owned(),
        ));
    }
    let steps = corpus::steps(case);
    let real_root = std::fs::canonicalize(root).map_err(|error| error.to_string())?;
    let mut results = Vec::new();
    for step in steps {
        if has(step, "remove") {
            remove(root, corpus::list(step, "remove")?)?;
            continue;
        }
        if has(step, "shell") {
            let mut shell = Command::new("/bin/sh");
            shell
                .arg("-c")
                .arg(string(step, "shell")?)
                .current_dir(root)
                .env_clear()
                .envs(environment.iter().cloned());
            let output = run_child(&mut shell, Vec::new(), COMMAND_TIMEOUT)?;
            if output.exit != 0 {
                return Err(Failure::Error(format!(
                    "shell step exited {}: {}",
                    output.exit,
                    String::from_utf8_lossy(&output.stderr)
                )));
            }
            continue;
        }
        if has(step, "write") {
            populate(root, corpus::list(step, "write")?)?;
            continue;
        }
        let argv = field(step, "argv")?
            .as_array()
            .ok_or("argv is not a list")?
            .iter()
            .map(|arg| match arg {
                Value::Str(text) => Ok(OsString::from(text)),
                Value::Object(_) => Ok(OsString::from_vec(b64::decode(string(arg, "bytes_b64")?)?)),
                other => Err(format!(
                    "argument {} is neither a string nor bytes",
                    other.py_repr()
                )),
            })
            .collect::<Result<Vec<_>, String>>()?;
        let stdin = b64::decode(string(step, "stdin_b64")?)?;
        let program = match step.as_object().and_then(|object| object.get("program")) {
            Some(program) => program,
            None => field(case, "program")?,
        }
        .as_str()
        .ok_or("program is not a string")?;
        let output = if program == "fixture-git" {
            let _spawning = SPAWN.read().map_err(|error| error.to_string())?;
            match fixture_git::run(root, &argv, Some(&environment)) {
                Ok(()) => Output {
                    exit: 0,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                },
                Err(error) => Output {
                    exit: 1,
                    stdout: Vec::new(),
                    stderr: format!("{error}\n").into_bytes(),
                },
            }
        } else {
            let prefix = if let Some((_, line)) = programs.iter().find(|(name, _)| *name == program)
            {
                line.clone()
            } else if program == "python-resolve" {
                return Err(Failure::Unavailable(
                    "python-resolve: evidence_store.py was deleted with its last caller"
                        .to_string(),
                ));
            } else {
                command_for(program, &scratch.executable)
                    .ok_or_else(|| format!("no command for program {program}"))?
            };
            let mut line: Vec<OsString> = Vec::new();
            if has(step, "file_size_limit") {
                let limit = field(step, "file_size_limit")?.py_str();
                line.extend(
                    ["/bin/sh", "-c", "trap \"\" XFSZ; exec \"$0\" \"$@\""].map(OsString::from),
                );
                line.push(
                    std::env::current_exe()
                        .map_err(|error| error.to_string())?
                        .into_os_string(),
                );
                line.push("--contract-file-size-limit".into());
                line.push(limit.into());
            }
            line.extend(prefix);
            line.extend(argv);
            let mut command = Command::new(&line[0]);
            command
                .args(&line[1..])
                .current_dir(root)
                .env_clear()
                .envs(environment.iter().cloned());
            if has(step, "fail_write_b64") {
                let (variable, library) = fail_write_library(&scratch.tools)?;
                let target = real_root.join(os(&relative(field(step, "fail_write_b64")?)?));
                command
                    .env(variable, library)
                    .env("BEHAVIOR_CONTRACT_FAIL_WRITE", target);
            }
            run_child(&mut command, stdin, COMMAND_TIMEOUT)?
        };
        let observed = Observed {
            exit: output.exit,
            stdout: output.stdout,
            stderr: output.stderr,
            filesystem: snapshot(root, modes)?,
            inconsistencies: Vec::new(),
        };
        results.push(normalize::normalize(case, root, observed)?);
    }
    Ok(results)
}

/// `--contract-file-size-limit N COMMAND...`: limits the files COMMAND and its children write to N bytes, so a
/// longer write fails with EFBIG. The caller's shell has already ignored SIGXFSZ, which survives exec.
pub fn file_size_limit_role(args: &[OsString]) -> i32 {
    use std::os::unix::process::CommandExt;
    let Some((limit, command)) = args.split_first() else {
        eprintln!("--contract-file-size-limit needs a byte count and a command");
        return 2;
    };
    let Some(limit) = limit.to_str().and_then(|limit| limit.parse::<u64>().ok()) else {
        eprintln!("--contract-file-size-limit needs a byte count");
        return 2;
    };
    if let Err(error) =
        nix::sys::resource::setrlimit(nix::sys::resource::Resource::RLIMIT_FSIZE, limit, limit)
    {
        eprintln!("setrlimit: {error}");
        return 2;
    }
    let Some((program, rest)) = command.split_first() else {
        eprintln!("--contract-file-size-limit needs a command");
        return 2;
    };
    let error = Command::new(program).args(rest).exec();
    eprintln!("{program:?}: {error}");
    127
}

/// `--contract-fail-write-probe PATH`: exits with the errno of creating PATH, or 0.
pub fn fail_write_probe_role(args: &[OsString]) -> i32 {
    match args.first().map(std::fs::File::create) {
        Some(Ok(_)) => 0,
        Some(Err(error)) => error.raw_os_error().unwrap_or(1),
        None => 2,
    }
}

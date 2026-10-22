//! The sandbox `provision` runs a task recipe's build, post-clone and smoke commands in.
//!
//! [`PROFILE`] lays out the filesystem as the reviewer's `bwrap-v1` does: the system read-only, an empty tmpfs over
//! every home directory, Windows mount and shared scratch area, and the host's home, all of `/run` and `/tmp`. The
//! clone, its working directory and its cache are the only paths a step can write, and the toolchains it needs are
//! shown read-only. Steps start from an empty environment plus [`INHERITED`], [`OFFLINE`] when offline and the
//! recipe's `env`, so host credential variables stay out. The vsock filter and the host Unix socket masks come from
//! the network gate. Build steps share the host network, since they download dependencies; post-clone and smoke
//! steps get their own empty network namespace and a private copy of each [`SEEDED`] cache.

use crate::network_gate;
use crate::pyjson::Value;
use crate::review_isolation::{resolve, which};
use crate::skill_runner::{SANDBOX_HIDDEN, mise_paths};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

type P<T> = Result<T, String>;

pub const PROFILE: &str = "provision-bwrap-v1";

/// The host variables a step keeps; `PATH` and `TMPDIR` are set, and everything else is dropped.
pub const INHERITED: [&str; 7] = [
    "HOME", "USER", "LOGNAME", "LANG", "LC_ALL", "LC_CTYPE", "TZ",
];
const SYSTEM_PATH: [&str; 6] = [
    "/usr/local/sbin",
    "/usr/local/bin",
    "/usr/sbin",
    "/usr/bin",
    "/sbin",
    "/bin",
];
/// What offline steps add: they turn off install-time checks that need a download or the registry, which unconfined
/// post-clone steps met through the host's network and caches. Playwright's install script downloads browsers
/// (j-trpc-5017 found them in `~/.cache/ms-playwright`). pnpm 11 checks the lockfile against the workspace's
/// `trustPolicy` and `minimumReleaseAge` using registry metadata (r-base-ui-5460 used `~/.cache/pnpm`); the build
/// step that filled the store from the same lockfile ran those checks online.
const OFFLINE: [(&str, &str); 3] = [
    ("PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD", "1"),
    ("pnpm_config_trust_policy", "off"),
    ("pnpm_config_minimum_release_age", "0"),
];
/// Host download caches, under the home, that offline steps get a private writable copy of. Prisma's install script
/// generates a client in the clone from cached engines and updates their timestamps (j-trpc-5017's examples).
const SEEDED: [&str; 1] = [".cache/prisma"];
/// mise's installs, uv's Pythons (a uv virtualenv links its interpreter there) and bun, under the home. Only uv's
/// `python` directory: its data directory also holds `uv auth login` credentials.
const HOME_TOOLCHAINS: [&str; 3] = [".local/share/mise", ".local/share/uv/python", ".bun"];
/// Executables shown by themselves: uv sits in `~/.local/bin` beside the clients' launchers.
const EXECUTABLES: [&str; 2] = ["uv", "bun"];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Network {
    Shared,
    Off,
}

/// One phase's sandbox; [`Sandbox::run`] runs each step of the phase.
pub struct Sandbox {
    bwrap: String,
    network: Network,
    hidden: Vec<String>,
    readonly: Vec<String>,
    readwrite: Vec<String>,
    masks: Vec<String>,
    environment: Vec<(String, String)>,
    /// Each seeded cache's private copy and the host path it is shown at.
    seeded: Vec<(String, String)>,
    /// A private directory holding the vsock filter and the seeded copies, removed on drop.
    private: PathBuf,
}

fn texts(items: &[String]) -> Value {
    Value::Array(items.iter().cloned().map(Value::Str).collect())
}

fn set(environment: &mut Vec<(String, String)>, name: &str, value: String) {
    environment.retain(|(key, _)| key != name);
    environment.push((name.to_string(), value));
}

impl Sandbox {
    /// A sandbox that can write only `readwrite`, with `recipe` over the host's [`INHERITED`] variables.
    pub fn new(network: Network, readwrite: &[PathBuf], recipe: &[(String, String)]) -> P<Self> {
        let host: Vec<(String, String)> = std::env::vars_os()
            .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
            .collect();
        Self::for_host(network, readwrite, recipe, &host)
    }

    fn for_host(
        network: Network,
        readwrite: &[PathBuf],
        recipe: &[(String, String)],
        host: &[(String, String)],
    ) -> P<Self> {
        let bwrap = which("bwrap")
            .ok_or("the provisioning sandbox needs bwrap, which is not installed")?
            .to_string_lossy()
            .into_owned();
        let home = host
            .iter()
            .find(|(name, _)| name == "HOME")
            .map(|(_, value)| PathBuf::from(value))
            .filter(|home| home.is_absolute() && home.parent().is_some());
        let mut hidden: Vec<String> = SANDBOX_HIDDEN
            .iter()
            .filter(|path| !path.starts_with("/run/"))
            .map(ToString::to_string)
            .chain(["/run".to_string(), "/tmp".to_string()])
            .filter(|path| Path::new(path).is_dir())
            .collect();
        if let Some(home) = home.as_ref().filter(|home| home.is_dir()) {
            let home = resolve(home);
            if !hidden.iter().any(|path| home.starts_with(path)) {
                hidden.push(home.to_string_lossy().into_owned());
            }
        }
        let executables: Vec<PathBuf> = EXECUTABLES
            .iter()
            .filter_map(|name| which(name))
            .map(|path| resolve(&path))
            .collect();
        // WSL links /etc/resolv.conf into /mnt/wsl, which is hidden; build steps need DNS.
        let mut readonly: Vec<String> = home
            .iter()
            .flat_map(|home| HOME_TOOLCHAINS.iter().map(|path| resolve(&home.join(path))))
            .chain(executables.iter().cloned())
            .chain(std::iter::once(resolve(Path::new("/etc/resolv.conf"))))
            .filter(|path| path.exists())
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        readonly.sort();
        readonly.dedup();
        let listed = readonly.clone();
        readonly.retain(|path| {
            !listed
                .iter()
                .any(|other| other != path && Path::new(path).starts_with(other))
        });
        let readwrite: Vec<String> = readwrite
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
        let shown: Vec<String> = readonly.iter().chain(&readwrite).cloned().collect();
        let masks = network_gate::socket_masks(&hidden, &shown, None)?;
        let visible = |entry: &str| {
            let entry = Path::new(entry);
            !hidden.iter().any(|path| entry.starts_with(path))
                || readonly
                    .iter()
                    .any(|path| entry.starts_with(path) || Path::new(path).parent() == Some(entry))
        };
        let mise = mise_paths()?;
        let mut search: Vec<String> = Vec::new();
        for entry in mise
            .split(':')
            .map(str::to_string)
            .chain(executables.iter().filter_map(|path| {
                path.parent()
                    .map(|parent| parent.to_string_lossy().into_owned())
            }))
            .chain(SYSTEM_PATH.iter().map(ToString::to_string))
        {
            if !entry.is_empty() && visible(&entry) && !search.contains(&entry) {
                search.push(entry);
            }
        }
        let mut environment: Vec<(String, String)> = host
            .iter()
            .filter(|(name, _)| INHERITED.contains(&name.as_str()))
            .cloned()
            .collect();
        set(&mut environment, "PATH", search.join(":"));
        set(&mut environment, "TMPDIR", "/tmp".to_string());
        if network == Network::Off {
            for (name, value) in OFFLINE {
                set(&mut environment, name, value.to_string());
            }
        }
        for (name, value) in recipe {
            set(&mut environment, name, value.clone());
        }
        let private =
            nix::unistd::mkdtemp(&network_gate::runtime_dir().join("bench-provision-XXXXXX"))
                .map_err(|error| {
                    format!("cannot make the provisioning sandbox directory: {error}")
                })?;
        let mut sandbox = Self {
            bwrap,
            network,
            hidden,
            readonly,
            readwrite,
            masks,
            environment,
            seeded: Vec::new(),
            private,
        };
        network_gate::write_vsock_filter(&sandbox.filter())?;
        if network == Network::Off
            && let Some(home) = &home
        {
            for (index, path) in SEEDED.iter().enumerate() {
                let source = home.join(path);
                if source.is_dir() {
                    let copy = sandbox.private.join(format!("seed-{index}"));
                    seed(&source, &copy)?;
                    sandbox.seeded.push((
                        copy.to_string_lossy().into_owned(),
                        source.to_string_lossy().into_owned(),
                    ));
                }
            }
        }
        Ok(sandbox)
    }

    fn filter(&self) -> PathBuf {
        self.private.join("vsock.bpf")
    }

    fn arguments(&self, seccomp: i32, cwd: &Path, command: &str) -> Vec<String> {
        let mut arguments: Vec<String> = [
            "--ro-bind",
            "/",
            "/",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "--unshare-pid",
            "--unshare-ipc",
            "--die-with-parent",
        ]
        .map(String::from)
        .to_vec();
        if self.network == Network::Off {
            arguments.push("--unshare-net".to_string());
        }
        for path in &self.hidden {
            arguments.extend(["--tmpfs".to_string(), path.clone()]);
        }
        for path in &self.readonly {
            arguments.extend(["--ro-bind".to_string(), path.clone(), path.clone()]);
        }
        for path in &self.readwrite {
            arguments.extend(["--bind".to_string(), path.clone(), path.clone()]);
        }
        for (copy, shown) in &self.seeded {
            arguments.extend(["--bind".to_string(), copy.clone(), shown.clone()]);
        }
        arguments.extend(self.masks.iter().cloned());
        arguments.extend([
            "--seccomp".to_string(),
            seccomp.to_string(),
            "--chdir".to_string(),
            cwd.to_string_lossy().into_owned(),
            "--".to_string(),
            "sh".to_string(),
            "-c".to_string(),
            command.to_string(),
        ]);
        arguments
    }

    /// Run `command` through `sh -c` in `cwd` inside the sandbox.
    pub fn run(&self, command: &str, cwd: &Path) -> P<Output> {
        use std::os::fd::AsRawFd;
        let filter = network_gate::inheritable(&self.filter())?;
        Command::new(&self.bwrap)
            .args(self.arguments(filter.as_raw_fd(), cwd, command))
            .env_clear()
            .envs(self.environment.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("cannot run {}: {error}", self.bwrap))
    }

    /// What the provisioning receipt records about this sandbox.
    pub fn receipt(&self) -> Value {
        Value::Object(
            [
                ("profile", Value::Str(PROFILE.to_string())),
                ("bwrap", Value::Str(self.bwrap.clone())),
                (
                    "network",
                    Value::Str(
                        match self.network {
                            Network::Shared => "shared",
                            Network::Off => "off",
                        }
                        .to_string(),
                    ),
                ),
                ("hidden", texts(&self.hidden)),
                ("readonly", texts(&self.readonly)),
                ("readwrite", texts(&self.readwrite)),
                (
                    "seeded",
                    texts(
                        &self
                            .seeded
                            .iter()
                            .map(|(_, shown)| shown.clone())
                            .collect::<Vec<_>>(),
                    ),
                ),
                (
                    "masked_sockets",
                    Value::Array(
                        self.masks
                            .chunks(3)
                            .filter_map(|mask| mask.get(2).cloned().map(Value::Str))
                            .collect(),
                    ),
                ),
                ("seccomp", Value::Str("vsock-denied".to_string())),
                (
                    "environment",
                    Value::Object(
                        self.environment
                            .iter()
                            .map(|(name, value)| (name.clone(), Value::Str(value.clone())))
                            .collect(),
                    ),
                ),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
        )
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.private);
    }
}

fn seed(source: &Path, copy: &Path) -> P<()> {
    let done = Command::new("cp")
        .args(["-a", "--reflink=auto"])
        .arg(source)
        .arg(copy)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run cp: {error}"))?;
    if !done.status.success() {
        return Err(format!(
            "cannot copy {} for the sandbox: {}",
            source.display(),
            String::from_utf8_lossy(&done.stderr).trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Scratch(PathBuf);

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scratch(name: &str) -> Scratch {
        let root = std::env::temp_dir().join(crate::evidence::random_name(name));
        std::fs::create_dir_all(&root).unwrap();
        Scratch(resolve(&root))
    }

    fn output(sandbox: &Sandbox, command: &str, cwd: &Path) -> (i32, String) {
        let done = sandbox.run(command, cwd).unwrap();
        (
            done.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&done.stdout).into_owned()
                + &String::from_utf8_lossy(&done.stderr),
        )
    }

    #[test]
    fn a_step_cannot_read_the_home_credentials_or_a_sibling_attempt() {
        if which("bwrap").is_none() {
            return;
        }
        let root = scratch("provision-sandbox-");
        let home = root.0.join("home");
        for (path, content) in [
            (home.join(".claude/.credentials.json"), "claude secret"),
            (home.join(".codex/auth.json"), "codex secret"),
            (
                home.join(".local/share/uv/credentials/credentials.toml"),
                "uv secret",
            ),
            (home.join(".local/share/uv/python/marker"), "uv python"),
            (root.0.join("sibling/clone/answer.txt"), "sibling answer"),
        ] {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        let clone = root.0.join("attempt/clone");
        std::fs::create_dir_all(&clone).unwrap();
        let host: Vec<(String, String)> = [
            ("HOME", home.to_string_lossy().into_owned()),
            ("ANTHROPIC_API_KEY", "canary-key".to_string()),
            ("GH_TOKEN", "canary-token".to_string()),
            ("LANG", "C.UTF-8".to_string()),
        ]
        .map(|(name, value)| (name.to_string(), value))
        .to_vec();
        let recipe = [("RECIPE_VALUE".to_string(), "kept".to_string())];
        let sandbox =
            Sandbox::for_host(Network::Off, std::slice::from_ref(&clone), &recipe, &host).unwrap();
        let sibling = root.0.join("sibling/clone/answer.txt");
        let (code, seen) = output(
            &sandbox,
            &format!(
                "cat $HOME/.claude/.credentials.json $HOME/.codex/auth.json $HOME/.local/share/uv/credentials/credentials.toml {} 2>/dev/null; cat $HOME/.local/share/uv/python/marker; env; printf x > planted; mkdir -p {sibling_dir} && printf y > {sibling_dir}/planted",
                sibling.display(),
                sibling_dir = root.0.join("sibling").display()
            ),
            &clone,
        );
        assert_eq!(code, 0, "{seen}");
        for secret in [
            "claude secret",
            "codex secret",
            "uv secret",
            "sibling answer",
            "canary-key",
            "canary-token",
        ] {
            assert!(!seen.contains(secret), "{secret} leaked: {seen}");
        }
        assert!(
            seen.contains("uv python"),
            "uv's Pythons stay visible: {seen}"
        );
        assert!(seen.contains("RECIPE_VALUE=kept"), "{seen}");
        assert!(seen.contains("LANG=C.UTF-8"), "{seen}");
        assert!(clone.join("planted").is_file(), "the clone stays writable");
        assert!(
            !root.0.join("sibling/planted").exists(),
            "a write outside the clone must not reach the host"
        );
        let receipt = sandbox.receipt();
        let field = |key: &str| {
            receipt
                .as_object()
                .and_then(|object| object.get(key))
                .map(Value::py_str)
        };
        assert_eq!(field("profile").as_deref(), Some(PROFILE));
        assert_eq!(field("network").as_deref(), Some("off"));
    }

    #[test]
    fn an_offline_step_writes_only_a_private_copy_of_a_seeded_cache() {
        if which("bwrap").is_none() {
            return;
        }
        let root = scratch("provision-seed-");
        let engine = root.0.join("home/.cache/prisma/engine");
        std::fs::create_dir_all(engine.parent().unwrap()).unwrap();
        std::fs::write(&engine, "engine").unwrap();
        let clone = root.0.join("clone");
        std::fs::create_dir_all(&clone).unwrap();
        let host = [(
            "HOME".to_string(),
            root.0.join("home").to_string_lossy().into_owned(),
        )];
        let sandbox =
            Sandbox::for_host(Network::Off, std::slice::from_ref(&clone), &[], &host).unwrap();
        let (code, seen) = output(
            &sandbox,
            "cat $HOME/.cache/prisma/engine > copied && printf changed > $HOME/.cache/prisma/engine",
            &clone,
        );
        assert_eq!(code, 0, "{seen}");
        assert_eq!(
            std::fs::read_to_string(clone.join("copied")).unwrap(),
            "engine"
        );
        assert_eq!(std::fs::read_to_string(&engine).unwrap(), "engine");
        let private = sandbox.private.clone();
        drop(sandbox);
        assert!(!private.exists(), "the private copy must be removed");
    }

    #[test]
    fn only_build_steps_share_the_network() {
        if which("bwrap").is_none() {
            return;
        }
        let root = scratch("provision-network-");
        let interfaces = "tail -n +3 /proc/net/dev | cut -d: -f1 | tr -d ' ' | sort | tr '\\n' ' '";
        let offline = Sandbox::new(Network::Off, std::slice::from_ref(&root.0), &[]).unwrap();
        assert_eq!(
            output(&offline, interfaces, &root.0),
            (0, "lo ".to_string())
        );
        let shared = Sandbox::new(Network::Shared, std::slice::from_ref(&root.0), &[]).unwrap();
        let host = std::fs::read_to_string("/proc/net/dev").unwrap();
        let (code, seen) = output(&shared, interfaces, &root.0);
        assert_eq!(code, 0, "{seen}");
        assert_eq!(seen.split_whitespace().count(), host.lines().count() - 2);
        let skips = |sandbox: &Sandbox| {
            output(sandbox, "env", &root.0)
                .1
                .contains("pnpm_config_trust_policy=off")
        };
        assert!(skips(&offline) && !skips(&shared));
    }
}

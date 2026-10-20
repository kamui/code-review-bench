# Rust runtime pinning

This is the runtime stage of the migration, [#153](https://github.com/kamui/code-review-bench/issues/153),
part of [#149](https://github.com/kamui/code-review-bench/issues/149). It proceeds under the
[2026-10-18 decision to continue](https://github.com/kamui/code-review-bench/issues/149#issuecomment-6248537015).
The [pilot result](pilot/README.md) and its stop recommendation stay unchanged as historical evidence.
This stage does not claim that the small-CLI target passed.

`bench/tools/rust_runtime.py` builds, retains, selects and checks Rust executables.
`bench/tools/review_queue.py` refuses a launch whose pinned executable is missing, changed, built
for another host or built from another source. `bench-tools`, the crate of
[#154](processing.md), holds the first maintained commands, which run as
[a maintained command](#running-a-maintained-command) does.

## What a receipt pins

Each retained version is `<store>/<name>/<sha256>/`, holding the executable and `receipt.json`.
Both files are read-only. The tool never overwrites or removes an entry. The receipt records:

- `crate`: name, version, binary, and the crate path relative to the repository.
- `source`: the commit, the crate's Git tree at that commit, the SHA-256 of `Cargo.toml`,
  `Cargo.lock` and `rust-toolchain.toml`, and the `origin` URL when it is a network URL.
- `toolchain`: the pinned channel, `rustc -vV` and `cargo -V`.
- `target` and `libc`: the target triple and the C library of the build host.
- `build`: the command, the path remapping flags, and the names of the `PATH`, `HOME`, `CARGO_HOME`,
  `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN` and `TMPDIR` variables the build saw.
- `artifact`: the file name, SHA-256 and size.

A receipt holds no absolute path, so it stays valid when a store moves to another directory or host.
The pin that queues and runs use is the executable's SHA-256 together with the receipt's SHA-256.

## Install

1. Install rustup. The crate's `rust-toolchain.toml` names the toolchain; `rustup show` in the
   crate installs it.
2. From the crate, run `cargo fetch --locked`, and again after every `Cargo.lock` change. This is the only step that
   needs the network.
3. From a clean, committed checkout, run:

   ```sh
   python3 bench/tools/rust_runtime.py install --crate rust/usage-pilot --select
   ```

`install` needs Python 3.11 or later; the other commands and `review_queue.py` do not.
After the cleanliness check it creates a private Cargo home for the build. That home holds no
configuration, only links to the host Cargo home's registry and Git caches and the lock files that
guard them. It writes the crate's Git tree at `HEAD` into a snapshot directly inside that home,
copying each file's committed bytes. It then selects the toolchain once, through rustup or another
proxy, from the snapshot's `rust-toolchain.toml`. Every later step runs that toolchain's own `cargo`
and `rustc` from `/`. The manifest and `cargo metadata` checks, the toolchain query and the build use
the snapshot, and the receipt hashes its files.

Cargo looks for configuration in its working directory, each parent of it and its home, and stops
its workspace search at its home. The build therefore reads configuration only from `/.cargo`, which
only an administrator can write, and from the private home, which holds none. Cargo configuration
saved to the checkout, the host Cargo home, the temporary directory or a parent of the snapshot, and a
workspace `Cargo.toml` saved in a parent of the snapshot, cannot change the executable during an
install, even when removed before it ends. Configuration in those places is ignored, not refused. A
later `rustup override` or toolchain change cannot switch the toolchain partway through an install.
A toolchain whose `cargo` is not beside `rustc` in its sysroot's `bin` is refused. The shared caches
and the toolchain remain inputs, trusted as described below; this is not a hermetic build.
It refuses what a receipt could not restore:

- Uncommitted or untracked files in the crate, and a selected `rustc` whose release differs from
  `rust-toolchain.toml`.
- A Cargo configuration file in `/.cargo`, or in a `.cargo` directory the crate tracks, which the
  build would ignore. A relative `HOME`, `CARGO_HOME`, `RUSTUP_HOME` or `TMPDIR`.
- A parent Cargo workspace of the crate in the checkout, `[patch]` or `[replace]` sections, and a
  `Cargo.toml`, `Cargo.lock` or `rust-toolchain.toml` that Git does not track.
- A tracked symbolic link or submodule, whose target the tree does not hold.
- A local package in `cargo metadata --all-features`, such as an absolute path dependency, whose
  manifest is outside the snapshot, even when that dependency is optional and never compiled.
- After the build, a local file in cargo's dep-info outside the snapshot, such as an absolute
  `include_str!` path.

A relative path to a file the crate's tree does not hold, such as an ignored `include_str!` target or
a sibling path dependency, finds nothing in the snapshot, so `cargo metadata` or the build fails.

The crate's Git tree then covers every local input that cargo records, and `Cargo.lock` covers
registry and Git dependencies. A build script or procedural macro that reads a file outside the
snapshot without declaring it is not detected; `usage-pilot` has neither. The installed toolchain, the
system linker and Cargo's registry and Git caches are trusted as installed: the receipt records the
`rustc` and `cargo` identities, but nothing re-verifies those files' contents. The build runs
`cargo build --locked --offline --release --target <host> --bin <bin>` in a temporary target directory.
The build sees only `PATH`, led by the selected toolchain's `bin` directory, `HOME`, the private
`CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN` and `TMPDIR`, so
`RUSTFLAGS`, wrappers, profile overrides and any variable a crate reads with `env!` are absent. A
build is refused when rustc's dep-info or a build script's `rerun-if-env-changed` shows it read one of
those six. A build script that reads a variable without declaring it is not detected. The build also
remaps the snapshot and private `CARGO_HOME` paths. Installing the same build again keeps the existing entry.
An entry with the same hash but another receipt is refused.

The default store is `~/.t3/bench-runtime`. Every command takes `--store`.

## Offline retention and restore

A retained version needs no network, toolchain or registry to run or to roll back to.
To move a version to another store, copy its entry and run:

```sh
python3 bench/tools/rust_runtime.py restore --receipt ENTRY/receipt.json --executable ENTRY/<bin> --store OTHER
```

`restore` accepts the executable only when it hashes to the receipt. A rebuild from the receipt's
revision with the same toolchain can also be restored this way. On this host, two checkouts at
different paths produced the same executable hash; that is an observation, not a guarantee for
other hosts. Rebuilding offline also needs the locked dependencies in the Cargo registry cache.

## Freezing and running

Freeze a run's Rust runtimes with the run, as with `clients.frozen.json`:

```sh
python3 bench/tools/rust_runtime.py pin usage-pilot > bench/runs/<run>/runtimes.frozen.json
```

A run frozen at a commit that has `rust/bench-tools` must pin `bench-tools`, whose `normalize-review` and
`normalize-thermo` normalize its reviews ([#194](processing.md#the-review-normalizers)). `pin` prints one name; merge
the objects when a run pins more than one. `run_cell.py`, the skill runners and `file_attempt.py --replay` refuse a run
that pins none before anything is claimed, and `review_queue.py` refuses its queue before any launch. A run frozen
earlier pins none and keeps running from its own freeze checkout, with the Python normalizers.

Copy the same object into the queue file as `runtimes`. Set `runtime_store` when the store is not
the default. Before every launch, `review_queue.py` checks the following:

- Each run's `runtimes.frozen.json` equals the queue's `runtimes`. A run without the file needs a
  queue that pins no runtime, so earlier runs and queues behave as before. A run whose `freeze_commit` has
  `rust/bench-tools` must pin `bench-tools`.
- The executable and receipt hash to the pin, and the executable is a regular executable file.
- The receipt's target is this host's, the host is a supported target (below), and the host has the
  receipt's C library at the same or a newer version.
- The receipt's crate tree equals the crate tree at each run's `freeze_commit`.
- `RUNTIME_PATHS` now includes `rust`, so changed Rust sources, `Cargo.lock` or toolchain files in
  the checkout also differ from `freeze_commit`.

A launch receives `BENCH_RUST_<NAME>` and `BENCH_RUST_<NAME>_SHA256`, and its launch record keeps
the pins. The controller drops any `BENCH_RUST_` variable it inherits, and the queue environment may
not set these names. Restart and recovery run the same checks
against the same pinned version. One controller per work root still holds `serial.lock`.

## Running a maintained command

`resolve` prints the executable an operation runs, and `exec` runs it with the arguments after `--`, replacing
the Python process so the program's exit code and output are the command's:

```sh
python3 bench/tools/rust_runtime.py resolve bench-tools [--run bench/runs/<run>]
python3 bench/tools/rust_runtime.py exec bench-tools [--run bench/runs/<run>] -- roster
```

Both take the first of these and refuse everything else, printing the install command:

1. `BENCH_RUST_<NAME>` with `BENCH_RUST_<NAME>_SHA256`, as a queue launch sets them. The file must be an
   absolute path to an executable that hashes to the second variable. Setting only one of them is refused.
2. With `--run`, the pin for the name in the run's `runtimes.frozen.json`, checked as a queue checks it. A run
   without that pin is refused; the selection is never read.
3. Otherwise the store's selected version, checked against its pin and this host, only while its receipt's crate
   tree is the crate tree at the checkout's `HEAD`. A commit that changes the crate needs a new install.

There is no Python fallback, no `cargo run` and no unreceipted `target/` executable. The store is `--store`, else
`BENCH_RUNTIME_STORE`, else `~/.t3/bench-runtime`, for every command. A queue still names its store with
`runtime_store`. CI installs into a store under the runner's temporary directory.

## Upgrade and rollback

To upgrade, install the new version and `select` it. Only runs frozen after that take it.
An unfinished run keeps its frozen pin until it ends. Never edit a started run's `runtimes.frozen.json`.

`rollback` selects the version that was selected before the current one. Like `select`, it changes
only `selected.json` and appends to `selections.jsonl`. It reads the current selection and writes the
new one under a single hold of the store lock, so a concurrent `select` cannot make it return to a
stale version. Pinned
operations do not read the selection. If a pinned version is missing or changed, the queue blocks.
It does not fall back to the selected version, to another retained version or to Python. A
possibly successful side effect is therefore never replayed through another implementation.

## Deviations

When a run needs another runtime, freeze a new run, or record the change and settle it by hand:

```sh
python3 bench/tools/rust_runtime.py deviation --run bench/runs/<run> --name usage-pilot \
  --from OLD_SHA256 --to NEW_SHA256 --reason ... --scope ... --validation ... --accounting ...
```

This writes `deviations/runtime-<name>-<old>-to-<new>.v1.json` with schema
`rust-runtime-deviation/1`. The record holds both pins and receipts and the four statements, and the
tool never overwrites it. It has no `predecessor` or `cause`, so attempt-diagnosis readers pass over
it. No check reads it as permission: the queue still requires the frozen pin.

## Platform coverage

The [script policy](../agents/scripts.md) supports macOS on Apple Silicon and Linux on x86_64 or aarch64.
`SUPPORTED_TARGETS` holds one target for each of those hosts, and the `Rust runtime` workflow runs on a
matching GitHub runner for each: the runtime tests with real cargo builds, an install of `usage-pilot`, the corpus with the
retained executable, and [`rollback.sh`](runtime-evidence/rollback.sh). It also checks `bench-tools` with
`cargo fmt`, `clippy` and `cargo test`, installs it, runs its corpora with the retained executable and the
`diff-identity`, `normalize-review`, `check-manifest`, `rates` and `upstream` differentials against the retained
Python libraries, and runs `roster` on the repository. The `CI and GitHub Pages` workflow installs `bench-tools` the
same way for the explorer export. Merge a change to the
runtime only after every target's job passes.

| Target | Runner | Local run |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | Linux 6.6 under WSL2, glibc 2.35, Rust 1.99.1, Python 3.14.7 |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | None |
| `aarch64-apple-darwin` | `macos-latest` | None |

Each workflow run records its own results; this table records only where the checks run. Native
Windows is outside the script policy, and every check refuses an unknown host. `review_queue.py`
reads `/proc` and runs only on Linux, so queue pins apply only there. The C library check compares
the build host's glibc with this host's, which is stricter than the symbol versions the executable
needs; macOS receipts record no C library.

## Ownership of the other runtime modules

The [v2 inventory](README.md#responsibilities-and-contracts) also maps `seal`, `skill_provenance` and
`fetch_freeze_commits` to #153. This stage adds `rust_runtime` and ports none of them. They keep their
Python implementations until these stages move them:

| Module | Owner | Reason |
| --- | --- | --- |
| `skill_provenance` | #157 | A freeze helper for new skill arms; #157 covers provisioning and freeze helpers. |
| `fetch_freeze_commits` | #157 | Fetches freeze commits for runs and CI; #157 covers freeze helpers. |
| `seal` | #160 | No narrower stage names it; #160 resolves every remaining responsibility. |

The v2 census stays as recorded. The [v3 census](README.md#reproducible-records) records these owners.

## Rollback evidence

[`runtime-evidence/`](runtime-evidence/) holds a local install of two versions of `usage-pilot`,
selection, rollback, and corpus runs of both retained executables.

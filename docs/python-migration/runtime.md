# Rust runtime pinning

This is the runtime stage of the migration, [#153](https://github.com/kamui/code-review-bench/issues/153),
part of [#149](https://github.com/kamui/code-review-bench/issues/149). It proceeds under the
[2026-10-18 decision to continue](https://github.com/kamui/code-review-bench/issues/149#issuecomment-6248537015).
The [pilot result](pilot/README.md) and its stop recommendation stay unchanged as historical evidence.
This stage does not claim that the small-CLI target passed.

`bench/tools/rust_runtime.py` builds, retains, selects and checks Rust executables.
`bench/tools/review_queue.py` refuses a launch whose pinned executable is missing, changed, built
for another host or built from another source. No production caller runs a Rust executable yet;
#154 moves the first readers onto `BENCH_RUST_<NAME>`.

## What a receipt pins

Each retained version is `<store>/<name>/<sha256>/`, holding the executable and `receipt.json`.
Both files are read-only. The tool never overwrites or removes an entry. The receipt records:

- `crate`: name, version, binary, and the crate path relative to the repository.
- `source`: the commit, the crate's Git tree at that commit, the SHA-256 of `Cargo.toml`,
  `Cargo.lock` and `rust-toolchain.toml`, and the `origin` URL when it is a network URL.
- `toolchain`: the pinned channel, `rustc -vV` and `cargo -V`.
- `target` and `libc`: the target triple and the C library of the build host.
- `build`: the command, the path remapping flags, and the names of the environment variables the build saw.
- `artifact`: the file name, SHA-256 and size.

A receipt holds no absolute path, so it stays valid when a store moves to another directory or host.
The pin that queues and runs use is the executable's SHA-256 together with the receipt's SHA-256.

## Install

1. Install rustup. The crate's `rust-toolchain.toml` names the toolchain; `rustup show` in the
   crate installs it.
2. From the crate, run `cargo fetch --locked` once. This is the only step that needs the network.
3. From a clean, committed checkout, run:

   ```sh
   python3 bench/tools/rust_runtime.py install --crate rust/usage-pilot --select
   ```

`install` needs Python 3.11 or later; the other commands and `review_queue.py` do not.
It refuses what a receipt could not restore:

- Uncommitted or untracked files in the crate, and an active `rustc` whose release differs from
  `rust-toolchain.toml`.
- Any Cargo configuration file in the `.cargo` directory of the crate or a parent, or in `CARGO_HOME`,
  each checked even when `CARGO_HOME` is one of those directories, and a relative
  `HOME`, `CARGO_HOME`, `RUSTUP_HOME` or `TMPDIR`, which cargo would resolve against the crate.
- A parent Cargo workspace, `[patch]` or `[replace]` sections, and a `Cargo.toml`, `Cargo.lock` or
  `rust-toolchain.toml` that Git does not track.
- A tracked symbolic link, whose target the tree does not hold.
- A local package in `cargo metadata --all-features`, such as a path dependency, whose manifest Git
  does not track in the crate, even when that dependency is optional and never compiled.
- After the build, a local file in cargo's dep-info that Git does not track in the crate, such as an
  ignored `include_str!` target.

The crate's Git tree then covers every local input that cargo records, and `Cargo.lock` covers
registry and Git dependencies. A build script or procedural macro that reads a file without declaring
it is not detected; `usage-pilot` has neither. The build runs
`cargo build --locked --offline --release --target <host> --bin <bin>` in a temporary target directory.
The build sees only `PATH`, `HOME`, `CARGO_HOME`, `RUSTUP_HOME`, `RUSTUP_TOOLCHAIN` and `TMPDIR`, so
`RUSTFLAGS`, wrappers, profile overrides and any variable a crate reads with `env!` are absent. A
build is refused when rustc's dep-info or a build script's `rerun-if-env-changed` shows it read one of
those six. A build script that reads a variable without declaring it is not detected. The build also
remaps the crate and `CARGO_HOME` paths. Installing the same build again keeps the existing entry.
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

Copy the same object into the queue file as `runtimes`. Set `runtime_store` when the store is not
the default. Before every launch, `review_queue.py` checks the following:

- Each run's `runtimes.frozen.json` equals the queue's `runtimes`. A run without the file needs a
  queue that pins no runtime, so earlier runs and queues behave as before.
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

The [script policy](../agents/scripts.md) supports macOS and Linux. `SUPPORTED_TARGETS` holds one
target for each of those hosts, and the `Rust runtime` workflow runs on a matching GitHub runner for
each: the runtime tests with real cargo builds, an install of `usage-pilot`, the corpus with the
retained executable, and [`rollback.sh`](runtime-evidence/rollback.sh). Merge a change to the
runtime only after every target's job passes.

| Target | Runner | Local run |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | `ubuntu-latest` | Linux 6.6 under WSL2, glibc 2.35, Rust 1.99.1, Python 3.14.7 |
| `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | None |
| `x86_64-apple-darwin` | `macos-15-intel` | None |
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

The v2 census stays as recorded. Its next version records these owners.

## Rollback evidence

[`runtime-evidence/`](runtime-evidence/) holds a local install of two versions of `usage-pilot`,
selection, rollback, and corpus runs of both retained executables.

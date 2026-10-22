# Migration completion report

This closes out [#160](https://github.com/kamui/code-review-bench/issues/160), the last stage of
[#149](https://github.com/kamui/code-review-bench/issues/149), under the
[simplified plan](https://github.com/kamui/code-review-bench/issues/149#issuecomment-6303498397): port what the
benchmark uses, delete the rest, verify with Rust tests and one run of the real commands. Step 4c-1
([#336](https://github.com/kamui/code-review-bench/pull/336)) ported `seal.py`, `compare.py`, the `refresh` command of
`rates.py`, `check_manifest.py` and `tools/fetch_freeze_commits.py` to `bench-tools` and moved the
`bench/tools/test_*.py` suites into Rust tests. Step 4c-2 replaced the Python launcher `bench/tools/rust_runtime.py`
with `bench-tools runtime` and the shell launcher `bench/tools/bench-tools` ([runtime](runtime.md)) and moved the
`tools/` tests to bun and Rust.

## Measurements

All numbers were measured on 2026-10-22 on one shared host: Linux 6.6 under WSL2, 24 cores, glibc 2.35,
Rust 1.99.1. The host ran other work throughout; each row gives the 1-minute load average around it. Before is
`main` at `e5faab583`, which includes step 4c-1 and still starts `bench-tools` through `rust_runtime.py`; after is
`4d27be61f`. Later commits on the branch change only documentation. Each pair ran back to back from two worktrees with
private stores. Single runs on a loaded host are not benchmarks. Only the launcher rows, per command and in the
fake-client cell, reflect code that runs faster.

| Measure | Before → after | Load | What the number counts |
| --- | --- | --- | --- |
| Launcher, 20 runs of `roster` | 4.68 s → 0.69 s (234 → 35 ms per command) | 7 | Python start-up and `rust_runtime.py` versus the shell launcher and `bench-tools runtime exec`; both hash the executable before running it. |
| Fake-client cell, median of 5 | 0.94 s → 0.35 s | 7 | [`tools/fake-client-cell.sh`](../../tools/fake-client-cell.sh) and its `rust_runtime.py` equivalent: dispatch, audit, normalize and `run-cell --file`. Three launcher calls account for the difference. |
| `bench-tools` install | 37.4 s → 40.3 s | 3–7 | One release build of the crate snapshot. The after run excludes the installer's own debug build, which `cargo run` reuses; building it from nothing took 22.3 s at load 4. |
| `cargo test --locked`, tests prebuilt | 56.8 s → 61.2 s | 14–21 | The whole suite and every corpus: 698 → 720 tests passed with 22 ignored on both sides, and 1,403 corpus cases passed with 2 skipped on both sides. After adds `tests/runtime.rs`, `tests/evidence_consumers.rs` and two publisher tests. |
| `bun run test`, evidence cached, by part | 171.7 s → 25.5 s | 4–6 | Not a speed-up. `test:web:site` 19.4 → 16.8 s; the 29 Python `tools/` tests 136.0 s → the 22 bun `tools/` tests 4.5 s; `test:bench` 15.0 → 3.2 s, where before also ran `test_rust_runtime.py` (34 tests, 8.6 s); `test:client` 1.3 → 1.0 s. A first whole run took 446.0 s → 26.9 s at load 10 → 2, the before run being main's first export in a fresh worktree. |

Agent iteration cost is unavailable: no per-step token or time ledger was kept for the migration.

## Python-unavailable evidence

`tools/without-python.sh COMMAND` runs a command with every `python*` entry removed from the `PATH` directories that
hold one. With it, at `4d27be61f` on this host:

- `cargo test --locked --test behavior_contracts`: 1,403 passed, 0 failed, 2 skipped, the same totals as `main`,
  including grade-prepare (17), grade-map (34) and export-explorer (49), in 21.0 s.
- The CI step, in 55.9 s: install and select into an empty store, `runtime resolve`, `roster`, the fake-client cell,
  `fetch-freeze-commits`, `bun run data` and `bun run scorecard`. The cell's stand-in Claude client was dispatched
  through `dispatch.sh`, audited and normalized by the run's pinned `bench-tools`, then filed by `run-cell --file`
  through `bench/tools/bench-tools --run` as `valid completed` with complete metering. The three freeze commits were
  already present, so none was fetched. The scorecard was byte-identical to the one written with Python available.

CI's `Rust runtime` workflow runs the corpora that way, and its x86_64 runtime job runs the same step with no Python on
`PATH`. The export (2,710 files under `public/`) and the scorecard, as text and JSON, are byte-identical to `main`'s.
`verify:import`, `verify:current` and `verify:claims` pass, and `rollback.sh` installed a second version and rolled
back on disposable stores in 120.7 s.

A fresh `run-cell --next` claim is not part of this evidence: before it dispatches, rates-check-v1 compares the frozen
price with the providers' live pricing pages, which a fixture cannot answer. The cell is dispatched through the
same `dispatch.sh` that `run-cell` launches and filed through `run-cell` itself.

## Python that remains, and why

Outside frozen runs (1,970 tracked `.py` files under `bench/runs/`), research records (415 under `docs/research/`),
imported originals (27 under `artifacts/`) and saved fixtures (18 under `bench/fixtures/`), the repository tracks one
Python file, `bench/claims/evidence/CL-l-initial-display.v1_probe.py`. It is the probe the `CL-l-initial-display` claim
records cite as evidence, kept byte-unchanged with them.

| Python | Why it stays |
| --- | --- |
| Review and grading targets | Tasks whose code is Python. The review sandbox and the grading sandbox expose the host's `python3` (Python 3.10 on the grading runner) so reviewers and graders can run a target's tests. It is a target language, not the engine. |
| Skills | Skill scripts are standard-library Python by the [script policy](../agents/scripts.md). `frozen-forge probe` evaluates a frozen skill's own `scripts/forge_packet.py` with `python3 -I` to read its queries. |
| Historical replay | A run frozen before it pinned `bench-tools` runs from a checkout of its freeze commit, with that commit's Python. `grade --provision SCRIPT` runs a recorded Python provisioner for such replays. Frozen runs, imported originals, saved fixtures and research records keep their Python byte-unchanged. |
| Optional probes and tests | Provisioning records `python3 --version` in a reviewer's platform line when one exists. The tests' stand-in clients, provisioner and probes are POSIX `sh` or the standard-library Rust program [`tests/support/stand_in/main.rs`](../../rust/bench-tools/tests/support/stand_in/main.rs), so no CI job installs Python. `grade prepare --provision` still names `python3`, so `grade_dispatch` puts a `python3` on `PATH` that hands the shell provisioner to `sh`. The tests that still run Python are listed below. |

### Python in tests

Each of these runs the host's `python3`. On CI it comes with the runner image; no job installs one.

| Test | Why it stays |
| --- | --- |
| `tests/dispatch.rs`: `bytecode_left_in_the_clone_changes_the_tree_identity_and_python_b_leaves_none` | Target Python. A reviewer's command imports a Python target's module in the clone, to show the bytecode it leaves there. |
| `tests/grading_sandbox.rs`, the Python-target cases | Target Python. The grading sandbox exposes the host's Python for Python targets; `the_namespace_blocks_reads_writes_and_external_network_and_allows_loopback` runs `/usr/bin/python3` inside it by absolute path. |
| `tests/review_isolation.rs`: `settings_name_this_executable_as_the_hook_and_verify` | Target toolchains. The review sandbox's settings expose `python3`, `node` and `go` for targets, so the test skips on a host that lacks any of them. |
| `tests/review_isolation_cli.rs`: `native_shell_filesystem_and_network_denials`, `worker_inherits_file_hook` | Target Python. They run `python3` inside the review sandbox, which exposes it for Python targets, and check Python's own errors. They run only with `BENCH_ISOLATION_CLI` set, never on CI. |
| `src/network_gate.rs`: `the_namespace_masks_host_sockets_and_refuses_vsock` | An exception. The probe opens an `AF_VSOCK` socket and calls `io_uring_setup`. The standard library cannot make either call without `unsafe`, which the crate forbids, and no POSIX tool makes them. The test skips without `python3`. |
| `tools/behavior_contracts/grade-prepare.v1.json` (3 cases), `grade-map.v1.json` (26 cases) | An exception. Their `shell` steps edit fixture JSON with `python3`. The corpus runner sets `PATH` to `/bin:/usr/bin`, so these steps use the host's Python even under `tools/without-python.sh`. Each saved case is pinned by a decision record, so rewriting a step needs a new decision version for that case. |

## Remaining limitations

- `tools/without-python.sh` removes interpreters from `PATH` only. A program that names one by absolute path is not
  stopped; every maintained caller finds programs on `PATH`.
- The fake-client cell runs only on Linux, as `dispatch.sh` does. aarch64 Linux and macOS run the runtime tests,
  install and rollback in CI; neither was run locally.
- The Python launcher's `python3` could be a version manager's shim that put the manager's tools on `PATH`. The shell
  launcher passes the environment through, so on such a host the grading sandbox refuses a shimmed `go` until the
  manager is activated ([runtime](runtime.md#running-a-maintained-command)).
- Installing again at a later commit whose crate tree is unchanged builds the same executable and is refused, because
  the stored receipt names the earlier commit. The selected entry still runs at that commit. This is the Python
  launcher's rule, ported unchanged.
- The shell launcher runs the store's `launcher` entry before that entry checks anything. Entries are read-only and
  the store is trusted as the checkout's Python file was; it is not defended against tampering.
- The store's `launcher` is the selected entry, and every checkout that shares the store runs it. Selecting an older
  version from any checkout moves every checkout to that entry's resolution rules. An entry that is only installed or
  restored never becomes the launcher.
- `tests/runtime.rs` has 14 tests where `test_rust_runtime.py` had 34. A rebuild from another checkout, an edit
  during the build, cargo configuration outside the private home, a foreign toolchain and a store reached through a
  symlink are no longer tested. Concurrent selects and rollbacks are.

# Migration completion report

This closes out [#160](https://github.com/kamui/code-review-bench/issues/160), the last stage of
[#149](https://github.com/kamui/code-review-bench/issues/149), under the
[simplified plan](https://github.com/kamui/code-review-bench/issues/149#issuecomment-6303498397): port what the
benchmark uses, delete the rest, verify with Rust tests and one run of the real commands. It records step 4c-2,
which replaced the Python launcher `bench/tools/rust_runtime.py` with `bench-tools runtime` and the shell launcher
`bench/tools/bench-tools` ([runtime](runtime.md)) and moved the `tools/` tests off Python. Step 4c-1, a separate
pull request, ports or deletes `seal.py`, `compare.py`, `rates.py`, `check_manifest.py`, the `bench/tools/test_*.py`
suites and `tools/fetch_freeze_commits.py`.

## Measurements

All numbers were measured on 2026-10-22 on one shared host: Linux 6.6 under WSL2, 24 cores, glibc 2.35,
Rust 1.99.1. The host ran other work throughout; each row gives the 1-minute load average when it ran. Before is
`main` at `f524dab48`, run from a clone of it; after is this branch, whose code was unchanged after the measurement
except for documentation. Each pair ran back to back. Single runs on a loaded host are not benchmarks. Only the launcher
rows, per command and in the fake-client cell, reflect code that runs faster.

| Measure | Before → after | Load | What the number counts |
| --- | --- | --- | --- |
| Launcher, 20 runs of `roster` | 5.30 s → 0.66 s (265 → 33 ms per command) | 12 | Python start-up and `rust_runtime.py` versus the shell launcher and `bench-tools runtime exec`; both hash the 19 MB executable before running it. |
| Fake-client cell, median of 5 | 0.81 s → 0.34 s | 17–20 | [`tools/fake-client-cell.sh`](../../tools/fake-client-cell.sh): dispatch, audit, normalize and `run-cell --file`. Three launcher calls account for the difference. |
| `bench-tools` install | 45.0 s → 42.0 s | 11–12 | One release build of the crate snapshot. The after run excludes the installer's own debug build, which `cargo run` reuses; building it from nothing took 26.7 s in a fresh clone. |
| `cargo test --locked`, tests prebuilt | 48.2 s → 49.2 s | 13–14 | The whole suite and every corpus; after adds `tests/runtime.rs` and `tests/evidence_consumers.rs`. An earlier before run at load 33–60 took 133.6 s. |
| `bun run test`, evidence cached | 165.9 s → 29.4 s | 5–13 | Not a speed-up. Before ran 22 Python `tools/` tests (112.5 s) and the 117 Python `bench/tools` tests (24.6 s). After runs the 14 ported `tools/` tests under bun (2.7 s); the evidence-consumer tests moved into `cargo test` (about 3 s there); the `bench/tools` suites no longer run from this script while step 4c-1 ports them. |

Agent iteration cost is unavailable: no per-step token or time ledger was kept for the migration.

## Python-unavailable evidence

`tools/without-python.sh COMMAND` runs a command with every `python*` entry removed from the `PATH` directories that
hold one. With it, on this host:

- `cargo test --locked --test behavior_contracts`: 1403 passed, 0 failed, 2 skipped, the same totals as `main`,
  including grade-prepare (17), grade-map (34) and export-explorer (49).
- `tools/fake-client-cell.sh`: a stand-in Claude client dispatched through `dispatch.sh`, audited and normalized by
  the run's pinned `bench-tools`, then filed by `run-cell --file` through `bench/tools/bench-tools --run`, filed
  `valid completed` with complete metering.

CI's `Rust runtime` workflow runs the corpora that way, and its x86_64 runtime job runs the install, `roster`, the
fake-client cell, `bun run data` and `bun run scorecard` with no Python on `PATH`. The export (2,710 files under
`public/`) and the scorecard, as text and JSON, are byte-identical to `main`'s.

A fresh `run-cell --next` claim is not part of this evidence: before it dispatches, rates-check-v1 compares the frozen
price with the providers' live pricing pages, which a fixture cannot answer. The cell is dispatched through the
same `dispatch.sh` that `run-cell` launches and filed through `run-cell` itself.

## Python that remains, and why

| Python | Why it stays |
| --- | --- |
| Review and grading targets | Tasks whose code is Python. The review sandbox and the grading sandbox expose the host's `python3` (Python 3.10 on the grading runner) so reviewers and graders can run a target's tests. It is a target language, not the engine. |
| Skills | Skill scripts are standard-library Python by the [script policy](../agents/scripts.md). `frozen-forge probe` evaluates a frozen skill's own `scripts/forge_packet.py` with `python3 -I` to read its queries. |
| Historical replay | A run frozen before it pinned `bench-tools` runs from a checkout of its freeze commit, with that commit's Python. `grade --provision SCRIPT` runs a recorded Python provisioner for such replays. Frozen runs, imported originals, saved fixtures and research records keep their Python byte-unchanged. |
| Optional probes and test doubles | Provisioning records `python3 --version` in a reviewer's platform line when one exists. The network-gate test uses `python3` as its in-sandbox probe and skips without it. The ignored `grade_dispatch` tests, which CI's grading job runs, write their fake Claude and Codex clients as Python scripts. |
| Step 4c-1 | `rates.py` (with `rates:refresh` and `rates.yml`), `seal.py`, `compare.py`, `check_manifest.py`, the `bench/tools/test_*.py` suites and `tools/fetch_freeze_commits.py`, which `pages.yml` still runs. |

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
- The store's `launcher` links the entry most recently installed or restored, and every checkout that shares the
  store runs it. Installing from an older checkout moves every checkout to that entry's resolution rules.
- `tests/runtime.rs` has 9 tests where `test_rust_runtime.py` had 34. Lock serialization under concurrent writers,
  a rebuild from another checkout, an edit during the build, cargo configuration outside the private home, a foreign
  toolchain and a store reached through a symlink are no longer tested.
- Until step 4c-1 lands, six of its seven `bench/tools/test_*.py` suites import the deleted `rust_runtime`, directly
  or through `test_claude_dispatch.py`, so no CI job runs their 71 tests of `dispatch.sh`, attempt
  recovery, review isolation, packet selection and cache replacements. `test_rates.py` still runs in `rates.yml`, and
  `tools/test_fetch_freeze_commits.py` runs in the Pages build job.

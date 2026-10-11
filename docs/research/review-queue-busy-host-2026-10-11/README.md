# Review queue verification on a busy host

Twenty consecutive `python3 tools/run_bench_tests.py` runs passed at clean commit `7db4d97971bccc5e712c592be19713dbc41b85f1` on Linux 6.6 WSL2. Each reported 843 tests with 18 skips, including all 58 queue tests. The campaign ran from 09:12:09 to 10:08:54 UTC on 2026-10-11 with the pinned Codex CLI 0.160.0.

A separate session of the same user repeatedly started Python children that allocated and hashed 32 MiB, then exited. It carried no queue launch marker and used a directory outside the checkout. Its 23,774 successful children continued through every suite. Each suite contained at least 1,135 completed workload children; the largest pause between children was 0.116 seconds. [The integrity record](integrity.json) checks the raw results, digests, monotonic timestamps and file hashes. [The summary](summary.json) records each suite's bounds and unchanged source state.

This meets [#103's remaining criterion](https://github.com/kamui/code-review-bench/issues/103): twenty consecutive full suites while the host does other work. The [earlier criteria](https://github.com/kamui/code-review-bench/issues/103#issuecomment-6106382926) were met by #106, and [the owner's scope decision](https://github.com/kamui/code-review-bench/issues/107#issuecomment-6107142539) limits the no-false-absence requirement to unreadable processes. Existing `ProcessObservation` coverage remains in the eight tests that passed in each suite.

## Failed attempt and shared evidence

The initial full-suite attempt reported five failures and eight errors from missing pinned evidence, while all 58 queue tests passed. `bun run evidence:fetch` restored those inputs. The 42 ruling-record tests and six claims integration tests then passed before the replacement campaign started. Source and client stayed unchanged. The [publication plan](publication-plan.json) preserves that replacement lineage and identifies the immutable raw evidence package. The [shared manifest](../../../bench/evidence/manifests/review-queue-busy-host-2026-10-11-v1.json) pins all 34 files and the public release asset. The [cold restoration receipt](../../../bench/evidence/receipts/review-queue-busy-host-2026-10-11-v1-restoration.json) records a separate remote download, hash verification and restoration into an empty temporary directory.

Retrieve the original logs and workload records from another checkout:

```sh
python3 bench/tools/evidence_store.py fetch --manifest bench/evidence/manifests/review-queue-busy-host-2026-10-11-v1.json
python3 docs/research/review-queue-busy-host-2026-10-11/verify.py artifacts/test-campaigns/review-queue-busy-host-2026-10-11/results-v2 --out /tmp/restored-integrity.json
```

## Repeat the checks

Use a clean checkout of the recorded commit, restore the pinned evidence, and put Codex CLI 0.160.0 in a separate client directory. The suite needs its namespace support. Choose a new output directory:

```sh
python3 campaign.py --repo /path/to/checkout --out /tmp/new-campaign --client-bin /path/to/client-bin
python3 verify.py /tmp/new-campaign --out /tmp/new-integrity.json
```

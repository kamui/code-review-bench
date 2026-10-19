# Python migration inventory

This is the scope record for [#150](https://github.com/kamui/code-review-bench/issues/150),
part of [#149](https://github.com/kamui/code-review-bench/issues/149).
It assigns migration work. It authorizes no port, dispatch, cleanup, runtime change,
grading decision, or publication of benchmark evidence.

## Reproducible records

[`inventory.v3.json.gz`](inventory.v3.json.gz) contains the census, compressed with
an empty gzip filename and a zero timestamp. Its `source_commit` identifies all
input bytes. The snapshot commit precedes only the generated inventory artifact,
and stays reachable from `main` only through a merge commit; a squash or rebase
merge would leave `--check` without it.
[`runtime-observation.v1.json`](runtime-observation.v1.json) is a separate, dated
observation of this delivery host, not a claim about other hosts or current liveness.
[`inventory.v1.json.gz`](inventory.v1.json.gz) and [`inventory.v2.json.gz`](inventory.v2.json.gz)
preserve the earlier censuses. v3 records the ownership after [#153](runtime.md) and
[#154](processing.md#close-out); `--check` verifies v3 only, because the assignment
tables now describe v3.

Generating or checking the census requires Python 3.11 or later to parse the
tracked sources, including research modules that use Python 3.11 syntax.

```sh
python3 tools/python_inventory.py --check
gzip -dc docs/python-migration/inventory.v3.json.gz > /tmp/python-inventory.json
python3 tools/python_inventory.py --ref HEAD --output /tmp/python-inventory-current.json.gz
python3 -m unittest discover -s tools -p test_python_inventory.py -v
```

`--check` reconstructs the recorded source commit, rather than comparing a historical
snapshot with a changing checkout. To refresh the scope, generate a new version at
an explicit committed revision and compare its classifications and callers. Preserve
earlier versions. Run the generator from the repository containing that revision.
It never imports the inventoried modules or runs their commands. Generation writes
the selected output file and creates missing parent directories; `--check` only
compares the reconstructed census without writing it.

The census enumerates every tracked `.py` path with its class, hash, physical line
count, code-resolution command, imports, parse errors, and incoming references.
Maintained and active research modules also have a responsibility, public symbols,
current tests, owner, migration issue, and contract-record pointer. An empty
`current_tests` means no directly resolved test was found, not that behavior is tested.
The generator itself belongs to #160 and its tests to #151.

`references` covers tracked text regardless of extension, including shell scripts,
package scripts, workflow YAML, container builders, Markdown, agent instructions,
JSON, and saved sessions. Each row has a path, line, matched tokens, and class.
Recover the original text with `git show <source_commit>:<path>`. Binary files are
outside the text scan; archives require their manifest and restoration procedure.
Physical lines are counts of source lines, never migration effort estimates.

The scan includes interpreter names, `sys.executable`, file references, dynamic
imports, and executable Python fragments. It intentionally retains prose and saved
commands as references. A reference is not proof that a command ran. Import edges
resolve same-directory modules and the maintained tooling import roots. Literal
paths resolve from the repository or the source document's directory, with parent
segments normalized. Shell `$(dirname "$0")/` paths and variables assigned that
sibling directory resolve relative to the caller. Other unresolved paths in Python
sources and current non-Python callers fall back to their basenames and produce
`possible-name-reference` caller records, without activating their targets. Current
TypeScript, harness and schema sources, `bench/README.md`, and the contributor
invocation guide are live callers. The preserved copy under `artifacts/import-source`
remains an imported original. Embedded `import` and
`from` clauses link their tracked Python modules as `embedded-import-reference`
callers; this is a conservative text scan, not proof that a command executed.
Python target commands in `bench/policies/grading-commands.v1.json` are target
dependencies. No scan can infer arbitrary computed Python.
The audited dynamic rules retain directories for loaders that assemble filenames
from parameters and exact files for fixed-name loaders. Refresh those rules when a loader changes.

## Classification and preservation

| Class | Meaning | Disposition |
| --- | --- | --- |
| `maintained` | `bench/tools` and `tools`, including tests | Assigned below. Translate callers with their implementation. |
| `active-outside-tooling` | Research code reachable from current tools, tests, docs, or instructions | Included in the migration boundary. Preserve the saved record separately from its current caller. |
| `frozen-evidence` | Run sources, fixture snapshots, and claim probes | Preserve bytes. Resolve code at `source_commit`, then use the recorded run runtime. |
| `imported-original` | `artifacts/import-source` | Preserve imported bytes and hashes. Never port these copies. |
| `historical-replay` | Other research helpers without a resolved live root | Retain original code for replay. No current-engine port is assigned. |
| `target-or-skill-dependency` | Frozen skill code and target probes or upstream excerpts | Keep the declared target or skill environment. Python remains allowed there. |

Classification is by role, not age. In particular, a research directory is not a
removal boundary. `test_dispatch_serial.py` imports the serial dispatcher. The historical answer
tables are separate from the current scorecard, whose sole owner remains `src/lib/scoring.ts`.

The explicit dynamic loaders in `test_packet_replacements.py`,
`test_review_code_selection.py`, and `test_last_push_move.py` also retain the staged
recut selection and the selection and filing code. Their local imports extend the closure
to `record`, `move`, and related helpers. `move.py` and `selection.py` import
`stage_edition` after adding its directory to `sys.path`, which v3 records as audited edges. `test_regrade_filing.py`
reaches the saved filing and activation implementations through `test_last_push_move`.
Parameterized directory rules include sibling replay commands as candidates;
the regrade and activation loaders retain only their named `record.py` files.
They do not authorize running a command that writes the current ledger.

No maintained responsibility is proven dead by this census. A missing caller alone
cannot establish that a CLI is unused. Historical duplicate runners and replay-only
helpers need no translation while retained as immutable evidence. #160 owns removal
of current entrypoints only after their callers migrate. This includes removing this
Python census generator or replacing it with an equivalent reproducible check.

The usage pilot now has a [behavioral contract corpus](contracts/README.md) for #151.

## Responsibilities and contracts

All migration owners below are **kamui**. Issue assignment denotes the target slice,
conditional on the #152 pilot and that slice's dependencies. It does not expand the
existing issue. Tests are assigned to #151 for contract capture and move with their
owning implementation later. The inventory's per-module assignment is exhaustive;
an unassigned maintained or active research module fails generation. `RESEARCH_GROUPS`
assigns each retained research helper explicitly, including usage to #152, freeze-ref
resolution to #153, the answer table that `import_benchmark` copies to #155, grading,
calibration and the staged recut selection to #156, and run preparation and dispatch to #157.

| Group and issue | Responsibilities and callers | Mutable state and public or serialized contracts | Existing verification |
| --- | --- | --- | --- |
| Behavioral contracts, #151 | `behavior_contract`; local usage corpus and implementation command maps | Versioned argv, input bytes, disposable roots, exit and output comparisons, filesystem effects, independent expectations and explicit change decisions. Later slices extend the corpus. | `test_behavior_contract.py`; [contract reference](contracts/README.md). |
| Usage, #152 | `codex_usage`, `transcript_usage`; dispatchers, runners, accounting and explorer consumers | Parse native usage, retain missing versus zero counts, cache categories, failed and replacement usage. Preserve JSON and TSV or CSV field names, cost units, flags, and output ordering. Read-only pilot. | `test_codex_usage.py`, `test_transcript_usage.py`; independent expected outcomes and differentials belong to #151. |
| Runtime, #153 | `rust_runtime`; run preparation and CI. v3 moves `seal` to #160 and `skill_provenance` and `fetch_freeze_commits` to #157 | Freeze commits, run-relative skill pins, SHA-256, exact runtime release provenance, client versions and artifacts, deviations, freeze fetches. Started operations retain their runtime. | `test_skill_provenance.py`, manifest checks, clean-context checks. |
| Processing, #154 | Packets, diff identity, normalizers, comparisons, legacy scores, SVG, rates, roster, upstream, methodology, explorer export and collection; package scripts, Pages and `dispatch.sh`. Each is now migrated, retired or assigned to a later stage, as [the close-out](processing.md#close-out) lists; v3 assigns no module to #154 | Packet and normalized-review schemas, rate catalog append semantics, method/task selection, exports and evidence URLs. Preserve byte hashes and unknown values. `score.py` and historical answer tables do not own the current scorecard kernel. | Per-module tests in the census; `test_export_explorer.py`, `test_collect_run.py`, `test_score_release.py`, `test_rates.py`, web scoring tests. |
| Storage, #155 | Evidence inventory, store, resolution, recovery, reconciliation, native artifacts, import verification; all readers and `verify:import` | Manifest logical paths and hashes, canonical release payloads, restoration receipts, missing and superseded evidence, refusal to overwrite changed files. Keep import verification until its replacement works. | Storage and import tests, `bun run verify:import`; [storage contract](../evidence-storage.md). |
| Grading, #156 | Schemas, audit, claim links, adjudications, calibration, current-grading validation, dossiers, rulings, grading and regrading; current tools and live filing tests | Owner-saved rulings, approved versus pending decisions, authorization, blinded grading, ledgers and reservations, mapping versions, exclusive batch state, transactional filing, quotas and disk reserve. Preserve every schema under `bench/schema`, rubric version, CLI exit status, and recovery protocol consumed by these modules. | Claim, grading, filing and ruling tests listed per module; [adjudication](../claim-adjudication.md), [record](../adjudication-record.md), [readiness](../grading-readiness.md). |
| Supervision, #157 | Provisioning, clean contexts, run cells, filing, review queues, meters, runner adapters, container and fake-client probes; dispatch scripts and contributor adapters | Claims, attempts, fresh homes, isolation receipts, PID/process lineage, locks, launch and recovery logs, rate limits, client pins, interrupted attempts, replacements, cleanup receipts, caps and authorization. One controller per run. Preserve command flags, environment names, native payloads and exit codes. | Runner, queue, interruption, container, isolation and client tests; [clean contexts](../clean-context.md). |
| Contributor, #158 | `contributor*` modules and `bin/bench`; README and contribution guide, harness adapters | `run`, `status`, `submit`, `run --resume`; `.bench/runs`, `pending`, `submitted`; locks, immutable freeze trees, submissions and receipts. Snapshot pending selection, evidence-only PRs, retry-safe upload and PR recovery, no automatic grading or scoreboard publication. Preserve requested and effective reasoning controls. | `test_contributor.py`, no-charge adapter probe; [contribution guide](../contributing-reviews.md). |
| Cleanup, #159 | Pruning and retirement; run filing, regrading, controllers | Valid-only, verified clone cleanup and receipts; whole-workspace inventory, investigation closure, restoration and exclusive retirement. Preserve protected active, failed, modified or unverified work and ledger-dependent bytes. | `test_prune_workspace.py`, `test_retire_workspace.py`; [storage contract](../evidence-storage.md). |
| Retirement, #160 | Test aggregator, census, wrappers, docs, CI and package commands | Keep each public entrypoint available through cutover, then remove the maintained Python implementation and migrate every caller. Retained targets, frozen skills and historical replay still declare Python separately. | Census check, contract suite, Python-unavailable current-engine acceptance after migration. |

[Rust runtime pinning](runtime.md#ownership-of-the-other-runtime-modules) moves `seal`,
`skill_provenance` and `fetch_freeze_commits` from the runtime group to #157 and #160.
[Processing retirement](processing-retirement.md) removes `score` and `scoreboard_svg`, keeps
`compare`, classifies five research helpers, and moves `methodology` to #156 and `collect_run` to #155.
v3 records both, assigns each retained library to the stage that removes it, as the
[retained libraries](processing.md#retained-libraries) table lists, and assigns the `tools/usage_pilot`
modules to #152, which owns the pilot.

Active research helpers carry the responsibility assignments in `RESEARCH_GROUPS`.
Their tests run on disposable copies because some APIs write whole
record generations. Preserve `before.v1.json`, validation, atomic replacement,
idempotent replay, and undo semantics. Do not run those helpers against live records
to validate this inventory. Each helper's owning slice in `RESEARCH_GROUPS` extends #151's corpus with those
behaviors before porting that helper.

`dispatch.sh` is a maintained #157 caller, including inline command validation,
client hash checks and native review launch. `regrade_queues.sh` is a maintained
#156 caller, including the inline Python `status` heredoc and the quota exit code 4.
`pages.yml` and `rates.yml` belong to #160 for caller retirement; their underlying
responsibilities retain the owning slices above. `package.json` delegates data,
tests, evidence, claims, current grading, imports and rates to Python. These callers
must move together with the relevant implementation, not after Python is removed.

## Runtime resolution and unfinished work

`runs` records every tracked run manifest, its `freeze_commit`, planned cells, saved
attempt dispositions, cells lacking a saved valid attempt, arms, execution policy,
deviations, and runtime-record paths. Missing valid attempts identify unfinished or
unavailable recorded coverage, not permission to resume it. Failed and replacement
attempts remain part of accounting. A saved valid attempt does not establish that
its process ended or that all cleanup completed.

The host observation records known workspace directories, attempt-artifact presence,
six saved queue pins and launches lacking saved ends, without reading credentials,
homes, process environments, or acquiring controller locks. It observes only
`~/.t3/bench-runs`, `~/.t3/bench-queues`, and presence of this checkout's `.local`
and `.bench`. It does not inspect the parent T3 worktree. Other hosts, overridden
work roots, untracked external queues, and process termination remain unknown.

```sh
python3 tools/python_inventory.py --observe-runtime \
  --work-root "$HOME/.t3/bench-runs" --queue-root "$HOME/.t3/bench-queues"
```

Observation mode emits JSON to stdout and accepts no `--output`, `--ref`, or
`--check`. It opens no output file. Capture stdout separately in a caller-owned
evidence path outside live or frozen state; shell redirection remains the caller's
responsibility.

Historical resolution follows the recorded dependency, in order:

1. Resolve a path with its recorded SHA-256 at the inventory's source commit, or
   resolve the run engine at its `freeze_commit`. If the Git object is absent,
   `tools/fetch_freeze_commits.py` documents the fetch route. Never substitute main.
2. Use that run's `inputs/runner.json`, client pins, skill pin and provenance,
   execution policy, rate snapshot and deviations. A frozen `source/` copy is
   evidence of the code saved there, not automatically a complete executable tree.
3. Restore detached payloads using their evidence manifest and verify hashes with
   `evidence_store`; imported originals use `bench/import-manifest.json`'s
   repository, revision, path and SHA-256. `imported_without_preserved_path` lists
   every imported entry still sharing its current location. Before an intended edit,
   follow `verify:import`'s printed `--preserve` command. This census changes none.
4. Resolve the interpreter from saved invocation, container receipt, image digest,
   build output, or runtime version record. An interpreter token such as `python3`
   proves only a lookup name. Where no exact interpreter or image was recorded,
   its provenance is **unknown**. Keep that replay environment unreconstructed
   until evidence supplies it. Installing today's Python is an exploratory replay,
   not proof of the original runtime. No version, image or interpreter pin is invented.

`review_queue.RUNTIME_PATHS` was `bench/tools`, `bench/schema`, `bench/policies`,
and `docs/clean-context.md` at this census. `runtime_problem` compares those paths
with `freeze_commit` before dispatch. #153 added `rust` to them and pins each Rust
executable a run uses; see [Rust runtime pinning](runtime.md). Development proceeds on separate
branches and checkouts. Unfinished runs retain the old checkout, Python environment,
client executables, companions, state roots and exclusive controller. Never point
an existing controller at this development checkout or start a second controller.
Rollback selects a retained version for new operations. Started operations keep
their pinned runtime and recovery protocol, without a cross-language side-effect retry.

## Platforms and container contents

The existing [script policy](../agents/scripts.md) names macOS and Linux, including
WSL, and Python 3.9+ for the general script contract. Native Windows is outside that
contract. This does not prove that every module runs on Python 3.9. Pages and rates
CI select Python 3.12 on Ubuntu. Linux namespace, `/proc`, bubblewrap and container
probes have narrower requirements, as [grading readiness](../grading-readiness.md)
states. Preserve those boundaries during the pilot and define supported Rust
artifacts in #153 rather than inferring support from a build on one host.

The container runtime is built by Python, not a tracked Dockerfile. In
`claude_container_probe.copy_runtime`, the supplied binaries and their `ldd`
libraries are copied, followed by the interpreter's `sysconfig` standard library
and extension dependencies. Site and dist packages and bytecode caches are excluded.
It copies `probe.py` and `clean_context.py`. The v4 controller adds the v4 probe,
resource controller and fixture Git scripts. `codex_container_probe.build` includes
`/usr/bin/python3`, its libraries and standard library, bash, bubblewrap, Git,
fixture Git, Codex, and the shared Claude probe and resource modules. Its sandbox
symlink and rootfs paths are public probe contracts. #157 owns these Python
dependencies even if the current host engine later runs in Rust. Frozen fixture
sources remain unchanged. The census reference rows expose these copy lists,
absolute interpreter commands, embedded `-c` payloads, hooks and generated scripts.

## Evolving contributor input

#148 was open in the delivery packet. Current forge state is closed and current main
contains `440cea9077f522a3a27d04573ea1a53090d04ba7`, `feat(bench): add contributor
review workflow`. This census includes that merged implementation, rather than
copying the other agent's untracked research. #158 must refresh this inventory from
the implementation actually merged at its integration point, including later adapter,
submission and maintainer-design changes. #150 does not implement or revise #148.

## Verification limits

The source scan and contract mappings are read-only. Focused tests exercise tracked
path enumeration, dynamic path reachability, embedded shell and package commands,
preservation classes, missing runtime provenance and required ownership. The
inventory check rebuilds all records from their committed source. Import verification
checks existing evidence identity. These checks make no model calls and do not
validate historical environments by launching them. Runtime liveness, unknown exact
interpreters and unavailable external host state remain unknown by design.

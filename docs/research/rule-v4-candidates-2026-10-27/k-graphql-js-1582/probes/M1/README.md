# Repeating the probes

Create fresh `base` and `head` clones in the scratch directory named in the brief, from the packet's mirror. Check out the two packet SHAs. Copy the saved `bun.lock` into `head`, then install its package dependencies with Bun, scripts disabled, and Bun's cache and `TMPDIR` set inside scratch. The original direct install command is recorded in `install.txt` and the preparation transcript; dependency versions used are in `environment.txt`.

From the preparation repository, run `python3 docs/research/rule-v4-candidates-2026-10-27/k-graphql-js-1582/probes/M1/run.py`. It links base to head's dependencies, executes both original test files, and runs the 16 head fixture/mutation combinations. It restores the source files in a `finally` block. Outputs have explicit exit codes; failing tests under a deliberate mutation are the observation being measured.

Run `documented-way.cjs` from each checkout with Node to repeat the README-style query. `neighbours-head.txt` is the unmodified baseline for `location-mutant-neighbours.txt`. One AST-inspection formatting test fails on modern Node regardless of the mutation. Initial harness outputs, superseded by the final `.cjs` probe and isolated stack mutation, are retained under `initial-run/`.

The local `check.py` at the output-directory root checks saved artifacts, exact case quotations, and the expected mutation matrix without recreating dependencies. The preparation clones and dependencies are deleted after filing; `cleanup.json` records the literal paths.

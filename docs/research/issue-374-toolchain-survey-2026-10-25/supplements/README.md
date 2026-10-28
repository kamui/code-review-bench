# Toolchain supplements v1

These archives add the five toolchains missing from the original target caches. Each profile in `bench/policies/grading-commands.v3.json` pins its archive. Preparation extracts the archive into `clone-cache/toolchain-extra`, then runs the profile's offline setup steps. Existing target records and dependency archives remain unchanged.

The shared destination is the public `kamui/code-review-bench` repository, release `evidence-issue-374-toolchains-v1`. The payload category is reproducibility dependencies and compiler runtime bytes. The owner's 2026-10-25 resolve instruction for PR #378 authorizes preparing, pinning and publishing these supplements with the implementation, using the repository's required evidence storage contract.

The published manifest is [`toolchain-supplements-2026-10-25-v1.json`](../../../../bench/evidence/manifests/toolchain-supplements-2026-10-25-v1.json). Its independent restoration receipt is [`toolchain-supplements-2026-10-25-v1.json`](../../../../bench/evidence/receipts/toolchain-supplements-2026-10-25-v1.json).

Restore all supplements before offline grading preparation:

```sh
export BENCH_RUNTIME_STORE="$HOME/.cache/issue-374-part2/runtime-store"
mise exec -- bench/tools/bench-tools evidence fetch \
  --manifest bench/evidence/manifests/toolchain-supplements-2026-10-25-v1.json
```

After restoration, grading preparation uses local archives only. A missing or changed archive stops preparation. An existing prepared batch keeps its previous policy hash. Rebuilding an archive requires a new published identity and policy version.

`build.py TARGET PREPARED_WORK NEW_OUTPUT` captures new payloads online. It expects the original target clone and cache in `PREPARED_WORK`. Run it through `mise exec -- python3 -I` with scratch output and `UV_CACHE_DIR` under the private store. It writes a command receipt beside the payload. `sphinx-requirements.txt` and `bokeh.bun.lock` pin transitive versions. The Bokeh package file replaces only the five original Git references with tarballs at the exact same commits. Cargo uses the target's original `Cargo.lock`. Go uses the original tools-module files. Biome uses the version pinned by Astro's root importer.

The archived compiler is Rust 1.99.1 for Linux x86_64, including only Cargo, rustc and `lib`. No credentials, source mirrors, grading records, paid client state or other sessions' stores are included.

# Dependency caches published on 2026-11-04

The rebuilt trpc dependency archive and every git mirror on `host-02` existed only on that host. On 2026-11-04 the owner deleted the local benchmark data on `host-01`, and with it, probably, the trpc archive `8f53201c` that `cache-replacements.v2.json` pins ([the record](../trpc-cache-rebuild-2026-11-04/README.md)). This release keeps the `host-02`-only copies on GitHub. [`proposal.md`](proposal.md) is the storage design it starts.

**Published:**
- **Destination:** the public `kamui/code-review-bench` repository, release `evidence-dependency-caches-2026-11-04-v1`.
- **Visibility:** public.
- **Category:** reproducibility inputs: dependency archives and source mirrors.
- **Authorization:** the owner approved this release on 2026-11-04 at about 4:00 AM US Eastern. The question: "The trpc archive rebuilt today (386 MB) exists only on host-02, and the audit depends on it. The 16 git mirrors (2.4 GB) aren't stored anywhere. The repo is public, so a release is public; it's all open-source dependency and git data, with no secrets or client binaries. Can I publish these as a new release, evidence-dependency-caches-2026-11-04-v1?" The answer: "Yes, publish both (Recommended)": "Publish the trpc archive and the 16 mirror bundles (about 2.8 GB), verify the download, and commit the manifests. No code changes yet."

**Contents:** one manifest per task, `bench/evidence/manifests/dependency-caches-<task>-2026-11-04-v1.json`, so a host fetches only the task it needs.
- `artifacts/reproducibility/mirrors/<task>.bundle`: `git bundle create --all` of the task's mirror in `.local/regrade-v5-2/dependency-cache/mirrors`, checked with `git bundle verify`. It holds the refs the mirror's receipt pins.
- `artifacts/reproducibility/mirrors/<task>.json`: the mirror's `provision mirror` receipt.
- For j-trpc-5017 also `artifacts/reproducibility/archives/j-trpc-5017.afd66914e90b51a9eab976d7ba905ef10cb9b7c95770edcf7364a3295513da3b.tar.gz`, the archive [`cache-replacements.v1.json`](../trpc-cache-rebuild-2026-11-04/cache-replacements.v1.json) pins, at the hash-keyed name `provision::archive_path` prefers.

`t-rclone-9699` has no mirror on `host-02` and is not included.

**Restore a mirror:**

```sh
bench/tools/bench-tools evidence fetch \
  --manifest bench/evidence/manifests/dependency-caches-<task>-2026-11-04-v1.json
git clone --mirror artifacts/reproducibility/mirrors/<task>.bundle <cache-root>/mirrors/<task>.git
git -C <cache-root>/mirrors/<task>.git remote remove origin
cp artifacts/reproducibility/mirrors/<task>.json <cache-root>/mirrors/
```

Then run `bench-tools provision check` against the cache root. A restored mirror has the same refs and objects as the original; its pack bytes can differ.

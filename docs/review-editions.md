# Review editions and exact provenance

The chart groups by review method and `review_edition` in `bench/scoreboard.current.json`. A review edition describes the procedure being tested, independently of the client's release number or a skill's latest commit.

Keep routine harness updates, skill patch bumps, packaging changes, and test-script changes in the same edition. Major or minor version bumps trigger a changelog or diff check. Split only when the review procedure materially changes, such as its review criteria, verification steps, or finding selection. A patch can still require a split when its actual behavior changes. Unversioned skills follow the same rule using their pinned source history.

Record an edition change in `review_change` with a short explanation and source URL. The explorer shows this link beside the setup and in its evidence. When rerunning the same method, edition, model, and effort after a routine update, update the current registry's selected run instead of adding another release-specific point. Preserve the earlier run and its raw records.

## Current grouping

Codex and Claude built-ins each use one baseline edition across the recorded client versions. No review-specific changelog boundary is registered for these runs. The checked sources are [Codex 0.157.0](https://github.com/openai/codex/releases/tag/rust-v0.157.0), [Codex 0.158.0](https://github.com/openai/codex/releases/tag/rust-v0.158.0), and the [Claude Code changelog](https://github.com/anthropics/claude-code/blob/main/CHANGELOG.md). General runtime, sandbox, or authorization-review changes remain execution metadata. The recorded Claude prompts differ by model; this is visible in the evidence and does not by itself establish a release boundary.

The imported `/review-code` snapshots retain their workflow editions, `v5b-25` and `v5b-30-x382`. The later edition changes the regression-test review rule and verification/report workflow. The [source comparison](https://github.com/kamui/skills/compare/bfed2957be58232dabb8a247a109d75a2b48993b...05e336791ffd7f42abb74421df47032405fe0b52) includes these changes in the referenced rubric, output procedure, renderer, and design. Its later test-script-only changes do not create another edition.

## Raw evidence

Each attempt retains the observed harness version, model IDs, reasoning effort, prompt hash, skill tree when applicable, and execution timestamps. Frozen manifests identify the exact arm and skill tree used. These records are not rewritten when chart grouping changes.

`bench/skill-provenance.json` maps the imported skill trees to verified source commits and commit timestamps. Tree SHA and commit SHA are different Git objects. Semantic version and release timestamp remain null when no published version or release date has been established. A commit timestamp is not a release timestamp.

For new skill runs, record the upstream version if declared, source repository, full commit SHA, skill subtree SHA, commit timestamp, and published release timestamp if known before freezing the run. Chart edition labels never replace these exact identities.

## Release provenance

The explorer displays the skill version when established and its release date. When no skill release date is established, it displays `Updated` with the date of the last commit affecting the skill's runtime files. Benchmark dates and filesystem modification times are not release dates. Workflow editions still control chart grouping independently of this display.

Capture provenance before freezing each new skill arm:

```sh
python3 bench/tools/skill_provenance.py \
  --repository /path/to/upstream --revision <full-commit> \
  --skill-path skills/<skill> --frozen bench/runs/<run>/inputs/skill \
  --benchmark-tree <resolved-skill-tree> \
  --out bench/runs/<run>/inputs/skill-release.json
```

The command checks the frozen runtime files against the pinned upstream commit, records their content hashes, and prints the provenance file's SHA-256. Add `skill_provenance: {"path": "inputs/skill-release.json", "sha256": "<printed-hash>"}` to the manifest arm before freezing it. Dispatch checks the file's hash and benchmark tree. Historical manifests without this field remain readable.

Check upstream releases before collecting. If using a published release, save the upstream release object with its `tag_name`, `published_at`, and `html_url`, then pass `--release-json <saved-release.json>`. The collector checks the tag's runtime files against the pinned skill before accepting its date or version. A plugin version alone does not establish the version of a standalone skill installed from main. Keep the version absent when no version is established.

The fallback includes `SKILL.md`, references, runtime scripts, agents, and other operational files. It excludes design, changelog, history, README, licenses, and test-only files by default. Deleting a runtime file also changes the fallback date. Inspect the recorded `runtime_files` and use `--exclude <glob>` for additional files that cannot affect execution. Keep all Markdown the skill may read; use `--include <glob>` to override an exclusion for a file used at runtime. Use complete Git history; shallow clones and unmatched runtime snapshots are refused. When a benchmark adapter changes a frozen file, verify the preserved upstream source and record the adapter deviation separately.

Recovered historical identities live in [provenance v2](../bench/skill-provenance.v2.json), which supersedes the earlier provenance file without rewriting frozen manifests or raw reviews. CE's preserved source matches upstream commit `41b36f0ba506e316a9d5921d1ce3eb6f42149e62`; its artifact-directory adaptation remains in the frozen skill pin. Thermo's frozen `SKILL.md` matches `09f3a50fc35eea711d00fb6f8cbabf5613c55696`. Both installations are unversioned standalone skills. Their fallback dates are September 28 and September 7, 2026. The two imported `/review-code` runtime dates are September 24 and September 27, 2026.

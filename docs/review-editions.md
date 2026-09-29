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

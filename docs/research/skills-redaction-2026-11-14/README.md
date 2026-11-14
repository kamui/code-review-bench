# Skills repository redaction, 2026-11-14

`kamui/skills` was re-created on 2026-11-14 from a rewritten history, the same way this repository was on 2026-11-13. Its research folders held the same account identifiers and machine names. The earlier repository is kept as a private archive, `kamui/skills-archive`. The record of that redaction is in the skills repository, at [`docs/research/account-redaction-2026-11-14/`](https://github.com/kamui/skills/tree/main/docs/research/account-redaction-2026-11-14).

This note says what that means for the records here.

## What this repository pins

Nothing here is fetched from `kamui/skills` to run a benchmark. `bench-tools` extracts a pinned skill tree from this repository's own objects. The dependency is provenance: run manifests and arms name commits and trees of the skills repository.

- 39 commit ids in `bench/runs/*/manifest.json` are commits of the skills repository. Every one changed in the rewrite, because each descends from a commit that changed. The frozen manifests keep the earlier ids, which now exist only in the private archive.
- Five tree ids are pinned by arms and manifests. A tree id does not depend on history, and the skill files held no identifier, so none changed. Four are reachable in the new skills repository. The fifth, `59e7df1b15`, was never in it: applying `bench/runs/2026-09-28-verification-off-staged/candidate.patch` to tree `5e12864b52` produces it.

[`skills-pins.v1.json`](skills-pins.v1.json) lists each of the 39 commits with its earlier id, its new id, the tags that reach it and the records here that cite it, and each tree with where it is reachable.

## Where the pinned commits are now

Pull request refs do not exist in a re-created repository, and most of these commits were reached only through them. Three tags in the new skills repository keep every one reachable: `provenance/pr-397-head`, `provenance/pr-407-head` and `provenance/pr-413-head`.

## What did not change

No file in this repository changed for the skills redaction except this folder. Frozen runs and their manifests are as they were. Issue and pull request numbers of the skills repository were kept, so links such as `kamui/skills#413` still resolve, to an issue that describes the earlier pull request.

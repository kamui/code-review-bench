# Brief: screen saved rulings of re-cut benchmark tasks for reasons that come from after the last push

You are reading records in the repository at
`/home/jack/.t3/worktrees/code-review-bench/t3code-d7655b09` (a git checkout; read it, do not edit,
create, move or delete anything inside it). Write your result only to the output file named at the
end of this brief.

## Background

This repository is a benchmark of code-review tools. Each task is one past pull request. Reviewers
get a "packet" describing the pull request as it stood at a cut-off time. The owner (called "the
user") has ruled, for each task, which review comments describe real problems ("known problems",
with ids like `GT-i1`) and which are advice. Those rulings were made while the cut-off was the moment
the pull request merged.

The cut-off has now moved earlier, to the last push to the pull request. The rule in force says:

> A ruling made under an earlier version used the task's recorded cut-off. Before carrying such a
> ruling to a re-cut task, check that none of its reasons comes from between the last push and that
> cut-off.

and, from the same rule:

> A reviewer could know the diff, the code and documentation at that commit, what running them
> shows, what the task packet gave them, and anything public by the cut-off. Later evidence is never
> the reason for an answer. Later evidence means reports, fixes, reverts and anything maintainers
> said after the cut-off. It has two uses: to check a ruling already made from earlier facts, and to
> confirm a fact from before the cut-off.

Your job is a screen, not a ruling. For each saved decision of your task(s), say whether any of its
reasons rests on something that became knowable only in the window between the last push and the old
cut-off. You do not decide what happens to a ruling. Never guess: when the files do not settle a
point, say "cannot tell" and name what would settle it.

## Where things are

For a task `<task>` (for example `i-requests-6667`):

- `docs/research/last-push-recut-2026-10-07/receipts/<task>.json`: `cutoff` is the last push;
  `pinned_cutoff` is the old cut-off; `omitted_records` lists what the old packet showed and the new
  one does not; `text_as_of_cutoff` lists text that was rewritten inside the window (the new packet
  shows the earlier wording).
- `bench/targets/<task>/packet.md` is the old packet and `bench/targets/<task>/packet.v2.md` the new
  one. Diff them to see exactly what text exists only in the old one.
- `docs/research/last-push-recut-2026-10-07/README.md`, section "What the pinned packets gave away",
  has a paragraph on each task.
- `bench/grading/current/adjudications.json`: `decisions`, each with `target`, `id`, `subject`,
  `dimension` (eligibility, impact or control), `outcome`, `reason`, `evidence`, `receipt` (a file
  path) and `receipt_scope` (the passage of that file that holds the user's ruling).
- `bench/grading/current/references.json`: `targets`, each with `families` (the known problems) and
  their trigger, mechanism and evidence.
- `bench/grading/current/claims.json`, `candidates.json` and `credits.json` hold the claims,
  candidates and rulings on single comments for the task.
- The user's rulings in full: `bench/grading/rulings/*.md` (receipts), `bench/claims/rulings/*.md`,
  `docs/research/cohort-rebuild-2026-10-05/rulings/*.md` (first round),
  `docs/research/cohort-rebuild-2026-10-05/second-pass/rulings/*.md` (second pass), and the dossiers
  under `docs/research/cohort-rebuild-2026-10-05/second-pass/candidates/` and
  `docs/research/cohort-rebuild-2026-10-05/candidates/` if present. Search them for your task's name,
  pull request number, known-problem ids and decision subjects.
- `docs/research/impact-boundary-2026-10-04/` and `docs/research/reference-calibration-2026-10-03/`
  hold impact cards and control audits that some decisions cite.

Use `rg` to search. You may fetch public GitHub data read-only with `gh api` (GET only) when you
need the date of a comment, review or edit that a ruling cites and the files do not give it.

## What to decide for each decision

Take every decision in `adjudications.json` whose `target` is your task, and every ruling in
`credits.json` for it. For each one, read its `reason`, its `receipt_scope` passage in the receipt
file, and the fuller ruling file or dossier when one exists. Then give one verdict:

- `clear`: every reason rests on the diff, the code or documentation at the reviewed commit, on what
  running them shows, on statements dated at or before the last push, or on later evidence that the
  ruling uses only to confirm an earlier fact.
- `window`: at least one reason cites a statement, record or wording that exists only between the
  last push and the old cut-off (an omitted record, or the later wording of rewritten text). Quote
  the cited words, say which file and line cites them, say which omitted record or rewritten text
  they come from, and say whether something knowable at the last push says the same thing (for
  example a code comment in the diff), with its quotation and location.
- `cannot-tell`: the files do not show when a cited statement was made, or the reasoning is too
  thin to judge. Say what would settle it.

Also note, separately, any decision whose reasons rest on evidence from after the old cut-off as the
reason itself (not as confirmation). That is outside your screen's question, so only list it.

A ruling in `credits.json` is about one saved review comment. Those reviews are being replaced, so
for each such ruling just record its id and the known problem it concerns; no verdict is needed.

Mark how you know each fact: `read` (you saw it in a file, give path and line) or `fetched` (from
`gh api`, give the command).

## Output

Write one JSON file per task to the path given below, with this shape, and nothing else to disk:

```json
{
  "task": "<task>",
  "last_push": "<cutoff>", "old_cutoff": "<pinned_cutoff>",
  "window_material": [{"what": "...", "kind": "omitted-record|rewritten-text|other", "author": "...", "at": "..."}],
  "decisions": [
    {"id": "...", "subject": "...", "dimension": "...", "outcome": "...",
     "verdict": "clear|window|cannot-tell",
     "basis": "one or two sentences on what the reasons rest on",
     "window_citation": null,
     "same_thing_knowable_at_push": null,
     "sources": ["path:line", "..."]}
  ],
  "credit_rulings": [{"id": "...", "known_problem": "..."}],
  "rests_on_evidence_after_old_cutoff": [{"id": "...", "note": "..."}],
  "summary": {"clear": 0, "window": 0, "cannot_tell": 0}
}
```

For a `window` verdict, `window_citation` is `{"quote": "...", "cited_in": "path:line", "comes_from": "..."}`
and `same_thing_knowable_at_push` is `{"quote": "...", "where": "path:line"}` or `null`.

Finish with a short plain-text report: the counts per verdict for each task, and each `window` or
`cannot-tell` decision in one or two sentences. Report only what you checked.

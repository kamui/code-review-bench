# Sources for group N1

Fetched or run on 2026-11-07 (UTC) unless marked as copied.

## Fetched or run in this round

| File | Command or URL | Date |
| --- | --- | --- |
| `maintainers-first-keystroke.json` | `gh api --method GET search/issues -f q='repo:mui/base-ui "first keystroke" created:<2026-08-10T13:33:57Z' -f per_page=50` (4 hits: #5315, #5460, #3819, #3820; #5460 matches through its current body, rewritten after the cut-off) | 2026-11-07 |
| `maintainers-field-control-render-keystroke.json` | `gh api --method GET search/issues -f q='repo:mui/base-ui Field.Control render keystroke created:<2026-08-10T13:33:57Z' -f per_page=50` (3 hits: #5460, #5348, #3819) | 2026-11-07 |
| `pr-5460-base-to-head.diff` | `git diff 30b8ea2004fa999bed151204208676c6c0a9d261 14d39e5d1ad6b7aca2fb067415dba09c6bea219b` in a scratch repository fetched from https://github.com/mui/base-ui.git | 2026-11-07 |
| `change-search.txt` | `rg -n -i 'render\|keystroke\|first\|flip\|perform'` over the diff above and over the description at the cut-off (`bench/targets/r-base-ui-5460/packet.v3.md`, section 3) | 2026-11-07 |

## Copied unchanged from the earlier round (regrade-v10, 2026-10-17)

Each file below is a byte copy of `docs/research/regrade-v10-rulings-2026-10-17/r-base-ui-5460/upstream/<name>` without the `regrade-v10-` prefix. The commands and dates are that round's, recorded in its dossier `docs/research/regrade-v10-rulings-2026-10-17/r-base-ui-5460/dossiers/C1na.md` and summary.

| File | What it is (that round's command) |
| --- | --- |
| `regrade-v10-C1na-project-docs-search.txt` | `rg` over the Field, Form and Input pages, the forms handbook, the rest of the handbook and the overview at head 14d39e5 for re-render, renders once/twice, data-dirty/data-filled, performance (37 matching lines, all read) |
| `regrade-v10-C1na-owner-docs-react-useLayoutEffect.md`, `regrade-v10-C1na-owner-docs-search.txt` | React's `useLayoutEffect` page at react.dev commit b4610fb (2026-06-22), and the `rg` over it (11 hits) |
| `regrade-v10-C1na-change-search.txt` | that round's search of the description and the diff |
| `regrade-v10-C1na-maintainers-searches.txt` | five `search/issues` queries limited to `created:<2026-08-10T13:33:57Z` (6 hits) |
| `regrade-v10-C1na-thread-3819.json`, `regrade-v10-C1na-thread-3819-comments.json` | `gh api repos/mui/base-ui/issues/3819` and its comments |
| `regrade-v10-C1na-public-code-controlled-field-control.json` | `gh api search/code -f q='"Field.Control" "onValueChange" "value=" language:TSX'` (984 hits, first 100 returned, 3 read) |
| `regrade-v10-C1na-code-read.txt` | that round's notes on the three code hits read |
| `regrade-v10-pr-5460.json` | `gh api repos/mui/base-ui/pulls/5460` (merged_at 2026-08-13T11:23:20Z) |
| `regrade-v10-pr-5460-usercontentedits.json` | GraphQL `userContentEdits` of the pull request body (edit at 2026-08-10T13:34:21Z) |
| `regrade-v10-releases.json` | `gh api repos/mui/base-ui/releases` (v1.8.0, 2026-09-04, lists #5460) |
| `regrade-v10-master-FieldControl.tsx` | `FieldControl.tsx` on upstream master as fetched 2026-10-17 |

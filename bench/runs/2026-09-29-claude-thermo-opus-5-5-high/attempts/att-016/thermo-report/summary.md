# Thermo-nuclear code quality review — BurntSushi/ripgrep#2957

Range: `79cbe89d..855bfa6c` (`git diff main...review-head`). Two files: `crates/core/flags/complete/rg.zsh` (+9/−1) and `FAQ.md` (+20/−3).

## Verdict

**Request changes (small, focused).** The goal is right and the headline scenario works: `source <(rg --generate complete-zsh)` after `compinit` now registers `_rg`. I verified this by running it. The implementation, though, solves the problem with a single mixed-up predicate. It treats "is this being autoloaded?", "is the completion system loaded?" and "is the CI harness running?" as one boolean. As a result the shipped script contains a branch that exists only for the test harness, and in production that branch reproduces the exact error the PR closes. A clean code-judo move exists: dispatch on how the file was loaded (`zsh_eval_context`) and move the harness's needs into the harness. That makes the two unresolved findings below disappear, not just move. No file-size concerns: `rg.zsh` is 645 lines, and `FAQ.md` was already over 1k at the base.

## Findings

**F1 — Test-harness mode is encoded as production dispatch, and it re-opens #2956 for a common `.zshrc` ordering.** In `crates/core/flags/complete/rg.zsh:441`, the `|| (( ! $+functions[compdef] ))` clause was added only so `ci/test-complete:16`, which sources the file with compsys not loaded, keeps working. For real users, "sourced while `compdef` does not exist" means only that `source <(rg --generate complete-zsh)` was placed before `compinit`. In that case the script calls `_rg` outside a completion context and prints the same `_arguments` error the PR exists to remove. I verified this by running zsh 5.8.1: `_rg:341: command not found: _arguments` with exit 1 when sourced before compinit, and the issue's `comparguments: can only be called from completion function` when compsys functions are already available. The CI harness now exercises a mode that no real user hits and covers neither real load mode. The remedy is to dispatch on `[[ $zsh_eval_context[-1] == loadautofunc ]]` (run `_rg "$@"`) and otherwise `compdef _rg rg`. `ci/test-complete` then does `( _RG_COMPLETE_LIST_ARGS=1; compdef() { : }; source $1; _rg )`, which removes the harness-only branch from shipped code. A mis-ordered `.zshrc` then gets a plain `compdef` error, or an optional one-line "run compinit first" guard. Full evidence and the worked proposal are in `01_zsh_completion_dispatch.md` (Finding 01-A).

**F2 — `$funcstack[1] == _rg` silently couples behaviour to the install filename.** The autoload check at `crates/core/flags/complete/rg.zsh:441` compares against the autoloaded function's name. That name comes from the file name on `$fpath`, not from `#compdef rg` or the `_rg()` definition. I verified that when the unchanged generated script is installed as `_ripgrep` and invoked the way compinit would, it does not run the completion body. It only executes `compdef _rg rg`, so the first TAB yields nothing. `zsh_eval_context[-1] == loadautofunc` reports autoloading regardless of file name; I verified it for `_probe` and `_other`. The same one-line change as F1 therefore fixes this. Details are in `01_zsh_completion_dispatch.md` (Finding 01-B).

**F3 — The FAQ's `.zshrc` snippet is not copy-pasteable, and neither zsh method states its required ordering relative to `compinit`.** At `FAQ.md:135`, the line users are told to add to `$HOME/.zshrc` is written as `$ source <(rg --generate complete-zsh)`, with an interactive prompt marker. The sibling `fpath=(...)` snippet at `FAQ.md:128` correctly leaves the marker out. The section also never says that `fpath=(...)` must come before `compinit` while `source <(...)` must come after it. Given F1, getting the second ordering wrong produces the original #2956 error, so the doc gap and the code gap make the same failure worse together. The remedy is to drop the `$ `, add a "before `compinit`" sentence to the fpath method and an "after `compinit`" sentence to the source method, and fix F1 so a mis-ordering fails clearly anyway. Details are in `02_faq_zsh_docs.md` (Finding 02-A).

**F4 — The new comment in `rg.zsh` misdescribes the branch it documents.** `crates/core/flags/complete/rg.zsh:437` says "Don't run the completion function when being sourced by itself". The code does run it when sourced without `compdef`, and it gives only two URLs instead of stating which load modes the file supports. The file is otherwise very carefully documented, and its header (`rg.zsh:1-13`) still describes only the fpath use. Replace the comment with a two-line statement of the autoload-vs-source contract, as in the F1 proposal, and mention the `source` usage in the header. This is legibility, not a blocker. Details are in `01_zsh_completion_dispatch.md` (Finding 01-C).

**F5 — The trade-off caveat sentence is ungrammatical.** `FAQ.md:138-139` reads "while this approach is easier to setup, is generally slower…". The main clause has no subject, and "setup" is used where the verb "set up" is meant. This is the maintainer's deliberate performance caveat, so it should read cleanly: "while this approach is easier to set up, it is generally slower than the previous method and will add time to loading your shell prompt." Low severity. Details are in `02_faq_zsh_docs.md` (Finding 02-B).

## Proposed remediation sequence

1. Replace the tail of `rg.zsh` with the `zsh_eval_context`-based two-way dispatch, plus a contract comment (fixes F1, F2, F4).
2. Update `ci/test-complete` to stub `compdef` and call `_rg` explicitly after sourcing. Re-run it against a built `rg`; this was not possible in this review.
3. Optionally add a one-line "run compinit first" guard before `compdef` for a friendlier error.
4. Fix the FAQ: remove the `$ ` from the `.zshrc` snippet, state the `compinit` ordering for both methods, and repair the caveat sentence (F3, F5).

## Verification notes

I verified all behavioural claims by running scratch zsh 5.8.1 scripts under `clone-work/scratch/` (`t1`–`t6`, `proposed.zsh`) against a copy of the generated script. The real `ci/test-complete` and `cargo` builds could not be run under this review's execution policy, so the harness side of the F1 remedy is checked only in simulation.

## Detail files

- `01_zsh_completion_dispatch.md` — the load-mode dispatch in `rg.zsh`, the probes, and the worked code-judo proposal.
- `02_faq_zsh_docs.md` — the FAQ zsh section.

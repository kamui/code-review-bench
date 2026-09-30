# Review blind-d12f20

### Item 1
Location: crates/core/flags/complete/rg.zsh:441
Claim: **F1 — Test-harness mode is encoded as production dispatch, and it re-opens #2956 for a common `.zshrc` ordering.** In `crates/core/flags/complete/rg.zsh:441`, the `|| (( ! $+functions[compdef] ))` clause was added only so `ci/test-complete:16`, which sources the file with compsys not loaded, keeps working. For real users, "sourced while `compdef` does not exist" means only that `source <(rg --generate complete-zsh)` was placed before `compinit`. In that case the script calls `_rg` outside a completion context and prints the same `_arguments` error the PR exists to remove. I verified this by running zsh 5.8.1: `_rg:341: command not found: _arguments` with exit 1 when sourced before compinit, and the issue's `comparguments: can only be called from completion function` when compsys functions are already available. The CI harness now exercises a mode that no real user hits and covers neither real load mode. The remedy is to dispatch on `[[ $zsh_eval_context[-1] == loadautofunc ]]` (run `_rg "$@"`) and otherwise `compdef _rg rg`. `ci/test-complete` then does `( _RG_COMPLETE_LIST_ARGS=1; compdef() { : }; source $1; _rg )`, which removes the harness-only branch from shipped code. A mis-ordered `.zshrc` then gets a plain `compdef` error, or an optional one-line "run compinit first" guard. Full evidence and the worked proposal are in `01_zsh_completion_dispatch.md` (Finding 01-A).
Consequence: —
Fix: —

### Item 2
Location: crates/core/flags/complete/rg.zsh:441
Claim: **F2 — `$funcstack[1] == _rg` silently couples behaviour to the install filename.** The autoload check at `crates/core/flags/complete/rg.zsh:441` compares against the autoloaded function's name. That name comes from the file name on `$fpath`, not from `#compdef rg` or the `_rg()` definition. I verified that when the unchanged generated script is installed as `_ripgrep` and invoked the way compinit would, it does not run the completion body. It only executes `compdef _rg rg`, so the first TAB yields nothing. `zsh_eval_context[-1] == loadautofunc` reports autoloading regardless of file name; I verified it for `_probe` and `_other`. The same one-line change as F1 therefore fixes this. Details are in `01_zsh_completion_dispatch.md` (Finding 01-B).
Consequence: —
Fix: —

### Item 3
Location: FAQ.md:135
Claim: **F3 — The FAQ's `.zshrc` snippet is not copy-pasteable, and neither zsh method states its required ordering relative to `compinit`.** At `FAQ.md:135`, the line users are told to add to `$HOME/.zshrc` is written as `$ source <(rg --generate complete-zsh)`, with an interactive prompt marker. The sibling `fpath=(...)` snippet at `FAQ.md:128` correctly leaves the marker out. The section also never says that `fpath=(...)` must come before `compinit` while `source <(...)` must come after it. Given F1, getting the second ordering wrong produces the original #2956 error, so the doc gap and the code gap make the same failure worse together. The remedy is to drop the `$ `, add a "before `compinit`" sentence to the fpath method and an "after `compinit`" sentence to the source method, and fix F1 so a mis-ordering fails clearly anyway. Details are in `02_faq_zsh_docs.md` (Finding 02-A).
Consequence: —
Fix: —

### Item 4
Location: crates/core/flags/complete/rg.zsh:437
Claim: **F4 — The new comment in `rg.zsh` misdescribes the branch it documents.** `crates/core/flags/complete/rg.zsh:437` says "Don't run the completion function when being sourced by itself". The code does run it when sourced without `compdef`, and it gives only two URLs instead of stating which load modes the file supports. The file is otherwise very carefully documented, and its header (`rg.zsh:1-13`) still describes only the fpath use. Replace the comment with a two-line statement of the autoload-vs-source contract, as in the F1 proposal, and mention the `source` usage in the header. This is legibility, not a blocker. Details are in `01_zsh_completion_dispatch.md` (Finding 01-C).
Consequence: —
Fix: —

### Item 5
Location: FAQ.md:138-139
Claim: **F5 — The trade-off caveat sentence is ungrammatical.** `FAQ.md:138-139` reads "while this approach is easier to setup, is generally slower…". The main clause has no subject, and "setup" is used where the verb "set up" is meant. This is the maintainer's deliberate performance caveat, so it should read cleanly: "while this approach is easier to set up, it is generally slower than the previous method and will add time to loading your shell prompt." Low severity. Details are in `02_faq_zsh_docs.md` (Finding 02-B).
Consequence: —
Fix: —

# Thermo-nuclear code quality review: ripgrep PR 2957 (zsh completion sourcing)

Range: `79cbe89..855bfa6` (2 files, +29/-4). Detail files: `01_zsh_completion_script.md`, `02_faq_docs.md`.

## Verdict

Request changes on structure, though this is a small and mostly tidy diff. File-size rules are not triggered: `rg.zsh` goes from 637 to 645 lines and `FAQ.md` is about 1063 lines (it was already over 1k, and this PR is not what crossed the line). The diff has one substantive design problem. The new dispatch at the bottom of `rg.zsh` has a fallback arm that, in one common configuration, reproduces the exact failure the PR was written to fix, and that arm exists to satisfy a CI script instead of a user need. The FAQ additions also have a copy-paste bug and a broken sentence.

## Findings

**1. The `! $+functions[compdef]` disjunct reruns the original bug, and it is there for the test harness (`crates/core/flags/complete/rg.zsh:441`).**
The condition is `[[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] ))`. The first half is a sound "am I being autoloaded as the `_rg` function" test. The second half means "compinit has not run, so call `_rg` right now." I ran that logic under `zsh -f`. `source <(...)` before `compinit` prints `_rg: command not found: _arguments` and returns 127. That is the same class of error as issue #2956, only less obvious. The only caller that needs the "no completion system, just run it" path is `ci/test-complete`, which sources the file with `_RG_COMPLETE_LIST_ARGS=1` and no `compdef`. The PR thread shows the arm was added to make that script pass. So a CI concern leaked into the shipped script and made the user-facing behavior worse in a plausible setup, since many `.zshrc` files source tools before `compinit`. The script already has a dedicated test hook, `_RG_COMPLETE_LIST_ARGS` (`rg.zsh:30` and `:347`). The code-judo move is to key the "run `_rg` immediately" case on that hook, or on `funcstack[1]`. Then the no-`compdef` case is free to do something sensible, such as registering later or warning. The dispatch becomes three intent-named cases, not a condition that merges a user case and a test case. Detail: `01_zsh_completion_script.md`.

**2. The FAQ never says `source <(rg --generate complete-zsh)` must come after `compinit` (`FAQ.md:132-139`).**
Given finding 1, a reader who follows the new one-liner and puts it above `compinit` in `.zshrc` gets a confusing error. The note that follows only discusses speed. Either the script should handle the ordering (preferred, see finding 1) or the FAQ must state the ordering requirement. Detail: `02_faq_docs.md`.

**3. The new FAQ snippets are inconsistent, and one is broken to copy (`FAQ.md:135`, `:138-139`).**
The `fpath=` block has no `$` prompt, but the `source` block inside a ```` ```zsh ```` fence keeps a leading `$ `. Someone pasting it into `.zshrc` gets `$: command not found`. That is worse here than in the older blocks, because those are run as shell commands and this one is meant to be saved to a file. The caveat sentence "while this approach is easier to setup, is generally slower than the previous method" is ungrammatical: it has a stray comma and a doubled predicate, and "setup" should be the verb "set up". Detail: `02_faq_docs.md`.

**4. The comment above the dispatch explains the wrong thing (`rg.zsh:437-440`).**
"Don't run the completion function when being sourced by itself" says nothing about the two conditions, and it points to an issue and a PR instead of stating the invariant. If finding 1 is adopted, one line per case would replace it. This is minor and should be folded into the finding 1 rewrite.

## Remediation sequence

1. Restructure the `rg.zsh` tail so the three states (autoloaded as `_rg`, sourced with `compdef` available, sourced with no completion system) are handled explicitly. Key the test harness on `_RG_COMPLETE_LIST_ARGS`. Make the no-`compdef` case non-erroring, or make the failure message clear.
2. Fix the FAQ snippet: drop the `$ `, fix the sentence, and state the `compinit` ordering unless the script now tolerates it.
3. Rewrite the comment to describe the cases.

## What was checked and how

The diff was read in full. The dispatch logic was exercised in scratch zsh scripts (autoload from `fpath`, sourcing after `compinit`, and sourcing without `compinit`), with the results in the detail file. Not run: `cargo build`, `cargo test` and `ci/test-complete`, which are unavailable here. So the claim that the harness needs the fallback arm rests on reading `ci/test-complete` and the PR thread. It was not confirmed by running the script.

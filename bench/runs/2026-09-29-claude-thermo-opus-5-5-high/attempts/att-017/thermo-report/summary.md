# Thermo-nuclear code quality review — BurntSushi/ripgrep#2957

Range: `79cbe89deb1151e703f4d91b19af9cdcc128b765..855bfa6cdae4f4fe8762f892fc4957635397083e` (2 files, +29/−4). The PR makes the hand-written zsh completion script usable through `source <(rg --generate complete-zsh)`, and it updates the FAQ's zsh instructions.

## Verdict

Request changes. The changes are small, and the core fix is correct: sourcing after `compinit` now registers `_rg` and no longer fails with the `comparguments` error from #2956. This was verified by running the generated script under zsh 5.8.1 (see `01_zsh-completion-dispatch.md`, scenarios B and B'). Two problems remain. The shipped dispatch contains a test-only branch, and the FAQ text as written would break a user's `.zshrc`. No file crosses the 1000-line threshold because of this PR: `rg.zsh` is at 645 lines, and `FAQ.md` was already over 1000 lines at the merge-base.

## Findings

**1. The third branch of the new dispatch is CI scaffolding in the shipped script, and in production it can only produce an error** (`crates/core/flags/complete/rg.zsh:437-445`; detail in `01_zsh-completion-dispatch.md`, Finding 1.1). The standard fix is two-way: when autoloaded as `_rg`, run the function; when sourced, call `compdef _rg rg`. The PR adds a third case, `|| (( ! $+functions[compdef] ))`, only because `ci/test-complete` sources the file in a bare shell that has no completion system. For users, that branch is reached when they source the script before `compinit`. The branch then calls the completion body outside completion, and the user gets `command not found: _arguments` at every shell start (verified, scenario A). The comment "Don't run the completion function when being sourced by itself" contradicts the branch it sits above, and the two issue/PR URLs replace an explanation. The code-judo fix is to make the harness handle its own unusual invocation. Reduce the product dispatch to `if [[ $funcstack[1] == _rg ]]; then _rg "$@"; else compdef _rg rg; fi`, and change `get_comp_args` in `ci/test-complete` to `( compdef() { : }; source $1; _RG_COMPLETE_LIST_ARGS=1 _rg )`. That harness form was run and prints the same 341 spec lines as the current harness does against the head script. The `compdef` probe, the `||`, and the misleading comment all go away. Optionally, test `[[ $zsh_eval_context[-1] == loadautofunc ]]` instead of the function name (verified: it gives `loadautofunc` when autoloaded and `file` when sourced). If a friendly message for the "sourced before compinit" mistake is wanted, add it as an explicit diagnostic branch rather than falling into `_rg`.

**2. The `.zshrc` snippet includes a prompt `$`, so pasting it verbatim breaks shell startup** (`FAQ.md:135`; detail in `02_faq-docs.md`, Finding 2.1). The text says "add the following to your `$HOME/.zshrc` file" and then shows `$ source <(rg --generate complete-zsh)`. Verified: `zsh -c '$ source ...'` fails with `command not found: $`. The `fpath=` block a few lines earlier correctly omits the prompt, so the two `.zshrc` snippets are inconsistent. Remove the `$ `. Keep `$` only for commands typed at the prompt.

**3. The FAQ does not say where either zsh method goes relative to `compinit`** (`FAQ.md:124-139`; detail in `02_faq-docs.md`, Finding 2.2). The `fpath=` line only works if it comes before `compinit`, which scans `$fpath` once. The `source <(...)` line only works if it comes after `compinit`. Otherwise it reaches the failure path described in Finding 1. The `source` method is presented as the easy option for users who don't want to manage `fpath`, and those users are the least likely to know about this ordering. Add one sentence to each method stating where it belongs relative to `compinit`.

**4. The caveat sentence is ungrammatical** (`FAQ.md:138-139`; detail in `02_faq-docs.md`, Finding 2.3). "while this approach is easier to setup, is generally slower" has no subject for its main clause and uses "setup" as a verb. Change it to "while this approach is easier to set up, it is generally slower than the previous method and will add time to loading your shell prompt."

## Proposed remediation sequence

1. Remove the `$ ` from the `.zshrc` `source` snippet (Finding 2). This is a one-character docs fix and the user-facing breakage is certain.
2. Reduce the `rg.zsh` trailer to the two-way autoload/source dispatch, move the "no completion system" handling into `ci/test-complete`, and rewrite the comment so it describes both branches (Finding 1). Run `ci/test-complete` against a built `rg` to confirm the harness still passes.
3. Add the `compinit` ordering sentences for both zsh methods (Finding 3).
4. Fix the caveat sentence's grammar (Finding 4).

## Detail files

- `01_zsh-completion-dispatch.md`: scratch zsh scenarios A–F with commands and results, Finding 1.1, and the worked two-way dispatch plus harness patch.
- `02_faq-docs.md`: Findings 2.1–2.3 on the FAQ text, with the verification commands.

## Verification limits

Cargo could not be run and there was no network, so the generated script was reproduced by substituting the `!ENCODINGS!` placeholder by hand, which is exactly what `zsh.rs` does. The real `ci/test-complete` needs a built `rg` and was not run. Its `get_comp_args` invocation was reproduced exactly. The host ships a vendor `_rg` in `/usr/share/zsh/vendor-completions`, which made the "source before compinit, then compinit" registration scenario inconclusive. It was therefore not used as evidence.

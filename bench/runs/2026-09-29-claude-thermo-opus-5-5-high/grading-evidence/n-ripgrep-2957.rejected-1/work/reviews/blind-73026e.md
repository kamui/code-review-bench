# Review blind-73026e

### Item 1
Location: crates/core/flags/complete/rg.zsh:437-445
Claim: **1. The third branch of the new dispatch is CI scaffolding in the shipped script, and in production it can only produce an error** (`crates/core/flags/complete/rg.zsh:437-445`; detail in `01_zsh-completion-dispatch.md`, Finding 1.1). The standard fix is two-way: when autoloaded as `_rg`, run the function; when sourced, call `compdef _rg rg`. The PR adds a third case, `|| (( ! $+functions[compdef] ))`, only because `ci/test-complete` sources the file in a bare shell that has no completion system. For users, that branch is reached when they source the script before `compinit`. The branch then calls the completion body outside completion, and the user gets `command not found: _arguments` at every shell start (verified, scenario A). The comment "Don't run the completion function when being sourced by itself" contradicts the branch it sits above, and the two issue/PR URLs replace an explanation. The code-judo fix is to make the harness handle its own unusual invocation. Reduce the product dispatch to `if [[ $funcstack[1] == _rg ]]; then _rg "$@"; else compdef _rg rg; fi`, and change `get_comp_args` in `ci/test-complete` to `( compdef() { : }; source $1; _RG_COMPLETE_LIST_ARGS=1 _rg )`. That harness form was run and prints the same 341 spec lines as the current harness does against the head script. The `compdef` probe, the `||`, and the misleading comment all go away. Optionally, test `[[ $zsh_eval_context[-1] == loadautofunc ]]` instead of the function name (verified: it gives `loadautofunc` when autoloaded and `file` when sourced). If a friendly message for the "sourced before compinit" mistake is wanted, add it as an explicit diagnostic branch rather than falling into `_rg`.
Consequence: —
Fix: —

### Item 2
Location: FAQ.md:135
Claim: **2. The `.zshrc` snippet includes a prompt `$`, so pasting it verbatim breaks shell startup** (`FAQ.md:135`; detail in `02_faq-docs.md`, Finding 2.1). The text says "add the following to your `$HOME/.zshrc` file" and then shows `$ source <(rg --generate complete-zsh)`. Verified: `zsh -c '$ source ...'` fails with `command not found: $`. The `fpath=` block a few lines earlier correctly omits the prompt, so the two `.zshrc` snippets are inconsistent. Remove the `$ `. Keep `$` only for commands typed at the prompt.
Consequence: —
Fix: —

### Item 3
Location: FAQ.md:124-139
Claim: **3. The FAQ does not say where either zsh method goes relative to `compinit`** (`FAQ.md:124-139`; detail in `02_faq-docs.md`, Finding 2.2). The `fpath=` line only works if it comes before `compinit`, which scans `$fpath` once. The `source <(...)` line only works if it comes after `compinit`. Otherwise it reaches the failure path described in Finding 1. The `source` method is presented as the easy option for users who don't want to manage `fpath`, and those users are the least likely to know about this ordering. Add one sentence to each method stating where it belongs relative to `compinit`.
Consequence: —
Fix: —

### Item 4
Location: FAQ.md:138-139
Claim: **4. The caveat sentence is ungrammatical** (`FAQ.md:138-139`; detail in `02_faq-docs.md`, Finding 2.3). "while this approach is easier to setup, is generally slower" has no subject for its main clause and uses "setup" as a verb. Change it to "while this approach is easier to set up, it is generally slower than the previous method and will add time to loading your shell prompt."
Consequence: —
Fix: —

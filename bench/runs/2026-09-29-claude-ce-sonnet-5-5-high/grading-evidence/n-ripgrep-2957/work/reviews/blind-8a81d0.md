# Review blind-8a81d0

### Item 1
Location: crates/core/flags/complete/rg.zsh:446
Claim: Sourcing before compinit still calls _rg and errors on every shell start
Consequence: The FAQ tells users to put `source <(rg --generate complete-zsh)` in `~/.zshrc` but never says it must come after `compinit`. If it comes first, `compdef` is undefined, the fallback branch runs `_rg` outside completion, and each new shell prints `_rg:341: command not found: _arguments`. This is the same class of startup error issue #2956 reported.
Fix: Keep the `(( ! $+functions[compdef] ))` fallback out of the interactive path: gate the direct `_rg "$@"` call on the test-harness case (for example `_RG_COMPLETE_LIST_ARGS`) or make it a silent no-op, and have the FAQ say the `source` line must come after `compinit`.

### Item 2
Location: FAQ.md:128
Claim: FAQ zsh instructions: ordering and wording gaps
Consequence: The sentence is ungrammatical (missing subject, 'setup' used as a verb). The `$ ` prompt inside a `.zshrc` snippet is a copy-paste hazard. The `fpath` line only takes effect if it runs before `compinit`, and the `source` line only works if it runs after `compinit`. Neither ordering is stated.
Fix: Reword to 'while this approach is easier to set up, it is generally slower than the previous method and adds time to shell startup'. Drop the leading `$` from the `.zshrc` snippet, as the adjacent fpath snippet does. Note that both the `fpath=` line and the `source` line must come before/after `compinit` respectively.

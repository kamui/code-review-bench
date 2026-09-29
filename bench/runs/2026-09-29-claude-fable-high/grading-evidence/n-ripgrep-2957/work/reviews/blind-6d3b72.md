# Review blind-6d3b72

### Item 1
Location: crates/core/flags/complete/rg.zsh:441
Claim: Sourcing the script before `compinit` has run still executes `_rg "$@"` outside a completion context, because the `(( ! $+functions[compdef] ))` fallback treats "compdef not defined" as "run the completion function".
Consequence: A user puts `source <(rg --generate complete-zsh)` in .zshrc above their `compinit` call, or uses no compinit at all. Reproduced with `zsh -f -c 'source _rg'`: prints `_rg:341: command not found: _arguments`, rc=1, and no completion is registered for rg. This is the same class of failure issue #2956 reported, and the FAQ does not say compinit must come first.
Fix: —

### Item 2
Location: crates/core/flags/complete/rg.zsh:441
Claim: The autoload check hard-codes the function name `_rg`, so a completion file installed in fpath under any other name takes the `compdef` branch on its first invocation instead of completing.
Consequence: The file is saved as `$fpath/_ripgrep` (the `#compdef rg` line still registers it). On the first Tab, funcstack[1] is `_ripgrep` and compdef exists, so the script only runs `compdef _rg rg` and returns 0 with no matches. Reproduced: `_comps[rg]` goes from `_ripgrep` to `_rg` and the call returns 0 without reaching `_arguments`. Before this change the first Tab completed; now it silently yields nothing until the second Tab.
Fix: —

### Item 3
Location: FAQ.md:135
Claim: The snippet users are told to add to `$HOME/.zshrc` includes a literal `$ ` shell-prompt prefix.
Consequence: A user copies the line `$ source <(rg --generate complete-zsh)` verbatim into .zshrc as instructed. zsh then reports `command not found: $` on every shell startup and completions are never loaded. The fpath snippet at line 128, also meant for .zshrc, correctly has no prompt prefix.
Fix: —

### Item 4
Location: crates/core/flags/complete/rg.zsh:441
Claim: `$funcstack[1] == _rg` also matches when the file is sourced by the bare relative name `_rg`, since funcstack records sourced file names as given, so the sourced case is misdetected as autoload.
Consequence: After compinit, a user runs `cd ~/.zsh-complete && source _rg`. Reproduced: prints `_arguments:comparguments:325: can only be called from completion function`, rc=1, and compdef is never called. `source ./_rg` and a full path work, so behaviour depends on how the path is spelled.
Fix: —

### Item 5
Location: crates/core/flags/complete/rg.zsh:441
Claim: The `ci/test-complete` spec-dump path now depends implicitly on `compdef` being undefined, rather than on the explicit `_RG_COMPLETE_LIST_ARGS` test variable.
Consequence: `ci/test-complete` is run where `compdef` is defined, e.g. a developer's `.zshenv` runs compinit. `source $1` happens inside `get_comp_args`, so funcstack[1] is the file path and the script calls `compdef _rg rg` without printing specs. Reproduced: 0 lines with compdef defined versus 341 without, so the test fails with `Failed to get comp_args`. Checking `_RG_COMPLETE_LIST_ARGS` in the condition would fix this at the right level.
Fix: —

### Item 6
Location: FAQ.md:128
Claim: The new fpath instructions omit that the assignment must come before `compinit` (and that compinit must be run), so following them as written can still leave completions inactive.
Consequence: A user appends `fpath=($HOME/.zsh-complete $fpath)` to the end of .zshrc, after an existing `compinit` call or framework init. compinit has already scanned fpath, and the cached `~/.zcompdump` has no `_rg` entry, so `rg <TAB>` falls back to file completion. The same ordering requirement applies in reverse to the `source` method, which must come after compinit, and neither is stated.
Fix: —

### Item 7
Location: crates/core/flags/complete/rg.zsh:444
Claim: The newly supported `source` load path parses the function bodies with the user's aliases expanded, whereas the fpath path uses `autoload -U`, which suppresses alias expansion.
Consequence: In an interactive shell with an alias on a word the script uses in command position, sourcing bakes the alias into `_rg`. Reproduced with `alias local='typeset -g'`: `whence -f _rg` shows `typeset -g curcontext=... ret=1`, so completion state leaks into globals. The same alias left the `autoload -U` version untouched. The script has no `emulate -L zsh` or `unalias` guard for this path.
Fix: —

### Item 8
Location: FAQ.md:138
Claim: The caveat sentence is ungrammatical: it is missing its subject ("it") and uses the noun "setup" where the verb "set up" is meant.
Consequence: The text reads "while this approach is easier to setup, is generally slower than the previous method". It should read "while this approach is easier to set up, it is generally slower...". This is a documentation quality cost in user-facing text.
Fix: —

### Item 9
Location: crates/core/flags/doc/template.rg.1:380
Claim: The man page's SHELL COMPLETION section and CHANGELOG.md were not updated for the new behaviour, leaving the documentation inconsistent with the FAQ.
Consequence: The man page still documents only "move _rg to one of your $fpath directories". The CHANGELOG "TBD" section has no entry for #2956/#2957, so the feature may be left out of release notes and man-page readers never learn of the `source <(rg --generate complete-zsh)` method.
Fix: —

### Item 10
Location: FAQ.md:135
Claim: The recommended .zshrc line is unguarded and spawns `rg` on every shell startup.
Consequence: The same dotfiles are used on a machine where rg is not installed or not yet on PATH when .zshrc runs. Every new shell prints `command not found: rg` and sources an empty stream. A guard such as `(( $+commands[rg] )) && source <(...)`, as in the issue's own examples, avoids this.
Fix: —

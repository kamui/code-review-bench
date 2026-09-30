# Review blind-a4c0fd

### Item 1
Location: crates/core/flags/complete/rg.zsh:441
Claim: Sourcing before compinit runs _rg at top level and errors
Consequence: The FAQ tells users to add `source <(rg --generate complete-zsh)` to .zshrc but does not say where. If that line runs before compinit, or when compinit is never run, compdef is undefined. The guard then calls `_rg "$@"` at top level, and every new shell prints `_rg:341: command not found: _arguments`. No completion is registered. This is the same kind of startup error that issue #2956 reported. Keeping the direct-call fallback for the test harness only, and saying 'after compinit' in the FAQ, avoids it.
Fix: Limit the no-compdef fallback to the test harness, e.g. `if [[ $funcstack[1] == _rg || $_RG_COMPLETE_LIST_ARGS == (1|t*|y*) ]]; then _rg "$@"; elif (( $+functions[compdef] )); then compdef _rg rg; fi`. Optionally print a hint in a final else branch. Also say in FAQ.md that the `source` line must come after `compinit`. ci/test-complete sets _RG_COMPLETE_LIST_ARGS=1, so it keeps working.

### Item 2
Location: FAQ.md:135
Claim: FAQ .zshrc snippet includes a literal `$ ` prompt
Consequence: The text says to add this block to .zshrc. A user who pastes it as-is gets `zsh: command not found: $` on every shell start, and completions never load. The fpath snippet three lines above correctly leaves out the prompt, so the two .zshrc snippets are inconsistent.
Fix: Change the line to `source <(rg --generate complete-zsh)` with no `$ `, matching the prompt-free `fpath=(...)` .zshrc snippet just above. While there, fix 'easier to setup, is generally slower' -> 'easier to set up, it is generally slower'.

### Item 3
Location: crates/core/flags/complete/rg.zsh:441
Claim: Autoload under a name other than _rg misses the first TAB
Consequence: The parent commit completed on the first TAB whatever the file was named. With this change, a copy installed in fpath under another name (e.g. `_ripgrep`) runs `compdef _rg rg` on its first call and offers no completions. Later TABs work because the mapping has moved to `_rg`. All official artifacts (release.yml, ci/build-and-publish-m2, the FAQ) install the file as `_rg`, and the funcstack check is a common idiom, so this is a narrow edge left to the maintainer.
Fix: Decide based on how the file is being run, not on the function's name: `[[ $zsh_eval_context[-1] == loadautofunc ]]` is true when the file is autoloaded and false when it is sourced.

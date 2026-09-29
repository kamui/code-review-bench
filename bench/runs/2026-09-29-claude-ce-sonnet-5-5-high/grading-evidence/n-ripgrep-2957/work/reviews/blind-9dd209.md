# Review blind-9dd209

### Item 1
Location: crates/core/flags/complete/rg.zsh:441
Claim: Sourcing before compinit still errors with `_arguments` command not found
Consequence: The `|| (( ! $+functions[compdef] ))` arm runs `_rg "$@"` at top level whenever compdef is not yet defined. That is the normal state for `source <(rg --generate complete-zsh)` placed in ~/.zshrc before compinit runs (oh-my-zsh and many setups call compinit late). Sourcing the real script that way in zsh -f prints `_rg:341: command not found: _arguments` and returns 1, the same class of startup error issue #2956 reported. The FAQ tells users to add the line to $HOME/.zshrc and says nothing about ordering after compinit.
Fix: When compdef is absent and the script is not running as the autoloaded _rg (funcstack[1] != _rg), define _rg without invoking it (or defer registration until compinit). Keep the `_rg "$@"` call only for the autoload case and for ci/test-complete, which needs a non-compdef way to opt in. Alternatively, document in FAQ.md that the source line must come after compinit.

### Item 2
Location: FAQ.md:139
Claim: FAQ sentence is ungrammatical and the source snippet has a prompt prefix
Consequence: The sentence is missing a subject (`it is`) and uses the noun `setup` as a verb. The snippet is labelled as content to add to $HOME/.zshrc, but it begins with `$ `, so a copy-paste puts a literal `$ source ...` in the file, which fails to parse as intended.
Fix: Reword to: "Note though that while this approach is easier to set up, it is generally slower than the previous method and will add more time to loading your shell prompt." Drop the leading `$ ` from the .zshrc snippet, since it is file content rather than a shell command.

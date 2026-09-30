# Review blind-e360d3

### Item 1
Location: crates/core/flags/complete/rg.zsh:441
Claim: FAQ never says where the zsh lines go relative to compinit; sourcing before compinit still errors at every shell startup
Consequence: If compdef is not defined, the new guard falls through to calling `_rg "$@"` outside a completion context. That means `source <(rg --generate complete-zsh)` placed before compinit prints an error on every shell start and registers no completion. It is the same class of failure issue #2956 reports, just triggered by line order. The new fpath instruction has the mirror-image problem: if `fpath=(...)` is added after compinit (common when users append to an oh-my-zsh .zshrc), completion is simply absent and nothing reports an error. The FAQ gives neither ordering constraint.
Fix: In FAQ.md, say that `source <(rg --generate complete-zsh)` must come after `autoload -U compinit && compinit` (or after the framework line that runs compinit, e.g. oh-my-zsh), and that `fpath=($HOME/.zsh-complete $fpath)` must come before compinit. Optional hardening in rg.zsh (assumes ci/test-complete is the only caller that intentionally sources without compsys): key the direct-call branch on `$funcstack[1] == _rg` or `$_RG_COMPLETE_LIST_ARGS`, and when compdef is missing print a one-line 'load compinit first' message instead of calling _rg.

### Item 2
Location: FAQ.md:135
Claim: FAQ .zshrc snippet for the source method keeps the `$ ` prompt, so pasting it runs a `$` command
Consequence: The block is introduced as text to add to $HOME/.zshrc, not as a command to type at a prompt. Pasting it verbatim gives `zsh: command not found: $` (exit 127) on every shell start, and completion is not registered by that line.
Fix: Change FAQ.md:135 to `source <(rg --generate complete-zsh)` (no leading `$ `), matching the prompt-less fpath snippet at FAQ.md:128.

### Item 3
Location: FAQ.md:138
Claim: FAQ caveat sentence is missing its subject ("..., is generally slower")
Consequence: This is a minor readability problem in the caveat the maintainer added to explain the startup-time cost.
Fix: Reword to: "Note though that while this approach is easier to set up, it is generally slower than the previous method and will add more time to loading your shell prompt."

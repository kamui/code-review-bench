## Finding

**P2 — Sourcing before zsh completion initialization still executes `_rg` at top level** — `crates/core/flags/complete/rg.zsh:441–444`

When `compdef` is not defined, this condition takes the `_rg "$@"` branch even when the file is being sourced directly. In a clean zsh without completion initialization (`zsh -f -c 'source crates/core/flags/complete/rg.zsh'`), that invokes `_arguments` outside the completion system and fails with `_rg:341: command not found: _arguments` (exit status 1). Thus the newly documented `source <(rg --generate complete-zsh)` setup still emits an error if placed before `compinit` or run in a shell where the completion system has not been initialized; users must know to initialize completion first, but the FAQ does not state that ordering requirement.

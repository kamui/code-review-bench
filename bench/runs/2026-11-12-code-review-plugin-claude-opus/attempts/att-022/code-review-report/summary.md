# Code review — BurntSushi/ripgrep#2957

"feat(completion): support sourcing zsh completion dynamically" (`main...review-head`, 79cbe89..855bfa6)

Step 1: the PR is not a draft and needs review. Step 2: there are no CLAUDE.md files in scope. Step 6 kept 1 issue.

## Issues

### 1. The `.zshrc` snippet in the FAQ keeps a literal `$ ` prompt, so following it fails

- **Reason flagged:** bug (an incorrect documentation instruction that fails when followed)
- **File / lines:** `FAQ.md` lines 131–136 (the bad line is 135). Link: https://github.com/BurntSushi/ripgrep/blob/855bfa6cdae4f4fe8762f892fc4957635397083e/FAQ.md#L131-L136
- **Why it fails:** The prose tells users to "add the following to your `$HOME/.zshrc` file". The block that follows contains `$ source <(rg --generate complete-zsh)`. If a user pastes that line into `.zshrc`, zsh treats `$` as the command name. Each new shell then prints `command not found: $`, and `source` never runs, so the completions this PR enables never load. `rg --generate complete-zsh` still runs, but its output is thrown away. A scratch zsh script reproduced this. The `fpath=(...)` snippet just above, which is also meant for `.zshrc`, has no `$` prefix. The PR introduced these lines. Fix: drop the leading `$ ` so the line reads `source <(rg --generate complete-zsh)`.

The change to `crates/core/flags/complete/rg.zsh` (the `$funcstack[1] == _rg` / `$+functions[compdef]` guard) was checked and found correct.

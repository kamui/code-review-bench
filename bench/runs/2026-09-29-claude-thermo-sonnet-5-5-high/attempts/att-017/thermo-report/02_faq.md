# Detail 02: `FAQ.md` zsh section

Scope: `FAQ.md:94-139`. The diff fixes a typo ("completes" to "completions"), relabels the existing recipe as "recommended", adds an `fpath` line, and adds the `source <(...)` alternative with a speed caveat.

## Finding 3: `compinit` ordering is not documented

Both recipes depend on `compinit` ordering. The `fpath` recipe needs the directory on `fpath` before `compinit` runs. The `source` recipe needs `compinit` to have run first so that `compdef` exists (measured in scenario A of detail 01: sourcing first gives `command not found: _arguments`). The FAQ states neither. The new text says "add the following to your `$HOME/.zshrc`" without any ordering. Suggested remedy: one sentence per recipe, for example "This must come after `compinit`" for the `source` recipe and "before `compinit`" for the `fpath` recipe.

Verification status: ordering behavior confirmed by execution; the documentation gap is by reading.

## Finding 4: wording and snippet inconsistency

At `FAQ.md:138-139`: "Note though that while this approach is easier to setup, is generally slower than the previous method, and will add more time to loading your shell prompt." The clause "is generally slower" has no subject, and "setup" is a noun where the verb "set up" is needed. The last clause also repeats the previous one. A cleaner version: "This approach is easier to set up, but it is slower than the previous method because it runs `rg` and re-parses the completion script every time your shell starts."

At `FAQ.md:135`, the snippet is `$ source <(rg --generate complete-zsh)` inside a `zsh` fenced block, but the instruction is to put it in `.zshrc`. The `fpath=(...)` snippet directly above has no prompt marker. A pasted `$ ` at the start of a `.zshrc` line is a command-not-found error. Remove the `$`.

Verification status: by reading.

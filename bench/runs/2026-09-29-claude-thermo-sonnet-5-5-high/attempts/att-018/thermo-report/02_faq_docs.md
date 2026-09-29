# Detail 02: `FAQ.md` zsh section

## Change

Lines 116-140: the zsh block now says "the recommended approach", adds an `fpath=` step (fixing the pre-existing "these instructions don't do anything" gap), and adds a `source <(rg --generate complete-zsh)` alternative with a speed caveat.

## Finding 2: missing ordering requirement (`FAQ.md:132-139`)

The recommended one-liner works only when `compdef` already exists (verified by running a scratch script under zsh: `source` before `compinit` gives `_rg: command not found: _arguments`, exit 127). The FAQ tells the reader to add it to `.zshrc` and says nothing about placing it after `compinit`. Either fix the script (Detail 01) or add the sentence "this must come after `compinit`."

## Finding 3: snippet and sentence defects (`FAQ.md:135`, `:138-139`)

- Line 135 is inside a ```` ```zsh ```` fence that instructs the reader to add it to `.zshrc`, but it starts with `$ `. Pasted into `.zshrc` it fails with `$: command not found`. The `fpath=` snippet directly above correctly omits the prompt. Drop the `$ `.
- Lines 138-139: "Note though that while this approach is easier to setup, is generally slower than the previous method, and will add more time to loading your shell prompt." The clause structure is broken (a stray comma and a missing subject), and "setup" is used as a verb. Suggested: "Note that while this approach is easier to set up, it is generally slower than the previous method and will add time to your shell startup."

# 02 — FAQ zsh completion instructions (`FAQ.md`)

Scope: the rewritten zsh block at `FAQ.md:116-139`, plus the `completes`→`completions` typo fix at `FAQ.md:97`. `FAQ.md` was already over 1000 lines before this PR (1046 at the merge-base, 1063 at head). It is a prose FAQ, so the file-size rule does not apply and no decomposition finding is raised.

## Measurements and how they were produced

- `grep -n` on the head `FAQ.md` placed the new text at lines 116 (heading), 128 (`fpath=` line), 135 (`$ source <(...)` line) and 138-139 (caveat sentence).
- `zsh -f -c '$ source <(echo echo hi)'` printed `zsh:1: command not found: $`. This confirms that a line copied verbatim from the new snippet into `.zshrc` fails.
- The ordering behaviour behind Finding 2.2 comes from scenario A in `01_zsh-completion-dispatch.md`: sourcing the script without `compinit` loaded prints `command not found: _arguments`.

Verification status: 2.1 was confirmed by execution. 2.2 was confirmed by execution for the sourcing path. The `fpath`-after-`compinit` half of 2.2 is standard zsh behaviour (`compinit` scans `$fpath` once, when it runs); it was not run separately here because the host's vendor `_rg` would have hidden the result. 2.3 is a wording issue and was checked by reading.

## Finding 2.1 — The `.zshrc` snippet includes a shell-prompt `$`, so copying it verbatim breaks the user's shell startup

At `FAQ.md:131-136`, the text says "add the following to your `$HOME/.zshrc` file" and then shows:

```zsh
$ source <(rg --generate complete-zsh)
```

The `$ ` is prompt notation, which is fine for the "run these commands" block at lines 118-122. This block, though, is described as file content, and the `fpath=(...)` block a few lines earlier (line 128) correctly leaves the prompt out. A user who pastes line 135 into `.zshrc` gets `command not found: $` on every new shell, and completions never load. This undoes the convenience the section is trying to offer. The fix is to delete the `$ ` so the block reads `source <(rg --generate complete-zsh)`. The FAQ should also use one convention consistently: `$` for commands typed at the prompt, and no `$` for lines that go into a file.

## Finding 2.2 — Neither zsh method states its ordering requirement relative to `compinit`, and both fail silently or noisily when it is wrong

Both methods depend on `compinit`, and the FAQ mentions neither dependency.

- The `fpath=($HOME/.zsh-complete $fpath)` line must run before `compinit`, because `compinit` scans `$fpath` only once. If it runs after `compinit`, nothing happens and completion quietly stays missing. On some setups the user also has to clear a stale `~/.zcompdump`.
- The `source <(rg --generate complete-zsh)` line must run after `compinit`. If it runs before, the dispatch at `rg.zsh:441` takes the `! $+functions[compdef]` branch and prints `command not found: _arguments` (Finding 1.1). The FAQ offers this method as the easy option for users who don't want to manage `fpath`, and those are the users least likely to know that `compinit` ordering matters.

Remedy: add one sentence to each method, for example "put this line before your `compinit` call" for `fpath` and "put this line after your `compinit` call (or after your framework, such as oh-my-zsh, has initialized completion)" for `source`. This documentation fix goes with the code-judo proposal in `01_zsh-completion-dispatch.md`. After that proposal, the shipped script has only two branches, and the FAQ says which one the user's setup will reach.

## Finding 2.3 — Caveat sentence is ungrammatical

`FAQ.md:138-139`: "Note though that while this approach is easier to setup, is generally slower than the previous method, …". The main clause has no subject ("…, it is generally slower…"), and "setup" is used as a verb where it should be "set up". Suggested wording: "Note that while this approach is easier to set up, it is generally slower than the previous method and will add time to loading your shell prompt." This is minor, but this is the sentence the maintainer added specifically as the caveat, so it should read cleanly.

Minor, not raised as a finding: the zsh fences now use ` ```zsh ` while the bash, fish and PowerShell fences next to them are untagged. This is harmless and can be ignored.

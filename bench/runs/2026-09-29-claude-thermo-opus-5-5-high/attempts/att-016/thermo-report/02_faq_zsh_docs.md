# 02 — FAQ zsh completion instructions (`FAQ.md:94-139`)

Scope: the rewritten zsh section of the "shell auto-completion" FAQ entry, read against the behaviour verified in `01_zsh_completion_dispatch.md`.

## Size

`FAQ.md` was already 1046 lines at the merge-base (`git show main:FAQ.md | wc -l`). The PR takes it to 1063. It is a flat FAQ document, not code, and it did not cross the threshold because of this PR, so there is no decomposition concern.

## Finding 02-A — the `.zshrc` snippet is not copy-pasteable, and neither method states its ordering with `compinit`

At `FAQ.md:132-136` the text says "add the following to your `$HOME/.zshrc` file" and then shows:

```zsh
$ source <(rg --generate complete-zsh)
```

The `$ ` prompt marker belongs to the interactive-command blocks above it (`FAQ.md:118-122`), not to file contents. The `fpath=(...)` snippet four lines earlier (`FAQ.md:128`), which is also "add to your `.zshrc`", correctly leaves it out. A user who pastes the `source` line as written gets `command not found: $` on every new shell. That is two conventions inside one short section.

More importantly, neither method says where the line has to go relative to `compinit`, and each has the opposite constraint. The `fpath=(...)` line must come **before** `compinit`, because compinit scans `$fpath` when it runs. The `source <(...)` line must come **after** `compinit`, because `01-A` shows that sourcing before compinit hits the PR's `! compdef` branch and prints the same `_arguments` error as issue #2956. The docs and the code fail together here. The FAQ leaves the ordering implicit, and the script turns the wrong ordering into the original bug instead of a clear message.

Remedy: drop the `$ ` from the `.zshrc` snippet. Add a sentence to each method, "put this before the `compinit` call in your `.zshrc`" and "put this after the `compinit` call". Fixing 01-A makes the misordered case fail clearly, so the doc sentence becomes a convenience instead of the only protection.

Status: **verified by reading** (prompt marker, missing ordering). The ordering consequence is **verified by execution** in 01 (`t1.zsh`, `t2.zsh`).

## Finding 02-B — broken sentence in the trade-off caveat

`FAQ.md:138-139`: "Note though that while this approach is easier to setup, is generally slower than the previous method, and will add more time to loading your shell prompt." The main clause has no subject ("…easier to setup, is generally slower"), and "setup" is the noun where the verb "set up" is meant. This sentence is the maintainer's deliberate caveat about the cost of the generate-and-source approach, so it should read cleanly. A fix: "Note that while this approach is easier to set up, it is generally slower than the previous method and will add time to loading your shell prompt."

Status: **verified by reading**. Low severity.

## Not flagged

- Whether the source method should be documented at all was settled by the maintainer before the merge, so this review does not reopen it.
- Repeating the literal `$HOME/.zsh-complete` in the prose and the `fpath` line, instead of reusing `$dir`, is deliberate. The `fpath` line lives in a different file, where `$dir` is not set.

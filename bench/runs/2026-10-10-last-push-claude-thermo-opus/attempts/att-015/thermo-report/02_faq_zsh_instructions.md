# 02 — FAQ zsh completion instructions (`FAQ.md`)

Carries the evidence for Finding 3 in `summary.md`.

## What changed

`FAQ.md:116-139` at the head:

````
For **zsh**, the recommended approach is:

```zsh
$ dir="$HOME/.zsh-complete"
$ mkdir -p "$dir"
$ rg --generate complete-zsh > "$dir/_rg"
```

And then add `$HOME/.zsh-complete` to your `fpath` in, e.g., your
`$HOME/.zshrc` file:

```zsh
fpath=($HOME/.zsh-complete $fpath)
```

Or if you'd prefer to load and generate completions at the same time, you can
add the following to your `$HOME/.zshrc` file:

```zsh
$ source <(rg --generate complete-zsh)
```

Note though that while this approach is easier to setup, is generally slower
than the previous method, and will add more time to loading your shell prompt.
````

## Evidence

**The `$ ` prefix on line 135.** The first block is a terminal session, so the prompt prefix is right there. The `fpath=` block on line 128 is file content and has no prefix. The `source` block on line 135 is also introduced as file content ("add the following to your `$HOME/.zshrc` file") but carries the prefix. A scratch script containing the line as printed:

```
$ zsh -f t/dollar.zsh
t/dollar.zsh:1: command not found: $
rc=127
```

Nothing is sourced. A reader who copies the block gets an error on every shell start and no completions.

**Ordering relative to `compinit`.** The two methods need opposite placement:

- `fpath=(...)` must run before `compinit`, because `compinit` scans `fpath` once.
- `source <(...)` must run after `compinit`, because it calls `compdef`.

Neither is stated. For the `source` method the wrong order was reproduced against the head script: startup prints `_rg:341: command not found: _arguments` and TAB completes nothing (see the matrix in `01_zsh_completion_dispatch.md`). The feature this PR adds is aimed at people who want a one-line setup, and those are the readers least likely to work out the ordering from that message.

**Wording on line 138.** "while this approach is easier to setup, is generally slower" is missing its subject, and "setup" is the noun form.

## Proposed wording

````
And then add `$HOME/.zsh-complete` to your `fpath` in, e.g., your
`$HOME/.zshrc` file, before the line that runs `compinit`:

```zsh
fpath=($HOME/.zsh-complete $fpath)
```

Or if you'd prefer to load and generate completions at the same time, you can
add the following to your `$HOME/.zshrc` file, after the line that runs
`compinit`:

```zsh
source <(rg --generate complete-zsh)
```

Note though that while this approach is easier to set up, it is generally
slower than the previous method, and will add more time to loading your shell
prompt.
````

## Not verified

The startup-time claim in the FAQ ("generally slower") was not measured, since no `rg` binary could be built here.

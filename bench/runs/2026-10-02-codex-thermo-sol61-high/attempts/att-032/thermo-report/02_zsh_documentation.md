# Zsh installation documentation

## Scope and measurements

This subsystem covers the FAQ changes at head lines 95–139. `git diff --numstat main...review-head` reports +20/−3 for `FAQ.md`. The file grows from 1,046 to 1,063 lines, so it is not pushed across the skill's 1,000-line boundary. The added material stays in the existing shell-completion question; extraction into another document is not justified by this small, cohesive addition.

The spelling correction and the startup-cost caveat need no remediation. No startup benchmark was run, and no particular time estimate is asserted. The two findings below concern executable configuration and the completion system's initialization boundary, rather than prose preferences.

## Finding: state the completion initialization order for both zsh methods

`FAQ.md:124–135` supplies a new `fpath` configuration snippet and a new dynamically sourced alternative. Neither mentions `compinit` or the position of those snippets relative to an existing framework's completion initialization. The audience includes users for whom package-manager installation has not supplied usable completion, as stated at lines 95–98. A working packaged `_rg` therefore cannot be assumed.

`compinit` is the discovery boundary for the saved-file route. Adding a directory afterward does not itself create `_comps[rg]`. Direct source takes the opposite route: the new shell branch calls `compdef`, which `compinit` defines. Before that initialization, the missing-function fallback executes `_rg`, and `_arguments` is unavailable in a fresh zsh. Even running initialization afterward does not register the sourced function when there is no `_rg` file in `fpath` for it to discover. Thus merely naming `.zshrc` as the destination does not supply a complete installation contract.

This is a newly documented direct-source route that lacks a prerequisite, and newly added placement guidance for the saved-file route that omits its discovery order. The older instructions were incomplete too; the finding does not attribute their preexisting lack of initialization to this PR. It asks the new configuration guidance to express the dependency it relies on.

### Reproduction and evidence

`../thermo-checks/order.zsh` starts from `emulate zsh` in `zsh -f`, restricts `fpath` to the installed framework directories `/usr/share/zsh/functions/Completion` and `/usr/share/zsh/functions/Completion/Base`, and uses a nonexistent scratch dump path. Neither framework directory contains a packaged `_rg`. Each case runs in a subshell to isolate functions and mappings. The four results saved in `order-results.txt` are:

```text
CASE: source before compinit, no packaged _rg
_rg:341: command not found: _arguments
source-status=1
mapping=missing function=1
CASE: source after compinit, no packaged _rg
source-status=0 mapping=_rg
CASE: add fpath after compinit, no packaged _rg
mapping=missing function=0
CASE: add fpath before compinit, no packaged _rg
mapping=_rg function=1
```

The generated `_rg` file exists in the scratch work directory for both saved-file cases; the only difference is whether that directory is added before or after discovery. In the failing source case `_rg` exists as a function, but its existence is not registration. The mapping remains absent after `compinit`, which is the meaningful completion failure beyond the startup error.

This reproduction uses the equivalent embedded template rather than a built ripgrep command. The generator's unchanged encoding replacement was inspected in `zsh.rs`; the source helper only emits the generated template. Real Tab completion after the correct initialization order was independently checked, as described in `01_zsh_completion.md`.

### Actionable remediation

Tell readers to put the saved-file `fpath` addition before their existing `compinit` call or framework initialization, and to put dynamic sourcing after completion initialization. Provide `autoload -Uz compinit` followed by `compinit` for users without existing initialization. Explain placement relative to frameworks instead of asking users to initialize twice. This repairs the boundary without increasing production dispatch complexity.

## Finding: remove the prompt marker from the startup-file snippet

`FAQ.md:131–135` says to add the following to `$HOME/.zshrc`, then supplies `$ source <(rg --generate complete-zsh)`. Here the `$` is executable input, not a shell prompt. The preceding startup-file snippet at line 128 correctly omits a prompt marker, so this alternate snippet has an inconsistent copy contract. Unlike the earlier interactive generation examples, this is explicitly file content.

### Reproduction and evidence

The literal-line case in `../thermo-checks/check.zsh` initializes completion and then executes precisely:

```zsh
$ source <(rg --generate complete-zsh)
```

It produces `command not found: $` and returns status 127. This isolates the prompt-marker defect from the missing-initialization defect. The process substitution can produce completion text, but a nonexistent `$` command does not invoke `source` on it. In the corrected direct-source case, deleting the marker returns status 0 and registers `_comps[rg]=_rg`; the real Tab check then completes `complete-zs` to `complete-zsh`.

### Actionable remediation

Change the startup-file line to `source <(rg --generate complete-zsh)`. Retain prompt markers only in examples that represent commands typed at a displayed prompt. No helper, feature change, or shell refactor is needed.

## Worked code-judo proposal

The FAQ can carry the shared lifecycle rule once, then give copyable snippets with the method-specific placement. This eliminates the unstated initialization mode rather than adding fallback heuristics to the generated shell script. A concrete replacement for the zsh subsection is:

> Zsh completion uses `compinit`. If your `.zshrc` or shell framework already initializes completion, use that existing initialization. Otherwise, initialize it with `autoload -Uz compinit` followed by `compinit` as shown below.
>
> The recommended approach is to generate a completion file once:

```zsh
$ dir="$HOME/.zsh-complete"
$ mkdir -p "$dir"
$ rg --generate complete-zsh > "$dir/_rg"
```

> Add the directory to `fpath` before completion initialization in `.zshrc`. For a setup without existing initialization:

```zsh
fpath=("$HOME/.zsh-complete" $fpath)
autoload -Uz compinit
compinit
```

> Alternatively, generate and source completion on startup. Put the source line after your existing completion initialization. For a setup without existing initialization:

```zsh
autoload -Uz compinit
compinit
source <(rg --generate complete-zsh)
```

> Generating completion on every startup adds work to opening the shell; loading the saved completion file avoids running ripgrep each time.

These are alternative configurations, not instructions to install both or to duplicate a framework's initialization. Quoting the new directory in the worked `fpath` snippet is conventional clarity, not a separate finding: default zsh behavior does not establish a whitespace bug in the submitted array assignment.

The source example intentionally preserves the supported generation command. The remedy is documentation of the existing lifecycle, not a new automatic `compinit` call from the generated script. Automatically initializing the global completion system there would change ownership, affect user configuration, and add coupling for a local completion function.

## Verification status and next checks

Both failures and their ordering corrections were verified locally on zsh 5.8.1, offline and outside the clone. The literal prompt-marker failure was tested after valid initialization. The lifecycle failures were tested without a packaged `_rg` or user dump. Correct dynamic loading and saved-file autoloading passed real interactive Tab checks. The full `ci/test-complete` script and Cargo commands were unavailable and not attempted.

After editing documentation, run the focused ordering harness and try copying the new startup-file snippets into a clean zsh fixture. Preserve the current isolated argument-list behavior and repeat cold autoload and direct-source Tab completion. The evidence does not justify expanding tests into unrelated ripgrep functionality.

# FAQ setup contract

## Scope and measurements

The changed completion section in `FAQ.md:94–139` was read with its surrounding man-page and PowerShell examples. The patch corrects a typo, adds the missing directory-to-`fpath` step, describes dynamic sourcing, and explains its startup overhead. `git diff --numstat main...review-head` reports +20/−3. `git show main:FAQ.md | wc -l` reports 1,046 lines, and `wc -l FAQ.md` reports 1,063. The file was already over 1,000 lines; this localized change does not create a threshold crossing or justify splitting unrelated FAQ material.

The prose keeps feature documentation in its existing section. The actionable problems concern the executable setup boundary: the text does not state the completion lifecycle required by either new snippet, and the startup-file snippet contains an interactive prompt character.

## Finding: State the completion initialization order for both methods

`FAQ.md:131–135` tells the reader to add dynamic sourcing to `.zshrc` without requiring an initialized completion system. `rg.zsh:441–444` registers only when `compdef` exists; without it, the fallback executes `_rg`, which reaches `_arguments` outside a configured completion system. A plain zsh configuration following the new instructions therefore still fails. This is a new documentation path promising a setup method without its prerequisite; it is not a regression in the standalone shell script's previously unsupported initialization behavior.

In the `direct_uninitialized` head reproduction, `zsh -f` sources the generated fixture through process substitution with no `compinit` call. Its logged result is `source_status=1 compdef=0 defined=1`, and stderr is `_rg:341: command not found: _arguments`. Having a function definition is insufficient: there is no registration system to associate it with `rg`. The corresponding initialized head reproduction removes any installed `_rg`, calls `compinit`, then sources the fixture successfully with `_comps[rg]=_rg` and status 0.

`FAQ.md:124–128` introduces the complementary file-based `fpath` setup without stating that discovery must occur afterward. The `fpath_after_compinit` reproduction initializes completion, clears any existing `rg` mapping to simulate the uninstalled completion, then adds the fixture directory. `_comps[rg]` stays missing. The initialized-autoload and interactive checks instead add the fixture directory before `compinit`, and registration and Tab completion both succeed. An already initialized shell needs an explicit new registration or another discovery step; adding a directory alone does not create that mapping.

The actionable remedy is to document ordering around the user's existing completion initialization. Explain that a framework may already call `compinit`, put the file-based directory before that call, and put dynamic sourcing after it. For users with a plain configuration, show the initialization explicitly. Do not insert `compinit` into the generated script: that would change its shell-state boundary and would complicate the standalone enumerator.

## Finding: Remove the prompt marker from the startup-file snippet

`FAQ.md:135` contains `$ source <(rg --generate complete-zsh)` directly beneath the instruction to add it to `.zshrc`. Unlike the earlier generation commands shown as interactive terminal input, this is a startup-file code block. The neighboring `fpath` startup snippet correctly omits a prompt. The literal dollar sign names the command zsh tries to execute, so the shell never invokes `source`.

The `literal_faq` reproduction initializes completion, removes the installed `_rg` definition and mapping, then executes `$ source <(cat "$1/_rg")`. Here `cat` merely substitutes for the unavailable built generator; it cannot affect which command the shell executes. The head fixture produces `command not found: $`, `line_status=127`, and `defined=0`. This remains a failure even when all completion prerequisites are met, so it is independent of the initialization finding.

The remedy is the one-character deletion in the startup example:

```zsh
source <(rg --generate complete-zsh)
```

The prompt convention may remain in snippets explicitly depicting commands entered at a terminal. No wholesale documentation style change is necessary.

## Worked code-judo proposal

The documentation can eliminate implicit shell modes by stating one shared boundary: zsh discovers completion files during initialization, and direct `compdef` registration needs initialization already completed. Present both alternatives in terms of that boundary. For a plain configuration, the file-based startup example becomes:

```zsh
# Add this directory before the existing completion initialization.
fpath=("$HOME/.zsh-complete" $fpath)
autoload -Uz compinit
compinit
```

Keep the existing terminal commands that generate `_rg` before this startup-file example. Explain that users whose shell framework initializes completion should place the directory assignment before the framework's initialization rather than add a redundant initialization call.

The dynamic alternative for a plain configuration becomes:

```zsh
autoload -Uz compinit
compinit
source <(rg --generate complete-zsh)
```

For an existing framework configuration, only the last line is added, after the framework has initialized completions. This preserves the desired simple installation method while making its execution contract visible. It also resolves both actionable findings without any new code branch, helper, mode flag, or delayed registration machinery. The exact initialization flags and framework-specific insertion point need not be standardized by this PR; documenting the ordering invariant is sufficient.

## Verification and remediation status

Both failure claims are verified with focused offline shell execution. The ordering-correct forms were verified through registration checks and interactive Tab completion; the terminal output and generator substitution are documented in [01_zsh_entrypoint.md](01_zsh_entrypoint.md). Reproduction commands are `python ../thermo-checks/check.py` and `python ../thermo-checks/interactive.py` from this directory. Scripts, fixtures, raw option dumps, and logs are retained in the adjacent checks directory.

The worked examples are review proposals, not checkout edits. No Rust binary was built, no network was used, and the full CI help-comparison command remains unavailable. The actionable changes are limited to the new documentation. There are no unresolved questions or additional actionable findings in this subsystem.

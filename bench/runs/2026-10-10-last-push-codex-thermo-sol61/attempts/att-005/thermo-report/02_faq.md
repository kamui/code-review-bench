# FAQ installation boundary

## Scope and judgment

Reviewed the committed FAQ diff and the completion section at `FAQ.md:91–150`, including the distinction between commands to run interactively and configuration to put in a startup file. Both actionable findings are newly introduced instructions for dynamic loading. Their severity is a failed installation path, not cosmetic wording. The implementation itself can remain unchanged.

## F1: Remove the prompt marker from the `.zshrc` snippet

`FAQ.md:131–135` says readers can add the following to `.zshrc`, but the following block is `$ source <(rg --generate complete-zsh)`. `$` is an ordinary command token in that position. It is not a valid zsh startup-file prompt marker. The neighboring `fpath` configuration block at line 128 correctly omits an interactive prompt, while the earlier generation blocks explicitly show terminal commands. The new dynamic block mixes these two contracts.

The literal-documentation mode in [check.zsh](../thermo-checks/check.zsh) starts with a clean `fpath`, initializes completion, and defines a test `rg` function that prints the scratch template. This isolates shell syntax from the unavailable Rust binary. Executing the exact new line then produces `command not found: $`, `source_status=127`, and `registration=missing`. The same result occurs for the base and head template because the source command is never invoked. The new configuration example exists only at head, so the finding is attributable to the new FAQ text rather than an existing template defect. Output is retained in [matrix.txt](../thermo-checks/matrix.txt).

The direct remedy at lines 134–135 is:

```zsh
source <(rg --generate complete-zsh)
```

Do not retain the prompt marker in a code block explicitly intended for literal configuration. Interactive generation commands elsewhere can retain their existing presentation. This correction removes an unnecessary translation step for readers and has no effect on completion behavior.

## F2: Document that dynamic sourcing must follow completion initialization

`FAQ.md:131–135` directs readers to add the dynamic source operation to `.zshrc` without specifying its completion-system prerequisite. The new footer in `rg.zsh:441–445` invokes `_rg` when `compdef` is absent; that is necessary for the existing standalone argument-list harness, but it does not install completions in an uninitialized interactive shell. The comment about not running the function when sourced is conditioned on the completion system being available.

This matters to a profile that already calls `compinit` later in the file, or to a minimal profile following the FAQ for the first time. With no separately installed `_rg` on `fpath`, sourcing the corrected line before initialization prints `_rg:341: command not found: _arguments` and returns status 1. Subsequently running `autoload -Uz compinit; compinit -D -i` leaves `_comps[rg]` absent. The function definition alone does not create the missing command registration. Source-after-initialization returns zero and sets `_comps[rg]=_rg`. These observations are in [matrix.txt](../thermo-checks/matrix.txt); the test filters preinstalled `_rg` directories specifically to prevent accidental success.

The no-initialization failure is present at both base and head. It is not being presented as a newly introduced execution regression. The actionable change is the new installation instruction: it now advertises a `.zshrc` operation whose prerequisite and ordering determine whether it does what the paragraph promises.

Explain that the dynamic source command must follow the profile's existing completion initialization, and give initialization instructions for a profile that lacks them. Also explain that the recommended `fpath` modification must precede that initialization. This states the actual ownership boundary and avoids forcing users to infer two opposite ordering requirements from zsh internals.

## Worked code-judo proposal: one initialization owner, two explicit recipes

Replace the implicit installation contracts with short, executable recipes that keep completion initialization in the user's profile. For the recommended on-disk installation, retain the existing interactive generation commands and explain that this profile ordering is required:

```zsh
fpath=("$HOME/.zsh-complete" $fpath)
autoload -Uz compinit
compinit
```

If the profile already initializes completion, put the `fpath` line before its existing initialization rather than adding another call. For dynamic installation, describe this ordering:

```zsh
autoload -Uz compinit
compinit
source <(rg --generate complete-zsh)
```

If completion is already initialized, readers need only place the source line after that existing initialization. These are proposed documentation examples, not edits applied to the checkout. The existing performance note can remain: the proposal preserves both installation choices and adds no startup work beyond the initialization already required.

This is the useful code-judo move for the patch: make the canonical initialization boundary explicit instead of introducing automatic `compinit` calls, extra fallback branches, or delayed registration into `rg.zsh`. It removes ambiguity and the temptation to grow the loader into a profile-management mechanism. The local tests validate both underlying orderings with `compinit -D -i`; those test-only flags prevent cache writes and interactive security prompts and need not appear in the public recipe.

## Verification status and remediation checks

The literal prompt failure and initialization-order failure were both reproduced independently on zsh 5.8.1. The proposed prompt-free, initialized source operation was exercised through actual interactive Tab completion using [tab-check.py](../thermo-checks/tab-check.py), with results in [tab-results.txt](../thermo-checks/tab-results.txt). The recommended `fpath`-before-initialization route also passed actual Tab completion. No Rust binary was needed for the generation-value candidate tested.

After changing the documentation, recheck its literal startup-file blocks in a profile without another installed `_rg`. The expected result is registration of `_rg` and completion of `complete-zs` to `complete-zsh`. Keep configuration blocks free of terminal prompt markers, and make the existing-initialization wording prevent duplicate `compinit` calls. This can be fixed entirely in `FAQ.md`.

There are no open questions required to act on these findings. The full binary-based CI comparison was unavailable, and these tests do not establish all zsh-version behavior. The report makes no external performance measurement or recommendation beyond the diff's documented installation choices.

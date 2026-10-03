# Zsh completion loading

## Scope and evidence

Reviewed `crates/core/flags/complete/rg.zsh` in full, its unchanged generator in `crates/core/flags/complete/zsh.rs`, the `CompleteZsh` dispatch in `crates/core/main.rs`, and the argument-list consumer in `ci/test-complete`. No ambient repository guidance was loaded. The checkout was not edited and no built ripgrep was invoked.

The committed change adds nine lines and removes one from the shell script. The file is 637 lines at base and 645 at head. The changed footer at lines 437–445 replaces the unconditional `_rg "$@"` with one conditional containing two predicates. No argument specifications, helper bodies, Rust APIs, option types, or asynchronous orchestration changed.

The measurements came from `git diff --numstat main...review-head`, `git show main:crates/core/flags/complete/rg.zsh | wc -l`, and the corresponding `review-head` command. `git diff --check main...review-head` passed. `zsh --version` reported `zsh 5.8.1 (x86_64-ubuntu-linux-gnu)`.

## Loading contract

The header’s `#compdef rg` is the conventional discovery marker for the saved `_rg` file. `compinit` registers that file from `fpath`; invoking its autoloaded `_rg` executes the whole file once. That first invocation defines the real `_rg` function and the two helpers, then must immediately invoke the newly defined `_rg` with the original arguments. Otherwise the first completion request gets no matches. The `$funcstack[1] == _rg` predicate preserves this invocation.

Direct sourcing is different. The file should define its functions and register `_rg` for `rg`, without executing `_arguments` outside a completion context. With `compdef` available and the top call-stack frame different from `_rg`, the new else branch registers the function and returns successfully. The registered function handles subsequent completions normally.

The no-`compdef` predicate preserves the existing CI harness contract. `ci/test-complete:8–16` sources the file in a subshell with `_RG_COMPLETE_LIST_ARGS=1`, without initializing the completion system. `_rg:347–349` prints the argument array and returns before `_arguments`. This is a real supported consumer of the file, rather than a redundant fallback invented by the PR.

Outside that test context, absence of `compdef` does not imply that running `_rg` is safe. An ordinary source before completion initialization still reaches `_arguments` and fails. The new FAQ must state its initialization prerequisite. This is the documentation finding carried in `summary.md`, not an additional shell-code finding: the older file also ran `_rg` unconditionally, and the reviewed supported path with initialized completion works.

## Verification

Source copies were prepared with `git show main:crates/core/flags/complete/rg.zsh` and `git show review-head:crates/core/flags/complete/rg.zsh`, redirected only into `clone-work/zsh-probes/base/_rg` and `head/_rg`. Every shell probe ran from `clone-work`, using `zsh -df` and `compinit -D -i` to avoid user startup files and completion dump writes. Any installed `fpath` directory containing `_rg` was removed from the probe’s search path so an unrelated packaged completion could not mask a missing registration. The scratch `_rg` directory was then added for the autoload case.

The scratch harness uses zsh’s normal unset-variable behavior outside completion and recreates the CI helper’s local option setup for its list scenario. For first and second autoload measurements, output is redirected to scratch files in the same shell before counting lines; a pipeline would run the function in a subshell and fail to preserve the autoload state between calls.

| Probe | Base | Head | Status |
| --- | --- | --- | --- |
| Existing CI argument-list source hook | 341 lines, no stderr | 341 identical lines, no stderr | Passed |
| Source after `compinit` | Status 1; completion-context error | Status 0; `_comps[rg]=_rg`; no stderr | Intended fix reproduced |
| First autoloaded list invocation | 341 lines | 341 lines | Preserved |
| Second invocation in same shell | 341 lines | 341 lines | Preserved |
| Interactive source path, generate-value tab completion | Not run at base | Produces `rg --generate=complete-zsh ` | Passed |
| Interactive autoload path, generate-value tab completion | Not run at base | Produces `rg --generate=complete-zsh ` | Passed |
| Syntax-only shell parse | Pass | Pass | Passed |

The list comparisons used `zsh -df zsh-probes/check.zsh base list`, the corresponding head command, and `diff -u` on their captured outputs. Direct-source and autoload checks used the same harness with `source` and `autoload` scenarios. The initial autoload check has `fpath` set before `compinit` and invokes `_rg` twice with `_RG_COMPLETE_LIST_ARGS=1`.

The interactive checks used `python3 zsh-probes/interactive.py` to start a zsh PTY, initialize completion without dump writes, source or autoload the scratch script, then send `rg --generate=complete-zs` followed by a tab. The observed output in both cases was exactly `rg --generate=complete-zsh ` with the trailing space. This checks actual completion machinery rather than only a stubbed `_arguments` call. It does not execute the generated command or require a ripgrep binary.

Both copies also passed `zsh -df -n zsh-probes/head/_rg` and the equivalent base command. A secondary `_arguments` stub probe checked that normal function execution reaches argument dispatch; the interactive probes provide the stronger evidence.

The base source failure was `_arguments:comparguments:325: can only be called from completion function`, matching the issue’s behavior. Head produces no such error after initialization. Supporting captures are [base source stderr](evidence/base-source.stderr), [head source output](evidence/head-source.stdout), [head autoload output](evidence/head-autoload.stdout), [source PTY transcript](evidence/head-interactive-source.txt), and [autoload PTY transcript](evidence/head-interactive-autoload.txt).

## Worked code-judo assessment

A plausible simplification is to discard the call-stack test and choose solely on whether `compdef` exists:

```zsh
if (( $+functions[compdef] )); then
  compdef _rg rg
else
  _rg "$@"
fi
```

That candidate was written only into `clone-work/zsh-probes/registration-only/_rg`. The same autoload probe was run with `zsh -df zsh-probes/check.zsh registration-only autoload`. Its first invocation printed zero argument lines; its second printed 341. Both invocations of the committed implementation print 341. See [the candidate output](evidence/registration-only-autoload.stdout).

This apparent simplification loses the first autoloaded completion because the initial file execution registers a function instead of running it. Removing only the fallback instead would break the existing CI source hook. Always running `_rg` restores the original issue. An extracted loading helper still needs to represent these contexts and would add another shell function and naming boundary without deleting any condition.

There is no verified dramatic simplification here. The current footer is a small dispatcher at the canonical boundary, and its code-quality cost is justified by actual consumers. Keeping the existing script as the sole source for both generated and packaged completions also avoids maintaining two implementations. No structural blocker, wrapper duplication, new type optionality, file-size threshold crossing, or scattered feature logic was established.

The useful boundary cleanup is in the documentation: express the `compinit` ordering once and place each alternative around it. The worked replacement appears in [the documentation report](02_zsh_setup_documentation.md#worked-code-judo-proposal).

## Limits

The encoding placeholder remains in the scratch source because the Rust generator could not be built. None of the exercised paths call `_rg_encodings`. The actual binary’s generator and full option-to-help comparison were inspected statically but not executed. Older zsh versions were not available for validation. Neither `cargo build`, `cargo test`, nor `ci/test-complete` was attempted, in accordance with the execution allowance.

There are no separate actionable findings in this subsystem report. Its initialization evidence supports the FAQ finding already stated in the summary.

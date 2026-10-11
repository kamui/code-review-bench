# Zsh completion entry point

## Scope and structural judgment

Reviewed `crates/core/flags/complete/rg.zsh`, especially lines 437–445, its existing `_rg` definition and argument-list hook, `crates/core/flags/complete/zsh.rs`, and the source-extraction path in `ci/test-complete`. There are no actionable findings in this subsystem. The FAQ's installation-contract findings are documented separately in [02_faq.md](02_faq.md).

The diff adds one terminal dispatcher rather than inserting feature checks throughout the 302-line option specification. `_rg` already owns option completion, `_rg_encodings` and `_rg_types` already own their specialized candidates, and the generator already embeds the shell template with a single encoding substitution. The new feature uses that canonical layer without changing the Rust API or creating another registry, wrapper, nullable mode, or helper. Existing complexity in the option table and color/type handling is untouched and is not a regression attributable to this PR.

The dispatcher observes the invocation boundary using zsh's native function stack and function table. These are shell completion integration mechanisms, not an invented generic abstraction. The first predicate recognizes the autoload invocation of `_rg`; the second preserves raw sourcing in the test harness, where the completion system is absent. The remaining case registers a definition loaded directly into an initialized completion system. A dedicated policy object or helper would add indirection to five executable lines and would not remove any required behavior.

## Measurements and source evidence

`git diff --stat main...review-head` reports two changed files, 29 insertions and four deletions overall. The shell template changes from 637 to 645 lines, with nine additions and one deletion. `FAQ.md` changes from 1,046 to 1,063 lines; it was already over the threshold at the base. The PR pushes neither file across 1,000 lines. There is no reason to split the shell template solely for this entry-point addition.

The measurements came from `git show main:crates/core/flags/complete/rg.zsh | wc -l`, `wc -l crates/core/flags/complete/rg.zsh FAQ.md`, and `git show main:FAQ.md | wc -l`. The committed diff and complete shell template were read. The generator in `zsh.rs:21–22` embeds `rg.zsh` and replaces `!ENCODINGS!`; no parallel completion implementation needs modification.

The existing test harness deliberately invokes `( _RG_COMPLETE_LIST_ARGS=1 source $1 )` with `unset` enabled inside its extraction function. `_rg`, lines 345–350, prints its argument specs and returns before `_arguments` is needed. Preserving execution when `compdef` is absent therefore has a concrete existing consumer; deleting it as an apparently redundant fallback would break that consumer.

## Executed shell verification

All execution was offline and outside the clone, using zsh 5.8.1. The test driver is [check.zsh](../thermo-checks/check.zsh), its captured matrix is [matrix.txt](../thermo-checks/matrix.txt), and the actual completion driver and results are [tab-check.py](../thermo-checks/tab-check.py) and [tab-results.txt](../thermo-checks/tab-results.txt). Each child shell used `zsh -f`, and `compinit -D -i` avoided writing a completion cache. Tests removed any system `fpath` directory containing an installed `_rg` before constructing the intended scenario, so a preinstalled completion could not mask failed registration.

Scratch copies were made with `git show main:crates/core/flags/complete/rg.zsh > ../clone-work/thermo-checks/base/_rg` and `cp crates/core/flags/complete/rg.zsh ../clone-work/thermo-checks/head/_rg`. The executable matrix used `zsh -f ../clone-work/thermo-checks/check.zsh MODE ../clone-work/thermo-checks/REVISION/_rg` for each mode and revision. The template's unresolved encoding placeholder is inert for the paths exercised here.

| Path | Base | Head | Verification scope |
| --- | --- | --- | --- |
| Source after `compinit` | `_arguments:comparguments` error, status 1, no registration | Status 0, `_comps[rg]=_rg` | Real completion initialization and real sourcing |
| Source before `compinit` | `_arguments` missing, no registration after initialization | Same failure and missing registration | Establishes the documentation prerequisite |
| Autoload via `fpath` | Registers and invokes `_rg`, status 0 | Same | `_arguments` stub observes 318 arguments and the generation spec |
| Argument-list extraction without completion initialization | 341 lines | Identical 341 lines | Exact relevant extraction function copied from CI harness |
| Actual Tab completion through sourcing | Does not extend `complete-zs` | Extends to `complete-zsh ` | Interactive PTY, real completion system |
| Actual Tab completion through autoloading | Extends to `complete-zsh ` | Same | Interactive PTY, real completion system |

Argument-list output was compared using `diff -u ../clone-work/thermo-checks/base/list.txt ../clone-work/thermo-checks/head/list.txt`; the comparison exited zero. These checks do not claim to compare the specs against binary help output, since the binary-dependent CI command was unavailable.

The actual Tab check launches `zsh -f` in a PTY, initializes completion, loads the chosen scratch template, and sends `rg --generate=complete-zs` followed by Tab. A ZLE widget invokes `expand-or-complete` and records `BUFFER`. It does not execute an `rg` binary. Head outputs contain `THERMO_BUFFER=rg --generate=complete-zsh ` through both loading modes. Raw PTY captures are retained beside the driver. The first driver run was interrupted during process cleanup; the subsequent completed run used forced child cleanup and produced all four results.

## Worked code-judo assessment

The most tempting reduction is to replace the entire footer with registration:

```zsh
compdef _rg rg
```

That deletes a branch but changes the contract: raw CI sourcing has no `compdef`, and an autoloaded file must actually run the freshly defined function on its first invocation. The measured autoload and extraction paths show why registration alone is not equivalent. Keeping only `_rg "$@"` is the base behavior and reproduces the direct-sourcing error. Neither reduction earns a remediation request.

Inverting the condition yields the same two paths and the same dependencies:

```zsh
if [[ $funcstack[1] != _rg ]] && (( $+functions[compdef] )); then
  compdef _rg rg
else
  _rg "$@"
fi
```

This alternative is an explanatory rearrangement, not a material simplification, and is not proposed as an actionable finding. The existing form already collapses autoload and harness execution into one body call. An added helper, explicit mode enum, or separate generated loader would increase the number of concepts without deleting behavior.

The useful simplification is at the documented boundary: make profile initialization own `compinit` and state the ordering once. Do not make the generated completion script initialize the entire user's completion system or introduce a delayed-registration mechanism merely to compensate for an incomplete FAQ. The worked documentation remedy is in [02_faq.md](02_faq.md).

## Verification limits

Only the local installed zsh version was executed. Rust generation, encoding completion, file-type subprocesses, and the full binary/help consistency comparison were not executed. No evidence supports a finding about these unexercised paths. The clone was not edited, no tests requiring a build were attempted, and no external information was fetched.

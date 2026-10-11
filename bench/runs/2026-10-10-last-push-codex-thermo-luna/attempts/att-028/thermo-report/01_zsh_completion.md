# Zsh completion dispatch and documentation

## Scope

Reviewed the zsh completion template change at `crates/core/flags/complete/rg.zsh:437-445`, its existing test harness in `ci/test-complete`, and the zsh installation guidance at `FAQ.md:91-139`.

## Evidence and design assessment

The changed dispatch is:

```zsh
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

This chooses execution when the file is being invoked as `_rg`, or when `compdef` is unavailable; otherwise it registers the function for `rg`. These conditions cover the two existing entry paths: zsh's completion machinery invokes the autoloaded completion function, while the FAQ's process-substitution form sources the generated script into an initialized interactive shell. The fallback also preserves the way `ci/test-complete` sources the template with `_RG_COMPLETE_LIST_ARGS=1` and no completion-system setup; that harness relies on the top-level `_rg` invocation to emit its argument specifications.

This introduces one conditional at the completion-script entry boundary. It does not add scattered feature checks or make the completion implementation itself more stateful. The two branches each express a distinct action, so merging them would obscure rather than simplify the behavior.

The FAQ first documents the fpath-based installation and then offers process substitution as a simpler, slower-to-start alternative. It also explains adding the generated-completion directory to `fpath`, which completes the prior instructions. The added advice is kept beside the existing completion setup rather than introducing a new documentation mechanism.

## Code-judo alternative considered

A tempting simplification is to replace the dispatch with unconditional `compdef _rg rg`, or to add only zsh's `#compdef rg` metadata. That does not preserve the direct function invocation required when zsh loads an autoloaded completion, nor does metadata alone register a script sourced through process substitution. Another option is to move mode selection into `_rg`, but then invocation setup would be mixed into the completion function and still need to distinguish source-time registration from completion-time execution. The current short boundary dispatch keeps those modes explicit without adding a helper or another layer.

No stronger code-judo move is apparent from the surrounding architecture. The patch adds no helper, type boundary, or abstraction that could be removed while preserving the two invocation modes and the harness contract.

## Measurements and commands

- `git diff --stat main...review-head`: 2 files changed, 29 insertions, 4 deletions.
- `git diff --check main...review-head`: passed with no whitespace errors.
- `wc -l FAQ.md crates/core/flags/complete/rg.zsh ci/test-complete`: 1,063, 645, and 97 lines respectively. The FAQ was not pushed across the 1,000-line threshold by this patch.
- `ci/test-complete` could not be run because the execution policy says it requires a built `rg`; `cargo build` and `cargo test` are unavailable. No generated-script execution was attempted because the source template contains `!ENCODINGS!`, replaced during generation.

## Verification status

Static diff and surrounding-harness inspection only. Functional behavior was not independently executed. No actionable maintainability issue was found in the available evidence.

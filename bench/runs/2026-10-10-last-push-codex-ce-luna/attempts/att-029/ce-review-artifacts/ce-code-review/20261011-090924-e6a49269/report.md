# Code review

## Findings

| # | Severity | Location | Finding | Route |
|---|---|---|---|---|
| 1 | P2 | `crates/core/flags/complete/rg.zsh:441` | Sourcing before compinit still runs the completion body | gated_auto / downstream-resolver |

When `compinit` has not defined `compdef`, the new branch invokes `_rg` while the file is sourced. The completion body then reaches completion-only commands outside a completion context, so the FAQ's dynamic-source method still errors in this initialization order. Document the prerequisite or change the guard; verify the startup sequence.

## Actionable Findings

1. Document that `compinit` must be run before the dynamic-source line, or distinguish completion invocation from top-level sourcing without using the absence of `compdef` as a reason to invoke `_rg`.

## Coverage

Focused review: correctness plus one independent same-model adversarial read. No other reviewer agents or validator ran. No criteria files govern the changed paths; no Compound Packs or plan were found. Focused zsh execution confirmed the no-`compdef` path fails. Rust build/cargo tests were unavailable, and `ci/test-complete` requires a built `rg`.

## Verdict

Ready with fixes.

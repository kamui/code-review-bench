# Zsh completion and FAQ

## Scope and evidence

The reviewed range is `79cbe89deb1151e703f4d91b19af9cdcc128b765..855bfa6cdae4f4fe8762f892fc4957635397083e`. It changes `crates/core/flags/complete/rg.zsh` and `FAQ.md`.

At `crates/core/flags/complete/rg.zsh:437-445`, the patch replaces the unconditional `_rg "$@"` tail with a documented dispatch. When the current function stack is `_rg`, or when `compdef` is unavailable, it calls `_rg`; otherwise it registers `_rg` for `rg` through `compdef`. The completion file defines `_rg` earlier in the same file. This is a localized choice between the existing direct/source execution path and registration for a user who sources the generated completion after completion support is available.

The repository’s `ci/test-complete` is relevant to that boundary. Its `get_comp_args` function sets `_RG_COMPLETE_LIST_ARGS=1` and sources the completion file in a subshell. The completion function recognizes that variable to expose its argument specs. The branch retains this use while avoiding an unconditional completion-function call when sourced in an interactive zsh session that has `compdef` available. The generator in `crates/core/flags/complete/zsh.rs` embeds this file directly, so the dispatch stays with the completion script rather than introducing a separate generation layer.

The FAQ changes correct “completes” to “completions,” retain the file-based zsh setup as the recommended path, explain adding the directory to `fpath`, and offer dynamic sourcing as an easier but slower alternative. This communicates the startup cost instead of concealing it.

## Maintainability assessment and code-judo analysis

The new conditional is a special case in the narrow sense that it distinguishes two invocation contexts, but the contexts are real parts of the script’s contract: source-time setup and direct completion-function execution. Keeping the distinction at the file’s final dispatch means readers can understand the behavior without tracing mode flags through `_rg` itself. The predicate is compact and the function body remains unchanged.

A possible restructuring would make sourcing always register the function and move the CI checker to call `_rg` directly after sourcing. That would remove the fallback dispatch, but it changes the test harness’s current extraction contract and still needs to account for the zsh completion system’s autoload invocation. Another possibility would be to wrap the registration and execution paths in a helper; that would only hide this one conditional behind an extra function and would not remove a concept. On the inspected evidence, neither is a meaningful simplification. No more elaborate abstraction, mode variable, or module split is warranted for this one decision.

The patch does not create file-size pressure. `FAQ.md` is 1,046 lines at `main` and 1,063 lines at the reviewed head; it was already above 1,000 lines before the change. `rg.zsh` is 637 lines at `main` and 645 lines at the reviewed head. Neither file crosses the skill’s under-1,000-to-over-1,000 threshold due to this patch.

## Verification status

Read-only review commands included `git diff --no-ext-diff --unified=80 main...review-head -- FAQ.md crates/core/flags/complete/rg.zsh`, `rg -n` searches for the completion generator and test call sites, inspection of `ci/test-complete` and `crates/core/flags/complete/zsh.rs`, and line-count measurements using `wc -l` on the checkout and `git show main:<path>`. `git diff --check main...review-head` completed without reporting whitespace errors. The checkout was clean before and after inspection.

No zsh execution was performed. The packet permits focused execution, but the review did not need runtime validation to assess the structural change. `ci/test-complete` was not run because it requires a built `rg`; Rust build and test commands are unavailable under the packet’s execution policy.

## Findings

No actionable maintainability finding for this subsystem.

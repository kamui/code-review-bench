# Completion and documentation subsystem

## Scope and measurements

The reviewed range changes `crates/core/flags/complete/rg.zsh` by nine added lines and one removed line, and changes `FAQ.md` by twenty added lines and three removed lines. The completion file is 645 lines at the head. `FAQ.md` is 1,046 lines at the base and 1,063 lines at the head; it was already above 1,000 lines before this patch.

The zsh generator in `crates/core/flags/complete/zsh.rs` embeds `rg.zsh` directly (with only the encoding placeholder substituted). `ci/test-complete` sources the completion file in a subshell after setting `_RG_COMPLETE_LIST_ARGS=1`; it does not arrange to load `compdef` first. These existing consumers explain why the tail must support both standalone sourcing and autoloaded completion use.

## Structural assessment

At `crates/core/flags/complete/rg.zsh:441-446`, the new conditional calls `_rg` when the file is executing as the `_rg` completion function or when `compdef` is unavailable. Otherwise it registers `_rg` for the command `rg` with `compdef`. This isolates the new behavior at the file's existing load boundary and leaves the substantial completion specification and helper functions untouched.

The two paths correspond to two distinct execution contexts required by the feature and existing test harness. Folding the condition into `_rg`, adding setup helpers, or moving registration into generated Rust would add concepts and make the shell script harder to follow. The current change is already the code-judo-sized solution: one explicit dispatch lets direct sourcing register completion while retaining function execution for autoload and the existing source-based checker. No stronger simplification is apparent without losing one of those behaviors.

The FAQ change at `FAQ.md:112-143` now completes the existing persistent setup by adding the generated directory to `fpath`, and offers dynamic sourcing as an alternative with a startup-time caveat. The documentation sits in an already-large FAQ, but the file was over 1,000 lines at the base, so the patch does not cross the skill's file-size threshold. No decomposition is warranted for this narrowly scoped documentation update.

## Findings

No actionable structural, abstraction, branching, boundary, or file-size finding was identified. The wording "Note though that while this approach is easier to setup, is generally slower" in `FAQ.md` could be copyedited for grammar, but it is a cosmetic issue and does not merit an actionable code-quality finding under this review's bar.

## Verification status

- Inspected the full changed-file diff and the relevant completion generator and test harness.
- `git diff --check main...review-head`: passed with no output.
- Runtime execution was not performed. The packet says the built `rg` binary is unavailable and disallows `cargo build`/`cargo test`; no tests were added or run.
- Working tree was clean at inspection (`git status --short --branch` showed only the checked-out `review-head` branch).

## Worked remediation proposal

No code changes are proposed. The current conditional is the smallest readable structure that distinguishes direct source from completion dispatch and preserves the test harness behavior. Reworking it would either introduce an extra helper/abstraction or obscure the two runtime contexts rather than deleting complexity.

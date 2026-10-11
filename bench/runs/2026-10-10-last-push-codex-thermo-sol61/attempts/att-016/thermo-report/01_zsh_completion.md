# Zsh completion implementation

## Scope and evidence

The substantive implementation change is `crates/core/flags/complete/rg.zsh:437–445`. The previous unconditional `_rg "$@"` becomes a single conditional:

```zsh
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

The whole completion file was read, including its option table, helper functions, and reference section. Adjacent source evidence was `crates/core/flags/complete/zsh.rs:20–22` and `ci/test-complete:8–17`. The generator embeds `rg.zsh` and replaces the encoding placeholder; it adds no separate loading policy. The CI script sources the completion with `_RG_COMPLETE_LIST_ARGS=1`, deliberately without loading the completion system.

The existing `_rg` function constructs its specifications and returns them before calling `_arguments` when that special variable matches `(1|t*|y*)` (`rg.zsh:345–350`). This explains why the no-`compdef` invocation route remains useful without a completion context.

## Measurements and commands

Read-only inspection commands included:

```text
git diff main...review-head
git diff --numstat main...review-head
nl -ba crates/core/flags/complete/rg.zsh
nl -ba ci/test-complete
cat crates/core/flags/complete/zsh.rs
git show main:crates/core/flags/complete/rg.zsh | wc -l
wc -l FAQ.md crates/core/flags/complete/rg.zsh
git diff --check main...review-head
```

The script adds nine lines and removes one, growing 637 → 645 lines. The FAQ adds twenty and removes three, growing 1,046 → 1,063 lines. Neither crosses the skill's threshold. The implementation adds one dispatch statement with two predicates, both at the canonical loading boundary. It does not add conditions inside option generation, helpers, or Rust dispatch.

## Focused execution

Scratch sources and harnesses are preserved in `../thermo-check/`. Base and head `_rg` copies came directly from `git show main:crates/core/flags/complete/rg.zsh` and `git show review-head:crates/core/flags/complete/rg.zsh`. Commands ran outside the clone using zsh 5.8.1:

```zsh
for revision in base head; do
  for scenario in source-ready source-before-init inspection inspection-ready autoload fpath-before-init fpath-after-init; do
    print -r -- "$revision $scenario"
    zsh -f matrix.zsh "$revision" "$scenario"
  done
done
zsh -f interactive.zsh
zsh -n head/_rg
zsh -n base/_rg
```

The matrix uses `compinit -D` to avoid writing dump files and `zsh -f` to skip user startup configuration. It removes system completion directories containing `_rg` from the harness's default `fpath`, preventing a preinstalled completion from falsely proving successful registration. This isolation was necessary: an initial unisolated probe found an existing system mapping.

| Check | Base | Head | What this verifies |
| --- | --- | --- | --- |
| Source process substitution after compinit | Status 1; completion-context error; no mapping | Status 0; rg maps to _rg | New source registration behavior |
| Source process substitution before compinit | Status 1; _arguments missing | Same; mapping still missing after compinit | Setup prerequisite; documented finding |
| Inspection without compinit | 341 lines; status 0 | 341 lines; status 0 | Existing CI-style source route |
| Inspection after compinit | 341 lines; status 0 | Zero lines; status 0 | Limit of environment-based inspection dispatch |
| First autoload call | _arguments called; status 0 | Same | Completion body runs on first invocation |
| Subsequent call | _arguments called; status 0 | Same | Definition installed by autoload remains usable |
| fpath before compinit | rg maps to _rg | Same | Discovery during initialization |
| fpath after compinit | Mapping missing | Same | Ordering requirement |

For the autoload checks only, `_arguments` was replaced with an instrumented function that reports invocation and specification count. Both versions delivered 318 arguments to that function on the first and second calls. This verifies dispatch, not completion matching or argument forwarding to all helpers.

The independent interactive harness uses `zsh/zpty`, initializes real completion, sources the head script through process substitution, types `rg --generate=complete-zs<TAB>`, and reads the resulting editor buffer with a ZLE widget. It passed with:

```text
PASS: TAB produced BUFFER_RESULT=rg --generate=complete-zsh
```

No `rg` subprocess is needed to complete this enum. These checks exercise the raw embedded template; the encoding placeholder occurs inside a helper unused by this test.

## Structural assessment

The change belongs exactly where it is: the end of the standalone zsh script. First autoload invocation evaluates the script as a function named `_rg`; the script defines the permanent function and invokes it to serve that first request. Subsequent calls execute the permanent definition directly. Independent sourcing defines the functions and registers the command through `compdef` instead of invoking `_arguments` outside its valid context.

A condition on invocation context is necessary to distinguish these routes. It is not scattered special-case growth. The native `functions` table avoids a subprocess or command-name lookup. No generic policy object, wrapper, or extracted helper earns its keep for this localized branch.

The comment that the function does not run when independently sourced is qualified by the actual implementation: without `compdef`, it still runs. The existing CI inspection route depends on that exception. The details above make the contract explicit for this review; the mismatch is not promoted to another finding because the relevant runtime prerequisite is covered by the FAQ finding.

## Worked code-judo proposals and disposition

One proposed simplification is to keep only `compdef _rg rg` at the footer and let the function definition handle all completion calls. That loses execution of the initial autoload request: evaluating a file that only defines a function and registers a mapping does not serve the already active request. The observed first-call check shows why execution must remain in the autoload branch. Reject this proposal.

Moving source-specific registration into the Rust generator would leave different loading semantics for generated output and the canonical source consumed by CI. It would duplicate policy and fail to delete the autoload requirement. Reject this proposal.

Extracting a `_rg_register_or_complete` helper merely moves the same condition, and changes `funcstack` depth, into another abstraction. It creates a new name and coupling to call-stack layout without simplifying the state model. Reject this proposal.

The useful code-judo move is outside the implementation: show the initialization sequence in the FAQ. This makes the two modes understandable without teaching users the inspection fallback or adding implicit `compinit` calls to generated code. The worked proposal is in [02_zsh_setup_docs.md](02_zsh_setup_docs.md).

Replacing the absence-of-`compdef` predicate with the explicit inspection variable was also considered. It would make inspection work in an initialized shell, but is not a behavior-preserving deletion: it changes the fallback behavior and would need a deliberate decision about how uninitialized sourcing should fail. The actual CI entrypoint does not initialize completion. No requirement for the broader hook behavior was established, so this remains an observation, not an actionable finding or requested refactor.

## Verification limits

The full `ci/test-complete` script was read but not run because it needs a built `rg`. No Cargo command was attempted. All dispatch checks used the pinned sources in scratch space. Only zsh 5.8.1 was executed; older versions and other user option combinations were not claimed verified. The Rust generator was inspected, not executed.

There are no actionable implementation findings. Retain the localized dispatch and address the two documentation findings in the summary.


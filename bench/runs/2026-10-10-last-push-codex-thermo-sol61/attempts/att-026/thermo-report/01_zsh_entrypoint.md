# Zsh entry-point and generation boundary

## Scope and measurements

The changed implementation is `crates/core/flags/complete/rg.zsh:437–445`. The entire completion function, its argument specification, and the encoding/type helpers were read, along with `crates/core/flags/complete/zsh.rs`, the generator's dispatch in `crates/core/main.rs`, the encoding inclusion in `crates/core/flags/complete/mod.rs`, and the relevant consumers in `ci/test-complete` and release packaging scripts. All findings below use the pinned source, rather than discussion history.

`git diff main...review-head` shows +9/−1 in the shell script. `git show main:crates/core/flags/complete/rg.zsh | wc -l` reports 637 lines; `wc -l crates/core/flags/complete/rg.zsh` reports 645. Five of the nine added lines are comments or comment spacing. The executable change introduces one `if` with two outcomes and keeps the existing `_rg "$@"` execution. There is no threshold crossing, duplicated completion body, new mutable mode, helper wrapper, or spread of registration checks into the body.

## Judgment

There are no actionable structural findings in this subsystem. The dispatch is in the canonical location: the existing script entry point after the function definitions. Its distinction is necessary to support both a script that registers a function and an autoloaded function that performs completion. The additional capability check also preserves the standalone option-enumeration contract. Moving this distinction elsewhere would obscure the ownership boundary without reducing the number of modes.

The comment's claim about being sourced should be interpreted with the condition that follows it: when `compdef` is absent, sourcing still executes `_rg`. This is deliberate compatibility with the standalone enumerator, not an initialization mechanism. The missing initialization explanation is an actionable documentation finding in [02_faq.md](02_faq.md), not a recommendation to mutate the user's shell setup inside the completion script.

## Dispatch contracts

The script begins with `#compdef rg`, defines `_rg` and its helpers, then dispatches. `crates/core/flags/complete/zsh.rs:21–22` embeds the script and substitutes the encoding list; it does not introduce another loader. Release packaging writes this generated script to a file named `_rg` for ordinary completion-system discovery.

When zsh invokes the autoloaded `_rg` file, `funcstack[1]` identifies that outer `_rg` function. The file defines the implementation and invokes it with the original arguments. Merely registering at this point would omit the first completion attempt. When the file is sourced with the completion system initialized, the stack does not identify that autoload invocation and `compdef` exists; defining and registering `_rg` is the correct action. Calling `_arguments` at this point would run completion logic outside a completion context, reproducing the original error.

`ci/test-complete:8–17` deliberately sources the file without the completion system. It sets `_RG_COMPLETE_LIST_ARGS=1`, and `rg.zsh:345–350` prints the specifications and returns before calling `_arguments`. Testing for the absence of `compdef` preserves this existing loading contract. The full CI command also compares the resulting options with a built binary's help; that final comparison was unavailable and was not claimed as verified.

## Worked code-judo attempts

Unconditional registration would make the end of the script smaller:

```zsh
compdef _rg rg
```

It would fail the standalone enumerator because `compdef` is not loaded there, and it would register without executing the implementation during the first autoload invocation. This candidate deletes necessary behavior rather than incidental complexity. It was rejected.

Unconditional execution is the previous implementation:

```zsh
_rg "$@"
```

It preserves enumeration and autoloading, but the initialized direct-source reproduction returns 1 with `_arguments:comparguments:325: can only be called from completion function`. It cannot implement the requested loading method.

A guard-clause form can express the same partition:

```zsh
if [[ $funcstack[1] != _rg ]] && (( $+functions[compdef] )); then
  compdef _rg rg
  return $?
fi
_rg "$@"
```

This keeps the same context predicate and two actions, adds a return, and does not remove any concepts. It does not offer a material maintainability improvement over the committed `if`/`else`, so it is not an actionable demand. A separate registration helper or a Rust-generated wrapper would add a new abstraction while retaining the same partition. Neither earns its keep in this patch.

## Focused verification

Scratch fixtures were built outside the checkout from the base and head script text. The only generation transformation was replacing `!ENCODINGS!` with the trimmed contents of `encodings.sh`, matching the inspected Rust generator. No Rust build or installed `rg` binary was used to claim generation correctness.

The reproducible driver is [../thermo-checks/check.py](../thermo-checks/check.py), run with `python ../thermo-checks/check.py` from this report directory. Individual `.zsh` scripts are retained beside it. Each shell is `zsh -f` with a 60-second subprocess limit. `compinit -D -i` avoids writing a dump and skips insecure completion paths. Existing `_rg` definitions and registrations were removed where necessary to prevent the system's installed completion from satisfying direct-source assertions accidentally.

| Case | Base result | Head result |
| --- | --- | --- |
| Source after initialization | Status 1, completion-context error, no new registration | Status 0, `_comps[rg]=_rg`, function defined |
| Source without initialization | Status 1, `_arguments` unavailable | Same status and error |
| CI-style source enumeration without `compdef` | Specifications emitted, status 0 | Same output, status 0 |
| Autoload after initialization under enumeration mode | Specifications emitted, status 0 | Same output, status 0 |
| Autoload without initialization under enumeration mode | Specifications emitted, status 0 | Same output, status 0 |

The driver also compares standalone enumeration bytes and asserts equality. There are 341 output lines with SHA-256 `fffa6e2fe9b53ea84c062ddad3030ccf6636e27ebbee5201989444a98f841b22`. Captured output is in [../thermo-checks/results.txt](../thermo-checks/results.txt) and [../thermo-checks/enumeration-comparison.txt](../thermo-checks/enumeration-comparison.txt). The `has_generate` field in the case logs is zsh's substring index, not a count of options.

The PTY driver [../thermo-checks/interactive.py](../thermo-checks/interactive.py) was run with `python ../thermo-checks/interactive.py`. In its direct-source case, a scratch `rg` function emits the generated fixture, and the exact unprefixed `source <(rg --generate complete-zsh)` form loads it. In its autoload case, the fixture directory precedes initialization. Both cases send `rg --generate=complete-zs`, Tab, and a widget that captures the resulting editor buffer without executing the command. Both captured `rg --generate=complete-zsh`. The complete terminal output is retained in [../thermo-checks/interactive-results.txt](../thermo-checks/interactive-results.txt).

`zsh -n crates/core/flags/complete/rg.zsh` and `git diff --check main...review-head` passed. Older zsh versions and the full option-to-help comparison are unverified. No source remedies were applied, and no checkout files were changed.

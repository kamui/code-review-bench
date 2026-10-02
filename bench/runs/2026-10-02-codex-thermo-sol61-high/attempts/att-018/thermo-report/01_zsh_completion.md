# Zsh completion entry and generation boundary

## Scope and source evidence

This detail covers `crates/core/flags/complete/rg.zsh`, especially changed lines 437–445, and the existing consumers of its entry behavior. The review read the whole script, `crates/core/flags/complete/zsh.rs`, the relevant generation routing, and `ci/test-complete`. The CI file was inspected as source; it was not executed.

The review used these read-only commands from the clone:

```sh
git diff main...review-head
nl -ba crates/core/flags/complete/rg.zsh
cat crates/core/flags/complete/zsh.rs
nl -ba ci/test-complete
rg -n 'CompleteZsh|complete::zsh|zsh::generate' crates/core
wc -l FAQ.md crates/core/flags/complete/rg.zsh
git show main:FAQ.md | wc -l
git show main:crates/core/flags/complete/rg.zsh | wc -l
```

`rg.zsh` grows from 637 to 645 lines, with nine additions and one deletion. Its `_rg` body begins at line 15 and ends at line 409. That body assembles option specifications, supports the existing `_RG_COMPLETE_LIST_ARGS` exit at lines 345–350, and then calls `_arguments`. Helpers for encoding and file-type completion follow. The changed dispatcher occupies the existing final executable position before the reference and license material. The PR leaves all option specs and helpers unchanged.

`zsh.rs:21–22` generates one string using `include_str!("rg.zsh").replace("!ENCODINGS!", super::ENCODINGS.trim_end())`. The shell text therefore owns invocation versus registration, while Rust owns embedding and the encoding list substitution. No new generator API or build dependency is introduced.

## Entry contracts

The old final `_rg "$@"` correctly supported the autoload case and the option-dump case, but directly sourcing it in an initialized shell called `_arguments` outside a completion context. The new entry guard handles three contracts with a single dispatch:

```zsh
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

When zsh autoloads the file as `_rg`, the outer loading function appears as `_rg` in the function stack. Executing the newly defined inner `_rg` preserves completion on that first invocation. Later invocations use the defined function directly. The dispatcher does not add checks to the core option flow.

When the file is directly sourced after `compinit`, `compdef` is available and the source frame is not `_rg`. The script defines its functions and registers `_rg` for `rg` without running `_arguments`.

When `compdef` is absent, the original execution behavior is retained. The existing CI extraction helper at `ci/test-complete:8–16` deliberately sources the file with `_RG_COMPLETE_LIST_ARGS=1` and no completion-system initialization. `_rg` prints its specs and returns before needing `_arguments`. This explains why deleting the no-`compdef` arm without changing that contract would break an existing consumer.

An uninitialized ordinary shell also enters this last path. Since its dump flag is absent, it reaches `_arguments` and fails. This is a setup precondition that the newly documented source method must expose. Finding 1 belongs in the FAQ; the source branch is not evidence that the generated script should initialize all of zsh's completion system itself.

## Structural judgment

There is no actionable structural finding in this subsystem. The new conditional belongs at the canonical entry point, does not duplicate any existing helper, and keeps mode differences out of the completion body. Zsh's function table and function stack are native facilities that describe the two entry contexts. They are a compact dispatch mechanism here rather than an added generic abstraction.

The guard has two predicates and two actions. The actions are shared across three supported contexts, with no duplicated completion definition. Moving the guard into a helper would add a wrapper without removing responsibility. Splitting the script would require the Rust generator to stitch files together or users to install several artifacts; neither removes the entry distinction. The existing large option table is unchanged and remains well below 1,000 lines with its extensive reference material included.

## Worked code-judo alternatives

One behavior-preserving inversion would make the registration predicate positive:

```zsh
if [[ $funcstack[1] != _rg ]] && (( $+functions[compdef] )); then
  compdef _rg rg
else
  _rg "$@"
fi
```

By De Morgan's law this selects exactly the same arms: direct sourcing with `compdef` registers, while autoload invocation or absence of `compdef` executes. It still requires two tests and two actions. It deletes no mode, layer, or helper and is not a material improvement, so the review does not recommend this change.

A more aggressive design could make sourcing definition-and-registration only, and have the CI helper call `_rg` explicitly after loading. That requires changing the source-only extraction contract, creates separate loading and invocation steps for an existing consumer, and still needs to distinguish first autoload execution from registration. It is not a demonstrated simplification of this PR and is not an actionable finding.

The useful boundary simplification is in the setup documentation:

```zsh
# Completion initialization belongs in the user's shell configuration.
autoload -Uz compinit
compinit
source <(rg --generate complete-zsh)
```

This uses the existing completion architecture without adding a new deferred-registration hook or implicit global initialization to `_rg`. Existing frameworks can retain their own initialization and source afterward. The worked file-based alternative and remediation text are in `02_faq.md`.

## Focused execution and isolation

All scratch files are outside the checkout in `../thermo-scratch`. The retained driver is `../thermo-scratch/check_zsh.py`, the individual `.zsh` scripts are alongside it, and the final output is `../thermo-scratch/results.txt`. The focused execution command was:

```sh
python3 /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-018/clone-work/thermo-scratch/check_zsh.py
```

The driver obtains the base shell text with `git show`, reads the head shell text, writes temporary scripts and a temporary `_rg` file, and runs `zsh -f` with timeouts. Interactive TAB checks use Python's pseudoterminal support and zsh's real completion functions. It does not invoke a built `rg`, cargo, or network commands.

The host already contains `/usr/share/zsh/vendor-completions/_rg`. Leaving that directory in `fpath` can make a broken registration test appear to succeed. Final runs remove that directory from each shell's `fpath` before initialization:

```zsh
fpath=(${fpath:#/usr/share/zsh/vendor-completions})
```

This is test isolation, not proposed product configuration. The file-based checks add only the scratch directory containing the head `_rg`. `compinit -D` disables completion dump-file writes. The raw encoding placeholder is not exercised by these tests; the selected option-completion path does not call `_rg_encodings`.

## Results and verification status

| Check | Observed result | Status |
| --- | --- | --- |
| Base source after `compinit -D` | Status 1; `_arguments:comparguments:325: can only be called from completion function`; no `rg` mapping | Original failure reproduced |
| Head source after `compinit -D` | Status 0; `_comps[rg]=_rg`; no stderr | Pass |
| Head source from a named shell function after initialization | Status 0; `_comps[rg]=_rg`; no stderr | Pass |
| Head source before initialization | Status 1; `_rg:341: command not found: _arguments`; later `compinit` leaves mapping absent | FAQ precondition failure reproduced |
| Scratch `_rg` present before `compinit` | `_comps[rg]=_rg`; first invocation reaches `_arguments` | Entry dispatch pass |
| Scratch directory added after `compinit` | `_comps[rg]` absent | File-based ordering requirement reproduced |
| Base option dump | 341 lines; status 0; no stderr | Pass |
| Head option dump | 341 lines; status 0; no stderr; exactly matches base output | Compatibility pass |
| Interactive head dynamic source | `rg --generate=complete-zs<TAB>` becomes `rg --generate=complete-zsh ` | Real completion pass |
| Interactive head file autoload | Same TAB expansion | Real completion pass |

The standalone dispatch probe temporarily replaces `_arguments` with a recorder and sees `arguments_called argc=318 stack=_arguments _rg _rg`, confirming that first autoload execution reaches the body. That probe alone is not a completion correctness test; the separate interactive checks exercise the real `_arguments` and TAB expansion.

The dump probe reproduces the source-extraction portion of `ci/test-complete`, including its shell options and `_RG_COMPLETE_LIST_ARGS=1`. It proves shell-side output equivalence, not agreement with the built binary's help options. The full CI script, Rust build, and Rust tests are unavailable under the execution policy and were not attempted.

Verification is limited to zsh 5.8.1. No claim is made about generation through the unavailable built binary, every completion option, historical zsh versions, or every shell framework. These limits do not prevent reproducing the two FAQ failures or checking the supported entry behavior on the installed version.

## Checkout integrity

`git status --porcelain=v1 --untracked-files=all` was empty before and after focused execution. `git diff --quiet`, `git diff --cached --quiet`, and `git diff --check main...review-head` all succeeded. `git ls-files -s | sha256sum` returned `a085bc9b983a0ba89f9860f45e87f874d3fdd8ab6b1f5c6d2e7ec2179ec92404` both before and after. All report and scratch writes were in the work directory.

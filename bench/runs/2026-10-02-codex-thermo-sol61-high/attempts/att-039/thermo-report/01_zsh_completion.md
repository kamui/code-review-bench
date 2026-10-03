# Zsh completion entry point

## Scope and verdict

Reviewed `crates/core/flags/complete/rg.zsh` in full, the unchanged generator in
`crates/core/flags/complete/zsh.rs`, and the sourcing contract in
`ci/test-complete`. No actionable structural or behavioral regression was found
in the shell implementation under the supported initialization conditions.
The FAQ's missing initialization contract is reported separately.

The committed change replaces the unconditional `_rg "$@"` with one dispatch
at lines 437–445. `_rg` at lines 15–410 remains the option-specification and
completion implementation; its encoding and type helpers remain unchanged.
The option-list escape path at lines 345–350 is still inside `_rg`.

## Measurements and source evidence

`git diff main...review-head` shows nine added lines and one removed line in
this script. `git show main:crates/core/flags/complete/rg.zsh | wc -l` reports
637; `wc -l crates/core/flags/complete/rg.zsh` reports 645. No threshold crossing
occurred. A large portion of the file is option data; lines 447–596 carry the
completion reference and the remaining tail carries licensing and editor data.
Splitting this cohesive, embedded script would not be earned by this patch.

`nl -ba ci/test-complete` establishes that `get_comp_args` runs
`(_RG_COMPLETE_LIST_ARGS=1 source $1)` at line 16 without initializing the
completion system. Its `setopt local_options unset` and outer emulation setup
provide the expected test shell context. Running the complete script requires a
built binary and was not permitted. Reading it was sufficient to identify why
the absence-of-`compdef` execution branch matters.

`zsh.rs:23–25` embeds `rg.zsh` and replaces `!ENCODINGS!` with
`super::ENCODINGS.trim_end()`. Scratch fixtures for both pinned commits use
that same substitution with the checked-in `encodings.sh`. This tests the script
actually represented by the generator rather than a different installed rg.

## Entry contract and branching audit

There are three relevant contexts but only two actions:

| Context | Observed discriminator | Required action |
| --- | --- | --- |
| First invocation of an autoloaded `_rg` | `$funcstack[1] == _rg` | Execute `_rg` with forwarded arguments |
| Fresh option-list test shell | No `compdef` function | Execute `_rg`, whose list mode returns before `_arguments` |
| Sourcing after completion initialization | `compdef` exists and caller is not `_rg` | Register `_rg` for `rg` |

The OR condition folds the first two contexts together rather than duplicating
the invocation. `$+functions[compdef]` tests zsh's canonical function table;
it does not spawn an external discovery command. Registration remains in the
shell script that owns completion setup, rather than leaking into the Rust
flag parser or generator. `$@` remains quoted and forwarded only on execution.
No partial-update orchestration or loosely typed model is introduced.

Sourcing without completion initialization still tries to execute `_rg` and
fails outside list mode. This behavior is shared with the base and is necessary
to consider when documenting the new dynamic method; it does not justify
removing the established test entry contract.

## Worked code-judo alternatives

The current architecture already supplies the useful simplification: define the
completion functions once and select execution or registration at the boundary.
The compact, behavior-preserving dispatch is:

```zsh
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

An unconditional `compdef _rg rg` would delete the conditional but stop the
fresh-shell option-list test from executing and fail without `compdef`.
An unconditional `_rg "$@"` restores the original source-time completion error.
Neither is a behavior-preserving simplification.

Moving the registration into a prefix or suffix emitted by `zsh.rs` would make
raw-script sourcing and generated-script sourcing have different contracts.
It would also move shell lifecycle logic across the ownership boundary and
still require detecting autoload execution somewhere. Splitting helpers into
separate files would require shipping and sourcing multiple artifacts instead
of the existing self-contained generated script.

An explicit three-case dispatcher or a named setup helper would merely unpack
the same two actions into more branches or another global function. Negating
the predicate and putting registration first would be a cosmetic reversal,
not a removal of complexity. These alternatives are rejected; there is no
structural blocker and no useful refactor to request here.

## Focused execution and results

All execution ran offline under
`../zsh-review-checks` relative to the report directory. `zsh --version` reports
`zsh 5.8.1 (x86_64-ubuntu-linux-gnu)`. The fixtures live in
`main/_rg` and `review-head/_rg`; the harness is `check.zsh`.

The reproducible harness command shape is:

```sh
zsh -f ../zsh-review-checks/check.zsh CASE ../zsh-review-checks/REVISION/_rg
```

The harness supplies a scratch `rg` function that prints the fixture for the
process-substitution cases. It does not invoke a separately installed binary.
`compinit -D -i` prevents completion-dump writes. Pre-existing rg mappings are
removed before checking explicit registration.

| Case | Base | Head |
| --- | --- | --- |
| `source` after `compinit` | Source status 1; completion-context error; no mapping | Source status 0; `rg` maps to `_rg` |
| `wrapper` after `compinit` | Same failure | Wrapper status 0; `rg` maps to `_rg` |
| `repeated` after `compinit` | Completion-context error twice | Both loads succeed; mapping remains `_rg` |
| `dump` without `compinit` | Status 0, 341 lines | Status 0, same 341 lines byte for byte |
| `autoload` with list mode | Status 0, 341 lines | Status 0, same 341 lines byte for byte |

`results.json` and the corresponding `.stdout` and `.stderr` files retain
these outputs. The base error was
`_arguments:comparguments:325: can only be called from completion function`.

A separate Python PTY check attached a controlling terminal to `zsh -f`, loaded
the head fixture either directly or through `autoload -Uz _rg`, and entered
`rg --generate=complete-zs<TAB>`. Both paths expanded the buffer to
`rg --generate=complete-zsh` with a trailing space. Transcripts are
`interactive-source.txt` and `interactive-autoload.txt`; their extracted
completion output is in `interactive-results.json`.

An initial PTY attempt without a controlling terminal failed to shut down
cleanly and did not count as verification. The corrected attempt used
`pty.fork()` and deterministic cleanup; only its completed transcripts support
the interactive result above.

## Verification limits and disposition

The option-list checks establish unchanged emitted specifications and preserved
entry behavior, not agreement with the output of a newly built rg binary.
The Rust build and full `ci/test-complete` script were unavailable and were not
run. Historical zsh compatibility was reviewed from the unchanged code, not
executed on old versions. No startup performance measurement was attempted.

Retain the implementation and correct the FAQ's executable examples and ordering
contract. No extraction, wrapper, additional mode, or cross-layer change is
recommended.

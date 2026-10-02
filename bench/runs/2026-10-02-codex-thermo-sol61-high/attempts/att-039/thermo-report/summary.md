# Thermo-nuclear review: ripgrep #2957

## Verdict

Request changes to the zsh installation instructions. The completion implementation
passes the structural review and the focused shell checks. There are two
reproducible documentation defects in the new setup instructions; neither
requires restructuring the completion implementation.

This review covers the committed range
`79cbe89deb1151e703f4d91b19af9cdcc128b765..855bfa6cdae4f4fe8762f892fc4957635397083e`,
inspected with `git diff main...review-head`. The patch changes two files with
29 additions and four deletions. No repository files were modified.

## Actionable findings

### [P2] Remove the prompt marker from the startup-file snippet

In `FAQ.md:131–135`, the text instructs users to add the following code to `.zshrc`, but line 135 starts with a literal `$`: `$ source <(rg --generate complete-zsh)`. Copied into the startup file as directed, zsh tries to execute a command named `$`, returns status 127 with `command not found: $`, and never sources or registers the completion. This is an executable configuration example, unlike the earlier interactive command transcripts. Remove the leading `$` so the snippet reads `source <(rg --generate complete-zsh)`, consistent with the preceding prompt-free `fpath` configuration example.

The failure was reproduced with the head completion fixture and an initialized
completion system; the corrected command registered `_rg` successfully.
Full evidence and the worked remedy are in [02_faq.md](02_faq.md).

### [P2] Specify the initialization order for the two zsh setup methods

In `FAQ.md:124–135`, both new `.zshrc` instructions omit their dependency on the position of `compinit`. The directory method must extend `fpath` before the existing `compinit` call scans completion files, while the dynamic method must source the script after that call has defined `compdef`. Adding the directory at the end of a startup file that already ran `compinit` leaves `rg` unregistered; sourcing before initialization reaches the compatibility fallback in `rg.zsh:441–442` and fails with `command not found: _arguments`. Both failures were reproduced in a shell without another installed `_rg` completion. State the ordering next to each method, and show completion initialization for users whose startup file does not already perform it. Keep the test-compatible fallback in the completion script; fix the installation contract in the FAQ.

Both correct orderings registered `rg` to `_rg` in the same isolated fixture.
Full evidence and worked startup layouts are in [02_faq.md](02_faq.md).

## Structural assessment

The new branch is at the canonical shell entry boundary, immediately after the
completion functions. It distinguishes genuine autoload execution and the
existing option-list test path from registration during direct sourcing.
It uses zsh's existing function table and call stack, preserves argument
forwarding, and introduces no helper, persistent mode, duplicated argument
specifications, or Rust-layer coupling. This is a justified dispatch boundary,
not scattered special-case growth.

A two-branch dispatch already folds the two execution contexts together. Moving
registration into the Rust generator, splitting the script into separate loading
modes, or adding a helper would increase the number of representations and moving
pieces. No dramatic behavior-preserving simplification was found. The worked
alternatives and their costs are in [01_zsh_completion.md](01_zsh_completion.md).

The completion script grows from 637 to 645 lines. The FAQ grows from 1046 to
1063 lines and was already above 1000 lines. Neither crosses the skill's
1000-line threshold. Most of the shell script is declarative option data,
embedded reference material, and licensing; these nine added lines do not
justify a decomposition project.

## Verification

Focused offline checks used zsh 5.8.1 and scratch copies outside the checkout.
The copies reproduce `zsh.rs` generation by substituting the checked-in encoding
list for `!ENCODINGS!`; no built `rg` binary was used.

Dynamic sourcing after `compinit`, sourcing from a wrapper, and repeated sourcing
all succeeded and registered `_rg`. Genuine interactive TAB completion expanded
`rg --generate=complete-zs` to `rg --generate=complete-zsh` through both directly
sourced and autoloaded head fixtures. The fresh-shell option-list path and the
autoload option-list path each produced 341 lines, byte-identical between base
and head. The base dynamic-sourcing failure was also reproduced, confirming the
feature fixes its intended trigger.

The literal FAQ command and the incorrect initialization orderings failed as
described in the findings. Corrected orderings succeeded. No timing claims were
benchmarked, and no historical zsh versions were tested.

Cargo build, cargo test, and the complete `ci/test-complete` script were
unavailable under the execution policy and were not attempted. Consequently,
the shell option-list compatibility check is not a fresh comparison against
binary help output. Verification details and scratch artifact locations are in
the subsystem reports.

## Remediation sequence

1. Remove the prompt marker from the dynamic `.zshrc` example.
2. Explain that the `fpath` edit precedes the user's existing `compinit` call and
   the dynamic source command follows it. Include a minimal initialization
   example for a startup file that has none; do not tell users to duplicate an
   existing initialization.
3. Recheck the literal configuration examples in a fresh zsh session. The
   isolated ordering checks already demonstrate the proposed layouts.

No completion-function refactor is required. The two documentation edits can
be made together without changing completion dispatch or option data.

## Questions

There are no open questions requiring user clarification.

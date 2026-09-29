# Thermo-nuclear code quality review — BurntSushi/ripgrep#2957

Range: `79cbe89..855bfa6` (`git diff main...review-head`), 2 files, +29/−4. Detail files: `01_zsh_completion_script.md` and `02_faq_documentation.md`, both in this directory.

## Verdict

Not approved as-is under this skill's bar, though the change is small and mostly sound. Nothing here is a size or layering problem: no file is near 1000 lines, and the change is 9 lines of script plus documentation. The real issues are one behavioral gap inside the new three-way conditional and a broken documentation snippet. The first of them keeps the user-visible failure from issue #2956 alive for a class of users, and the FAQ now tells those users to do exactly that.

Limits of this review: I could not build the crate or run `ci/test-complete`. I verified the shell behavior by running the head version of `rg.zsh` in scratch `zsh -f` sessions. The evidence is in the detail files.

## Findings

**1. The `compdef`-absent arm reintroduces the original failure and carries a harness accommodation in production code** (`crates/core/flags/complete/rg.zsh:441`, detail 01 Finding 1). The tail of the script is now `if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then _rg "$@"; else compdef _rg rg; fi`. When the script is sourced before `compinit` has run, `compdef` does not exist, so the second half of the condition sends control to `_rg "$@"`. That is exactly what issue #2956 reported. I ran this: `source rg.zsh` in a fresh shell without `compinit` prints `_rg:341: command not found: _arguments` and returns 1. The arm exists because `ci/test-complete` sources the script without a completion system and relies on `_RG_COMPLETE_LIST_ARGS` making `_rg` dump its specs and return. A test harness's needs are therefore encoded as a silent fallback in the shipped script, and it converts "no completion system yet" into a runtime error. The remedy is to give the three situations explicit outcomes: autoloaded runs `_rg`, sourced with `compdef` available registers it, and sourced with no completion system does nothing. The harness case should be gated on `_RG_COMPLETE_LIST_ARGS`, the variable it already sets, instead of on the absence of `compdef`. That deletes the misuse of `compdef` as a proxy for "am I in a test?", and the no-`compdef` case becomes a quiet no-op. Status: confirmed by execution.

**2. The new FAQ `.zshrc` snippet includes a `$ ` prompt marker** (`FAQ.md:135`, detail 02 Finding 1). The paragraph above it tells the reader to add the line to `$HOME/.zshrc`, but the block begins `$ source <(rg --generate complete-zsh)`. Pasted verbatim, a leading `$` is parsed as a command and fails at every shell start. The `fpath=` block a few lines earlier is written correctly. The remedy is to remove the marker so file-content blocks and terminal-transcript blocks are distinguishable. Status: confirmed by reading.

**3. The docs promote the source one-liner without its ordering precondition** (`FAQ.md:132-138`, detail 02 Finding 2). Finding 1's failure is only reachable because the FAQ tells users to put the line in their zshrc without saying it must come after `compinit`. The `fpath=` line at 128 has the mirror-image precondition: it must run before `compinit`. Either fix the script so ordering does not matter, or add one sentence per option. Fixing the script is the cleaner path and removes the documentation burden. Status: confirmed by execution.

**4. The comment above the new conditional says the opposite of what the code does** (`rg.zsh:437-440`, detail 01 Finding 2). "Don't run the completion function when being sourced by itself" is followed by code that does run `_rg` in two of three cases. The links to the issue and PR carry the rationale that belongs in the comment. Replace it with one line per arm. Status: confirmed by reading.

**5. Grammar defect in the new caveat** (`FAQ.md:137-138`, detail 02 Finding 3). "while this approach is easier to setup, is generally slower" has a missing subject, and "setup" should be "set up". This is minor, and I list it only because it sits in the paragraph that carries the performance caveat. Status: confirmed by reading.

## Code-judo summary

The dramatic simplification is to stop deriving "test mode" from the absence of `compdef` and to make the tail a plain dispatch on the situations that actually occur. If the third arm is a no-op, then the tail is three short lines with no negations and no second reason to enter the first arm. The FAQ can then be reframed as two self-contained options, each with its own precondition. Both changes delete concepts rather than rearranging them.

## Proposed remediation sequence

1. Change the tail of `rg.zsh` to the three-arm form and gate the harness on `_RG_COMPLETE_LIST_ARGS`. Re-run `ci/test-complete` once an `rg` binary is available, because I could not.
2. Rewrite the comment above it so each arm is described.
3. Remove `$ ` from the `.zshrc` block, fix the caveat grammar, and add the `compinit` ordering sentence to both zsh options if the script is not made order-independent.

## What was checked and is fine

The `$funcstack[1] == _rg` test correctly recognises the autoload-from-`fpath` case. Sourcing after `compinit` registers `rg` as expected, whether from top level or inside a function. The `#compdef rg` header is untouched. `$+functions[compdef]` is the right idiom rather than `type compdef`. No file size, layering or wrapper concerns.

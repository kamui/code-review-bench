# Review blind-f70011

### Item 1
Location: crates/core/flags/complete/rg.zsh:437-446
Claim: **1. The `compdef`-absent arm reintroduces the original failure and carries a harness accommodation in production code** (`crates/core/flags/complete/rg.zsh:441`, detail 01 Finding 1). The tail of the script is now `if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then _rg "$@"; else compdef _rg rg; fi`. When the script is sourced before `compinit` has run, `compdef` does not exist, so the second half of the condition sends control to `_rg "$@"`. That is exactly what issue #2956 reported. I ran this: `source rg.zsh` in a fresh shell without `compinit` prints `_rg:341: command not found: _arguments` and returns 1. The arm exists because `ci/test-complete` sources the script without a completion system and relies on `_RG_COMPLETE_LIST_ARGS` making `_rg` dump its specs and return. A test harness's needs are therefore encoded as a silent fallback in the shipped script, and it converts "no completion system yet" into a runtime error. The remedy is to give the three situations explicit outcomes: autoloaded runs `_rg`, sourced with `compdef` available registers it, and sourced with no completion system does nothing. The harness case should be gated on `_RG_COMPLETE_LIST_ARGS`, the variable it already sets, instead of on the absence of `compdef`. That deletes the misuse of `compdef` as a proxy for "am I in a test?", and the no-`compdef` case becomes a quiet no-op. Status: confirmed by execution.
Consequence: —
Fix: —

### Item 2
Location: FAQ.md:135
Claim: **2. The new FAQ `.zshrc` snippet includes a `$ ` prompt marker** (`FAQ.md:135`, detail 02 Finding 1). The paragraph above it tells the reader to add the line to `$HOME/.zshrc`, but the block begins `$ source <(rg --generate complete-zsh)`. Pasted verbatim, a leading `$` is parsed as a command and fails at every shell start. The `fpath=` block a few lines earlier is written correctly. The remedy is to remove the marker so file-content blocks and terminal-transcript blocks are distinguishable. Status: confirmed by reading.
Consequence: —
Fix: —

### Item 3
Location: FAQ.md:132-138
Claim: **3. The docs promote the source one-liner without its ordering precondition** (`FAQ.md:132-138`, detail 02 Finding 2). Finding 1's failure is only reachable because the FAQ tells users to put the line in their zshrc without saying it must come after `compinit`. The `fpath=` line at 128 has the mirror-image precondition: it must run before `compinit`. Either fix the script so ordering does not matter, or add one sentence per option. Fixing the script is the cleaner path and removes the documentation burden. Status: confirmed by execution.
Consequence: —
Fix: —

### Item 4
Location: crates/core/flags/complete/rg.zsh:437-440
Claim: **4. The comment above the new conditional says the opposite of what the code does** (`rg.zsh:437-440`, detail 01 Finding 2). "Don't run the completion function when being sourced by itself" is followed by code that does run `_rg` in two of three cases. The links to the issue and PR carry the rationale that belongs in the comment. Replace it with one line per arm. Status: confirmed by reading.
Consequence: —
Fix: —

### Item 5
Location: FAQ.md:137-138
Claim: **5. Grammar defect in the new caveat** (`FAQ.md:137-138`, detail 02 Finding 3). "while this approach is easier to setup, is generally slower" has a missing subject, and "setup" should be "set up". This is minor, and I list it only because it sits in the paragraph that carries the performance caveat. Status: confirmed by reading.
Consequence: —
Fix: —

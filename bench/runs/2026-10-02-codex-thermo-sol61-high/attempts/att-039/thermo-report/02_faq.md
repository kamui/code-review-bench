# FAQ completion installation contract

## Scope and verdict

Reviewed the completion section in `FAQ.md:91–150`, including the new directory
registration instructions and dynamic source alternative. The wording correctly
keeps the directory approach recommended and mentions the dynamic method's
startup cost. Two installation defects require correction; both are actionable
findings in `summary.md`.

`git show main:FAQ.md | wc -l` reports 1046 lines and `wc -l FAQ.md` reports
1063. The FAQ was already above 1000 lines. This patch does not trigger the
skill's threshold-crossing rule, and this localized documentation addition
is not evidence for splitting the FAQ.

## Finding: prompt marker in executable configuration

At `FAQ.md:131–135`, the prose says to add the following to `.zshrc`, followed by:

```zsh
$ source <(rg --generate complete-zsh)
```

The preceding interactive generation examples use `$` to identify the prompt,
but the adjacent startup-file `fpath` example at line 128 correctly omits it.
Here the requested action is to paste configuration into a file. The leading
`$` is a command word; it is not syntactic decoration to zsh. The shell runs the
producer in the process substitution but never executes the `source` builtin.

The focused `faq_literal` check initialized completion first and removed an
existing `rg` mapping, avoiding unrelated missing-initialization failures.
The exact documented line then produced `command not found: $`, command status
127, and `mapping=missing`. The corrected line in the `source` check returned
zero and gave `mapping=_rg` with the same head fixture. The harness reports the
command status explicitly; its overall process exit is zero because it prints
the outcome afterward.

Remove the marker:

```zsh
source <(rg --generate complete-zsh)
```

This is a behavior fix for the copied configuration, not a cosmetic consistency
request. No runtime implementation change is necessary.

## Finding: unstated completion initialization order

The new instructions at `FAQ.md:124–135` tell users where to place code but do
not state which side of completion initialization each method requires.
The two methods have opposite ordering requirements.

`fpath` is consumed when `compinit` scans completion files and builds command
mappings. Adding a directory after that scan does not automatically register
its `_rg` function. Direct sourcing uses `compdef` to create the mapping, so it
must happen after completion initialization. When `compdef` is absent, the
new dispatcher at `rg.zsh:441–442` executes `_rg`; its ordinary path needs
`_arguments`, which a fresh shell has not initialized.

The first `before_init` check encountered an installed `_rg` in the system
completion directories: source failed, but the subsequent `compinit` found that
separate installation. That environment cannot establish whether dynamic
sourcing registered the function. A second ordering harness removed directories
containing another `_rg` from `fpath` before each case, representing the FAQ's
stated use case where completions were not installed. It did not alter any
system file. This isolation avoids attributing the package manager's mapping
to the new script.

Run the isolated checks with:

```sh
zsh -f ../zsh-review-checks/ordering.zsh CASE ../zsh-review-checks/review-head/_rg
```

The harness and `ordering-results.json` are outside the checkout. Results were:

| Case | Output | Diagnostic |
| --- | --- | --- |
| Source before `compinit` | `source_status=1 mapping=missing` | `command not found: _arguments` |
| Source after `compinit` | `source_status=0 mapping=_rg` | None |
| Add fixture directory before `compinit` | `mapping=_rg` | None |
| Add fixture directory after `compinit` | `mapping=missing` | None |

The `_rg:341` error location in the source-before case is the internal function
line number emitted by zsh, not a source-file anchor; the relevant unchanged
call is at `rg.zsh:356`.

## Worked code-judo remedy: document the lifecycle once

The correction belongs at the installation boundary. Keeping the runtime
fallback preserves option-list testing; adding runtime modes, deferred hooks,
or repeated checks would make the code less direct. Explain that the reader
should place each command relative to their existing `compinit` call. For a
user without initialization, give a minimal complete layout.

After the existing interactive commands generate `$HOME/.zsh-complete/_rg`,
the directory-based startup layout is:

```zsh
# Place this before your existing completion initialization.
fpath=("$HOME/.zsh-complete" $fpath)
autoload -Uz compinit
compinit
```

The dynamic alternative is:

```zsh
# Source after your existing completion initialization.
autoload -Uz compinit
compinit
source <(rg --generate complete-zsh)
```

The prose should clarify that `autoload -Uz compinit; compinit` is needed only
if the shell setup does not already initialize completions. Users with a
framework should locate its initialization and place the directory edit before
it or the source command after it, without duplicating initialization.

These layouts reuse the completion system's canonical lifecycle instead of
adding an rg-specific loading abstraction. They remove ambiguity and repair
both installation paths in one documentation edit. The isolated correct-order
checks demonstrate their behavior; no checkout edits were applied.

## Verification status and limits

Both findings are reproduced on zsh 5.8.1 with generated-script equivalents
from the pinned source and a scratch producer function. Outputs are in
`../zsh-review-checks/results.json` and `ordering-results.json`.
The compilation and complete CLI-to-completion option comparison were not
available. The recommendation does not depend on a startup-time benchmark or
on assumptions about a particular shell framework.

No open question is needed to implement these corrections. The documentation
can state the two lifecycle requirements without prescribing a framework or
changing the established recommended method.

# FAQ completion setup

## Scope and measurements

This detail covers the changes to `FAQ.md:95–139`, read using `nl -ba FAQ.md | sed -n '75,165p'` and `git diff main...review-head`. The PR corrects a spelling mistake, recommends installing a generated `_rg` on `fpath`, adds the missing `fpath` assignment, and documents dynamic sourcing with a startup-cost caveat. It adds 20 lines and removes three. The complete FAQ grows from 1,046 to 1,063 lines.

The FAQ already exceeded 1,000 lines at the base. This change does not cross the skill's threshold, and splitting a general FAQ to accommodate a small setup explanation would not fix either failure below. The prose keeps the command-specific setup in its existing canonical documentation location. The startup-cost discussion is appropriately qualitative; the review makes no new benchmark claim.

## Finding 1: Specify the completion initialization order

`FAQ.md:131–135` introduces a source command to place in `.zshrc` but never establishes the initialization precondition. The new script only registers its function when `compdef` exists. If the user places the command above their existing completion initialization, or has no initialization, the absence-of-`compdef` arm runs `_rg`, which fails at `_arguments`. Calling `compinit` afterward does not register an arbitrary already-defined `_rg` whose file was never installed on `fpath`.

The adjacent file-based recipe has the reverse ordering requirement: its new `fpath` assignment at line 128 needs to precede `compinit`, so initialization sees the generated file and registers it. Appending that assignment to an otherwise initialized `.zshrc` cannot make `compinit` discover a file it has already finished scanning. A package-provided `_rg` can mask both missing-registration symptoms, which is why the focused tests remove the host's vendor-completion directory.

The focused script for the dynamic failure is equivalent to:

```zsh
fpath=(${fpath:#/usr/share/zsh/vendor-completions})
source <(cat /path/to/head/crates/core/flags/complete/rg.zsh)
print -r -- "source_status=$? compdef_present=$+functions[compdef]"
autoload -Uz compinit
compinit -D
print -r -- "mapping_after_compinit=${_comps[rg]-absent}"
```

The actual source path and retained runnable script are in `../thermo-scratch/head_source_before_compinit.zsh`. The output is:

```text
source_status=1 compdef_present=0
mapping_after_compinit=absent
```

Stderr is `_rg:341: command not found: _arguments`. The line number is relative to the function body. With initialization placed first, the same source command returns status 0, produces no stderr, and sets `_comps[rg]=_rg`. Real TAB completion also passes. A separate probe initializes before adding the scratch `fpath` directory and observes `mapping=absent`.

This is a reproduced documentation defect in the new setup path. It is not a reason to make the generator secretly initialize zsh, remove the CI fallback, or add a deferred registration mode. Make the consumer boundary explicit in the FAQ and retain the current production entry code.

## Finding 2: Remove the prompt marker from the startup-file snippet

`FAQ.md:131–135` explicitly says to add the block to `.zshrc`, but the block is `$ source <(rg --generate complete-zsh)`. The initial `$` is a terminal prompt convention, not valid prefix syntax for a shell configuration command. A reader who pastes the provided configuration line gets a failure even when completion initialization is correct.

The focused test initializes completions, then evaluates the shown line with `cat` producing the script instead of an unavailable built binary:

```zsh
fpath=(${fpath:#/usr/share/zsh/vendor-completions})
autoload -Uz compinit
compinit -D
$ source <(cat /path/to/head/crates/core/flags/complete/rg.zsh)
print -r -- "source_status=$? mapping=${_comps[rg]-absent}"
```

The retained runnable script is `../thermo-scratch/literal_faq_source.zsh`. It prints `source_status=127 mapping=absent` and reports `command not found: $` on stderr. The process substitution is not the problem: zsh selects `$` as the command to execute rather than invoking the `source` builtin.

Remove `$` from this startup-file snippet. The earlier commands used to create directories and generate files are introduced as interactive terminal operations and can keep their prompt markers. The difference here follows from the explicit instruction to put the block in a configuration file, not a general objection to prompt conventions.

## Worked remediation

For the recommended file-based method, retain the commands that create and generate `_rg`, then explain that the `fpath` addition belongs before the shell's completion initialization. For a standalone setup, show the following as `.zshrc` content:

```zsh
fpath=($HOME/.zsh-complete $fpath)
autoload -Uz compinit
compinit
```

For dynamic sourcing, explain that the source line belongs after initialization. A standalone example is:

```zsh
autoload -Uz compinit
compinit
source <(rg --generate complete-zsh)
```

Suggested explanatory prose is: “If your shell configuration already initializes completions, place the `fpath` addition before that initialization, or place the dynamic source command after it. Otherwise, initialize completions with `autoload -Uz compinit` and `compinit` as shown.” This avoids implying that users of frameworks need a second initialization call. Both blocks are configuration text and contain no terminal prompt marker.

This change reduces the number of unstated assumptions readers must infer. It reuses the canonical completion initializer and makes each entry contract visible at its consumer, with no new shell hook or wrapper.

## Verification status and limits

Both failures were reproduced under zsh 5.8.1, outside the checkout and with package-installed `_rg` completions excluded. Correctly ordered dynamic and file-based loading passed real interactive TAB completion. Full output and the driver are retained at `../thermo-scratch/results.txt` and `../thermo-scratch/check_zsh.py`.

The tests use checked-in zsh text via `cat`. They do not validate the unavailable Rust build, generation by a built `rg`, or framework-specific initialization. The generator's unchanged source shows that its only text substitution is the encoding list, which these option-completion tests do not use. No network access or checkout mutation was performed.

There are no additional actionable FAQ findings or open questions. The grammar of the startup-cost sentence and stylistic formatting choices do not warrant separate review comments.

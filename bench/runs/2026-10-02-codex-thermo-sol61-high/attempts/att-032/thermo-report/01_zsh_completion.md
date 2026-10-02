# Zsh completion implementation

## Scope and finding status

This subsystem covers `crates/core/flags/complete/rg.zsh`, its unchanged generator in `crates/core/flags/complete/zsh.rs`, the encoding include, and the entry contract in `ci/test-complete`. The reviewed change is the bottom-of-script dispatcher at head lines 437–445. There are no actionable structural findings in this subsystem.

The review used the pinned local base and head only. Packet review comments were not used as proof of behavior or design quality. No additional reviewer was started, and no network lookup was performed.

## Measurements and source evidence

`git diff main...review-head` shows one code hunk: an unconditional `_rg "$@"` becomes an `if` choosing execution or registration. The hunk is +9/−1. `wc -l crates/core/flags/complete/rg.zsh` reports 645; `git show main:crates/core/flags/complete/rg.zsh | wc -l` reports 637. This does not cross the skill's 1,000-line boundary. The new dispatch adds one `if`, one `||`, and two action branches, all in the preexisting entry point. No flag specifications or helper bodies change.

The template already defines `_rg` at line 15, `_rg_encodings` at line 412, and `_rg_types` at line 424. The main function has a deliberate non-completion inspection exit at lines 345–350: `_RG_COMPLETE_LIST_ARGS` matching `(1|t*|y*)` prints the argument array and returns before `_arguments` runs. `ci/test-complete:8–17` invokes precisely that path in a subshell, without initializing the completion system. Its enclosing script is a fresh zsh process. This is why replacing the old call with unconditional registration would break an existing contract.

`zsh.rs:21–22` embeds `rg.zsh` and replaces `!ENCODINGS!` using `ENCODINGS.trim_end()`. It does not wrap the script in another execution layer. The shell template is therefore the canonical place to decide whether loading should register a function or invoke it. Neither a CLI mode nor a Rust-side registration wrapper is needed.

The first line remains `#compdef rg`, preserving discovery by `compinit`. Local zsh's `compinit` source defines `compdef` and, at lines 496–515, scans the current `fpath`, reads the first-line metadata, and registers/autoloads the named function. This source was read as runtime implementation evidence, not as repository guidance. The default `compdef -a` autoload path uses `autoload -Uz`. A filename `_rg` on the completion path invokes the script in the `_rg` function context; the new first predicate preserves execution in that context even though `compdef` is already defined.

## Supported entry contexts

| Context | Predicate/action | Required result | Verification |
| --- | --- | --- | --- |
| First invocation of autoloaded `_rg` | `funcstack[1] == _rg`; call `_rg "$@"` | Define the actual functions and perform the first completion | Real Tab completion passed |
| Later invocation of already defined `_rg` | Calls the function body directly | Perform completion normally | Second real Tab completion passed |
| Direct source after `compinit` | Neither execution predicate holds; `compdef _rg rg` | Define functions and register completion without invoking `_arguments` during startup | Source returned 0; mapping and real Tab completion passed |
| Argument-list inspection without the completion system | `compdef` is absent; call `_rg "$@"` | Preserve the script's existing specification dump | 341 lines, status 0 |
| Direct source before `compinit` | `compdef` is absent; call `_rg "$@"` | Unsupported initialization order; document prerequisite | Status 1; no later mapping in isolated framework paths |

The last row is the boundary underlying the documentation finding. It does not make the centralized dispatch an unjustified structural regression. Supporting direct sourcing assumes a usable zsh completion system; the code has not promised to initialize the entire system itself.

One extra check sourced the argument-list path after initializing completion. It returns no specification lines because it registers instead. This is not the environment of `ci/test-complete`, which launches a separate zsh without `compinit`. Reproducing that different environment does not establish a regression in the current CI contract.

## Focused execution

All harnesses are outside the clone, in `../thermo-checks/`. They run with `zsh -f`, and initialization uses `compinit -u -D -d "$work/no-dump"`, where the scratch dump path does not exist. This prevents loading user zsh configuration or a cached user completion dump. `-u` is a fixture choice that avoids security prompts; it is not a proposed user setup instruction.

The scratch generated `_rg` was constructed by reading the template and encoding file, substituting the encoding text, and writing the result outside the clone. This follows the unchanged Rust generator's observed string replacement. The harness's `rg` function returns this text for the source test; it does not emulate ripgrep search behavior. The comparison deliberately does not claim CLI execution or a compiled generator integration test.

The main noninteractive checks are reproducible with:

```sh
zsh -f ../thermo-checks/check.zsh
zsh -f ../thermo-checks/order.zsh
zsh -f ../thermo-checks/base.zsh
zsh -f -n crates/core/flags/complete/rg.zsh
git diff --check main...review-head
```

Run these commands from the clone; the harnesses use absolute paths. `check.zsh` uses the installed default completion directories for broad checks. Because those directories include a packaged `_rg`, its before-initialization result is not evidence about absent packaged completion. `order.zsh` supplies the isolated reproduction: its `fpath` contains only `/usr/share/zsh/functions/Completion` and `/usr/share/zsh/functions/Completion/Base`, which contain no packaged `_rg`. That distinction prevents a system installation from masking the documentation defect.

`check.zsh` confirms that a valid direct source returns 0 and maps `rg` to `_rg`, that the main body reaches a stubbed `_arguments` with 318 arguments, and that two explicit autoload invocations return 0. It also confirms the 341-line inspection dump. The stub is solely an entry-path check; it is supplemented by actual completion, rather than treated as end-to-end proof.

For real completion, start a fresh `zsh -f -i` in `thermo-checks` and execute:

```zsh
source "$PWD/interactive-setup.zsh" direct
```

Type `rg --generate=complete-zs`, press Tab, then press Ctrl-X Ctrl-G. The helper records and clears the ZLE buffer. Repeat in a separate fresh shell with `autoload` instead of `direct`, pressing Tab twice on separate input buffers. These exact local checks recorded:

```text
direct BUFFER=rg --generate=complete-zsh
autoload BUFFER=rg --generate=complete-zsh
autoload BUFFER=rg --generate=complete-zsh
```

The logs retain the trailing completion space. Before the autoload test the function definition printed `builtin autoload -XU`, confirming that the first completion tested the cold loader rather than a function already installed by sourcing. There was no `_arguments` error in any of these valid completion contexts.

`base.zsh` sources a scratch copy obtained with `git show main:crates/core/flags/complete/rg.zsh`. After initialization, the baseline produces `_arguments:comparguments:325: can only be called from completion function`, returns status 1, and leaves the command mapping absent. The head corrects precisely this source-time invocation problem.

## Worked code-judo alternatives

The minimal behavior model is an execute-or-register decision at the script boundary. The current implementation is already that model:

```zsh
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

Removing the condition and keeping only `_rg "$@"` restores the source-time error demonstrated by the baseline. Keeping only `compdef _rg rg` loses first-invocation completion and the inspection path. Checking only `compdef` availability causes a cold autoload invocation to register rather than perform the requested completion. Checking only `funcstack` drops the non-completion-system inspection contract. Each apparent simplification deletes a supported behavior.

A dedicated dispatcher helper would retain exactly the same predicates and actions but introduce another globally named shell function. Moving the check to the Rust generator would add a generated wrapper while leaving the checked-in file's direct CI use to be solved separately. Neither move reduces the state model, and both separate related lifecycle policy from its current canonical entry point. They do not meet the skill's criterion of deleting complexity.

Replacing the missing-`compdef` predicate with explicit recognition of the inspection environment variable would couple this boundary to the function's existing variable interpretation and need another policy for source-before-initialization. That could be considered if initialization behavior later becomes a feature requirement, but it is not an obvious behavior-preserving simplification for this PR. No mandatory refactor is justified on that basis.

The recommended code-judo move instead makes the lifecycle explicit in the documentation, as worked in `02_zsh_documentation.md`. Preserve this small dispatch and avoid extra production layers.

## Verification status and limits

Source analysis, focused shell execution, real ZLE completion, syntax checking, and diff-whitespace checking are complete. Installed zsh is 5.8.1. The full help-versus-completion comparison in `ci/test-complete` was not run because it needs a built `rg`. Cargo build/test and older zsh execution were unavailable and were not attempted. No claim of broader Rust validation or all-version shell compatibility is made.

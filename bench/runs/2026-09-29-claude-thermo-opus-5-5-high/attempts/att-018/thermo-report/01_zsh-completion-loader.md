# 01 — zsh completion loader trailer (`crates/core/flags/complete/rg.zsh`)

Scope: the nine-line trailer that replaced the unconditional `_rg "$@"` at the end of
`crates/core/flags/complete/rg.zsh` (lines 437–445 at `review-head`). The generator
(`crates/core/flags/complete/zsh.rs`) embeds this file verbatim via `include_str!` and only
substitutes `!ENCODINGS!`, so the trailer ships unchanged in both the release archive `_rg`
file and in `rg --generate complete-zsh` output.

## The change

```zsh
# Don't run the completion function when being sourced by itself.
#
# See https://github.com/BurntSushi/ripgrep/issues/2956
# See https://github.com/BurntSushi/ripgrep/pull/2957
if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then
  _rg "$@"
else
  compdef _rg rg
fi
```

## Finding 1.1 — The trailer infers three loading contexts from two incidental signals, and the test-only branch also swallows a real user path

The file is now loaded in three distinct ways, each of which needs different behaviour:

1. **Autoloaded by compsys** from `fpath` (the file body *is* the body of the function
   `_rg`): define helpers, then run `_rg "$@"` to produce completions.
2. **Sourced by a user** (`source <(rg --generate complete-zsh)`) after `compinit`: define
   the functions and register them with `compdef _rg rg`.
3. **Sourced by `ci/test-complete`** (`( _RG_COMPLETE_LIST_ARGS=1 source $1 )`), where no
   completion system is loaded: run `_rg` so that it dumps its `_arguments` specs.

The trailer does not model these contexts; it guesses them. Context 1 is detected by
comparing `$funcstack[1]` with the literal string `_rg`, which couples correctness to the
*installed file name* rather than to how the file is being executed. Context 3 is detected
by the *absence* of `compdef`, which is not a property of the test harness at all — it is a
property of any shell in which `compinit` has not run yet. So the branch that exists only to
keep `ci/test-complete` working is also taken whenever a user sources the script before
`compinit` in their `.zshrc` (a common ordering mistake, and one the FAQ does not warn
about — see `02_faq-docs.md`). In that case the script calls `_rg` at shell start-up and
fails exactly the way issue #2956 described, just with a different message. The leading
comment ("Don't run the completion function when being sourced by itself") is also
inaccurate: in the no-`compdef` branch it does exactly that.

This is the skill's "one-off branch bolted onto a flow" smell in miniature: a test-harness
mode is encoded implicitly in production code via an unrelated signal, and the only
documentation for the dispatcher is two URLs.

### Evidence (verified; zsh 5.8.1, scratch scripts under `clone-work/scratch`, nothing written to the clone)

Context-signal probe (a file that prints its own context, loaded several ways):

```
autoload _probe:          funcstack[1]=_probe          eval_ctx=loadautofunc
autoload renamed:         funcstack[1]=_probe_renamed  eval_ctx=loadautofunc
sourced:                  funcstack[1]=fp/_probe       eval_ctx=file
sourced via procsub:      funcstack[1]=/proc/self/fd/11 eval_ctx=file
sourced inside function:  funcstack[1]=fp/_probe       eval_ctx=file
```

Behaviour of the PR's `rg.zsh` (unmodified copy of the file at `review-head`):

```
$ zsh -f -c 'source rg.zsh; print rc=$?'                       # sourced before compinit
_rg:341: command not found: _arguments
rc=1

$ zsh -f -c 'autoload -Uz compinit; compinit -D -u; source rg.zsh; print ${_comps[rg]}'
_rg                                                            # sourced after compinit: OK

$ # same file installed on fpath as _ripgrep (still "#compdef rg")
$ zsh -f -c 'fpath=(fp3 $fpath); autoload -Uz compinit; compinit -D -u;
             print comps=${_comps[rg]}; _ripgrep; print after=${_comps[rg]}'
comps=_ripgrep
after=_rg                  # first invocation re-registered instead of completing

$ zsh -f -c '( _RG_COMPLETE_LIST_ARGS=1 source rg.zsh ) | head -3'   # harness path: OK
+
(exclusive)
(: * -)-h[display help information]
```

The renamed-file case is an edge (distributions normally install `_rg`), but it
demonstrates that the `funcstack` check tests a name, not a mode: on the first TAB the
function silently registers itself instead of completing.

### Worked code-judo proposal (verified in scratch)

zsh already exposes the exact fact the trailer is trying to reconstruct:
`$zsh_eval_context[-1]` is `loadautofunc` while an autoloaded function's file body is
executing and `file` while a file is being `source`d (probe above). Dispatching on that
removes the file-name coupling, and moving the test harness's "run `_rg`" into the harness
removes the hidden test mode from shipped code:

```zsh
# Autoloaded by compsys (the file body *is* _rg): run it. Sourced: register it.
if [[ $zsh_eval_context[-1] == loadautofunc ]]; then
  _rg "$@"
elif (( $+functions[compdef] )); then
  compdef _rg rg
else
  print -ru2 "rg: run compinit before sourcing rg completions"
fi
```

and in `ci/test-complete`'s `get_comp_args`:

```zsh
( _RG_COMPLETE_LIST_ARGS=1; source $1 2>/dev/null; _rg )
```

Scratch results for that variant:

```
-- P: autoload _rg          -> runs _rg (comparguments error expected outside a real completion)
-- P: autoload renamed      -> runs _rg; _comps[rg] stays _ripgrep
-- P: source after compinit -> rc=0 comps=_rg
-- P: source before compinit-> "rg: run compinit before sourcing rg completions", rc=0
-- P: harness calls _rg     -> + / (exclusive) / (: * -)-h[display help information]
-- P: harness unchanged     -> only prints the warning (so the harness edit is required)
```

If printing a diagnostic is unwanted, the `else` branch can simply be dropped; the important
part is that "compinit not loaded" stops being treated as "please run the completer now".
If `zsh_eval_context` is considered too obscure, the minimum acceptable fix is still to take
the test-harness case out of the production condition (harness calls `_rg` itself) and
guard `compdef` with `(( $+functions[compdef] ))`, so the before-`compinit` user path no
longer calls `_arguments` at start-up.

Unverified: the minimum zsh version providing `zsh_eval_context`/`loadautofunc` (I believe
it predates 5.0, well below the 5.4 floor the file already special-cases, but I did not
check release notes offline), and behaviour under `KSH_AUTOLOAD` or `.zwc`-compiled
digests. The replacement comment should spell out the three contexts instead of pointing at
issue/PR URLs.

## Non-findings

- File size: `rg.zsh` goes from 637 to 645 lines; no threshold concern.
- Placing the dispatcher at the end of the completion file (rather than in `zsh.rs`) is the
  right layer: the generator is a verbatim `include_str!` and should stay that way.

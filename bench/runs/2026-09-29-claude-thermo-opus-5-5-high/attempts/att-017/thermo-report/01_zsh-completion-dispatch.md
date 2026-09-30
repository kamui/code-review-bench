# 01 — zsh completion dispatch tail (`crates/core/flags/complete/rg.zsh`)

Scope: the new trailer at `crates/core/flags/complete/rg.zsh:437-445`, which replaces the unconditional `_rg "$@"` with a three-way dispatch.

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

## Measurements and how they were produced

The Rust crate could not be built, so the generated script was reproduced by hand. `zsh.rs` only does `include_str!("rg.zsh").replace("!ENCODINGS!", ...)`, so replacing that placeholder with one encoding gives a script that behaves the same for dispatch purposes. The scratch copies live in `clone-work/scratch/`: `rg-gen.zsh` is the head version, `rg-old.zsh` is the merge-base version, and `rg-2way.zsh` is the head version with the `|| (( ! $+functions[compdef] ))` clause removed. Everything ran under `zsh -f` (zsh 5.8.1) with `HOME` set to the scratch directory.

| Scenario | Command shape | Result |
| --- | --- | --- |
| A. sourced, no `compinit` (head) | `source rg-gen.zsh` | `_rg:341: command not found: _arguments`, rc=1 |
| A'. same at merge-base | `source rg-old.zsh` | identical error, rc=1 |
| B. `compinit`, then sourced (head) | `compinit -D -u; source rg-gen.zsh` | rc=0, `_rg` is a function, `$_comps[rg]=_rg` |
| B'. same at merge-base | | `_arguments:comparguments:325: can only be called from completion function` (the bug in #2956) |
| D. `ci/test-complete` harness shape (head) | `setopt local_options unset; ( _RG_COMPLETE_LIST_ARGS=1 source rg-gen.zsh )` | 341 spec lines printed |
| D'. harness shape, two-way dispatch | same, on `rg-2way.zsh` | `rg-2way.zsh:444: command not found: compdef`, 0 lines |
| D''. harness adapted, two-way dispatch | `( compdef(){ :; }; source rg-2way.zsh; _RG_COMPLETE_LIST_ARGS=1 _rg )` | 341 spec lines, the same output as D |
| E. `$funcstack[1]` inside a sourced file | `source fs.zsh`, both top level and inside a function | the file's full path in both cases |
| F. `$funcstack[1]` / `$zsh_eval_context[-1]` when autoloaded | `autoload -Uz _foo2 _renamed; _foo2; _renamed` | `fs1=_foo2 ctx=loadautofunc`, `fs1=_renamed ctx=loadautofunc`; sourcing the same file gives `ctx=file` |

Scenario C (sourcing before `compinit`, then running `compinit`) was also run. It is not usable as evidence because the host has a vendor `/usr/share/zsh/vendor-completions/_rg` that registers `_comps[rg]` on its own. The only reliable part of C is that the source step prints the same `_arguments` error as A.

Verification status: A, A', B, B', D, D', D'', E and F were all executed and confirmed. The real `ci/test-complete` could not be run because it needs a built `rg`. D reproduces its `get_comp_args` function exactly: `emulate zsh -o extended_glob -o no_function_argzero -o no_unset`, then `setopt local_options unset`, then a subshell `source`.

## Finding 1.1 — The third branch is test-harness plumbing in the shipped script, and it sends a real user mistake into a guaranteed failure

The actual fix for #2956 is the standard two-way autoload-or-source dispatch: if zsh autoloaded the file as `_rg`, run the function; if the file was sourced, register it with `compdef`. The PR's own history shows this was the first version. The author wrote the two-way form, `ci/test-complete` failed, and the reviewer then suggested `|| (( ! $+functions[compdef] ))` because "the function is designed to be sourced by [the test script] without requiring anything from the completion system". So the extra clause exists only because a CI script sources the file in a bare shell. It has no purpose for users.

Once shipped, that clause does fire for users. It covers the case where `compdef` is missing because the user's `.zshrc` runs `source <(rg --generate complete-zsh)` before `compinit`. That is an easy ordering mistake, and the new FAQ text does not warn about it (see `02_faq-docs.md`). In that case the new branch calls `_rg "$@"` outside the completion system, and the user gets `command not found: _arguments` at every shell start (scenario A). Nothing in the product ever wants "run the completion body right now, outside completion". The branch is dead as a feature, and its only reachable user-facing effect is a confusing error.

The comment above the block also describes only one of the three cases. It says "Don't run the completion function when being sourced by itself", but the code does run the function when sourced without `compdef`. The two issue/PR URLs are standing in for an explanation that the code should give on its own. A reader has to go to GitHub to learn that the `compdef` probe is for CI.

This is the pattern the review standard calls "weird if statements in random places": a test-only condition sitting in the one production code path the feature touches.

### Worked code-judo proposal

Move the harness's needs into the harness, and reduce the shipped dispatch to the two cases that exist in production.

`crates/core/flags/complete/rg.zsh`:

```zsh
# When autoloaded from $fpath (the file is named _rg), we are the completion
# function body: run it. When sourced (e.g. `source <(rg --generate
# complete-zsh)` after compinit), just register the function.
if [[ $funcstack[1] == _rg ]]; then
  _rg "$@"
else
  compdef _rg rg
fi
```

`ci/test-complete`, `get_comp_args`:

```zsh
get_comp_args() {
    setopt local_options unset
    # The completion script registers itself with compdef when sourced; we
    # don't load the completion system here, so stub that out and call the
    # function directly.
    ( compdef() { : }; source $1; _RG_COMPLETE_LIST_ARGS=1 _rg )
}
```

This was checked as scenario D'': it prints the same 341 spec lines as the current harness does against the head script. Once the harness takes responsibility for its unusual invocation, the product script needs no knowledge of it. The `$+functions[compdef]` probe goes away, the `||` goes away, and the comment can describe both remaining branches accurately without pointing to URLs.

An optional follow-up makes the autoload detection independent of the function name. `$zsh_eval_context[-1]` is `loadautofunc` when zsh autoloads the file and `file` when it is sourced (scenario F). So `if [[ $zsh_eval_context[-1] == loadautofunc ]]` would also behave correctly if someone installs the file under another name. This is not required, because `#compdef rg` files are conventionally named `_rg`, but it tests the property we care about ("was I autoloaded?") instead of a stand-in for it ("is my caller's name `_rg`?").

If the maintainers want sourcing before `compinit` to fail with a clear message rather than the `_arguments` error, that belongs in the `else` branch as an explicit diagnostic. For example:

```zsh
elif (( ! $+functions[compdef] )); then
  print -ru2 -- 'rg completion: run compinit before sourcing this script'
  return 1
```

It should not be done by silently falling into the completion body. Still, the smaller change is the two-way dispatch plus a FAQ note about ordering.

Severity: moderate. The change works in the configurations that are documented and tested, but it puts test scaffolding into the shipped artifact, and the one user path that scaffolding reaches ends in a bad error.

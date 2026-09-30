# Thermo-nuclear code quality review — BurntSushi/ripgrep#2957

Range: `79cbe89deb1151e703f4d91b19af9cdcc128b765..855bfa6cdae4f4fe8762f892fc4957635397083e`
(`git diff main...review-head`), 2 files, +29/−4: a nine-line loader trailer in
`crates/core/flags/complete/rg.zsh` and a rewrite of the zsh part of the shell-completion
answer in `FAQ.md`.

## Verdict

Request changes (small). The goal — letting `source <(rg --generate complete-zsh)` work —
is reasonable and the change is in the right layer (the completion script itself, not the
Rust generator, which embeds it verbatim). But the dispatcher that makes it work does not
model how the file is loaded; it guesses from two incidental signals, and one of those
guesses exists only to keep the CI harness green while also capturing a real user path
that still fails the way issue #2956 did. The FAQ text then leaves users one ordering
mistake away from that path and ships a `.zshrc` snippet that cannot be pasted verbatim.
Both problems have short, verified fixes. No file-size threshold is crossed (`rg.zsh`
637→645 lines; `FAQ.md` was already 1046 lines and is now 1063).

## Findings

### 1. The loader trailer infers three loading contexts from two incidental signals, and its test-only branch reproduces the #2956 failure for users who source before `compinit`

`crates/core/flags/complete/rg.zsh:437-445` replaces `_rg "$@"` with `if [[ $funcstack[1] == _rg ]] || (( ! $+functions[compdef] )); then _rg "$@"; else compdef _rg rg; fi`. The file is now loaded three ways — autoloaded by compsys (must run `_rg`), sourced by a user (must `compdef`), and sourced by `ci/test-complete` (must run `_rg` to dump specs) — but the condition does not name any of them. The `funcstack` test checks the installed file name rather than the execution mode, and the `! $+functions[compdef]` test, which is there only for the CI harness, is equally true for any user who sources the script before `compinit`; I verified that this path prints `_rg:341: command not found: _arguments` at shell start-up, and that a copy installed on `fpath` as `_ripgrep` re-registers itself on first use instead of completing. The comment "Don't run the completion function when being sourced by itself" is also wrong for that branch. The code-judo move is to dispatch on the fact zsh already exposes — `$zsh_eval_context[-1] == loadautofunc` while an autoloaded body runs, `file` while sourced — register with `compdef` only when it exists (otherwise print a one-line "run compinit first" hint or do nothing), and have `ci/test-complete` call `_rg` itself after sourcing (`( _RG_COMPLETE_LIST_ARGS=1; source $1 2>/dev/null; _rg )`). That deletes the hidden test mode from shipped code and the file-name coupling in one step; I verified all five load paths of that variant in scratch zsh. Replace the two issue/PR URLs with a comment that names the three contexts. Full evidence and the worked variant are in `01_zsh-completion-loader.md`.

### 2. The FAQ's new `.zshrc` snippet is not paste-safe, and neither zsh recipe states the `compinit` ordering it depends on

`FAQ.md:131-139` tells users to "add the following to your `$HOME/.zshrc` file" and then shows `$ source <(rg --generate complete-zsh)`; the `$ ` prompt marker makes the line fail verbatim (`command not found: $`, verified), and it contradicts the `fpath=(...)` `.zshrc` block four lines earlier, which correctly has no prompt marker. More substantively, the `fpath` line only works if it runs before `compinit`, and the `source` line only works if it runs after `compinit` — with the trailer as merged, sourcing first reproduces the #2956 error on every start-up — yet the FAQ mentions neither. Drop the `$ `, add "before `compinit`" / "after `compinit`" to the two instructions, and fix the caveat sentence ("easier to set up, it is generally slower"). Proposed replacement text is in `02_faq-docs.md`.

## Remediation sequence

1. Rewrite the `rg.zsh` trailer to dispatch on `zsh_eval_context`, guard `compdef`, and
   document the three load contexts in the comment (Finding 1).
2. In the same change, make `ci/test-complete`'s `get_comp_args` call `_rg` explicitly
   after sourcing, so the harness no longer depends on a production fallback (Finding 1).
3. Fix the FAQ snippet and add the `compinit` ordering guidance (Finding 2).
4. Optionally confirm the minimum zsh version for `zsh_eval_context` against the 5.4
   compatibility floor the script already handles (noted as unverified in the detail file).

## Detail files

- `01_zsh-completion-loader.md` — loader trailer: context probes, failure reproductions,
  worked `zsh_eval_context` variant with scratch results, unverified points.
- `02_faq-docs.md` — FAQ instructions: paste failure, ordering gap, proposed text.

## Method

Read the frozen skill, the full diff, the whole of `rg.zsh`, `zsh.rs` and `ci/test-complete`.
Ran scratch zsh 5.8.1 scripts outside the clone (under `clone-work/scratch`) against an
unmodified copy of the head `rg.zsh` and against a modified scratch copy; no cargo build or
`ci/test-complete` run (unavailable). The clone was not modified.

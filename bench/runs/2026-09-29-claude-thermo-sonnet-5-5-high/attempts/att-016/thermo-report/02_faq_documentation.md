# 02 — FAQ shell-completion documentation (`FAQ.md`)

Scope: `FAQ.md` lines 94-140 at head `855bfa6` (+20/−3).

## Measurements and commands

- `git diff main...review-head` to read the change.
- `sed -n 90,145p FAQ.md` to read the resulting section in context.
- `grep -n 'fpath=\|source <(rg\|slower than' FAQ.md` to anchor the line numbers: `fpath=` at 128, `source <(rg` at 135, "slower than" at 138.
- I ran the shell snippets in scratch zsh; see `01_zsh_completion_script.md` for the results.

## Finding 1 — the `.zshrc` snippet includes a shell prompt marker, so it is not copy-pasteable (confirmed by reading)

At `FAQ.md:135` the block reads `$ source <(rg --generate complete-zsh)`. The prose right before it says to add this line to `$HOME/.zshrc`. A literal `$ ` at the start of a line in a zshrc is parsed as a command named `$`, which fails at every shell start-up. The neighbouring `fpath=(...)` block at line 128 correctly omits the prompt marker. The blocks are also inconsistent with each other in language tag treatment: the earlier `$ dir=...` block is a terminal transcript and the `fpath` and `source` blocks are file contents.

Remedy: drop the `$ ` at line 135 so it matches the `fpath=` block, and keep `$ ` only where the block is a terminal transcript (`mkdir`, `rg --generate ... > _rg`).

## Finding 2 — the docs promote the one-liner without stating its ordering precondition (confirmed by execution)

The text at lines 132-138 offers `source <(rg --generate complete-zsh)` as an alternative and mentions only the speed cost. It does not say that this works only after `compinit` has run. Scenarios A and C in report 01 show that when it runs earlier it prints `_arguments: command not found` and returns 1. The same gap exists on the `fpath` path at line 128, where the block does not say that `fpath` must be set before `compinit`. That was already true in the pre-existing instructions, so it is minor, but this PR now adds the `fpath` line and should get it right.

Remedy: add one sentence to each zsh alternative, "this must come before / after your `compinit` call", or fix the script so the ordering does not matter (report 01, Finding 1). The second option removes the docs burden.

## Finding 3 — grammar and wording defects in the new caveat (confirmed by reading)

`FAQ.md:137-138` reads "Note though that while this approach is easier to setup, is generally slower than the previous method". "while ... is" has no subject, and "setup" should be the verb form "set up". Suggested text: "Note that while this approach is easier to set up, it is generally slower than the previous method and will add time to your shell start-up." The claim "generally slower" is unmeasured in the PR (the thread mentions about 4 ms); the wording is acceptable but should not read as a stronger claim than that evidence.

The heading "For **zsh**, the recommended approach is:" now makes the older method the "recommended" one and then lists an alternative in a separate paragraph. That is fine, but the other shells use "For **bash**:" and so on. This is the only place where a recommendation is stated, and the rest of the section does not carry that weight.

## Structural note (code-judo)

The section now stitches one zsh recipe from a block, a paragraph, a second block, a paragraph, a third block and a caveat. A cleaner shape is two labelled sub-options ("Option 1: `fpath` (faster)", "Option 2: `source` (self-contained, slower)") with the ordering precondition for each stated inside its option. No extra content is needed, only a reframing that makes each option self-contained and copy-pasteable.

Verification status: findings 1 and 3 are confirmed by reading the file; finding 2 is confirmed by executed zsh scenarios A and C.

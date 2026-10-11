# Zsh completion and FAQ

## Scope and measurements

The reviewed range is `79cbe89deb1151e703f4d91b19af9cdcc128b765..855bfa6cdae4f4fe8762f892fc4957635397083e`. It changes two files: 23 lines in `FAQ.md` and a 10-line replacement of the zsh completion script's final invocation with an eight-line conditional. Neither file approaches the 1,000-line threshold. The completion file is already the canonical owner of `_rg` and the `#compdef rg` declaration.

The new dispatcher calls `_rg` when it is itself running as `_rg` or when `compdef` is unavailable; otherwise it registers `_rg` for `rg`. This is not arbitrary branching: `ci/test-complete` sources the completion with `compdef` absent and relies on `_rg` being invoked to dump its argument specs, while a regular sourced script with the completion system active needs `compdef` registration rather than running `_arguments` at source time. The conditional therefore keeps the two existing execution contexts at the same entry point without adding another abstraction or scattering checks.

## Finding: remove the prompt marker from the `.zshrc` example

`FAQ.md:131-135` introduces the second setup method with “add the following to your `$HOME/.zshrc` file” and then shows `$ source <(rg --generate complete-zsh)`. A dollar prompt is not part of a command to place in a zsh startup file. When that exact line is run by zsh, it fails with `zsh:1: command not found: $`; thus the documented setup does not load completions. The actionable correction is to remove the leading `$` from this file snippet. This finding is in the changed lines and is independently reproducible.

## Worked code-judo proposal

There is no useful structural rewrite for the dispatcher. Replacing its condition with another helper or a more general “load mode” would add a concept without removing either runtime case. The smaller design improvement is to keep this direct branch and make the user-facing distinction explicit through the examples: prompt-prefixed snippets are commands to type, while the `.zshrc` snippet is literal file content. In particular, the latter should read:

```zsh
source <(rg --generate complete-zsh)
```

This keeps the script's conditional logic at the completion boundary and removes the documentation ambiguity rather than introducing extra implementation machinery.

## Verification status

- Read `git diff main...review-head`, the complete completion entry point and relevant function body, `ci/test-complete`, and the surrounding FAQ section.
- Ran `zsh -c '$ source <(printf "# generated\\n")'`; zsh reported `command not found: $`, reproducing the copied-snippet failure.
- The focused `ci/test-complete` script could not be run because it requires a built `rg`; Rust builds and tests are unavailable under this run's execution policy. No build or test command was attempted.
- No checkout files were modified. The report files are outside the checkout.

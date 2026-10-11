# Zsh completion footer and FAQ guidance

## Scope and design

The change is limited to crates/core/flags/complete/rg.zsh and the zsh section of FAQ.md. The completion file already declares #compdef rg, defines _rg, and has a test mode controlled by _RG_COMPLETE_LIST_ARGS. The new footer tries to support both ordinary autoloaded completion and dynamically sourcing the generated file: it calls _rg when funcstack[1] is _rg or when compdef is unavailable, otherwise it registers _rg for rg.

The first condition is an invocation-context check. The second is not: missing compdef means the completion system has not been initialized, rather than that the file was invoked by the completion system. That makes sourcing have an unintended execution side effect in precisely the startup state where registration is unavailable. _rg calls zsh completion primitives such as _arguments, which require completion context.

## Finding: absence of compdef must not dispatch the completion function

At crates/core/flags/complete/rg.zsh:441-444, the ! $+functions[compdef] alternative enters _rg "$@". In a new zsh with no completion initialization, the FAQ's source <(rg --generate complete-zsh) therefore runs completion logic immediately. A local focused run of the checked-in completion file under zsh -fc 'source crates/core/flags/complete/rg.zsh' returned exit code 1 and printed _rg:341: command not found: _arguments. This confirms that sourcing without the completion system is not a harmless no-op.

With autoload -Uz compinit; compinit first, the same source command exited successfully and _comps[rg] was _rg, confirming the registration branch works once completion initialization has happened. The FAQ at FAQ.md:131-139 currently suggests putting the source command in .zshrc without explaining that ordering requirement. Users who put it before their framework's compinit, or have not initialized completions, encounter the failing fallback. If the fallback is corrected to a no-op, sourcing before later initialization would still fail to register the completion, so the documented order must be after compinit.

This is a single concrete behavior problem, not a request to split or abstract the footer. The new branch is only a few lines and does not create file-size or broad maintainability concerns. The code-quality improvement is to model the actual three contexts explicitly instead of using a missing helper as a proxy for invocation: completion dispatch, normal sourced registration, and test extraction.

## Worked code-judo proposal

Use the existing explicit list-args test switch as the test-only escape hatch, rather than treating lack of compdef as a reason to invoke a completion. In pseudocode preserving current semantics where needed:

    if [[ $funcstack[1] == _rg ]]; then
      _rg "$@"
    elif (( $+functions[compdef] )); then
      compdef _rg rg
    elif [[ $_RG_COMPLETE_LIST_ARGS == (1|t*|y*) ]]; then
      _rg "$@"
    fi

The final branch can be factored into a small predicate only if the codebase wants to centralize the already repeated _RG_COMPLETE_LIST_ARGS expression; for this footer alone, a direct condition is clearer. The important simplification is semantic: ordinary sourcing without compdef no longer pretends to be a completion call. The list-args test continues to source the file and execute _rg, because ci/test-complete sets _RG_COMPLETE_LIST_ARGS=1 while sourcing it.

Update the FAQ to say to place the dynamic source line after autoload -Uz compinit; compinit, or after the shell framework has initialized zsh completions. If the FAQ does not want to prescribe initialization itself, this short ordering requirement is enough to make the supported precondition visible.

## Verification

- Read git diff main...review-head, crates/core/flags/complete/rg.zsh, ci/test-complete, and the affected FAQ passage. ci/test-complete runs _RG_COMPLETE_LIST_ARGS=1 source $1; the proposed explicit test branch preserves that contract.
- zsh -fc 'source crates/core/flags/complete/rg.zsh' on zsh 5.8.1: failed, exit 1, _arguments unavailable. This was a focused reproduction of the no-compdef branch.
- zsh -fc 'autoload -Uz compinit; compinit; source crates/core/flags/complete/rg.zsh; print -r -- "registered=${_comps[rg]}"': passed, exit 0, printed registered=_rg.
- ci/test-complete and Rust build/test commands were not run: the execution policy says the script needs a built rg and Rust build/test are unavailable. No files in the checkout were changed.

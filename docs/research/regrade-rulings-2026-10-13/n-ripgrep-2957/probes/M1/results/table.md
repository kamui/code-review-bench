# Every scenario against every variant

Each cell is "first Tab / line after the second Tab". "completes" means the command line became `rg --generate=complete-zsh ` on the first Tab; "nothing" means it stayed `rg --generate=complete-z`. "(error printed)" means a setup command of the scenario printed an error. Source: `results/<variant>.json`, `results/extra-<variant>.json`, written by `tab_probe.py`.

| Scenario | base | head | fix-n5-only | fix-wrapper-only | fix-n4-only | fix-both |
| --- | --- | --- | --- | --- | --- | --- |
| `control-fpath-_rg` | completes / complete | completes / complete | completes / complete | completes / complete | completes / complete | completes / complete |
| `control-source-toplevel` | nothing / nothing (error printed) | completes / complete | completes / complete | completes / complete | completes / complete | completes / complete |
| `control-eval-toplevel` | nothing / nothing (error printed) | completes / complete | completes / complete | completes / complete | completes / complete | completes / complete |
| `n5-fpath-_ripgrep` | completes / complete | nothing / complete | completes / complete | nothing / complete | nothing / complete | completes / complete |
| `n5-fpath-_rg_completion` | completes / complete | nothing / complete | completes / complete | nothing / complete | nothing / complete | completes / complete |
| `n4-source-bare-_rg` | nothing / nothing (error printed) | nothing / nothing (error printed) | nothing / nothing (error printed) | nothing / nothing (error printed) | completes / complete | completes / complete |
| `wrapper-_rg-source` | completes / complete | nothing / complete | nothing / complete | completes / complete | nothing / complete | completes / complete |
| `wrapper-_rg-eval` | completes / complete | nothing / complete | nothing / complete | completes / complete | nothing / complete | completes / complete |
| `wrapper-_rg-source-then-call` | completes / complete | completes / complete | completes / complete | completes / complete | completes / complete | completes / complete |
| `wrapper-_ripgrep-eval-then-call` | completes / complete | completes / complete | completes / complete | completes / complete | completes / complete | completes / complete |
| `wrapper-_ripgrep-source` | completes / complete | nothing / complete | nothing / complete | nothing / complete | nothing / complete | completes / complete |
| `wrapper-_ripgrep-eval` | completes / complete | nothing / complete | nothing / complete | nothing / complete | nothing / complete | completes / complete |
| `wrapper-zshrc-function-_rg-source` | completes / complete | nothing / complete | nothing / complete | completes / complete | nothing / complete | completes / complete |
| `wrapper-zshrc-function-_rg-eval` | completes / complete | nothing / complete | nothing / complete | completes / complete | nothing / complete | completes / complete |
| `wrapper-zshrc-function-other-name-source` | completes / complete | nothing / complete | nothing / complete | nothing / complete | nothing / complete | completes / complete |
| `wrapper-_rg-source-fpath-after-compinit` | completes / complete | nothing / complete | nothing / complete | completes / complete | nothing / complete | completes / complete |
| `wrapper-_rg-source-no-compinit` | nothing / nothing (error printed) | nothing / nothing (error printed) | nothing / nothing (error printed) | nothing / nothing (error printed) | nothing / nothing (error printed) | nothing / nothing (error printed) |
| `wrapper-on-demand-explicit-call` | completes / complete | completes / complete | not run | not run | not run | not run |

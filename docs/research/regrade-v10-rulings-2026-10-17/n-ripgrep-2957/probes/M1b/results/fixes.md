# M1b: documentation fixes for the source way, at the head (ripgrep 14.1.1 (rev 855bfa6cda), zsh 5.8.1)

A cell is `works` when every layout the fix's words allow completes on the first Tab with nothing printed at startup.

| Fix | .zshrc | Layouts | First Tab | Printed at startup | Cell |
| --- | --- | --- | --- | --- | --- |
| head | runs compinit | source line after compinit | completes | nothing | FAILS |
| head | runs compinit | source line before compinit | nothing | _rg:341: command not found: _arguments | FAILS |
| head | no compinit | source line, no compinit | nothing | _rg:341: command not found: _arguments | FAILS |
| fix-n3 | runs compinit | source line after compinit (the only place allowed) | completes | nothing | works |
| fix-n3 | no compinit | source line, no compinit (no compinit line exists to put it after) | nothing | _rg:341: command not found: _arguments | FAILS |
| fix-m1b | runs compinit | source line after compinit | completes | nothing | FAILS |
| fix-m1b | runs compinit | source line before compinit (the sentence is for .zshrc files without compinit) | nothing | _rg:341: command not found: _arguments | FAILS |
| fix-m1b | no compinit | the added compinit line, then the source line | completes | nothing | works |
| fix-both | runs compinit | source line after compinit | completes | nothing | works |
| fix-both | no compinit | the added compinit line, then the source line | completes | nothing | works |
| fix-block | runs compinit | own compinit, then the block | completes | nothing | FAILS |
| fix-block | runs compinit | the block, then own compinit | nothing | nothing | FAILS |
| fix-block | no compinit | the block alone | completes | nothing | works |

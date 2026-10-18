# M1a: documentation fixes for the fpath way, at the head (ripgrep 14.1.1 (rev 855bfa6cda), zsh 5.8.1)

A cell is `works` when every layout the fix's words allow completes on the first Tab with nothing printed at startup.

| Fix | .zshrc | Layouts | First Tab | Printed at startup | Cell |
| --- | --- | --- | --- | --- | --- |
| head | runs compinit | fpath line after compinit | nothing | nothing | FAILS |
| head | runs compinit | fpath line before compinit | completes | nothing | FAILS |
| head | no compinit | fpath line, no compinit | nothing | nothing | FAILS |
| fix-n2 | runs compinit | fpath line before compinit (the only place allowed) | completes | nothing | works |
| fix-n2 | no compinit | fpath line, no compinit (no compinit line exists to put it before) | nothing | nothing | FAILS |
| fix-m1a | runs compinit | fpath line after compinit (the sentence is for .zshrc files without compinit) | nothing | nothing | FAILS |
| fix-m1a | runs compinit | fpath line before compinit | completes | nothing | FAILS |
| fix-m1a | no compinit | fpath line, then the added compinit line | completes | nothing | works |
| fix-both | runs compinit | fpath line before compinit | completes | nothing | works |
| fix-both | no compinit | fpath line, then the added compinit line | completes | nothing | works |
| fix-block | runs compinit | own compinit, then the block | completes | nothing | works |
| fix-block | runs compinit | the block, then own compinit | completes | nothing | works |
| fix-block | no compinit | the block alone | completes | nothing | works |

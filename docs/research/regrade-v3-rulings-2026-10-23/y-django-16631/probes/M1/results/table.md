| run | S1 req 2 | S1 next, resp 1 last | S1 next, resp 2 last | S7 req 2 (save-every) | S3 next, resp 2 last | S4 req 2 | S5 next | S6 next | S8 req 2 (login() from anonymous, no rotation) | S0L req 2 | S0L next |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| base-db | 400 | signed out, write lost | signed out, write lost | 400 | signed out | signed out | signed out | signed out | 400 | 400 | signed out |
| base-cache | 400 | signed out, write lost | signed out, write lost | not run | signed out | signed out | signed out | signed out | 400 | 400 | signed out |
| base-cached_db | 400 | signed out, write lost | signed out, write lost | not run | signed out | signed out | signed out | signed out | 400 | 400 | signed out |
| base-file | 400 | signed out, write lost | signed out, write lost | not run | signed out | signed out | signed out | signed out | 400 | 400 | signed out |
| base-signed_cookies | 200 | signed out, write lost | signed out, write lost | not run | signed out | signed out | signed out | signed out | 200 | 200 | signed in |
| head-db | 400 | signed in, write lost | signed in, write lost | 400 | signed out | signed out | signed out | signed in | 400 | 400 | signed out |
| head-cache | 400 | signed in, write lost | signed in, write lost | not run | signed out | signed out | signed out | signed in | 400 | 400 | signed out |
| head-cached_db | 400 | signed in, write lost | signed in, write lost | not run | signed out | signed out | signed out | signed in | 400 | 400 | signed out |
| head-file | 400 | signed in, write lost | signed in, write lost | not run | signed out | signed out | signed out | signed in | 400 | 400 | signed out |
| head-signed_cookies | 200 | signed in, write lost | signed in, write kept | not run | signed in | signed in | signed in | signed in | 200 | 200 | signed in |
| fix-gt-y2-db | 200 | signed in, write kept | signed in, write kept | 200 | signed in | signed in | signed in | signed in | 400 | 400 | signed out |
| fix-gt-y2-cache | 200 | signed in, write kept | signed in, write kept | not run | signed in | signed in | signed in | signed in | 400 | 400 | signed out |
| fix-m1-middleware-db | 200 | signed in, write lost | signed in, write kept | 200 | signed out | signed out | signed out | signed in | 200 | 200 | signed in |
| fix-m1-middleware-cache | 200 | signed in, write lost | signed in, write kept | not run | signed out | signed out | signed out | signed in | 200 | 200 | signed in |
| both-db | 200 | signed in, write kept | signed in, write kept | 200 | signed in | signed in | signed in | signed in | 200 | 200 | signed in |
| both-cache | 200 | signed in, write kept | signed in, write kept | not run | signed in | signed in | signed in | signed in | 200 | 200 | signed in |
| fix-gt-y2-keep-old-row-db | 200 | signed in, write lost | signed in, write kept | 200 | signed in | signed in | signed in | signed in | 400 | 400 | signed out |
| fix-gt-y2-keep-old-row-cache | 200 | signed in, write lost | signed in, write kept | not run | signed in | signed in | signed in | signed in | 400 | 400 | signed out |

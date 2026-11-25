| runtime | path | reader | body delivered as | body | media type | base | head | fix-reuse | fix-stream-both | fix-reuse-and-stream | fix-native-lower | fix-release | guard-only | head / base (medians) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| node v24.7.0 | hono | parseBody | stream | multipart, 100 MB file | mixed | 0.025 [0.024-0.033] n=3 | 372.6 [368.3-401.6] n=3 | 407.6 [391.1-445.5] n=3 | 183.4 [167.2-185.1] n=3 | 179.0 [163.2-207.7] n=3 | 393.6 [369.6-482.4] n=3 | 433.2 [355.9-480.4] n=3 | 177.7 [155.8-190.1] n=3 | 14903.64 |
| node v24.7.0 | plain | parseBody(Request) | stream | multipart, 8 MB file | lower | 5.3 [4.5-5.7] n=3 | 14.3 [12.4-15.4] n=3 | 15.2 [12.9-15.7] n=3 | 4.7 [4.6-4.7] n=3 | 4.9 [4.7-5.1] n=3 | 4.7 [4.6-5.1] n=3 | 13.8 [10.6-14.2] n=3 | 4.9 [4.6-5.4] n=3 | 2.69 |
| node v24.7.0 | hono | formData | stream | multipart, 8 MB file | lower | 4.2 [3.8-4.4] n=3 | 4.4 [3.9-4.5] n=3 |  |  |  |  |  |  | 1.05 |
| node v24.7.0 | hono | formData | bytes | multipart, 8 MB file | lower | 4.8 [4.5-5.1] n=3 | 4.8 [4.4-4.8] n=3 |  |  |  |  |  |  | 0.98 |
| node v24.7.0 | plain | parseBody(Request) | stream | multipart, 100 MB file | lower | 180.3 [163.0-231.2] n=3 | 381.5 [359.2-389.0] n=3 | 363.5 [360.0-375.2] n=3 | 168.3 [158.4-175.7] n=3 | 173.4 [164.7-205.2] n=3 | 164.6 [151.5-200.7] n=3 | 394.3 [340.0-430.7] n=3 | 167.8 [165.5-177.5] n=3 | 2.12 |
| node v24.7.0 | hono | formData | stream | multipart, 100 MB file | lower | 155.4 [154.9-158.5] n=3 | 155.1 [145.8-156.8] n=3 |  |  |  |  |  |  | 1.00 |
| node v24.7.0 | hono | formData | bytes | multipart, 100 MB file | lower | 141.9 [138.3-142.9] n=3 | 138.7 [138.4-145.6] n=3 |  |  |  |  |  |  | 0.98 |
| bun 1.3.10 | hono | parseBody | stream | multipart, 100 MB file | mixed | 0.009 [0.009-0.015] n=3 | 149.5 [149.2-151.1] n=3 | 151.9 [149.5-164.3] n=3 | 153.0 [147.8-154.7] n=3 | 149.7 [149.0-159.4] n=3 | 141.5 [140.9-159.9] n=3 | 144.3 [144.0-148.6] n=3 |  | 16611.67 |
| bun 1.3.10 | plain | parseBody(Request) | stream | multipart, 8 MB file | lower | 10.0 [9.9-10.4] n=3 | 10.1 [9.9-10.9] n=3 | 10.0 [9.5-11.6] n=3 | 10.5 [10.4-10.5] n=3 | 10.6 [10.6-11.1] n=3 | 10.3 [10.2-10.5] n=3 | 11.3 [10.9-11.9] n=3 | 10.6 [10.1-11.2] n=3 | 1.01 |
| bun 1.3.10 | hono | formData | stream | multipart, 8 MB file | lower | 10.7 [10.4-10.8] n=3 | 10.2 [10.1-10.3] n=3 |  |  |  |  |  |  | 0.95 |
| bun 1.3.10 | hono | formData | bytes | multipart, 8 MB file | lower | 4.9 [4.8-5.1] n=3 | 4.9 [4.6-5.0] n=3 |  |  |  |  |  |  | 1.01 |
| bun 1.3.10 | plain | parseBody(Request) | stream | multipart, 100 MB file | lower | 198.2 [190.5-214.9] n=3 | 175.5 [173.4-185.6] n=3 | 222.6 [166.1-227.3] n=3 | 175.4 [175.2-186.1] n=3 | 165.7 [163.4-193.6] n=3 | 163.6 [151.2-191.1] n=3 | 165.0 [156.8-175.4] n=3 | 160.9 [160.0-164.1] n=3 | 0.89 |
| bun 1.3.10 | hono | formData | stream | multipart, 100 MB file | lower | 163.6 [156.7-172.3] n=3 | 161.8 [159.0-179.9] n=3 |  |  |  |  |  |  | 0.99 |
| bun 1.3.10 | hono | formData | bytes | multipart, 100 MB file | lower | 64.8 [64.4-66.3] n=3 | 63.9 [60.8-67.5] n=3 |  |  |  |  |  |  | 0.99 |
| node v24.21.0 | hono | parseBody | stream | multipart, 128 MB file | lower | 197.5 [190.5-209.7] n=5 | 491.9 [486.7-519.4] n=5 |  |  |  |  |  |  | 2.49 |
| node v24.21.0 | hono | parseBody | bytes | multipart, 128 MB file | lower | 149.4 [139.6-168.8] n=5 | 469.8 [445.4-495.8] n=5 |  |  |  |  |  |  | 3.14 |

Error in 3 process(es): guard-only bun 1.3.10 file 100 MB: Error: HTTP 500 Internal Server Error

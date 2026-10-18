| runtime | path | reader | body delivered as | body | media type | base | head | fix-reuse | fix-stream-both | fix-reuse-and-stream | fix-native-lower | fix-release | guard-only | head / base (medians) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| node v24.7.0 | hono | parseBody | stream | multipart, 100 MB file | mixed | 0.316 [0.297-0.340] n=3 | 423.5 [380.1-500.5] n=3 | 385.7 [365.7-401.8] n=3 | 204.5 [195.0-210.1] n=3 | 214.8 [182.7-219.7] n=3 | 435.1 [428.2-470.0] n=3 | 400.3 [378.8-480.5] n=3 | 207.6 [188.5-221.4] n=3 | 1340.07 |
| node v24.7.0 | plain | parseBody(Request) | stream | multipart, 8 MB file | lower | 19.4 [17.3-19.7] n=3 | 33.7 [32.9-33.8] n=3 | 38.1 [32.2-42.6] n=3 | 18.1 [18.0-21.3] n=3 | 18.9 [17.9-19.1] n=3 | 18.4 [16.7-18.7] n=3 | 32.3 [31.0-33.8] n=3 | 19.5 [18.0-22.0] n=3 | 1.73 |
| node v24.7.0 | hono | formData | stream | multipart, 8 MB file | lower | 17.0 [15.7-18.6] n=3 | 17.3 [15.8-17.5] n=3 |  |  |  |  |  |  | 1.01 |
| node v24.7.0 | hono | formData | bytes | multipart, 8 MB file | lower | 11.2 [9.6-12.2] n=3 | 11.6 [11.4-12.4] n=3 |  |  |  |  |  |  | 1.04 |
| node v24.7.0 | plain | parseBody(Request) | stream | multipart, 100 MB file | lower | 199.2 [184.2-209.9] n=3 | 413.2 [371.4-575.7] n=3 | 384.7 [362.3-386.8] n=3 | 199.5 [188.8-222.0] n=3 | 199.0 [194.1-199.9] n=3 | 217.0 [187.7-222.5] n=3 | 364.5 [351.2-417.3] n=3 | 198.9 [187.0-208.4] n=3 | 2.07 |
| node v24.7.0 | hono | formData | stream | multipart, 100 MB file | lower | 192.8 [188.4-195.1] n=3 | 188.3 [182.7-189.4] n=3 |  |  |  |  |  |  | 0.98 |
| node v24.7.0 | hono | formData | bytes | multipart, 100 MB file | lower | 104.3 [103.6-120.4] n=3 | 105.0 [104.0-110.0] n=3 |  |  |  |  |  |  | 1.01 |
| bun 1.3.10 | hono | parseBody | stream | multipart, 100 MB file | mixed | 0.370 [0.357-0.450] n=3 | 229.1 [225.6-230.1] n=3 | 229.9 [224.7-249.0] n=3 | 180.5 [171.9-195.9] n=3 | 175.7 [167.3-185.9] n=3 | 228.7 [227.7-238.1] n=3 | 229.9 [226.3-234.7] n=3 |  | 619.17 |
| bun 1.3.10 | plain | parseBody(Request) | stream | multipart, 8 MB file | lower | 18.4 [16.4-18.4] n=3 | 23.0 [22.4-25.6] n=3 | 22.1 [21.5-23.2] n=3 | 17.2 [17.0-17.7] n=3 | 16.7 [16.7-18.1] n=3 | 17.3 [17.1-18.1] n=3 | 24.9 [23.1-29.8] n=3 | 17.1 [16.5-20.6] n=3 | 1.25 |
| bun 1.3.10 | hono | formData | stream | multipart, 8 MB file | lower | 18.6 [17.4-18.6] n=3 | 18.3 [18.0-19.7] n=3 |  |  |  |  |  |  | 0.98 |
| bun 1.3.10 | hono | formData | bytes | multipart, 8 MB file | lower | 5.3 [5.0-6.1] n=3 | 5.4 [5.4-6.3] n=3 |  |  |  |  |  |  | 1.01 |
| bun 1.3.10 | plain | parseBody(Request) | stream | multipart, 100 MB file | lower | 196.2 [189.7-259.9] n=3 | 295.3 [254.6-305.1] n=3 | 277.9 [268.7-411.4] n=3 | 197.6 [180.7-198.3] n=3 | 190.4 [183.8-272.3] n=3 | 184.6 [183.8-191.1] n=3 | 249.3 [237.6-366.0] n=3 | 178.2 [176.7-190.7] n=3 | 1.51 |
| bun 1.3.10 | hono | formData | stream | multipart, 100 MB file | lower | 185.2 [178.3-189.4] n=3 | 196.3 [185.8-224.0] n=3 |  |  |  |  |  |  | 1.06 |
| bun 1.3.10 | hono | formData | bytes | multipart, 100 MB file | lower | 68.6 [68.3-71.1] n=3 | 67.3 [64.9-67.7] n=3 |  |  |  |  |  |  | 0.98 |
| node v24.21.0 | hono | parseBody | stream | multipart, 128 MB file | lower | 240.3 [228.3-243.3] n=5 | 546.2 [521.1-585.7] n=5 |  |  |  |  |  |  | 2.27 |
| node v24.21.0 | hono | parseBody | bytes | multipart, 128 MB file | lower | 144.0 [141.0-158.6] n=5 | 440.2 [426.8-575.6] n=5 |  |  |  |  |  |  | 3.06 |

Error in 3 process(es): guard-only bun 1.3.10 file 100 MB: Error: HTTP 500 Internal Server Error

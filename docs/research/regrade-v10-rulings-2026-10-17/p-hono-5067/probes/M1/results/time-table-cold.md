| runtime | path | reader | body delivered as | body | media type | base | head | fix-reuse | fix-stream-both | fix-reuse-and-stream | fix-native-lower | fix-release | guard-only | head / base (medians) |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| node v24.7.0 | hono | parseBody | stream | urlencoded | lower | 1.5 [1.1-1.9] n=5 | 2.2 [2.1-3.8] n=5 | 2.5 [2.4-4.7] n=3 | 2.4 [1.5-2.8] n=3 | 2.2 [1.5-2.5] n=3 | 1.4 [1.2-2.4] n=3 | 2.2 [2.0-3.3] n=3 | 1.2 [1.1-1.3] n=3 | 1.43 |
| node v24.7.0 | hono | parseBody | stream | multipart-small | lower | 1.9 [1.8-2.6] n=5 | 3.4 [2.9-4.2] n=5 | 3.3 [2.4-3.6] n=3 | 2.6 [2.0-2.7] n=3 | 2.5 [2.1-2.5] n=3 | 2.0 [1.6-4.9] n=3 | 2.8 [2.7-3.6] n=3 | 3.3 [2.3-4.7] n=3 | 1.75 |
| node v24.7.0 | hono | parseBody | stream | multipart, 1 MB file | lower | 4.2 [4.0-6.7] n=5 | 10.6 [8.6-13.8] n=5 | 8.0 [6.9-9.4] n=3 | 6.0 [5.3-9.1] n=3 | 5.9 [4.2-5.9] n=3 | 5.6 [4.0-6.1] n=3 | 8.7 [7.8-9.3] n=3 | 4.1 [3.6-8.2] n=3 | 2.54 |
| node v24.7.0 | hono | parseBody | stream | multipart, 8 MB file | lower | 18.4 [16.5-19.3] n=5 | 35.5 [31.5-49.8] n=5 | 32.1 [30.1-45.9] n=3 | 19.3 [18.4-21.8] n=3 | 18.1 [17.9-20.4] n=3 | 17.5 [17.5-50.2] n=3 | 32.9 [32.6-35.2] n=3 | 19.0 [18.0-20.0] n=3 | 1.93 |
| node v24.7.0 | hono | parseBody | stream | multipart, 32 MB file | lower | 69.7 [66.8-84.7] n=5 | 133.8 [124.2-144.6] n=5 | 121.3 [114.3-122.6] n=3 | 64.7 [63.0-68.1] n=3 | 69.9 [69.0-74.3] n=3 | 66.5 [66.1-82.1] n=3 | 130.4 [120.1-137.0] n=3 | 61.8 [60.0-64.8] n=3 | 1.92 |
| node v24.7.0 | hono | parseBody | stream | multipart, 100 MB file | lower | 210.1 [192.3-221.5] n=5 | 426.7 [393.1-573.7] n=5 | 415.6 [364.3-417.2] n=3 | 226.0 [195.2-245.8] n=3 | 204.5 [203.9-213.1] n=3 | 205.4 [190.4-224.8] n=3 | 398.3 [369.2-445.4] n=3 | 200.2 [198.5-204.8] n=3 | 2.03 |
| node v24.7.0 | hono | parseBody | stream | multipart, 128 MB file | lower | 258.8 [248.4-308.5] n=5 | 499.7 [487.7-843.6] n=5 | 623.0 [539.9-1001.8] n=3 | 285.2 [241.0-311.8] n=3 | 287.4 [246.4-326.1] n=3 | 341.0 [255.0-360.8] n=3 | 662.1 [536.3-678.8] n=3 | 272.0 [265.6-377.4] n=3 | 1.93 |
| node v24.7.0 | hono | parseBody | bytes | urlencoded | lower | 1.2 [1.1-1.4] n=5 | 1.8 [1.6-2.3] n=5 |  |  |  |  |  |  | 1.52 |
| node v24.7.0 | hono | parseBody | bytes | multipart-small | lower | 2.6 [2.4-3.0] n=5 | 2.3 [2.1-3.5] n=5 |  |  |  |  |  |  | 0.88 |
| node v24.7.0 | hono | parseBody | bytes | multipart, 1 MB file | lower | 4.9 [3.0-7.1] n=5 | 6.7 [6.0-9.2] n=5 |  |  |  |  |  |  | 1.37 |
| node v24.7.0 | hono | parseBody | bytes | multipart, 8 MB file | lower | 14.0 [10.6-16.7] n=5 | 28.8 [24.5-37.0] n=5 |  |  |  |  |  |  | 2.05 |
| node v24.7.0 | hono | parseBody | bytes | multipart, 32 MB file | lower | 43.9 [39.7-50.9] n=5 | 104.5 [104.1-114.5] n=5 |  |  |  |  |  |  | 2.38 |
| node v24.7.0 | hono | parseBody | bytes | multipart, 100 MB file | lower | 116.6 [113.3-131.5] n=5 | 317.0 [285.6-374.2] n=5 |  |  |  |  |  |  | 2.72 |
| node v24.7.0 | hono | parseBody | bytes | multipart, 128 MB file | lower | 268.9 [185.5-410.6] n=5 | 727.3 [524.2-837.7] n=5 |  |  |  |  |  |  | 2.71 |
| bun 1.3.10 | hono | parseBody | stream | urlencoded | lower | 1.9 [1.5-11.8] n=5 | 2.3 [1.9-3.0] n=5 | 2.0 [1.7-2.7] n=3 | 1.8 [1.4-2.0] n=3 | 2.1 [1.5-3.9] n=3 | 2.4 [1.4-2.4] n=3 | 2.9 [1.5-3.8] n=3 | 2.3 [1.6-2.7] n=3 | 1.20 |
| bun 1.3.10 | hono | parseBody | stream | multipart-small | lower | 1.8 [1.3-3.5] n=5 | 2.3 [1.9-4.9] n=5 | 3.3 [2.3-3.7] n=3 | 2.1 [1.4-2.8] n=3 | 2.0 [1.7-2.4] n=3 | 1.9 [1.9-3.2] n=3 | 1.9 [1.8-2.8] n=3 | 2.3 [2.2-2.4] n=3 | 1.31 |
| bun 1.3.10 | hono | parseBody | stream | multipart, 1 MB file | lower | 6.7 [5.1-9.0] n=5 | 6.7 [6.5-7.5] n=5 | 6.3 [5.3-6.5] n=3 | 11.4 [7.4-11.6] n=3 | 6.4 [6.2-7.7] n=3 | 5.3 [5.2-5.7] n=3 | 7.4 [5.9-7.8] n=3 | 6.4 [5.1-6.7] n=3 | 1.00 |
| bun 1.3.10 | hono | parseBody | stream | multipart, 8 MB file | lower | 20.3 [18.3-23.0] n=5 | 30.2 [23.5-33.3] n=5 | 30.9 [23.7-32.3] n=3 | 20.5 [19.3-22.7] n=3 | 20.4 [18.9-22.9] n=3 | 21.4 [18.3-25.1] n=3 | 24.9 [21.3-27.5] n=3 | 18.9 [16.4-19.9] n=3 | 1.49 |
| bun 1.3.10 | hono | parseBody | stream | multipart, 32 MB file | lower | 76.3 [59.4-90.2] n=5 | 86.3 [84.7-134.6] n=5 | 93.4 [84.7-103.0] n=3 | 68.1 [64.3-76.2] n=3 | 71.5 [69.9-71.7] n=3 | 69.0 [64.9-69.7] n=3 | 89.4 [89.2-120.1] n=3 | 64.9 [62.7-72.7] n=3 | 1.13 |
| bun 1.3.10 | hono | parseBody | stream | multipart, 100 MB file | lower | 213.8 [188.7-241.0] n=5 | 286.3 [249.2-350.2] n=5 | 265.9 [256.8-364.3] n=3 | 203.0 [196.3-223.2] n=3 | 198.7 [193.2-255.4] n=3 | 193.4 [186.5-203.8] n=3 | 304.6 [274.7-306.7] n=3 | 199.8 [198.9-210.6] n=3 | 1.34 |
| bun 1.3.10 | hono | parseBody | stream | multipart, 128 MB file | lower | 250.4 [241.6-263.3] n=5 | 380.1 [340.0-425.9] n=5 | 340.0 [336.9-358.3] n=3 | 237.1 [233.7-287.9] n=3 | 239.3 [238.9-271.3] n=3 | 242.2 [232.9-251.2] n=3 | 350.1 [307.0-362.4] n=3 | 250.9 [246.7-251.4] n=3 | 1.52 |
| bun 1.3.10 | hono | parseBody | bytes | urlencoded | lower | 0.330 [0.284-0.466] n=5 | 0.551 [0.445-0.768] n=5 |  |  |  |  |  |  | 1.67 |
| bun 1.3.10 | hono | parseBody | bytes | multipart-small | lower | 0.303 [0.281-0.354] n=5 | 0.541 [0.387-0.639] n=5 |  |  |  |  |  |  | 1.79 |
| bun 1.3.10 | hono | parseBody | bytes | multipart, 1 MB file | lower | 1.1 [1.0-1.3] n=5 | 2.0 [1.7-2.3] n=5 |  |  |  |  |  |  | 1.81 |
| bun 1.3.10 | hono | parseBody | bytes | multipart, 8 MB file | lower | 6.7 [5.9-7.2] n=5 | 13.1 [12.0-14.0] n=5 |  |  |  |  |  |  | 1.95 |
| bun 1.3.10 | hono | parseBody | bytes | multipart, 32 MB file | lower | 18.8 [17.8-22.8] n=5 | 39.0 [35.3-44.6] n=5 |  |  |  |  |  |  | 2.07 |
| bun 1.3.10 | hono | parseBody | bytes | multipart, 100 MB file | lower | 90.9 [64.3-93.6] n=5 | 158.1 [133.4-180.1] n=5 |  |  |  |  |  |  | 1.74 |
| bun 1.3.10 | hono | parseBody | bytes | multipart, 128 MB file | lower | 77.1 [70.9-127.3] n=5 | 148.4 [136.6-185.8] n=5 |  |  |  |  |  |  | 1.93 |

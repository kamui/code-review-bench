Median parseBody() time per request in milliseconds: the median over three fresh processes of each process's median, with the lowest and highest in brackets.

| runtime | body | base ms | head ms | fix-stream-plain ms |
| --- | --- | --- | --- | --- |
| bun 1.3.10 | urlencoded, 26 bytes | 0.001 [0.001-0.002] | 0.003 [0.003-0.004] | 0.004 [0.003-0.004] |
| bun 1.3.10 | multipart, 1 MB file | 0.113 [0.103-0.126] | 0.195 [0.180-0.204] | 0.077 [0.072-0.089] |
| bun 1.3.10 | multipart, 8 MB file | 3.937 [3.935-4.004] | 5.117 [5.075-5.288] | 4.568 [4.367-4.670] |
| bun 1.3.10 | multipart, 100 MB file | 70.889 [70.058-86.655] | 119.924 [107.618-148.533] | 59.389 [56.751-67.241] |
| node v24.7.0 | urlencoded, 26 bytes | 0.004 [0.004-0.005] | 0.009 [0.009-0.010] | 0.007 [0.007-0.008] |
| node v24.7.0 | multipart, 1 MB file | 1.177 [1.167-1.201] | 1.561 [1.527-1.603] | 1.327 [1.214-1.471] |
| node v24.7.0 | multipart, 8 MB file | 3.860 [3.840-4.174] | 12.292 [11.579-14.341] | 4.440 [4.402-4.720] |
| node v24.7.0 | multipart, 100 MB file | 143.646 [138.533-149.378] | 349.760 [329.206-357.426] | 117.889 [117.537-127.511] |

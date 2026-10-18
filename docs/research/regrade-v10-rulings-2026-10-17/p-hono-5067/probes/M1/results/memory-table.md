
Peak, MiB above baseline, one 100 MB streamed upload, three fresh processes:

| runtime | path | base | head | fix-reuse | fix-stream-both | fix-reuse-and-stream | fix-native-lower | fix-release | guard-only |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| node | hono | 313 | 445 to 446 | 446 to 513 | 313 | 313 | 313 | 445 to 513 | 313 |
| node | plain | 312 to 313 | 445 to 509 | 445 | 313 | 312 to 313 | 312 to 313 | 445 to 509 | 313 |
| bun | hono | 213 | 310 | 310 | 210 to 211 | 210 to 211 | 210 to 211 | 310 to 311 | 211 to 214 |
| bun | plain | 209 to 213 | 309 to 310 | 310 | 210 | 210 to 211 | 210 | 309 to 310 | 209 to 210 |

Held after the read, MiB above baseline, one 100 MB streamed upload, three fresh processes:

| runtime | path | base | head | fix-reuse | fix-stream-both | fix-reuse-and-stream | fix-native-lower | fix-release | guard-only |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| node | hono | 145 to 146 | 245 to 246 | 246 to 313 | 145 to 170 | 145 to 209 | 145 | 145 to 213 | 203 to 213 |
| node | plain | 145 to 209 | 145 to 209 | 145 | 145 to 209 | 146 to 209 | 145 | 145 to 209 | 145 to 146 |
| bun | hono | 113 | 210 | 210 | 110 | 110 to 111 | 110 to 111 | 210 to 211 | 110 to 113 |
| bun | plain | 109 to 112 | 209 to 210 | 209 to 210 | 110 | 109 to 110 | 109 to 110 | 209 to 210 | 109 to 110 |

Time of the read, ms (includes making the upload's chunks), one 100 MB streamed upload, three fresh processes:

| runtime | path | base | head | fix-reuse | fix-stream-both | fix-reuse-and-stream | fix-native-lower | fix-release | guard-only |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| node | hono | 188 to 253 | 373 to 446 | 363 to 401 | 180 to 189 | 171 to 188 | 179 to 186 | 353 to 422 | 179 to 182 |
| node | plain | 165 to 178 | 351 to 411 | 326 to 355 | 168 to 179 | 172 to 186 | 173 to 181 | 371 to 408 | 166 to 174 |
| bun | hono | 166 to 174 | 234 to 254 | 229 to 278 | 162 to 213 | 162 to 178 | 161 to 171 | 232 to 246 | 169 to 186 |
| bun | plain | 161 to 178 | 226 to 239 | 227 to 239 | 165 to 179 | 167 to 177 | 162 to 168 | 225 to 252 | 165 to 178 |

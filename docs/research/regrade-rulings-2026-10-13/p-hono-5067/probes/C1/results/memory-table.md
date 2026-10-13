| runtime | upload | mode | base: peak | base: retained | head: peak | head: retained | fix-stream-both: peak | fix-stream-both: retained | fix-stream-plain: peak | fix-stream-plain: retained | fix-stream-hono: peak | fix-stream-hono: retained |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| bun 1.3.10 | 8 MB | plain | 25, 25, 25 | 25, 25, 25 | 32, 33, 33 | 32, 33, 33 | 25, 26, 25 | 24, 25, 25 | 25, 25, 25 | 25, 24, 25 | 33, 33, 32 | 33, 33, 32 |
| bun 1.3.10 | 8 MB | hono | 25, 26, 25 | 25, 26, 25 | 32, 33, 33 | 32, 33, 33 | 24, 24, 24 | 24, 24, 24 | 32, 32, 33 | 32, 32, 33 | 25, 25, 25 | 25, 25, 25 |
| bun 1.3.10 | 8 MB | native | 25, 25, 25 | 25, 25, 25 | 25, 25, 25 | 25, 25, 25 | 24, 25, 24 | 24, 25, 24 | 25, 25, 24 | 24, 24, 24 | 24, 25, 24 | 24, 25, 24 |
| bun 1.3.10 | 50 MB | plain | 110, 110, 110 | 110, 110, 110 | 160, 160, 160 | 160, 159, 160 | 110, 110, 110 | 110, 110, 110 | 110, 110, 110 | 109, 109, 109 | 159, 159, 159 | 158, 159, 159 |
| bun 1.3.10 | 50 MB | hono | 110, 110, 111 | 110, 110, 111 | 160, 160, 159 | 159, 159, 159 | 109, 110, 110 | 109, 110, 110 | 159, 160, 160 | 159, 160, 160 | 109, 110, 110 | 109, 110, 109 |
| bun 1.3.10 | 50 MB | native | 110, 110, 110 | 110, 110, 109 | 110, 110, 109 | 110, 110, 109 | 109, 110, 109 | 109, 109, 109 | 109, 109, 109 | 109, 109, 109 | 109, 110, 109 | 109, 110, 109 |
| bun 1.3.10 | 100 MB | plain | 214, 214, 214 | 114, 114, 114 | 313, 314, 313 | 214, 213, 213 | 213, 214, 214 | 113, 113, 113 | 213, 214, 214 | 113, 113, 113 | 314, 313, 313 | 213, 213, 214 |
| bun 1.3.10 | 100 MB | hono | 214, 214, 214 | 114, 114, 113 | 313, 314, 313 | 213, 214, 213 | 213, 214, 213 | 113, 113, 113 | 314, 313, 313 | 213, 213, 213 | 214, 213, 213 | 113, 113, 113 |
| bun 1.3.10 | 100 MB | native | 214, 214, 214 | 114, 114, 114 | 213, 213, 213 | 113, 113, 113 | 214, 213, 214 | 113, 113, 113 | 214, 214, 213 | 114, 113, 113 | 213, 213, 213 | 113, 113, 113 |
| node v24.7.0 | 8 MB | plain | 34, 35, 35 | 27, 27, 25 | 58, 59, 59 | 27, 27, 27 | 35, 35, 35 | 27, 27, 27 | 35, 35, 35 | 27, 27, 27 | 58, 59, 59 | 27, 27, 27 |
| node v24.7.0 | 8 MB | hono | 34, 35, 35 | 26, 27, 27 | 58, 51, 59 | 34, 35, 35 | 35, 34, 35 | 27, 26, 27 | 59, 59, 59 | 35, 35, 35 | 34, 34, 35 | 26, 26, 27 |
| node v24.7.0 | 8 MB | native | 35, 34, 34 | 27, 27, 27 | 35, 35, 35 | 27, 27, 27 | 35, 35, 35 | 27, 27, 27 | 35, 35, 35 | 27, 27, 27 | 35, 35, 35 | 27, 27, 24 |
| node v24.7.0 | 50 MB | plain | 162, 161, 162 | 95, 94, 95 | 312, 312, 312 | 94, 95, 94 | 162, 161, 162 | 94, 93, 94 | 162, 163, 162 | 95, 95, 95 | 312, 212, 311 | 95, 94, 94 |
| node v24.7.0 | 50 MB | hono | 162, 162, 162 | 94, 94, 95 | 312, 312, 312 | 144, 145, 144 | 162, 162, 162 | 95, 95, 94 | 313, 312, 312 | 145, 144, 144 | 162, 162, 162 | 95, 95, 94 |
| node v24.7.0 | 50 MB | native | 162, 162, 162 | 94, 95, 95 | 162, 162, 162 | 95, 94, 94 | 162, 162, 162 | 95, 94, 94 | 162, 162, 162 | 94, 94, 95 | 162, 162, 162 | 94, 94, 95 |
| node v24.7.0 | 100 MB | plain | 313, 313, 313 | 145, 146, 203 | 502, 445, 445 | 202, 145, 145 | 313, 313, 313 | 145, 145, 139 | 313, 313, 313 | 145, 145, 146 | 445, 445, 446 | 146, 145, 146 |
| node v24.7.0 | 100 MB | hono | 312, 313, 313 | 144, 145, 145 | 445, 445, 445 | 245, 246, 245 | 313, 313, 312 | 145, 145, 145 | 446, 446, 445 | 246, 246, 245 | 313, 313, 313 | 139, 145, 145 |
| node v24.7.0 | 100 MB | native | 312, 313, 313 | 145, 145, 209 | 312, 313, 313 | 144, 146, 146 | 312, 313, 313 | 145, 146, 145 | 313, 313, 313 | 145, 145, 145 | 312, 313, 313 | 145, 145, 203 |

Tracking pass, 100 MB upload: calls of Request.prototype.arrayBuffer, and whether the buffer it returned is still reachable after the parse and a forced collection, while the request and the parsed result are alive:
- base, node v24.7.0, plain: calls 0, still reachable [], bodyCache keys None
- base, node v24.7.0, hono: calls 0, still reachable [], bodyCache keys ['formData']
- base, bun 1.3.10, plain: calls 0, still reachable [], bodyCache keys None
- base, bun 1.3.10, hono: calls 0, still reachable [], bodyCache keys ['formData']
- base: 0 processes exited with an error
- head, node v24.7.0, plain: calls 1, still reachable [False], bodyCache keys None
- head, node v24.7.0, hono: calls 1, still reachable [True], bodyCache keys ['arrayBuffer', 'formData']
- head, bun 1.3.10, plain: calls 1, still reachable [False], bodyCache keys None
- head, bun 1.3.10, hono: calls 1, still reachable [True], bodyCache keys ['arrayBuffer', 'formData']
- head: 0 processes exited with an error
- fix-stream-both, node v24.7.0, plain: calls 0, still reachable [], bodyCache keys None
- fix-stream-both, node v24.7.0, hono: calls 0, still reachable [], bodyCache keys ['formData']
- fix-stream-both, bun 1.3.10, plain: calls 0, still reachable [], bodyCache keys None
- fix-stream-both, bun 1.3.10, hono: calls 0, still reachable [], bodyCache keys ['formData']
- fix-stream-both: 0 processes exited with an error
- fix-stream-plain, node v24.7.0, plain: calls 0, still reachable [], bodyCache keys None
- fix-stream-plain, node v24.7.0, hono: calls 1, still reachable [True], bodyCache keys ['arrayBuffer', 'formData']
- fix-stream-plain, bun 1.3.10, plain: calls 0, still reachable [], bodyCache keys None
- fix-stream-plain, bun 1.3.10, hono: calls 1, still reachable [True], bodyCache keys ['arrayBuffer', 'formData']
- fix-stream-plain: 0 processes exited with an error
- fix-stream-hono, node v24.7.0, plain: calls 1, still reachable [False], bodyCache keys None
- fix-stream-hono, node v24.7.0, hono: calls 0, still reachable [], bodyCache keys ['formData']
- fix-stream-hono, bun 1.3.10, plain: calls 1, still reachable [False], bodyCache keys None
- fix-stream-hono, bun 1.3.10, hono: calls 0, still reachable [], bodyCache keys ['formData']
- fix-stream-hono: 0 processes exited with an error

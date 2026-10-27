# Compiler diagnostics by scenario

Each cell counts diagnostics, including TS2578 when an intentionally invalid call is accepted. Zero means the saved probes for that column compile and their expected invalid calls are refused. Counts describe this probe set, not incident frequency or performance.

| variant | literal | PE-j1 | GT-j4 | GT-j7 | GT-j8 | GT-j9 |
| --- | --- | --- | --- | --- | --- | --- |
| base | 2 | 5 | 1 | 5 | 5 | 4 |
| head | 2 | 5 | 1 | 5 | 5 | 4 |
| literal-only | 0 | 5 | 1 | 5 | 3 | 4 |
| pe-j1-only | 2 | 0 | 0 | 3 | 5 | 4 |
| literal-and-pe-j1 | 0 | 0 | 0 | 0 | 0 | 4 |
| brand-only | 2 | 5 | 0 | 5 | 5 | 4 |
| array-only | 2 | 5 | 1 | 0 | 5 | 4 |
| literal-and-brand | 0 | 5 | 0 | 5 | 3 | 4 |
| literal-and-array | 0 | 5 | 1 | 0 | 3 | 4 |
| union-only | 2 | 5 | 1 | 5 | 0 | 4 |
| literal-and-union | 0 | 5 | 1 | 5 | 0 | 4 |
| optional-pairs-only | 2 | 5 | 1 | 5 | 5 | 0 |
| literal-and-optional-pairs | 0 | 5 | 1 | 5 | 3 | 0 |
| builder-input-intersection | 0 | 0 | 0 | 0 | 0 | 0 |

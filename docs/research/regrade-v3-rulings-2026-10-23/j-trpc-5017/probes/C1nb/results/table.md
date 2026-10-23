### known
| probe | base | head | pe-j1-copy-intersection | fn-check | both-copy-intersection-and-fn-check | gt-j7-array-check | array-or-fn-check | both-copy-intersection-and-array-or-fn-check | plain-record-guard |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| GT-j7-04-uses-tuple-spread.ts | FAIL(2556) | FAIL(2556) | FAIL(2556) | FAIL(2556) | FAIL(2556) | ok | ok | ok | ok |
| GT-j7-13-promise-all.ts | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) | ok | ok | ok | ok |
| GT-j7-15-object-values.ts | FAIL(2322) | FAIL(2322) | ok | FAIL(2322) | ok | ok | ok | ok | ok |
| PE-j1-07-uses-class-private.ts | FAIL(2345) | FAIL(2345) | ok | FAIL(2345) | ok | FAIL(2345) | FAIL(2345) | ok | ok |
| PE-j1-optional-1-call.ts | FAIL(2345) | FAIL(2345) | ok | FAIL(2345) | ok | FAIL(2345) | FAIL(2345) | ok | FAIL(2345) |
| PE-j1-optional-4-concat.ts | FAIL(2345) | FAIL(2345) | ok | FAIL(2345) | ok | FAIL(2345) | FAIL(2345) | ok | FAIL(2345) |
| control-string-input.ts | FAIL(2322,2345) | ok | ok | ok | ok | ok | ok | ok | ok |

### C1n
| probe | base | head | pe-j1-copy-intersection | fn-check | both-copy-intersection-and-fn-check | gt-j7-array-check | array-or-fn-check | both-copy-intersection-and-array-or-fn-check | plain-record-guard |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| f-01-function-input-resolver-call.ts | FAIL(2349) | FAIL(2349) | ok | ok | ok | FAIL(2349) | ok | ok | ok |
| f-02-control-no-middleware.ts | ok | ok | ok | ok | ok | ok | ok | ok | ok |
| f-03-function-input-callers.ts | FAIL(2578,7006) | FAIL(2578,7006) | ok | ok | ok | FAIL(2578,7006) | ok | ok | ok |
| f-04-plain-function-parser.ts | FAIL(2349) | FAIL(2349) | ok | ok | ok | FAIL(2349) | ok | ok | ok |
| f-05-middleware-before-input.ts | ok | ok | ok | ok | ok | ok | ok | ok | ok |
| f-06-function-inside-object.ts | ok | ok | ok | ok | ok | ok | ok | ok | ok |
| f-08-ways-round.ts | ok | ok | ok | ok | ok | ok | ok | ok | ok |

### C1
| probe | base | head | pe-j1-copy-intersection | fn-check | both-copy-intersection-and-fn-check | gt-j7-array-check | array-or-fn-check | both-copy-intersection-and-array-or-fn-check | plain-record-guard |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| c1-01-helper-types.ts | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) |
| c1-02-class-private-resolver.ts | FAIL(2345) | FAIL(2345) | ok | FAIL(2345) | ok | FAIL(2345) | FAIL(2345) | ok | ok |
| c1-03-class-public-only-resolver.ts | ok | ok | ok | ok | ok | ok | ok | ok | ok |
| c1-04-class-es-private-and-protected.ts | FAIL(2345) | FAIL(2345) | ok | FAIL(2345) | ok | FAIL(2345) | FAIL(2345) | ok | ok |
| c1-05-class-public-uses.ts | ok | ok | ok | ok | ok | ok | ok | ok | ok |
| c1-06-array-input-types.ts | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) | FAIL(2322) | ok | ok | ok | ok |
| c1-07-homomorphic-copy.ts | FAIL(2345,2741) | FAIL(2345,2741) | FAIL(2345) | FAIL(2345,2741) | FAIL(2345) | FAIL(2345,2741) | FAIL(2345,2741) | FAIL(2345) | FAIL(2345,2741) |


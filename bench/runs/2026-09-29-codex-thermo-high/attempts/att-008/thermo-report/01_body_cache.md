# Request body parsing and cache contract

## Finding

At `src/utils/body.ts:130`, the new cache write coerces `Promise<FormData>` through `unknown` into `FormData`. That is not the value being written. `HonoRequest.#cachedBody()` returns an existing cache entry immediately (`src/request.ts:220-225`) and `formData()` exposes that result as a promise (`src/request.ts:329`), so this new write happens to work at runtime because the stored value is a promise. The cast suppresses rather than documents this invariant.

The surrounding cache already has inconsistent representations: `BodyCache` is declared as `Partial<Body>` (`src/request.ts:1-22`), which describes resolved body values, while `#cachedBody()` stores the result of `raw[key]()` (`src/request.ts:238`), which is promise-valued. The validator awaits its parse and stores resolved `FormData` (`src/validator/validator.ts:115-121`). Its consumers also await the cache value, which tolerates both representations, but this means call sites cannot rely on one stable contract. The PR adds another write to the same public cache and makes the mismatch more explicit with a double cast.

## Why this is actionable

The mixed-case parser needs to preserve a parsed form for later `c.req.formData()` calls after consuming the request bytes. Caching that result is justified. The maintainability problem is that the cache shape is not typed to match how request methods use it, and this change hides that problem at the new write site. A later caller can treat an entry as a `FormData` based on the declaration even though it may actually be a promise.

## Worked code-judo proposal

Make the cache itself express the asynchronous body-read operation, for example as a mapped type whose properties are `Promise<Body[K]>`. Have `#cachedBody()` populate and return those typed promises. In `parseFormData()`, assign `formDataPromise` directly. In the validator, assign the promise returned by `bufferToFormData()` rather than awaiting it before caching, then await the selected promise for local processing. If public/internal typing constraints make that representation unsuitable, centralize reads and writes through one typed method that accepts a body key and a promise. Either shape deletes both the unsafe cast and the split between resolved and pending cache entries.

A minimal shape for the internal invariant is:

```ts
type BodyCache = Partial<{ [K in keyof Body]: Promise<Body[K]> }>
```

The accessor must then be typed so reading one key yields its corresponding `Promise<Body[K]>`; keep any conversion from a previously cached body type inside that accessor. This is a proposal, not an applied patch.

## Measurements and verification status

- The implementation files measured with `wc -l` are `src/utils/body.ts` (240), `src/utils/buffer.ts` (117), and `src/validator/validator.ts` (185). None crosses the skill's 1,000-line threshold.
- `git diff --check main...review-head` produced no output.
- Static review only. Tests were not run; the review execution policy permits focused tests but does not require them, and this review did not request execution.
- The finding is anchored to changed line `src/utils/body.ts:130`; cache declarations and consumers above are unchanged context.

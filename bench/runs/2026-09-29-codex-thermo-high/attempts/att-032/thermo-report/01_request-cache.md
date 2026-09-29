# Request body cache

## Finding

In `src/utils/body.ts:126-132`, `parseFormData()` stores the in-flight `Promise<FormData>` in `request.bodyCache.formData` by casting it through `unknown` to `FormData`. The runtime intent is sound: once `arrayBuffer()` has consumed the request stream, the pending form parse must be shared with a later `c.req.formData()` call. The declared contract is not sound, though. `BodyCache` in `src/request.ts:20-27` describes resolved values, but `#cachedBody()` in `src/request.ts:220-238` stores raw body method promises and treats an existing cache entry as a promise. This change adds a second, explicit path that relies on that mismatch and makes the cast a maintenance hazard for every future body reader.

## Evidence and measurements

- `src/utils/body.ts:126` first awaits the Hono request's cached `arrayBuffer()` so the underlying stream remains available through the body cache.
- `src/utils/body.ts:127-132` starts `bufferToFormData()`, stores its promise, then awaits that same promise to convert fields.
- `src/request.ts:27` declares `BodyCache = Partial<Body>`, whose body members are resolved values.
- `src/request.ts:230` separately casts an existing cache entry to `Promise<BodyInit>` before deriving another body representation.
- The affected implementation files remain small (`body.ts` 240 lines; `request.ts` is the existing request abstraction). This is a contract clarity issue, not file-size growth.

## Worked code-judo proposal

Represent the cache as pending values, for example a mapped type keyed by each body method whose entry is `Promise<Body[K]>`. Make `#cachedBody()` return the cached promise for that key, derive other body representations from a cached promise when needed, and cache the raw request method's promise on first access. Then assign `formDataPromise` directly to the form-data cache slot in `parseFormData()`. This makes the existing runtime model explicit, deletes the `unknown` bridge, and keeps the useful in-flight deduplication behavior.

The refactor should preserve the current special handling for cached JSON before constructing another `Response`; it should only clarify that all cache entries are asynchronous values. Check all body methods (`json`, `text`, `arrayBuffer`, `blob`, `formData`) against the same promise-shaped contract.

## Verification status

Static inspection only. Existing coverage in `src/request.test.ts` checks body methods after `parseBody()` and the validator tests cover a cached parse before validation. No tests were run during this review.

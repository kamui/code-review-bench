# 02 — Media-type matching and the tests that cover it

Carries the evidence for summary findings F4 and F6. Probe mechanics are described at the top of `01_form-body-pipeline.md`.

## F4 — three mechanisms for one rule

The rule the issue asks for is one sentence: compare the media type of a `Content-Type` value without regard to case, and leave the parameters alone. The head implements it three times.

`src/utils/body.ts:104-106` extracts and compares:

```ts
const mediaType = contentType?.split(';')[0].trim().toLowerCase()
if (mediaType === 'multipart/form-data' || mediaType === 'application/x-www-form-urlencoded') {
```

`src/utils/buffer.ts:113` rewrites the header in place:

```ts
'Content-Type': contentType.replace(/^[^;]+/, (mediaType) => mediaType.toLowerCase()),
```

`src/validator/validator.ts:24-26` makes three whole-header regexes case-insensitive:

```ts
const jsonRegex = /^application\/([a-z-\.]+\+)?json(;\s*[a-zA-Z0-9\-]+\=([^;]+))*$/i
const multipartRegex = /^multipart\/form-data(;\s?boundary=[a-zA-Z0-9'"()+_,\-./:=?]+)?$/i
const urlencodedRegex = /^application\/x-www-form-urlencoded(;\s*[a-zA-Z0-9\-]+\=([^;]+))*$/i
```

Each is individually correct for its inputs. The `i` flag also relaxes the `boundary` parameter name and the structured-suffix prefix class, both of which are case-insensitive by specification, and every character class that guards a parameter value already listed both cases, so it does not loosen value matching. The objection is structural: a reader who wants to know "what does Hono consider a form request" has to reconcile an exact-match on a trimmed prefix, a regex over the full header, and a rewrite, and the next call site will invent a fourth.

Note also that `parseBody`'s guard changed semantics slightly beyond case: the base used `startsWith`, the head uses equality on the part before the first `;`. A value such as `multipart/form-data-x` matched before and does not now. That is more correct; it is simply not mentioned.

A pre-existing disagreement that a shared helper would make visible (not introduced by this PR, listed for context only): `parseBody` accepts `multipart/form-data ; boundary=x` and `multipart/form-data; charset=utf-8; boundary=x`, while the validator's `multipartRegex` accepts neither, so the two entry points still disagree about which requests are forms. This is from reading the expressions; I did not probe it.

### The missed sibling: `method-override`

`src/middleware/method-override/index.ts:69-92` on the head:

```ts
const contentType = c.req.header('content-type')
…
if (contentType?.startsWith('multipart/form-data')) {
…
if (contentType?.startsWith('application/x-www-form-urlencoded')) {
  const params = await parseBody<Record<string, string>>(clonedRequest)
```

This is the exact pattern the PR removed from `body.ts`, and the second branch is a caller of `parseBody`. Probe `probe/scen-body.ts`, case S9: an app with `methodOverride({ app })`, a `DELETE /` handler returning `DELETE` and a `POST /` handler returning `POST`.

| Request | Base | Head | With shared helper |
| --- | --- | --- | --- |
| `Content-Type: application/x-www-form-urlencoded`, body `_method=DELETE` | `DELETE` | `DELETE` | `DELETE` |
| `Content-Type: Application/X-WWW-Form-Urlencoded`, body `_method=DELETE` | `POST` | `POST` | `DELETE` |

Status: verified by execution. Issue #5060 names only `parseBody` and `validator`, so this is not a regression; it is the same defect, one `grep` away, left in place by a PR whose stated purpose is to normalize media-type matching. I searched the non-test sources for the form and JSON media-type literals; the other hits are response-side (`pretty-json`, `compress`, `context`) or already case-insensitive (`csrf` uses an `i`-flagged regex), so `method-override` is the only remaining request-side site.

### Proposed shape

One exported helper beside `parseBody`, which `request.ts`, `validator.ts` and `method-override` already import from:

```ts
/** Returns the lower-cased media type of a `Content-Type` value, without its parameters. */
export const getMediaType = (contentType: string | null | undefined): string | undefined =>
  contentType?.split(';', 1)[0].trim().toLowerCase()
```

`parseBody` uses it for its guard; `method-override` computes `const mediaType = getMediaType(c.req.header('content-type'))` once and compares with `===` in both branches. `bufferToFormData` keeps its header rewrite because that is a different job (preparing a value for the platform parser, parameters included) and it then exists exactly once. The validator can keep its regexes with the `i` flag; if the maintainers want full unification, its guard for the `form` target can become the same `getMediaType` comparison, which would also remove the pre-existing disagreement noted above, but that is a behaviour change and should be its own PR.

The `method-override` half of this was applied in the scratch copy `clone-work/judo-src` and produces the right-hand column of the table. One leftover even then: the multipart branch of `method-override` calls `clonedRequest.formData()` on the raw request, so on a platform with case-sensitive parsing it would still fail after passing the guard. Under the simulated platform that branch was not probed.

## F6 — what the new tests do and do not pin

### The guards-only variant

`clone-work/minimal-src` is the base tree with exactly two edits: the `body.ts` guard from the head (lines 104-106) and the `i` flag on the three validator regexes. `parseFormData` and `bufferToFormData` are as on the base. Probe `probe/scen-body.ts` reproduces the seven scenarios the PR's new tests exercise.

| Scenario (new test it mirrors) | Base | Guards-only | Head |
| --- | --- | --- | --- |
| S1 `parseBody` urlencoded, mixed case (`body.test.ts:45-54`) | `{}` | parsed | parsed |
| S2 `parseBody` multipart via `HonoRequest`, mixed case | `{}` | parsed | parsed |
| S3 `parseBody` multipart via `Request`, mixed case (`body.test.ts:56-69`) | `{}` | parsed | parsed |
| S4 `bufferToFormData('Multipart/Form-Data; boundary=sampleBoundary')` (`buffer.test.ts:120-132`) | parsed | parsed | parsed |
| S5 `validator('json')` with `Application/JSON` (`validator.test.ts:149-159`) | `{}` | parsed | parsed |
| S6 `validator('form')` urlencoded, mixed case (`validator.test.ts:293-307`) | `{}` | parsed | parsed |
| S7 `validator('form')` multipart, mixed case (`validator.test.ts:309-327`) | `{}` | parsed | parsed |

Two conclusions. First, S4 passes on the unmodified base: the new `bufferToFormData` test cannot fail on Node 24 whether or not `buffer.ts:113` exists. Second, the guards-only column equals the head column, so none of the new tests exercises the difference between `request.formData()` and the `arrayBuffer()` + `bufferToFormData()` + cache-write rewrite. Status: verified through the probes on Node 24. I did not run vitest against the scratch trees, so "the tests stay green" is an inference from equivalent scenarios, not a test run.

Under the simulated case-sensitive platform (`SIM=1`) the guards-only variant fails S1 to S4, S6 and S7 while the head passes them, which shows what the rewrite is for. That purpose is not written anywhere in the code, and no test encodes it.

### Gaps worth closing

The comment at `src/utils/body.ts:129` states an intent, "Cache so that a later `c.req.formData()` reuses the already-consumed body". No test calls `formData()` after `parseBody()` on a form request and checks the result. The existing `request.test.ts:367-382` only asserts that `text()`, `arrayBuffer()`, `bytes()` and `blob()` do not throw synchronously after `parseBody()`, which they could not do anyway since they return promises.

No test calls a body method before `parseBody()`. That is the matrix that would have caught F1 and that would lock in the P2 improvement:

```ts
describe.each(['multipart', 'urlencoded'])('parseBody() after another body read (%s)', (kind) => {
  it.each(['formData', 'text', 'arrayBuffer', 'blob'] as const)('%s() first', async (method) => {
    const req = new HonoRequest(createFormRequest(kind))
    await req[method]()
    expect(await req.parseBody()).toEqual({ message: 'hello' })
  })
})
```

On the head the `formData` row fails for both kinds; on the F2 variant every row should pass (probed for `formData` and `text`, not for `arrayBuffer` and `blob`).

To make the normalization in `bufferToFormData` observable on a platform that does not need it, assert on what is handed to the platform, for example by spying on `Response.prototype.formData` and checking `this.headers.get('Content-Type')` is `multipart/form-data; boundary=sampleBoundary`, exactly. The current test does protect one thing: because `sampleBoundary` contains a capital letter, an implementation that lower-cased the whole header would fail it. What it cannot detect is the normalization being absent, and that is the half the production change is about.

### Smaller notes

`src/utils/body.test.ts:84-92`: replacing the `vi.spyOn(req, 'formData')` stub with real `FormData` entries is a better test. It was forced by `parseBody` no longer calling `request.formData()`, and it is the only place the diff reveals that change.

The mixed-case multipart fixture (build a request from `FormData`, read its generated header, replace the media type, rebuild from the bytes) appears verbatim in `src/utils/body.test.ts:56-66` and `src/validator/validator.test.ts:309-321`. A two-line helper would remove the duplication; this is minor.

`src/validator/validator.test.ts` was 1440 lines on the base and is 1488 on the head. The PR did not push it across the 1000-line boundary, so the file-size rule is not triggered, but the file is long past the point where new cases should go into a focused file such as `validator.content-type.test.ts`.

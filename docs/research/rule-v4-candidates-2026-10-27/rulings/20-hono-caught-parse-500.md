# Ruling 20-hono-caught-parse-500: p-hono-5067

Decisions in this question: `M2` (candidate); `M2` (label).

## `M2`: candidate

Record: [`20-hono-caught-parse-500.M2.before.json`](20-hono-caught-parse-500.M2.before.json)

```text
ASK
- recommender: problem stays with the owner
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cases/M2.md:

# A caught malformed parse leaves a rejection that escapes the validator’s 400 mapping

## Code and dates

*read.* Base `9728702911073aec5a63a3ba2840b7240e5d3205`; head `5226d4165d48643586152614cbd07422a0ab7a22`; last push 2026-07-01T09:39:48Z. Source and documentation below existed by that cut-off unless marked later.

*read, knowable at cut-off.* body.ts:126–132 replaces the call to request.formData with a byte read and explicitly stores the pending formDataPromise at new line 130. validator.ts:115–116, unchanged, awaits the remembered form outside the try/catch at lines 118–127. Before the change request.ts:238 cached raw.formData’s promise in the same pending state.

```diff
 async function parseFormData<T extends BodyData>(
   request: HonoRequest | Request,
   options: ParseBodyOptions
 ): Promise<T> {
-  const formData = await (request as Request).formData()
+  const headers = request instanceof HonoRequest ? request.raw.headers : request.headers
+  const arrayBuffer = await (request as Request).arrayBuffer()
+  const formDataPromise = bufferToFormData(arrayBuffer, headers.get('Content-Type') || '')
+  if (request instanceof HonoRequest) {
+    // Cache so that a later `c.req.formData()` reuses the already-consumed body
+    request.bodyCache.formData = formDataPromise as unknown as FormData
+  }
+  const formData = await formDataPromise
 
   if (formData) {
     return convertFormDataToBodyData<T>(formData, options)
```

*read, knowable at cut-off.* Description from task_packet, verbatim:

```text
Fixes #5060

### The author should do the following, if applicable

- [ ] Add tests
- [ ] Run tests
- [ ] `bun run format:fix && bun run lint:fix` to format the code
- [ ] Add [TSDoc](https://tsdoc.org/)/[JSDoc](https://jsdoc.app/about-getting-started) to document the code
```

## Executions

*run, reproducible at cut-off.* On both Node and Bun, direct validation of multipart/form-data; boundary=abc with body garbage returns 400. If middleware first catches parseBody’s rejection, the validator instead returns 500 Internal Server Error at both commits.

Full per-runtime values, status codes and exceptions are saved in the probe result files; the case reproduces them here.

### base on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "direct": {
    "status": 400,
    "body": "Malformed FormData request. Failed to parse body as FormData.",
    "errors": [
      "Error: Malformed FormData request. Failed to parse body as FormData."
    ]
  },
  "afterCaught": {
    "status": 500,
    "body": "Internal Server Error",
    "errors": [
      "caught: TypeError: Failed to parse body as FormData.",
      "TypeError: Failed to parse body as FormData."
    ]
  }
}
```

### base on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "direct": {
    "status": 400,
    "body": "Malformed FormData request. FormData parse error missing final boundary",
    "errors": [
      "Error: Malformed FormData request. FormData parse error missing final boundary"
    ]
  },
  "afterCaught": {
    "status": 500,
    "body": "Internal Server Error",
    "errors": [
      "caught: TypeError [ERR_FORMDATA_PARSE_ERROR]: FormData parse error missing final boundary",
      "TypeError [ERR_FORMDATA_PARSE_ERROR]: FormData parse error missing final boundary"
    ]
  }
}
```

### head on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "direct": {
    "status": 400,
    "body": "Malformed FormData request. Failed to parse body as FormData.",
    "errors": [
      "Error: Malformed FormData request. Failed to parse body as FormData."
    ]
  },
  "afterCaught": {
    "status": 500,
    "body": "Internal Server Error",
    "errors": [
      "caught: TypeError: Failed to parse body as FormData.",
      "TypeError: Failed to parse body as FormData."
    ]
  }
}
```

### head on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "direct": {
    "status": 400,
    "body": "Malformed FormData request. FormData parse error missing final boundary",
    "errors": [
      "Error: Malformed FormData request. FormData parse error missing final boundary"
    ]
  },
  "afterCaught": {
    "status": 500,
    "body": "Internal Server Error",
    "errors": [
      "caught: TypeError [ERR_FORMDATA_PARSE_ERROR]: FormData parse error missing final boundary",
      "TypeError [ERR_FORMDATA_PARSE_ERROR]: FormData parse error missing final boundary"
    ]
  }
}
```

### candidate-place patch on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "direct": {
    "status": 400,
    "body": "Malformed FormData request. Failed to parse body as FormData.",
    "errors": [
      "Error: Malformed FormData request. Failed to parse body as FormData."
    ]
  },
  "afterCaught": {
    "status": 400,
    "body": "Malformed FormData request. Failed to parse body as FormData.",
    "errors": [
      "caught: TypeError: Failed to parse body as FormData.",
      "Error: Malformed FormData request. Failed to parse body as FormData."
    ]
  }
}
```

### candidate-place patch on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "direct": {
    "status": 400,
    "body": "Malformed FormData request. FormData parse error missing final boundary",
    "errors": [
      "Error: Malformed FormData request. FormData parse error missing final boundary"
    ]
  },
  "afterCaught": {
    "status": 400,
    "body": "Malformed FormData request. FormData parse error missing final boundary",
    "errors": [
      "caught: TypeError [ERR_FORMDATA_PARSE_ERROR]: FormData parse error missing final boundary",
      "Error: Malformed FormData request. FormData parse error missing final boundary"
    ]
  }
}
```

## Documentation and contemporary statements

*read, knowable at cut-off.* Website documentation is pinned to b17f3d3f4eb3f4a4938d95b552aba5eb8a15d255, the latest website commit before 2026-07-01T09:39:48Z. The root README and the general concepts pages, including Web Standards and Middleware, were searched as well as the API and validation/formatting pages.

*read, knowable at cut-off.* honojs/website docs/guides/validation.md:

> To validate form data, specify `form` as the first argument and a callback as the second argument.

*read, published June 2022, before the cut-off.* RFC 9110 §8.3.1: The type and subtype tokens are case-insensitive. Section 5.6.6: Parameter names are case-insensitive. Its grammar is `parameters      = *( OWS ";" OWS [ parameter ] )`.

*read, knowable at cut-off.* The project’s test is `Should return 400 response, for malformed content type header`; it asserts `expect(res.status).toBe(400)` and a `Malformed FormData request.` prefix. The exact test and catch/cache branches are in the saved validator snapshots.

## Reader-visible behaviour

*run.* A client sending a malformed form gets a generic server failure after middleware has handled the initial rejection. The client loses the validator’s bad-request status and malformed-form detail. The body is invalid either way; no valid-form corruption or process crash is shown. Handling the exception explicitly requires application code.

## Other code places and patch executions

*read, knowable at cut-off from the pinned code.* The comparison scenarios use these code places:

| Id | Behaviour described by the answer key | Place |
| --- | --- | --- |
| GT-p1 | A remembered form is ignored: a following parseBody fails or garbles fields, loses intervening edits, and parses again on each further call. | body.ts:125–132, especially reading bytes without checking bodyCache.formData. |
| GT-p2 | A formData read overlapping parseBody rejects because its rebuilt Response has no Content-Type. | body.ts:126 before line 130; request.ts:234. |
| GT-p3 | A first parseBody collects the whole upload as bytes, parses a copy, and keeps the bytes on a HonoRequest, increasing memory and first-read work. | body.ts:126–127; request.ts:238. |
| PE-p1 | A mixed-case form POST with _method=DELETE goes to POST instead of DELETE. | method-override/index.ts:74 and 92, unchanged. |

*read, knowable at cut-off.* The diff in change.diff replaces native formData delegation with a byte read at body.ts:126–127 and adds the cache write at 130. The method-override checks have no diff. Patches for the respective places are saved beside the results. GT-p1’s patch adds remembered-form reuse; GT-p2’s patch supplies the media type to the reconstructed Response; GT-p3’s patch hands an unread stream to the platform parser instead of collecting bytes; PE-p1’s patch normalizes the middleware’s media-type token.

*run, reproducible at the cut-off.* The candidate-place patch is `fix-M2.patch`. Its output is `result-fix-M2.txt`. The comparison-place patch alone, candidate-place patch alone and both together were run on Node and Bun for GT-p1, GT-p2, GT-p3 and PE-p1; per-scenario outputs and patches are under `probes/M2/`. See `grouping-matrix.md`.

*run.* Candidate-only fixes leave the distinguishing comparison failure in place, and comparison-only fixes leave this candidate in place. The raw-Request-only M5 patch repairs the plain-Request branch’s delegation but does not repair the HonoRequest first-read buffering. Patches demonstrate specific causal separations; they are not proposed production fixes, and some retain other faults.

## Impact facts

*run/read, reproducible or documented at the cut-off.* A client sending a malformed form gets a generic server failure after middleware has handled the initial rejection. The client loses the validator’s bad-request status and malformed-form detail. The body is invalid either way; no valid-form corruption or process crash is shown. Handling the exception explicitly requires application code.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

## Known only later

*read, known only later.* The description was edited at 09:41:51–09:41:54Z, after the cut-off, only to tick checklist boxes. The saved GraphQL history includes the original body. The PR merged at 09:42:27Z and shipped in v4.12.28 on 2026-07-06. The release note lists its title; it announces no reader, diagnostic, whitespace or parameter exception. #5131 later adds remembered-form reuse. Later searches found #5226 (merged 2026-08-18) replacing prettyJSON’s match with a case-insensitive JSON/suffix regex and #5377 (merged 2026-09-20) handling invalid JSON bodies. Those are later source changes, not cut-off promises or acknowledgements of every candidate. Raw responses and files are under upstream/.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | problem | medium (no-precedent) | The added cache write stores the rejected parse, so after middleware catches a malformed form the validator answers 500 instead of the 400 its own test expects; the added line itself does the wrong thing. |
| assessor-1 | outside-scope | high | The uncaught cached rejection fails at the unchanged validator await at both commits, and the change neither worsens that failure nor says it fixes that error path. |
| assessor-2 | outside-scope | high | The unchanged validator await outside its catch produces the same 500 after a caught parse rejection at both commits, and the new cache write preserves the pre-existing pending-promise behavior. |

To reconcile before asking:

- assessor-1 picks outside-scope where the recommender picks problem. Its reason: The uncaught cached rejection fails at the unchanged validator await at both commits, and the change neither worsens that failure nor says it fixes that error path.
- assessor-2 picks outside-scope where the recommender picks problem. Its reason: The unchanged validator await outside its catch produces the same 500 after a caught parse rejection at both commits, and the new cache write preserves the pre-existing pending-promise behavior.

## `M2`: label

Record: [`20-hono-caught-parse-500.M2-label.before.json`](20-hono-caught-parse-500.M2-label.before.json)

```text
ASK
- band stays on its existing owner or reconciler route
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cards/M2.md:

# A caught malformed parse leaves a rejection that escapes the validator’s 400 mapping

Hono is a web framework. Base is the commit before the change; head is the reviewed commit. The evidence below comes from the pinned source and documentation available at its last push.

## Executions

*run.* On both Node and Bun, direct validation of multipart/form-data; boundary=abc with body garbage returns 400. If middleware first catches parseBody’s rejection, the validator instead returns 500 Internal Server Error at both commits.

*run.* The candidate-place patch changes only the described code place and repairs the demonstrated case. The corresponding cache, reader and method-override patch scenarios are retained separately.

## What the affected person sees

*run/read.* A client sending a malformed form gets a generic server failure after middleware has handled the initial rejection. The client loses the validator’s bad-request status and malformed-form detail. The body is invalid either way; no valid-form corruption or process crash is shown. Handling the exception explicitly requires application code.

## Impact facts

*run/read, reproducible or documented at the cut-off.* A client sending a malformed form gets a generic server failure after middleware has handled the initial rejection. The client loses the validator’s bad-request status and malformed-form detail. The body is invalid either way; no valid-form corruption or process crash is shown. Handling the exception explicitly requires application code.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

*read.* To validate form data, specify `form` as the first argument and a callback as the second argument.

No production frequency is known. Node and Bun were executed; Deno and Cloudflare Workers were not. No timing, operating-system memory-limit failure, deployment or security exploit was measured for this case.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | other-material | medium (no-precedent) | An already invalid body still fails; only the status and message change from 400 to 500. |
| assessor-1 | other-material | medium (no-precedent) | The malformed body still fails, with only its bad-request classification and explanatory detail lost, identically at both commits. |

To reconcile before asking:

- None: every party's pick is the recommender's.


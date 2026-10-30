# Ruling 21-hono-prettyjson-case: p-hono-5067

Decisions in this question: `P2` (admission as a pre-existing problem); `P2` (label).

## `P2`: admission as a pre-existing problem

Record: [`21-hono-prettyjson-case.P2.before.json`](21-hono-prettyjson-case.P2.before.json)

```text
ASK
- pre-existing-admission stays on its existing owner or reconciler route
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cases/P2.md:

# prettyJSON leaves mixed-case JSON response media types unformatted

## Code and dates

*read.* Base `9728702911073aec5a63a3ba2840b7240e5d3205`; head `5226d4165d48643586152614cbd07422a0ab7a22`; last push 2026-07-01T09:39:48Z. Source and documentation below existed by that cut-off unless marked later.

*read, knowable at cut-off.* pretty-json/index.ts:51 is unchanged. Its case-sensitive startsWith(application/json) skips the formatting branch. No line in that middleware is touched by this pull request.

```diff
 (no diff in src/middleware/pretty-json/index.ts)
  if (pretty && c.res.headers.get('Content-Type')?.startsWith('application/json')) {
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

*run, reproducible at cut-off.* With response Content-Type Application/JSON, both ?pretty and force:true return compact JSON at both commits on Node and Bun. With application/json both options produce the expected indented JSON. Every response remains 200 and the JSON data is identical.

Full per-runtime values, status codes and exceptions are saved in the probe result files; the case reproduces them here.

### base on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "application/json:query": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "application/json:force": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "Application/JSON:query": {
    "status": 200,
    "body": "{\"foo\":\"bar\"}"
  },
  "Application/JSON:force": {
    "status": 200,
    "body": "{\"foo\":\"bar\"}"
  }
}
```

### base on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "application/json:query": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "application/json:force": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "Application/JSON:query": {
    "status": 200,
    "body": "{\"foo\":\"bar\"}"
  },
  "Application/JSON:force": {
    "status": 200,
    "body": "{\"foo\":\"bar\"}"
  }
}
```

### head on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "application/json:query": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "application/json:force": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "Application/JSON:query": {
    "status": 200,
    "body": "{\"foo\":\"bar\"}"
  },
  "Application/JSON:force": {
    "status": 200,
    "body": "{\"foo\":\"bar\"}"
  }
}
```

### head on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "application/json:query": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "application/json:force": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "Application/JSON:query": {
    "status": 200,
    "body": "{\"foo\":\"bar\"}"
  },
  "Application/JSON:force": {
    "status": 200,
    "body": "{\"foo\":\"bar\"}"
  }
}
```

### candidate-place patch on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "application/json:query": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "application/json:force": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "Application/JSON:query": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "Application/JSON:force": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  }
}
```

### candidate-place patch on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "application/json:query": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "application/json:force": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "Application/JSON:query": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  },
  "Application/JSON:force": {
    "status": 200,
    "body": "{\n  \"foo\": \"bar\"\n}"
  }
}
```

## Documentation and contemporary statements

*read, knowable at cut-off.* Website documentation is pinned to b17f3d3f4eb3f4a4938d95b552aba5eb8a15d255, the latest website commit before 2026-07-01T09:39:48Z. The root README and the general concepts pages, including Web Standards and Middleware, were searched as well as the API and validation/formatting pages.

*read, knowable at cut-off.* honojs/website docs/middleware/builtin/pretty-json.md:

> When set to `true`, JSON responses are always prettified regardless of the query parameter. The default is `false`.

*read, published June 2022, before the cut-off.* RFC 9110 §8.3.1: The type and subtype tokens are case-insensitive. Section 5.6.6: Parameter names are case-insensitive. Its grammar is `parameters      = *( OWS ";" OWS [ parameter ] )`.

## Reader-visible behaviour

*run.* A reader asking for pretty JSON receives compact JSON instead. Developers lose the requested presentation, but parsers receive the same values and status. The application can emit a lower-case media type or format the JSON itself. No deployment frequency was measured.

## Other code places and patch executions

*read, knowable at cut-off from the pinned code.* The comparison scenarios use these code places:

| Id | Behaviour described by the answer key | Place |
| --- | --- | --- |
| GT-p1 | A remembered form is ignored: a following parseBody fails or garbles fields, loses intervening edits, and parses again on each further call. | body.ts:125–132, especially reading bytes without checking bodyCache.formData. |
| GT-p2 | A formData read overlapping parseBody rejects because its rebuilt Response has no Content-Type. | body.ts:126 before line 130; request.ts:234. |
| GT-p3 | A first parseBody collects the whole upload as bytes, parses a copy, and keeps the bytes on a HonoRequest, increasing memory and first-read work. | body.ts:126–127; request.ts:238. |
| PE-p1 | A mixed-case form POST with _method=DELETE goes to POST instead of DELETE. | method-override/index.ts:74 and 92, unchanged. |

*read, knowable at cut-off.* The diff in change.diff replaces native formData delegation with a byte read at body.ts:126–127 and adds the cache write at 130. The method-override checks have no diff. Patches for the respective places are saved beside the results. GT-p1’s patch adds remembered-form reuse; GT-p2’s patch supplies the media type to the reconstructed Response; GT-p3’s patch hands an unread stream to the platform parser instead of collecting bytes; PE-p1’s patch normalizes the middleware’s media-type token.

*run, reproducible at the cut-off.* The candidate-place patch is `fix-P2.patch`. Its output is `result-fix-P2.txt`. The comparison-place patch alone, candidate-place patch alone and both together were run on Node and Bun for GT-p1, GT-p2, GT-p3 and PE-p1; per-scenario outputs and patches are under `probes/P2/`. See `grouping-matrix.md`.

*run.* Candidate-only fixes leave the distinguishing comparison failure in place, and comparison-only fixes leave this candidate in place. The raw-Request-only M5 patch repairs the plain-Request branch’s delegation but does not repair the HonoRequest first-read buffering. Patches demonstrate specific causal separations; they are not proposed production fixes, and some retain other faults.

## Impact facts

*run/read, reproducible or documented at the cut-off.* A reader asking for pretty JSON receives compact JSON instead. Developers lose the requested presentation, but parsers receive the same values and status. The application can emit a lower-case media type or format the JSON itself. No deployment frequency was measured.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

## Known only later

*read, known only later.* The description was edited at 09:41:51–09:41:54Z, after the cut-off, only to tick checklist boxes. The saved GraphQL history includes the original body. The PR merged at 09:42:27Z and shipped in v4.12.28 on 2026-07-06. The release note lists its title; it announces no reader, diagnostic, whitespace or parameter exception. #5131 later adds remembered-form reuse. Later searches found #5226 (merged 2026-08-18) replacing prettyJSON’s match with a case-insensitive JSON/suffix regex and #5377 (merged 2026-09-20) handling invalid JSON bodies. Those are later source changes, not cut-off promises or acknowledgements of every candidate. Raw responses and files are under upstream/.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | admitted | medium (no-precedent) | The guide says force:true always prettifies JSON responses, and a response whose Content-Type is Application/JSON stays compact on both runtimes. |
| assessor-1 | admitted | medium (no-precedent) | The cut-off documentation promises that force:true always prettifies JSON responses, but valid Application/JSON responses remain compact at both commits, satisfying pre-existing-admission even though their data and status are correct. |
| assessor-2 | admitted | high | Although the documentation promises that force:true always prettifies JSON responses, valid Application/JSON responses remain compact at both commits, so the promised presentation is not delivered. |

To reconcile before asking:

- None: every party's pick is the recommender's.

## `P2`: label

Record: [`21-hono-prettyjson-case.P2-label.before.json`](21-hono-prettyjson-case.P2-label.before.json)

```text
ASK
- band stays on its existing owner or reconciler route
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cards/P2.md:

# prettyJSON leaves mixed-case JSON response media types unformatted

Hono is a web framework. Base is the commit before the change; head is the reviewed commit. The evidence below comes from the pinned source and documentation available at its last push.

## Executions

*run.* With response Content-Type Application/JSON, both ?pretty and force:true return compact JSON at both commits on Node and Bun. With application/json both options produce the expected indented JSON. Every response remains 200 and the JSON data is identical.

*run.* The candidate-place patch changes only the described code place and repairs the demonstrated case. The corresponding cache, reader and method-override patch scenarios are retained separately.

## What the affected person sees

*run/read.* A reader asking for pretty JSON receives compact JSON instead. Developers lose the requested presentation, but parsers receive the same values and status. The application can emit a lower-case media type or format the JSON itself. No deployment frequency was measured.

## Impact facts

*run/read, reproducible or documented at the cut-off.* A reader asking for pretty JSON receives compact JSON instead. Developers lose the requested presentation, but parsers receive the same values and status. The application can emit a lower-case media type or format the JSON itself. No deployment frequency was measured.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

*read.* When set to `true`, JSON responses are always prettified regardless of the query parameter. The default is `false`.

No production frequency is known. Node and Bun were executed; Deno and Cloudflare Workers were not. No timing, operating-system memory-limit failure, deployment or security exploit was measured for this case.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | other-material | medium (no-precedent) | Only presentation differs; the data and status are right. |
| assessor-1 | other-material | medium (no-precedent) | The executed responses retain identical JSON values and successful status, with only the requested indentation missing. |

To reconcile before asking:

- None: every party's pick is the recommender's.


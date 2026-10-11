# 02 — Media-type matching (`parseBody` guard, validator regexes, method-override)

Covers summary findings F4 and F5.

## Inventory of "is this a form / JSON body" guards in `src/` at head

Found with a content search for `content-type` header reads, `startsWith('multipart`, `startsWith('application/`, and the two form media-type literals, excluding tests.

| Location | Mechanism | Case-insensitive? | Touched by PR |
| --- | --- | --- | --- |
| `src/utils/body.ts:104-106` | `contentType?.split(';')[0].trim().toLowerCase()` then `===` against two literals | yes (new) | yes |
| `src/utils/buffer.ts:113` | `contentType.replace(/^[^;]+/, (m) => m.toLowerCase())` before `new Response(…).formData()` | yes (new) | yes |
| `src/validator/validator.ts:24-26` | three anchored regexes over the whole header, now with the `i` flag | yes (new) | yes |
| `src/middleware/method-override/index.ts:74` | `contentType?.startsWith('multipart/form-data')` | **no** | no |
| `src/middleware/method-override/index.ts:92` | `contentType?.startsWith('application/x-www-form-urlencoded')` | **no** | no |
| `src/middleware/csrf/index.ts:29-30` | `/^\b(application\/x-www-form-urlencoded\|multipart\/form-data\|text\/plain)\b/i` | yes (already) | no |

Six sites, five distinct mechanisms, for one question. The PR adds two of the five mechanisms.

## F4 — three normalizers in one PR

### They do not agree with each other

- `parseBody()` compares only the essence, so any parameter list is accepted. `multipartRegex` in the validator allows at most one parameter and it must be `boundary=`; `multipart/form-data; boundary=x; charset=utf-8` passes `parseBody()` and is skipped by `validator('form')`. That divergence predates the PR, but the PR had both guards open and made them more different rather than less.
- The validator `i` flag applies to the whole pattern. The parameter-name and boundary character classes already listed both cases explicitly, so the flag is redundant there, and `jsonRegex`'s `([a-z-\.]+\+)?` now matches uppercase while reading as lowercase-only. A reader has to notice the trailing `i` to know what the class means.
- `bufferToFormData()` lowercases leading whitespace-and-type up to the first `;` and leaves the rest. It is the only one of the three that mutates the header rather than classifying it.

### They do not agree with the platform

`main` used `startsWith`, head uses split-and-equal. Verified difference on Node 24:

```
Content-Type: application/x-www-form-urlencoded, application/x-www-form-urlencoded   body: a=b
main → { a: 'b' }
head → {}
```

The platform's own MIME extraction handles a comma-joined (duplicated) header; the hand-rolled split does not. This is a narrow case and arguably the stricter result is fine, but it is a silent behavior change that fell out of re-implementing header parsing inline, and nothing in the PR mentions or tests it. It is listed as supporting evidence for F4, not as a separate defect.

### Proposal

One helper in `src/utils/` (the header utilities or `body.ts` itself), exported so middleware can use it:

```ts
/** Lowercased `type/subtype` of a Content-Type header value, without parameters. */
export const getMediaType = (contentType: string | null | undefined): string | undefined =>
  contentType?.split(';', 1)[0].trim().toLowerCase()
```

`parseBody()`:

```ts
const mediaType = getMediaType(headers.get('Content-Type'))
if (mediaType === 'multipart/form-data' || mediaType === 'application/x-www-form-urlencoded') {
```

With F2 applied, `bufferToFormData()` needs no normalization, so that mechanism is deleted rather than centralized. The validator can either keep its regexes with the `i` flag (acceptable; they also police parameter syntax, and were adjusted repeatedly in #3165, #3195, #3199, #3286 and #3297) or split the check into `getMediaType()` for the essence plus its existing parameter patterns. Either way the count of bespoke case-handling mechanisms the PR introduces drops from three to one, and that one has a name.

Verification status: the helper is a rename of the expression already at `src/utils/body.ts:104`; no separate probe was needed. The parameter-strictness comparison between `parseBody()` and `multipartRegex` is by reading the regex, not by probe.

## F5 — method-override left behind

`src/middleware/method-override/index.ts:69-103`:

```ts
const contentType = c.req.header('content-type')
…
if (contentType?.startsWith('multipart/form-data')) { … }
…
if (contentType?.startsWith('application/x-www-form-urlencoded')) {
  const params = await parseBody<Record<string, string>>(clonedRequest)
```

This is the same guard shape the issue reports for `parseBody()` (`src/utils/body.ts:103-105` at the merge-base), and the second branch feeds directly into `parseBody()`.

Probe (head, Node 24), app with `methodOverride({ app })` on `/posts`, a `DELETE /posts` handler returning `DELETE`, and a `POST /posts` handler returning `{ handler: 'POST', body: await c.req.parseBody() }`; request body `_method=DELETE&a=b`:

```
application/x-www-form-urlencoded     => 200 DELETE
Application/X-WWW-Form-Urlencoded     => 200 {"handler":"POST","body":{"_method":"DELETE","a":"b"}}
```

Before the PR the second request was at least uniformly ignored (`parseBody()` returned `{}`). After it, the form is parsed by the handler but not by the middleware that is supposed to act on it, and the `_method` field leaks through to application code as ordinary data. The severity is low, since browsers send these media types in lowercase, but that is equally true of the cases the PR does fix; the issue is about non-browser clients.

Remedy:

```ts
const mediaType = getMediaType(c.req.header('content-type'))
…
if (mediaType === 'multipart/form-data') { … }
if (mediaType === 'application/x-www-form-urlencoded') { … }
```

plus one mixed-case test in `src/middleware/method-override/index.test.ts`.

CSRF was checked for the same gap because a case-sensitive guard there combined with a newly case-insensitive `parseBody()` would have been a security regression. It is not affected: `isRequestedByFormElementRe` already carries the `i` flag.

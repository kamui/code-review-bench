# M1a: a multipart header with a comma inside its media type now gives `{}` where Node and Deno raised an error

Hono pull request 5067. Base `9728702` (the commit before the change), head `5226d41` (the reviewed commit). Cut-off: the last push, 2026-07-01T09:39:48Z.

## Why the packet's group M1 is split

The packet's group M1 describes one thing in different words: the change's new exact test of the media type sends fewer headers to the form reader than the old prefix test did, and `parseBody()` now returns `{}` for them. They name two kinds of header: `multipart/form-data,foo` and `multipart/form-data-extra`.

Both kinds go wrong on the same line, for the same reason. But they are two different kinds of input, and the rules sort them differently:

- **M1a, this dossier.** `multipart/form-data,foo`, with or without `; boundary=X`. The value is not a valid media type: a comma is not allowed inside one (RFC 9110, section 5.6.2), and Content-Type does not allow a list of values (section 5.3). The platform's own reader reads it as `multipart/form-data` and then fails. This is malformed input, like the missing `;` of GT-p6.
- **M1b, its own dossier.** `multipart/form-data-extra` or `multipart/form-datax`. The value is a well-formed media type. It is just not `multipart/form-data`. The platform's reader treats it as a type that is not a form, the same way it treats `text/plain`.

Promised 3 turns on whether the input is invalid. That is why the two are kept apart. The owner has ruled same-line cases on this pull request differently by what the base did: first-round ruling 4 (a doubled urlencoded header that used to parse) was advice, and rule-v4-candidates ruling 16 (a missing `;` that used to raise an error on Node) was a problem.

## Already decided?

No. No saved ruling, ruled claim or known problem covers this exact header.

- GT-p6 (rule-v4-candidates ruling 16) has the same trigger shape, mechanism and consequence, but its wording admits only "the `;` before `boundary` is missing". Whether this header joins it is a grouping question (below).
- `CL-p-duplicate-content-type` (first-round ruling 4, advisory) covers a urlencoded header with a comma: "A urlencoded form request whose Content-Type value has a comma before any semicolon". This case is multipart, not urlencoded. Its consequence also differs: the doubled urlencoded header parsed at the base, and this one raised an error on Node and Deno. The base parsed a doubled multipart header too (`multipart/form-data, multipart/form-data; boundary=X`, run below). That neighbour has ruling 4's shape, not this one's.
- The candidate record does not misread a ruling. It says "GT-p6 admits only a missing semicolon before boundary", which is right. It ran only Node, and says so. On Bun with a boundary, the base parsed the fields rather than rejecting them (run below).

## Problem

*run.* A route calls `c.req.parseBody()`. A client posts a valid multipart body with `Content-Type: multipart/form-data,foo; boundary=X`.

- At the base, on Node 24.21.0, `parseBody()` throws `TypeError: Failed to parse body as FormData.` and an uncaught error gives HTTP 500. On Deno 2.8.3 it throws `TypeError: Cannot construct MultipartParser: multipart/form-data must provide a boundary`. On Bun 1.3.10 it returns `{"foo":"bar"}`.
- At the head, on all three, it returns `{}` and the route answers HTTP 200 with `{}`.

Without `; boundary=X` (`multipart/form-data,foo`), the base throws on all three runtimes (Bun: `TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary`). The head returns `{}` on all three.

## What changed

*read, knowable at the cut-off.* `parseBody()` decides whether a request is a form before it reads the body. Before the change it asked whether the raw header text starts with `multipart/form-data` or `application/x-www-form-urlencoded`. The change makes the test ignore letter case. To do that it cuts the header at the first `;`, trims it, lower-cases it, and asks for an exact match (`src/utils/body.ts`, head lines 104 to 106):

```diff
-  if (
-    contentType?.startsWith('multipart/form-data') ||
-    contentType?.startsWith('application/x-www-form-urlencoded')
-  ) {
+  const mediaType = contentType?.split(';')[0].trim().toLowerCase()
+
+  if (mediaType === 'multipart/form-data' || mediaType === 'application/x-www-form-urlencoded') {
     return parseFormData(request, { all, dot })
   }
 
   return {}
```

For `multipart/form-data,foo; boundary=X`, the text before the first `;` is `multipart/form-data,foo`. It starts with `multipart/form-data`, so the base sent the request to the runtime's form reader. It is not exactly `multipart/form-data`, so the head returns `{}` without reading the body.

*read.* Whose code fails, step by step. At the base, `parseFormData` called `request.formData()`, the runtime's reader. The Fetch standard says how that reader finds the media type ("extract a MIME type"): it splits the header at commas, ignores the piece it cannot parse (`foo; boundary=X`), and keeps `multipart/form-data`, which has no boundary. It then parses the body "using the value of the `boundary` parameter" and "If that fails for some reason, then throw a TypeError." (markup removed). Node and Deno do that; Deno's message says "multipart/form-data must provide a boundary". Bun parsed the fields anyway when the header carried `; boundary=X`. At the head Hono never calls a reader, so the result is Hono's own `{}`.

*read.* The description at the cut-off, from the task packet, is only "Fixes #5060" and the template checklist. The edit history (`upstream/pr-5067-edits.json`) shows four edits at 09:41:51 to 09:41:54Z, after the cut-off, that only tick checklist boxes.

## Promised?

**The operation.** `parseBody()`, a public Hono function, documented as `c.req.parseBody()` and exported from `hono/utils/body`. Hono owns its dispatch. The error that the head no longer raises came from the runtime's own form reader, `Request.formData()`, which the Fetch standard defines and Node, Deno and Bun implement.

**The input is invalid.** *read.* RFC 9110 section 8.3.1: `media-type = type "/" subtype parameters`, with `type = token` and `subtype = token`. A token is made of `tchar`, which has no comma (section 5.6.2). Section 5.3: "a sender MUST NOT generate multiple field lines with the same name in a message ... unless that field's definition allows multiple field line values to be recombined as a comma-separated list". Content-Type does not. RFC 7578 section 8, for `multipart/form-data`: "Required parameters:  boundary".

**What the six places say.**

| Place | Query, saved response | Hits / read | What was found |
| --- | --- | --- | --- |
| project-docs | `rg -n -i 'parseBody\|multipart/form-data\|Content-Type.*(form\|multipart)\|malformed\|form-data' --glob '*.md'` over the honojs/website tarball at `b17f3d3` (the last website commit before the cut-off), and `rg -n -i 'parseBody\|multipart/form-data\|form-data\|Content-Type'` over README.md, docs/ and the parseBody source at head; `upstream/project-docs-all.txt` | 52 / 52 | API page: "Parse Request body of type `multipart/form-data` or `application/x-www-form-urlencoded`". `docs/MIGRATION.md`: "`c.req.parseBody` now only parses FormData with content type `multipart/form` or `application/x-www-form-urlencoded`." Validation guide: a request without "a matching `content-type` header" gets "an empty object (`{}`)" in the validator's callback. Nothing on a malformed form header or on errors. |
| owner-docs | `rg` for the form reader, MIME parsing and media-type grammar over the Fetch standard at `aa0bd92` and the MIME Sniffing standard at `67bde18` (the last commits before the cut-off), RFC 9110, RFC 7578 and RFC 6838; `upstream/owner-docs-search.txt` | 102 / 102 | Fetch, `formData()`: parse with the `boundary` parameter, and "If that fails for some reason, then throw a TypeError." (markup removed). Fetch, after "extract a MIME type": "When extract a MIME type returns failure or a MIME type whose essence is incorrect for a given format, treat this as a fatal error." (markup removed). RFC grammar as above. |
| change | `rg -n -i 'Content-Type\|media.?type\|startsWith\|exact\|malformed\|invalid\|error\|throw\|boundary\|normaliz\|case-insensitive\|parameter'` over the diff and the task packet; the description's edit history read; `upstream/change-search.txt` | 41 / 41 | Title: "normalize Content-Type media type for case-insensitive matching". Linked issue #5060: "Supported media type matching should be case-insensitive for the media type portion. Parameter values such as multipart boundaries should remain intact." Code comment: "Normalize the media type (case-insensitive) while keeping parameters like the boundary". No word on malformed headers, on errors, or on narrowing what counts as a form. |
| maintainers | five `gh api search/issues` queries in honojs/hono, `created:<=2026-07-01T09:39:48Z`: `parseBody "Content-Type"`, `"multipart/form-data" error`, `parseBody malformed`, `parseBody empty object`, `"media type"`; `upstream/maintainers-search.txt` | 45 / 8 | Read in full: #428, #484, #487, #2689, #3285, #3707, #4093, #5064. #428 (2022): parseBody was changed to "return the blank object `{}` if the body is nothing". #487 (2022): "`c.req.parseBody` only parse `FormData`." #5064, an earlier pull request with the same dispatch, drew no maintainer comment before the cut-off. Nobody discussed malformed form headers or the error they raised. |
| public-code | `gh api search/code` for `"multipart/form-data-"`, `"Content-Type" "multipart/form-data," hono` and `"c.req.parseBody" catch language:TypeScript`; each first-page hit fetched and searched for the exact text; `upstream/public-code-read.txt` | 10,741 / 30 | No program from before the cut-off sends a comma inside a multipart Content-Type. The search ignores punctuation, so the comma hits are prose such as "multipart/form-data, or text/plain". Several programs wrap `c.req.parseBody()` in `try`/`catch` (for example GewoonJaap/gemini-cli-openai, file dated 2026-03-27, which answers 500 with the error message). |
| documented-way | `python3 -I probes/M1/run.py`; `probes/M1a/result-head.txt` | 3 / 3 | The documented header `multipart/form-data; boundary=X` gives `{"foo":"bar"}` at the head on Node, Bun and Deno. `validator('form')`, the documented way to have a bad form reported, gives HTTP 200 with `{}` for the comma header at both commits: its pattern never matches it. |

For public-code, the other 10,711 hits were not read: the search engine drops punctuation, so its counts cannot tell these headers from ordinary ones. A count is not a practice.

**Which way the promise is made.** *Built*, through Promised 3b and 7a. Before the change, `parseBody()` handed this header to the runtime's reader, and on Node and Deno that reader reported the malformed header with a `TypeError` the application could catch. Promised 3b: "A check that used to catch invalid input and report it is promised. Putting up with bad input is not promised." Promised 7a: how a promised use behaved before is part of its promise. This is the reading the owner gave GT-p6 in rule-v4-candidates ruling 16 ("i agre its a new problem"). On Bun, the base read the fields of this malformed header (with a boundary). That is putting up with bad input, Promised 3a, and is not promised.

Evidence against: no document says `parseBody()` reports a malformed header, and the project's own code returns `{}` for every request that is not a form (#428, #487, the base test "should return blank object if body is JSON"). The error came from the runtime, not from a check Hono wrote. The owner weighed the same point for GT-p6 and ruled it a problem.

## Delivered?

No. The promise is that a malformed multipart header is reported, as Node and Deno reported it before. At the head, on Node and Deno, `parseBody()` returns `{}` with no error, and an application that caught the error to answer 400 now continues with an empty form. *run.*

## Intended or announced

*read, knowable at the cut-off.* The title and the linked issue announce case-insensitive matching. Nothing announces that malformed headers will stop raising errors, or that the form test becomes narrower in any other way. Promised 2c: a choice made without saying so ends no promise.

*read, known only later.* The pull request merged at 09:42:27Z, under three minutes after the cut-off, and shipped in v4.12.28 (published 2026-07-06). The release note lists only the title. Three later commits touch `src/utils/body.ts`: #5071, #5131, and a commit of 2026-08-26 that limits dot-notation nesting. None changes the dispatch. `main`, fetched 2026-10-29, still has the exact match (line 111).

## What the affected person sees

*run.* An application that reads forms with `c.req.parseBody()` on Node or Deno, and a client that sends `Content-Type: multipart/form-data,foo; boundary=X` (or without the boundary).

- Before: `parseBody()` throws. Without a `try`, the client gets HTTP 500. With a `try`, the application can answer as it chooses, for example 400.
- After: `parseBody()` returns `{}`. The handler continues with an empty form. A handler that checks for a required field rejects the request in its own words; one that does not check goes on with no fields. Nothing tells the client or the application that the header was malformed. Over a real socket on 127.0.0.1 the same thing happens: HTTP 500 before, HTTP 200 `{}` after (`probes/M1a/http-*.txt`).
- On Bun with a boundary: before, the fields arrived (`{"foo":"bar"}`); after, `{}`.

The form itself is lost only in the Bun case, and only because the base read a header the standard rules out. A correct header works at both commits.

## What the maintainers did

*read.* No maintainer comment before the cut-off touches this. The pull request has no review comments or reviews (`upstream/pr-5067-review-comments.json`, `pr-5067-reviews.json`); its two conversation comments are a benchmark bot and a coverage bot.

*read, known only later.* No later issue or pull request reports this header (`upstream/later-parsebody.json`, 12 results read by title). The dispatch is unchanged on `main`. No acknowledgement, no fix.

## How each fact is known

- *run*: every result above, at base and head, on Node 24.21.0, Bun 1.3.10 and Deno 2.8.3, in process and over HTTP; every patch and the project's own tests. Probes in `probes/M1/`, rows for this group in `probes/M1a/`.
- *read*: the diff, the source at both commits, the task packet, the website at `b17f3d3`, `docs/MIGRATION.md`, the Fetch and MIME standards at their last commits before the cut-off, RFC 9110 and RFC 7578, and the saved GitHub responses under `upstream/`.
- *reported*: the candidate record's own run and the comments' descriptions. Nothing here rests on them; each fact was run again.

## Relation to existing reference families and ruled claims

**Grouping against GT-p6** (Ground rule 4). GT-p6: "`parseBody()` returns `{}` with no error for a multipart request whose Content-Type lacks the `;` before `boundary`, where on Node it raised a Content-Type error before the change". Its place is the same dispatch, `src/utils/body.ts` head lines 104 to 106.

- *At GT-p6's place:* the text before the first `;` is `multipart/form-data boundary=x`, which is not an exact match, so the head returns `{}`.
- *At this group's place:* the text before the first `;` is `multipart/form-data,foo`, which is not an exact match, so the head returns `{}`.

The three tests, run on Node, Bun and Deno (`probes/M1a/`, patches in `probes/M1a/patches/`):

| Patch on the head | GT-p6's header | This group's comma headers | Project tests |
| --- | --- | --- | --- |
| none (head) | `{}` everywhere | `{}` everywhere | 106 passed |
| `fix-GT-p6` (GT-p6's smallest fix, from last round) | Node and Deno throw again; Bun throws `missing final boundary` | still `{}` everywhere | 106 passed |
| `fix-M1` (this group's smallest fix) | still `{}` everywhere | the base's results again: Node and Deno throw; Bun returns the fields with a boundary and throws without | 106 passed |
| `fix-GT-p6+M1` (both) | as `fix-GT-p6` | as `fix-M1` | 106 passed |
| `fix-one-expression` (`mediaType?.startsWith('multipart/form-data')`) | as `fix-GT-p6` | as `fix-M1` | 106 passed |

- **Same lines:** yes. Both come from the one comparison on head line 106.
- **One fix:** yes. One expression cures both (`fix-one-expression`). The smallest fix for each leaves the other, which ruling 14 says is not the test.
- **One sentence:** yes. Proposed cause sentence: *`parseBody()` now sends a multipart header to the form reader only when the text before its first `;` is exactly `multipart/form-data`, so a malformed one gets `{}` where Node raised an error.* That is true of both.

Precedent: rule-v4-candidates ruling 14 joined a second header form on the same pattern line to GT-p4 by these three tests ("duplicate"). Its lesson: group by the three tests, "not by whether the smallest fix for one leaves the other".

**The other known problems.** *run.* The patches for GT-p1, GT-p2, GT-p3, GT-p4, GT-p5, GT-p7 and PE-p1 leave every comma header exactly as at the head. This group's patches leave their failures as at the head (`probes/M1a/known-*.txt`; only random boundary names differ in GT-p1's garbled key). Their lines are elsewhere: the body read and cache (GT-p1, GT-p2, GT-p3, GT-p7), the validator's patterns (GT-p4, GT-p5) and the method-override middleware (PE-p1).

**Clean control.** The packet's control is `known-problems`, not `audited-clean`, so no clean control is touched.

**Not part of this group.** The doubled multipart header `multipart/form-data, multipart/form-data; boundary=X` parsed at the base on all three runtimes and gives `{}` at the head. It has the shape of first-round ruling 4 (tolerance lost), not this one. The group's other remark, that no test covers the change, is a separate claim: a test someone could add is not a lost protection (Delivery rule 1).

## Both sides

**For a widened GT-p6.** The header is malformed by the standard's grammar, as GT-p6's is. Node and Deno reported it before and are silent now, through the same line. The owner ruled that loss a problem for GT-p6 (ruling 16) and joined two header forms on one line by the three tests (ruling 14). All three tests hold here.

**Against.** Hono never promised to report malformed headers; it inherited an error from the runtime because its prefix test was loose. The project's design since 2022 is that `parseBody()` returns `{}` for anything that is not a form (#428, #487). Read that way, the head treats a header the runtime itself cannot read as a form like any other non-form, and the old error was an accident. On Bun the change is only lost tolerance of bad input (Promised 3a). This is the argument the owner already heard and rejected for GT-p6; it is stronger here only in that the comma case is rarer still.

## Recommendation

> **What happens:** a multipart header with a comma inside its media type now gives `{}` with no error, where Node and Deno raised a `TypeError`. **Promised: yes**, built: before the change `parseBody()` passed the header to the runtime's reader, which reported it, and Promised 3b keeps a check that caught and reported invalid input (as ruled for GT-p6 in rule-v4-candidates ruling 16). **Delivered: no**, on Node and Deno the error is gone and the handler gets an empty form. **So: problem, as part of GT-p6 (duplicate: widen GT-p6's wording).** **Band, decided separately:** other-material, GT-p6's label, if it is joined; the same proposed if it is kept apart.

Recommend **duplicate of GT-p6**, at **high** confidence. One rule decides the promise (Promised 3b, with ruling 16 of the same shape), Ground rule 4's three tests all hold (run), and ruling 14 joined a case of the same shape. The fact that would reverse it is a base that did not raise an error on Node; the runs settle it. Grouping is always the owner's.

A widened GT-p6 might read (proposed wording): *`parseBody()` returns `{}` with no error for a multipart request whose Content-Type is malformed before its first `;`, such as `multipart/form-data boundary=X` (no `;`) or `multipart/form-data,foo; boundary=X` (a comma inside the media type), where on Node and Deno it raised the runtime's error before the change.*

**The label, if kept apart.** Propose other-material, as the owner labelled GT-p6.

- S1, protection: no. Nothing is bypassed. Outside input no longer crashes a request; it gets `{}`.
- S2, data: close. On Bun the fields of this header used to arrive and now do not, so an operation "returns success with data dropped". But the data arrived only because Bun put up with a header the standard rules out. On Node and Deno nothing was delivered before either. Reading rule 3: "A consequence that the same class of bad input already produced before the change is not a new serious consequence." The owner faced the same Bun fact for GT-p6, where the inspector chose serious on it, and ruled other-material.
- S3, operation: no. No ordinary or documented use relied on this header.
- S4, reported state: close. `{}` with HTTP 200 could be read as reporting success. But the request was never a valid form, and the application gets an empty form, not a wrong value it would act on as real. The owner did not apply S4 to GT-p6.
- S5, documented use: no. No instruction added by the change is involved.
- S6, cost: no.
- Other-material shape: "Lost diagnostic detail in a message while the behaviour it reports is unchanged" is the nearest. Here the whole error is lost, not only its detail, which is why S2 and S4 are close.
- Who is hurt and how stuck: a Node or Deno application that relied on the error to reject this malformed header. It can check the header itself, or use `validator('form')` with a required field. A correct client is not affected.

The label is a close call on S2 and S4, as GT-p6's was. No blind inspector was run, because this task forbids running a model.

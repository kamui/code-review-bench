# NC-2d6d8bcc45c7: the multipart validator skips a form with a parameter after `boundary`

Hono pull request 5067, base `9728702`, head `5226d41`, cut-off 2026-07-01T09:39:48Z. Each fact is marked **run**, **read** or **reported**. Facts after the cut-off are marked *later* and only confirm.

## Problem

**run** A client posts a valid multipart form with the Content-Type `multipart/form-data; boundary=X; charset=utf-8`. `validator('form')` hands its callback `{}` and the route answers `200 {}`. `parseBody()` and `c.req.parseBody()` return `{"foo":"bar"}`. The result is the same at the base and the head, on Node 24.22.2, Bun 1.4.4 and Deno 2.9.8 (`docs/research/audit-v2-r2-reconciliation-2026-11-04/probes/hono-media-type-headers/output.txt`, copied as `upstream/round2-hono-media-type-headers-output.txt`).

## What changed

**read** The change rewrote line 25 of `src/validator/validator.ts` to add the `i` flag:

```diff
-const multipartRegex = /^multipart\/form-data(;\s?boundary=[a-zA-Z0-9'"()+_,\-./:=?]+)?$/
+const multipartRegex = /^multipart\/form-data(;\s?boundary=[a-zA-Z0-9'"()+_,\-./:=?]+)?$/i
```

The pattern allows one optional group, `; boundary=<value>`, and then the end of the text. The boundary value cannot hold a `;` or a space. So any parameter after the boundary fails the pattern, as any parameter before it does. When the pattern fails, the validator does not read the body, and the callback gets `{}`. `parseBody()` (`src/utils/body.ts` head line 104) compares only the text before the first `;`, so it reads the form.

## Promised?

**Promised: yes, written.** This is the promise GT-p4 already rests on, established for rule-v4-candidates ruling 14 and copied here with its saved searches (`upstream/ruling14-*`, listed in `upstream/SOURCES.md`). Hono's validation guide says: "To validate form data, specify `form` as the first argument and a callback as the second argument." RFC 9110 section 5.6.6 gives parameters as `*( OWS ";" OWS [ parameter ] )`: any number, in any order. RFC 2045 section 5.1 says MIME readers ignore a parameter name they do not know. Nothing found limits the promise to a parameter before `boundary`.

## Delivered?

**Delivered: no.** The callback should get the form's fields. It gets `{}` (run).

## Intended or announced

**read** Issue 5060, which the change fixes, asks that `validator('form')` handle mixed-case multipart media types "while preserving multipart boundary behavior". Nothing in the change says other parameters are dropped on purpose. *Later* (**read**, fetched 2026-11-04): on 2026-11-01 Hono published advisory GHSA-4h7h-rv7c-7m2v, "`validator('form')` skips validation for `multipart/form-data` requests with extra parameters", and merged `d88fab37`, which changes line 25 to `/^multipart\/form-data(;\s*[a-zA-Z0-9\-]+\=([^;]+))*$/i` (`upstream/later-*.json`).

## What the affected person sees

An app that checks forms with `validator('form')` gets `{}` in its callback, with no error, when a client puts any parameter beside `boundary`, before it or after it. The handler can still read every field through `c.req.parseBody()`. A callback that does not reject `{}` lets an unchecked form through.

## What the maintainers did

**read** No acknowledgement before the cut-off (ruling 14's search, `upstream/ruling14-maintainers-validator-all.json`). *Later*: the advisory and `d88fab37` above. Its summary names "another legal parameter, such as `charset`". Its fix accepts any parameters in any order, so it cures the parameter before and after `boundary` with one line. It still needs the `;` right after `form-data`, so it does not cure GT-p4's space before `;`.

## How each fact is known

| Fact | How |
| --- | --- |
| The validator hands `{}` for a parameter after `boundary`, at both commits, on three runtimes | run (round 2 probe) |
| `fix-M4b` (ruling 14's patch) cures the after case and leaves the space case | run (round 2 probe) |
| `fix-M4a+M4b` (rulings 13 and 14's combined patch) cures all three header forms | run (`probes/combined-fix/`) |
| The pattern's grammar rejects any parameter after the boundary | read |
| The promise and its searches | read (ruling 14's saved searches) |
| Hono's advisory and fix of 2026-11-01 | read, later |
| How often clients send a parameter after `boundary` | not established |

## Relation to existing reference families and ruled claims

GT-p4 is "`validator('form')` hands its callback an empty object for a valid multipart form whose Content-Type has a space before `;` or another parameter before `boundary`, while `parseBody()` reads the fields". Ground rule 4's three tests, against GT-p4:

1. **The same lines cause both: yes.** Both fail the one pattern on `validator.ts` line 25 (read).
2. **One fix cures both: yes.** `fix-M4b`, the patch ruling 14 ran for the parameter before `boundary`, also cures the parameter after it, on Node, Bun and Deno (run, round 2). `fix-M4a+M4b`, the patch that cured both of GT-p4's forms in October, cures the after form too (run, this round). Hono's own fix changes the same line once for both orders (read, later).
3. **One cause sentence is true of both: yes.** Ruling 14's sentence: "the validator's hand-written header pattern rejects valid header forms that MIME allows."

It is apart from GT-p5 (`urlencodedRegex` on line 26) for the reason GT-p5's entry gives. The combined line-25 patch still leaves the urlencoded space case at `{}` (run, this round's output).

## Both sides

- **For joining it to GT-p4.** All three tests hold, the same way they held for ruling 14. GT-p4's mechanism already says the pattern "allows one parameter only, `boundary=`", which is true of a parameter after it. Hono fixed both orders with one change.
- **Against.** GT-p4's title, obligation and trigger name a parameter *before* `boundary` only, and ruling 14 widened it only that far. Ground rule 4 says the owner decides. On 2026-11-04 the owner chose to widen it (round 2 reconciliation, question 27 and audit call 1).

## Recommendation

> **What happens:** `validator('form')` hands `{}` for a valid multipart form with a parameter after `boundary`, while `parseBody()` reads it. **Promised: yes**, GT-p4's written promise in Hono's validation guide, with RFC 2045 section 5.1. **Delivered: no**, the callback gets `{}`. **So: duplicate of GT-p4**, by ground rule 4's three tests; GT-p4's wording widens to "before or after `boundary`". **Band, decided separately:** none; GT-p4 keeps its serious label.

# Ruling 19-hono-bun-direct-formdata: p-hono-5067

Decisions in this question: `P1` (admission as a pre-existing problem); `P1` (label).

## `P1`: admission as a pre-existing problem

Record: [`19-hono-bun-direct-formdata.P1.before.json`](19-hono-bun-direct-formdata.P1.before.json)

```text
ASK
- pre-existing-admission stays on its existing owner or reconciler route
- recommender: gap or conflict in the rule
- assessor-1: gap or conflict in the rule
- assessor-2: cannot-tell stays with the owner
- assessor-2: gap or conflict in the rule
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cases/P1.md:

# Direct HonoRequest.formData rejects mixed-case form media types on Bun

## Code and dates

*read.* Base `9728702911073aec5a63a3ba2840b7240e5d3205`; head `5226d4165d48643586152614cbd07422a0ab7a22`; last push 2026-07-01T09:39:48Z. Source and documentation below existed by that cut-off unless marked later.

*read, knowable at cut-off.* request.ts:329–331 is unchanged and delegates to #cachedBody(formData); its first read at unchanged line 238 calls raw.formData(). Only the parser utility, buffer helper and validator are changed. The direct reader does not use their normalization.

```diff
 (no diff in src/request.ts)
  formData(): Promise<FormData> {
    return this.#cachedBody('formData')
  }

  return (bodyCache[key] = raw[key]())
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

*run, reproducible at cut-off.* On Bun, direct HonoRequest.formData rejects Application/X-WWW-Form-Urlencoded and Multipart/Form-Data; boundary=X at both commits with the same incorrect MIME type/boundary TypeError. Native Request.formData rejects too. Node accepts both at both commits. Head parseBody and validator return the fields on Bun. Lower-case direct form reads work at both commits.

Full per-runtime values, status codes and exceptions are saved in the probe result files; the case reproduces them here.

### base on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "Application/X-WWW-Form-Urlencoded": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {}
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Multipart/Form-Data; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {}
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "application/x-www-form-urlencoded": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### base on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "Application/X-WWW-Form-Urlencoded": {
    "native": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "parseBody": {
      "ok": true,
      "value": {}
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Multipart/Form-Data; boundary=X": {
    "native": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "parseBody": {
      "ok": true,
      "value": {}
    },
    "validator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "application/x-www-form-urlencoded": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### head on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "Application/X-WWW-Form-Urlencoded": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "Multipart/Form-Data; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "application/x-www-form-urlencoded": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### head on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "Application/X-WWW-Form-Urlencoded": {
    "native": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "Multipart/Form-Data; boundary=X": {
    "native": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "application/x-www-form-urlencoded": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### candidate-place patch on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "Application/X-WWW-Form-Urlencoded": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "Multipart/Form-Data; boundary=X": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "application/x-www-form-urlencoded": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

### candidate-place patch on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "Application/X-WWW-Form-Urlencoded": {
    "native": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "Multipart/Form-Data; boundary=X": {
    "native": {
      "ok": false,
      "error": "TypeError [ERR_FORMDATA_PARSE_ERROR]: Can't decode form data from body because of incorrect MIME type/boundary"
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  },
  "application/x-www-form-urlencoded": {
    "native": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "comparableUndici": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "direct": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "parseBody": {
      "ok": true,
      "value": {
        "foo": "bar"
      }
    },
    "validator": {
      "status": 200,
      "body": "{\"foo\":\"bar\"}",
      "errors": []
    }
  }
}
```

## Documentation and contemporary statements

*read, knowable at cut-off.* Website documentation is pinned to b17f3d3f4eb3f4a4938d95b552aba5eb8a15d255, the latest website commit before 2026-07-01T09:39:48Z. The root README and the general concepts pages, including Web Standards and Middleware, were searched as well as the API and validation/formatting pages.

*read, knowable at cut-off.* honojs/website docs/api/request.md:

> Parses the request body as a `FormData`.

*read, published June 2022, before the cut-off.* RFC 9110 §8.3.1: The type and subtype tokens are case-insensitive. Section 5.6.6: Parameter names are case-insensitive. Its grammar is `parameters      = *( OWS ";" OWS [ parameter ] )`.

*read, knowable at cut-off.* The Web Standards guide says:

> The same code runs on all platforms.

*read.* The Request utility accepts `HonoRequest | Request`; its JSDoc and the direct formData JSDoc are saved in head source snapshots. The Fetch specification was fetched at its last pre-cut-off commit, and its Body formData algorithm parses the MIME type and boundary parameter, throwing TypeError on a parsing failure. It makes no statement here about Hono calling overridden methods.

## Reader-visible behaviour

*run.* A valid form sent to a documented direct-reader handler cannot be read on Bun when its media type uses another case. It throws rather than returning fields. The caller needs header normalization or a different parsing API, which is a code change. No production incidence was measured.

## Other code places and patch executions

*read, knowable at cut-off from the pinned code.* The comparison scenarios use these code places:

| Id | Behaviour described by the answer key | Place |
| --- | --- | --- |
| GT-p1 | A remembered form is ignored: a following parseBody fails or garbles fields, loses intervening edits, and parses again on each further call. | body.ts:125–132, especially reading bytes without checking bodyCache.formData. |
| GT-p2 | A formData read overlapping parseBody rejects because its rebuilt Response has no Content-Type. | body.ts:126 before line 130; request.ts:234. |
| GT-p3 | A first parseBody collects the whole upload as bytes, parses a copy, and keeps the bytes on a HonoRequest, increasing memory and first-read work. | body.ts:126–127; request.ts:238. |
| PE-p1 | A mixed-case form POST with _method=DELETE goes to POST instead of DELETE. | method-override/index.ts:74 and 92, unchanged. |

*read, knowable at cut-off.* The diff in change.diff replaces native formData delegation with a byte read at body.ts:126–127 and adds the cache write at 130. The method-override checks have no diff. Patches for the respective places are saved beside the results. GT-p1’s patch adds remembered-form reuse; GT-p2’s patch supplies the media type to the reconstructed Response; GT-p3’s patch hands an unread stream to the platform parser instead of collecting bytes; PE-p1’s patch normalizes the middleware’s media-type token.

*run, reproducible at the cut-off.* The candidate-place patch is `fix-P1.patch`. Its output is `result-fix-P1.txt`. The comparison-place patch alone, candidate-place patch alone and both together were run on Node and Bun for GT-p1, GT-p2, GT-p3 and PE-p1; per-scenario outputs and patches are under `probes/P1/`. See `grouping-matrix.md`.

*run.* Candidate-only fixes leave the distinguishing comparison failure in place, and comparison-only fixes leave this candidate in place. The raw-Request-only M5 patch repairs the plain-Request branch’s delegation but does not repair the HonoRequest first-read buffering. Patches demonstrate specific causal separations; they are not proposed production fixes, and some retain other faults.

*run.* Normalizing the boundary parameter name in bufferToFormData repairs M1 but leaves Bun’s direct HonoRequest.formData failure. Normalizing the direct reader’s media type repairs P1 but leaves the uppercase-boundary validator failure. Both together repair both. The comparable undici 6.21.3 reader succeeds under both hosts for these headers; native Bun rejects them. The Bun setup is documented, but neither its deployment share nor the header frequency is measured.

## Impact facts

*run/read, reproducible or documented at the cut-off.* A valid form sent to a documented direct-reader handler cannot be read on Bun when its media type uses another case. It throws rather than returning fields. The caller needs header normalization or a different parsing API, which is a code change. No production incidence was measured.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

## Known only later

*read, known only later.* The description was edited at 09:41:51–09:41:54Z, after the cut-off, only to tick checklist boxes. The saved GraphQL history includes the original body. The PR merged at 09:42:27Z and shipped in v4.12.28 on 2026-07-06. The release note lists its title; it announces no reader, diagnostic, whitespace or parameter exception. #5131 later adds remembered-form reuse. Later searches found #5226 (merged 2026-08-18) replacing prettyJSON’s match with a case-insensitive JSON/suffix regex and #5377 (merged 2026-09-20) handling invalid JSON bodies. Those are later source changes, not cut-off promises or acknowledgements of every candidate. Raw responses and files are under upstream/.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | not-admitted | low (conflict, no-precedent) | Direct HonoRequest.formData hands the request to the platform, and Bun's own native formData rejects the mixed-case header, so the fault is the runtime's, not Hono's. |
| assessor-1 | admitted | low (conflict, gap, no-precedent) | The cut-off documentation promises form parsing across platforms, but the documented direct reader throws on valid mixed-case forms on Bun, so I apply pre-existing-admission despite the uncertain platform exception. |
| assessor-2 | cannot-tell | low (no-single-rule, conflict, gap, no-precedent, flip-fact-open) | The documented Hono form reader rejects valid mixed-case form media types on Bun at both commits, but Bun's native reader also rejects them, leaving the platform exception's applicability unresolved. |

To reconcile before asking:

- assessor-1 picks admitted where the recommender picks not-admitted. Its reason: The cut-off documentation promises form parsing across platforms, but the documented direct reader throws on valid mixed-case forms on Bun, so I apply pre-existing-admission despite the uncertain platform exception.
- assessor-2 picks cannot-tell where the recommender picks not-admitted. Its reason: The documented Hono form reader rejects valid mixed-case form media types on Bun at both commits, but Bun's native reader also rejects them, leaving the platform exception's applicability unresolved.

## `P1`: label

Record: [`19-hono-bun-direct-formdata.P1-label.before.json`](19-hono-bun-direct-formdata.P1-label.before.json)

```text
ASK
- band stays on its existing owner or reconciler route
- assessor-1: gap or conflict in the rule
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cards/P1.md:

# Direct HonoRequest.formData rejects mixed-case form media types on Bun

Hono is a web framework. Base is the commit before the change; head is the reviewed commit. The evidence below comes from the pinned source and documentation available at its last push.

## Executions

*run.* On Bun, direct HonoRequest.formData rejects Application/X-WWW-Form-Urlencoded and Multipart/Form-Data; boundary=X at both commits with the same incorrect MIME type/boundary TypeError. Native Request.formData rejects too. Node accepts both at both commits. Head parseBody and validator return the fields on Bun. Lower-case direct form reads work at both commits.

*run.* The candidate-place patch changes only the described code place and repairs the demonstrated case. The corresponding cache, reader and method-override patch scenarios are retained separately.

## What the affected person sees

*run/read.* A valid form sent to a documented direct-reader handler cannot be read on Bun when its media type uses another case. It throws rather than returning fields. The caller needs header normalization or a different parsing API, which is a code change. No production incidence was measured.

## Impact facts

*run/read, reproducible or documented at the cut-off.* A valid form sent to a documented direct-reader handler cannot be read on Bun when its media type uses another case. It throws rather than returning fields. The caller needs header normalization or a different parsing API, which is a code change. No production incidence was measured.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

*read.* Parses the request body as a `FormData`.

No production frequency is known. Node and Bun were executed; Deno and Cloudflare Workers were not. No timing, operating-system memory-limit failure, deployment or security exploit was measured for this case.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | serious | medium (no-precedent) | The documented direct read cannot read a valid form on Bun without code changes. |
| assessor-1 | serious | low (no-single-rule, gap, no-precedent) | A documented direct-reader handler cannot process a valid mixed-case form on Bun and requires application code changes to restore that operation. |

To reconcile before asking:

- None: every party's pick is the recommender's.


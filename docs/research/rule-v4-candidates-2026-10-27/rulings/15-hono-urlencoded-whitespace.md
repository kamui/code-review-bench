# Ruling 15-hono-urlencoded-whitespace: p-hono-5067

Decisions in this question: `M6` (candidate); `M6` (label).

## `M6`: candidate

Record: [`15-hono-urlencoded-whitespace.M6.before.json`](15-hono-urlencoded-whitespace.M6.before.json)

```text
ASK
- recommender: problem stays with the owner
- assessor-1: problem stays with the owner
- assessor-2: problem stays with the owner
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cases/M6.md:

# The urlencoded validator skips whitespace before a charset parameter

## Code and dates

*read.* Base `9728702911073aec5a63a3ba2840b7240e5d3205`; head `5226d4165d48643586152614cbd07422a0ab7a22`; last push 2026-07-01T09:39:48Z. Source and documentation below existed by that cut-off unless marked later.

*read, knowable at cut-off.* validator.ts:26 adds /i to the urlencoded regex but still requires the semicolon immediately after urlencoded. body.ts:104 trims the token and accepts the header.

```diff
-const urlencodedRegex = /^application\/x-www-form-urlencoded(;\s*[a-zA-Z0-9\-]+\=([^;]+))*$/
+const urlencodedRegex = /^application\/x-www-form-urlencoded(;\s*[a-zA-Z0-9\-]+\=([^;]+))*$/i
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

*run, reproducible at cut-off.* For Application/X-WWW-Form-Urlencoded ; charset=utf-8, the head parseBody returns foo=bar on Node and Bun, while validator(form) receives {} and the route returns 200. The validator also receives {} at the base. The lower-case header with the same whitespace has the same failure at both commits; removing the whitespace makes head form validation return the fields. json validation returns {} for this urlencoded header at both commits, as its target differs.

Full per-runtime values, status codes and exceptions are saved in the probe result files; the case reproduces them here.

### base on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "application/x-www-form-urlencoded ; charset=utf-8": {
    "native": {
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
      "body": "{}",
      "errors": []
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded ; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  }
}
```

### base on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "application/x-www-form-urlencoded ; charset=utf-8": {
    "native": {
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
      "body": "{}",
      "errors": []
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded ; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  }
}
```

### head on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "application/x-www-form-urlencoded ; charset=utf-8": {
    "native": {
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
      "body": "{}",
      "errors": []
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded ; charset=utf-8": {
    "native": {
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
      "body": "{}",
      "errors": []
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  }
}
```

### head on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "application/x-www-form-urlencoded ; charset=utf-8": {
    "native": {
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
      "body": "{}",
      "errors": []
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded ; charset=utf-8": {
    "native": {
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
      "body": "{}",
      "errors": []
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  }
}
```

### candidate-place patch on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "application/x-www-form-urlencoded ; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded ; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  }
}
```

### candidate-place patch on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "application/x-www-form-urlencoded ; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded ; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  },
  "Application/X-WWW-Form-Urlencoded; charset=utf-8": {
    "native": {
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
    },
    "jsonValidator": {
      "status": 200,
      "body": "{}",
      "errors": []
    }
  }
}
```

## Documentation and contemporary statements

*read, knowable at cut-off.* Website documentation is pinned to b17f3d3f4eb3f4a4938d95b552aba5eb8a15d255, the latest website commit before 2026-07-01T09:39:48Z. The root README and the general concepts pages, including Web Standards and Middleware, were searched as well as the API and validation/formatting pages.

*read, knowable at cut-off.* honojs/website docs/guides/validation.md:

> To validate form data, specify `form` as the first argument and a callback as the second argument.

*read, published June 2022, before the cut-off.* RFC 9110 §8.3.1: The type and subtype tokens are case-insensitive. Section 5.6.6: Parameter names are case-insensitive. Its grammar is `parameters      = *( OWS ";" OWS [ parameter ] )`.

## Reader-visible behaviour

*run.* The form validation callback receives an empty object instead of foo=bar. It can reject a valid form as missing required fields or continue with empty data. Correcting the header or changing code avoids it; no middleware setting repairs it. No frequency estimate is established.

## Other code places and patch executions

*read, knowable at cut-off from the pinned code.* The comparison scenarios use these code places:

| Id | Behaviour described by the answer key | Place |
| --- | --- | --- |
| GT-p1 | A remembered form is ignored: a following parseBody fails or garbles fields, loses intervening edits, and parses again on each further call. | body.ts:125–132, especially reading bytes without checking bodyCache.formData. |
| GT-p2 | A formData read overlapping parseBody rejects because its rebuilt Response has no Content-Type. | body.ts:126 before line 130; request.ts:234. |
| GT-p3 | A first parseBody collects the whole upload as bytes, parses a copy, and keeps the bytes on a HonoRequest, increasing memory and first-read work. | body.ts:126–127; request.ts:238. |
| PE-p1 | A mixed-case form POST with _method=DELETE goes to POST instead of DELETE. | method-override/index.ts:74 and 92, unchanged. |

*read, knowable at cut-off.* The diff in change.diff replaces native formData delegation with a byte read at body.ts:126–127 and adds the cache write at 130. The method-override checks have no diff. Patches for the respective places are saved beside the results. GT-p1’s patch adds remembered-form reuse; GT-p2’s patch supplies the media type to the reconstructed Response; GT-p3’s patch hands an unread stream to the platform parser instead of collecting bytes; PE-p1’s patch normalizes the middleware’s media-type token.

*run, reproducible at the cut-off.* The candidate-place patch is `fix-M6.patch`. Its output is `result-fix-M6.txt`. The comparison-place patch alone, candidate-place patch alone and both together were run on Node and Bun for GT-p1, GT-p2, GT-p3 and PE-p1; per-scenario outputs and patches are under `probes/M6/`. See `grouping-matrix.md`.

*run.* Candidate-only fixes leave the distinguishing comparison failure in place, and comparison-only fixes leave this candidate in place. The raw-Request-only M5 patch repairs the plain-Request branch’s delegation but does not repair the HonoRequest first-read buffering. Patches demonstrate specific causal separations; they are not proposed production fixes, and some retain other faults.

*run.* The urlencoded-only patch leaves the multipart whitespace case; the multipart-only patch leaves the urlencoded case. Both patches together repair both.

## Impact facts

*run/read, reproducible or documented at the cut-off.* The form validation callback receives an empty object instead of foo=bar. It can reject a valid form as missing required fields or continue with empty data. Correcting the header or changing code avoids it; no middleware setting repairs it. No frequency estimate is established.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

## Known only later

*read, known only later.* The description was edited at 09:41:51–09:41:54Z, after the cut-off, only to tick checklist boxes. The saved GraphQL history includes the original body. The PR merged at 09:42:27Z and shipped in v4.12.28 on 2026-07-06. The release note lists its title; it announces no reader, diagnostic, whitespace or parameter exception. #5131 later adds remembered-form reuse. Later searches found #5226 (merged 2026-08-18) replacing prettyJSON’s match with a case-insensitive JSON/suffix regex and #5377 (merged 2026-09-20) handling invalid JSON bodies. Those are later source changes, not cut-off promises or acknowledgements of every candidate. Raw responses and files are under upstream/.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | problem | medium (no-precedent) | The rewritten urlencoded guard still rejects valid whitespace before the charset parameter, so form validation hands the callback {} while parseBody returns the fields. |
| assessor-1 | problem | high | The substantively rewritten urlencoded regex still rejects valid whitespace before the charset parameter and supplies an empty object to documented form validation. |
| assessor-2 | problem | high | The rewritten urlencoded guard gives documented form validation an empty object for valid whitespace even though parseBody succeeds, and its repair is on a different line and leaves the multipart failures unchanged. |

To reconcile before asking:

- None: every party's pick is the recommender's.

## `M6`: label

Record: [`15-hono-urlencoded-whitespace.M6-label.before.json`](15-hono-urlencoded-whitespace.M6-label.before.json)

```text
ASK
- band stays on its existing owner or reconciler route
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cards/M6.md:

# The urlencoded validator skips whitespace before a charset parameter

Hono is a web framework. Base is the commit before the change; head is the reviewed commit. The evidence below comes from the pinned source and documentation available at its last push.

## Executions

*run.* For Application/X-WWW-Form-Urlencoded ; charset=utf-8, the head parseBody returns foo=bar on Node and Bun, while validator(form) receives {} and the route returns 200. The validator also receives {} at the base. The lower-case header with the same whitespace has the same failure at both commits; removing the whitespace makes head form validation return the fields. json validation returns {} for this urlencoded header at both commits, as its target differs.

*run.* The candidate-place patch changes only the described code place and repairs the demonstrated case. The corresponding cache, reader and method-override patch scenarios are retained separately.

## What the affected person sees

*run/read.* The form validation callback receives an empty object instead of foo=bar. It can reject a valid form as missing required fields or continue with empty data. Correcting the header or changing code avoids it; no middleware setting repairs it. No frequency estimate is established.

## Impact facts

*run/read, reproducible or documented at the cut-off.* The form validation callback receives an empty object instead of foo=bar. It can reject a valid form as missing required fields or continue with empty data. Correcting the header or changing code avoids it; no middleware setting repairs it. No frequency estimate is established.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

*read.* To validate form data, specify `form` as the first argument and a callback as the second argument.

No production frequency is known. Node and Bun were executed; Deno and Cloudflare Workers were not. No timing, operating-system memory-limit failure, deployment or security exploit was measured for this case.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | serious | medium (no-precedent) | Validation silently hands the application {} for a valid urlencoded form (S4). |
| assessor-1 | serious | medium (no-precedent) | The validator silently receives no fields from a valid urlencoded form even though the parser reads foo=bar and the route returns success. |

To reconcile before asking:

- None: every party's pick is the recommender's.


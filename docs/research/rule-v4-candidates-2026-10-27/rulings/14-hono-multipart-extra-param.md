# Ruling 14-hono-multipart-extra-param: p-hono-5067

Decisions in this question: `M4b` (candidate); `M4b` (label).

## `M4b`: candidate

Record: [`14-hono-multipart-extra-param.M4b.before.json`](14-hono-multipart-extra-param.M4b.before.json)

```text
ASK
- recommender: problem stays with the owner
- assessor-1: problem stays with the owner
- assessor-2: problem stays with the owner
- assessor-2: gap or conflict in the rule
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cases/M4b.md:

# The multipart validator skips an extra charset parameter before boundary

## Code and dates

*read.* Base `9728702911073aec5a63a3ba2840b7240e5d3205`; head `5226d4165d48643586152614cbd07422a0ab7a22`; last push 2026-07-01T09:39:48Z. Source and documentation below existed by that cut-off unless marked later.

*read, knowable at cut-off.* validator.ts:25 is rewritten with /i, but its only optional parameter is still one boundary=. A charset parameter cannot match. body.ts:104 only uses the media-type token; bufferToFormData passes the parameters on to the native parser.

```diff
-const multipartRegex = /^multipart\/form-data(;\s?boundary=[a-zA-Z0-9'"()+_,\-./:=?]+)?$/
+const multipartRegex = /^multipart\/form-data(;\s?boundary=[a-zA-Z0-9'"()+_,\-./:=?]+)?$/i
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

*run, reproducible at cut-off.* For multipart/form-data; charset=utf-8; boundary=X, native formData and parseBody return foo=bar at both commits on Node and Bun. validator(form) receives {} and the pass-through route returns 200 at both commits. A sole boundary parameter is accepted.

Full per-runtime values, status codes and exceptions are saved in the probe result files; the case reproduces them here.

### base on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
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
    }
  },
  "Multipart/Form-Data ; boundary=X": {
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
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
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
    }
  },
  "multipart/form-data; boundary=X": {
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
    }
  }
}
```

### base on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
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
    }
  },
  "Multipart/Form-Data ; boundary=X": {
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
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
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
    }
  },
  "multipart/form-data; boundary=X": {
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
    }
  }
}
```

### head on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
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
    }
  },
  "Multipart/Form-Data ; boundary=X": {
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
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
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
    }
  },
  "multipart/form-data; boundary=X": {
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
    }
  }
}
```

### head on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
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
    }
  },
  "Multipart/Form-Data ; boundary=X": {
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
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
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
    }
  },
  "multipart/form-data; boundary=X": {
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
    }
  }
}
```

### candidate-place patch on Node v24.21.0

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
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
    }
  },
  "Multipart/Form-Data ; boundary=X": {
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
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
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
    }
  },
  "multipart/form-data; boundary=X": {
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
    }
  }
}
```

### candidate-place patch on Bun 1.3.10

*run, reproducible at the cut-off.*

```json
{
  "multipart/form-data ; boundary=X": {
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
    }
  },
  "Multipart/Form-Data ; boundary=X": {
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
    }
  },
  "multipart/form-data; charset=utf-8; boundary=X": {
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
    }
  },
  "multipart/form-data; boundary=X": {
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
    }
  }
}
```

## Documentation and contemporary statements

*read, knowable at cut-off.* Website documentation is pinned to b17f3d3f4eb3f4a4938d95b552aba5eb8a15d255, the latest website commit before 2026-07-01T09:39:48Z. The root README and the general concepts pages, including Web Standards and Middleware, were searched as well as the API and validation/formatting pages.

*read, knowable at cut-off.* honojs/website docs/guides/validation.md:

> To validate form data, specify `form` as the first argument and a callback as the second argument.

*read, published June 2022, before the cut-off.* RFC 9110 §8.3.1: The type and subtype tokens are case-insensitive. Section 5.6.6: Parameter names are case-insensitive. Its grammar is `parameters      = *( OWS ";" OWS [ parameter ] )`.

*read, published November 1996.* RFC 2045 §5.1:

```text
MIME
   implementations must ignore any parameters whose names they do not
   recognize.
```

*read, published July 2015.* RFC 7578 §8 registers `Optional parameters:  none`; the extra parameter is not interpreted as a charset instruction here. The platform runs report its handling independently.

## Reader-visible behaviour

*run.* The validation callback silently receives {} in place of all submitted fields. A required-field validator can reject the valid form; a permissive callback can continue with empty values. Removing the extra parameter or changing parsing code avoids it. No exposure rate is measured.

## Other code places and patch executions

*read, knowable at cut-off from the pinned code.* The comparison scenarios use these code places:

| Id | Behaviour described by the answer key | Place |
| --- | --- | --- |
| GT-p1 | A remembered form is ignored: a following parseBody fails or garbles fields, loses intervening edits, and parses again on each further call. | body.ts:125–132, especially reading bytes without checking bodyCache.formData. |
| GT-p2 | A formData read overlapping parseBody rejects because its rebuilt Response has no Content-Type. | body.ts:126 before line 130; request.ts:234. |
| GT-p3 | A first parseBody collects the whole upload as bytes, parses a copy, and keeps the bytes on a HonoRequest, increasing memory and first-read work. | body.ts:126–127; request.ts:238. |
| PE-p1 | A mixed-case form POST with _method=DELETE goes to POST instead of DELETE. | method-override/index.ts:74 and 92, unchanged. |

*read, knowable at cut-off.* The diff in change.diff replaces native formData delegation with a byte read at body.ts:126–127 and adds the cache write at 130. The method-override checks have no diff. Patches for the respective places are saved beside the results. GT-p1’s patch adds remembered-form reuse; GT-p2’s patch supplies the media type to the reconstructed Response; GT-p3’s patch hands an unread stream to the platform parser instead of collecting bytes; PE-p1’s patch normalizes the middleware’s media-type token.

*run, reproducible at the cut-off.* The candidate-place patch is `fix-M4b.patch`. Its output is `result-fix-M4b.txt`. The comparison-place patch alone, candidate-place patch alone and both together were run on Node and Bun for GT-p1, GT-p2, GT-p3 and PE-p1; per-scenario outputs and patches are under `probes/M4b/`. See `grouping-matrix.md`.

*run.* Candidate-only fixes leave the distinguishing comparison failure in place, and comparison-only fixes leave this candidate in place. The raw-Request-only M5 patch repairs the plain-Request branch’s delegation but does not repair the HonoRequest first-read buffering. Patches demonstrate specific causal separations; they are not proposed production fixes, and some retain other faults.

*run.* `fix-M4a` repairs only the whitespace case; `fix-M4b` repairs only the extra parameter; `fix-M4a+M4b` repairs both. `fix-M6` leaves both multipart cases unchanged.

## Impact facts

*run/read, reproducible or documented at the cut-off.* The validation callback silently receives {} in place of all submitted fields. A required-field validator can reject the valid form; a permissive callback can continue with empty values. Removing the extra parameter or changing parsing code avoids it. No exposure rate is measured.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

## Known only later

*read, known only later.* The description was edited at 09:41:51–09:41:54Z, after the cut-off, only to tick checklist boxes. The saved GraphQL history includes the original body. The PR merged at 09:42:27Z and shipped in v4.12.28 on 2026-07-06. The release note lists its title; it announces no reader, diagnostic, whitespace or parameter exception. #5131 later adds remembered-form reuse. Later searches found #5226 (merged 2026-08-18) replacing prettyJSON’s match with a case-insensitive JSON/suffix regex and #5377 (merged 2026-09-20) handling invalid JSON bodies. Those are later source changes, not cut-off promises or acknowledgements of every candidate. Raw responses and files are under upstream/.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | problem | medium (no-precedent) | The rewritten multipart guard rejects an extra parameter before boundary, which MIME requires a recipient to ignore, so form validation hands the callback {} while parseBody returns the fields. |
| assessor-1 | problem | high | The rewritten multipart regex rejects an ignorable extra parameter and discards the submitted fields, while the smallest parameter and whitespace fixes each leave the other's failure intact. |
| assessor-2 | problem | low (gap, no-precedent) | The rewritten multipart guard discards the form because of an extra parameter that MIME readers must ignore, and its smallest repair leaves M4a's whitespace failure intact. |

To reconcile before asking:

- None: every party's pick is the recommender's.

## `M4b`: label

Record: [`14-hono-multipart-extra-param.M4b-label.before.json`](14-hono-multipart-extra-param.M4b-label.before.json)

```text
ASK
- band stays on its existing owner or reconciler route
- two blind high-confidence answers and the medium-or-high recommender must agree
- an earlier decision of this type is still pending
- this decision type has not earned level 1 from earlier closed rounds
```

The case, docs/research/rule-v4-candidates-2026-10-27/p-hono-5067/cards/M4b.md:

# The multipart validator skips an extra charset parameter before boundary

Hono is a web framework. Base is the commit before the change; head is the reviewed commit. The evidence below comes from the pinned source and documentation available at its last push.

## Executions

*run.* For multipart/form-data; charset=utf-8; boundary=X, native formData and parseBody return foo=bar at both commits on Node and Bun. validator(form) receives {} and the pass-through route returns 200 at both commits. A sole boundary parameter is accepted.

*run.* The candidate-place patch changes only the described code place and repairs the demonstrated case. The corresponding cache, reader and method-override patch scenarios are retained separately.

## What the affected person sees

*run/read.* The validation callback silently receives {} in place of all submitted fields. A required-field validator can reject the valid form; a permissive callback can continue with empty values. Removing the extra parameter or changing parsing code avoids it. No exposure rate is measured.

## Impact facts

*run/read, reproducible or documented at the cut-off.* The validation callback silently receives {} in place of all submitted fields. A required-field validator can reject the valid form; a permissive callback can continue with empty values. Removing the extra parameter or changing parsing code avoids it. No exposure rate is measured.

The neighbouring cache scenarios produce errors, discarded edits or garbled values; method override reaches POST rather than DELETE. The first-read cache probe shows a retained raw arrayBuffer at the head and only formData at the base. No timing or process-memory magnitude was remeasured. No frequency or production harm is inferred from the bounded code search.

*read.* To validate form data, specify `form` as the first argument and a callback as the second argument.

No production frequency is known. Node and Bun were executed; Deno and Cloudflare Workers were not. No timing, operating-system memory-limit failure, deployment or security exploit was measured for this case.

First answers:

| Party | Pick | Confidence | Reason |
| --- | --- | --- | --- |
| recommender | serious | medium (no-precedent) | Validation silently hands the application {} for a valid form, so it acts on empty data without any error (S4). |
| assessor-1 | serious | medium (no-precedent) | A valid multipart form's submitted fields are silently replaced by an empty object supplied to validation. |

To reconcile before asking:

- None: every party's pick is the recommender's.


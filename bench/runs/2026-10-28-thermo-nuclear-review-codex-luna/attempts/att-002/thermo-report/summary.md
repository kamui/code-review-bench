No actionable findings.

The `Overwrite` change was traced through procedure input merging, middleware context merging, and `ResolveOptions`. It now replaces non-object types as intended, so middleware on a string input no longer maps string method keys into the inferred type. The new regression case covers that inference path, and the permitted offline TypeScript check passed (`packages/tests`: `tsc --noEmit -p tsconfig.json`).

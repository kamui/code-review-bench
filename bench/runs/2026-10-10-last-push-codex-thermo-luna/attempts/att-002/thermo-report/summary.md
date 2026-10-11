# Review summary

## Verdict

Request a small follow-up before approval. The change makes the error tests Flow-checked and corrects one public constructor declaration, but that correction leaves the declaration and implementation signatures inconsistent.

## Findings

**`src/error/GraphQLError.js:25,94` — Keep the nullability contract aligned.** The declaration now permits `null` for `nodes`, while the exported function signature still permits only arrays, AST nodes, or `void`. The implementation already normalizes falsey `nodes`, so the public type and implementation contract should agree. Add `null` to the implementation signature as well, or remove the duplicated signature source if the Flow structure allows it. See [01_error_types.md](01_error_types.md) for evidence and the code-judo proposal.

## Remediation sequence

1. Align the implementation parameter type with the declared constructor contract, then run the repository Flow check when available.
2. Keep the focused runtime behavior covered by the existing GraphQLError tests.

## Verification

The four changed test files passed using the clone's Mocha binary: 27 passing. Flow type-checking was not run; the supplied execution allowance permits focused Mocha execution, and the report does not claim that strict Flow validation succeeded. `git diff --check` was clean.

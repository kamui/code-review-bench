# Error typing and tests

## Scope and measurements

The reviewed range is 5384d218539dbb6bb39b25e0b7a5dcdd69ad8a11..7e39a122eea9292eeffa6905ffdf8a60c5161cfd. The diff changes five files, adding 48 lines and removing 47. The changed files remain small (the largest is 220 lines), and the test changes centralize a parsed source/AST fixture rather than introducing a new abstraction layer.

Commands used:

- git diff --stat main...review-head and git diff main...review-head to inspect the complete change.
- sed -n '1,270p' src/error/GraphQLError.js to inspect constructor behavior.
- git show main:src/error/__tests__/GraphQLError-test.js and nl -ba src/error/__tests__/GraphQLError-test.js to compare the original fixture and locate the changed case.
- wc -l on the changed files to check for file-size growth.
- git diff --check main...review-head to check patch whitespace.

## Finding

In src/error/__tests__/GraphQLError-test.js:58-59, the test named “creates new stack if original error has no stack” changed its fixture from a plain object with no stack to new Error('original'). In the constructor, lines 195-199 reuse originalError.stack whenever that value is truthy and only call Error.captureStackTrace in the fallback branch. The immediately preceding test already uses a normal Error to exercise stack reuse. Thus this edit leaves the fallback branch uncovered while preserving a test name and assertions that imply the branch is still tested.

This is a test-coverage regression rather than a demonstrated production behavior regression. The constructor’s fallback implementation is unchanged, but the suite no longer checks it through this case.

## Worked code-judo proposal

Keep strict Flow’s Error contract and make the absence of a stack explicit in the fixture. For example, create an Error, use Object.defineProperty(original, 'stack', { value: undefined }), then pass it to GraphQLError. That retains a real Error instance without widening production types or using any, and makes the branch precondition visible at the test site. A direct assignment to undefined is also suitable if accepted by this repository’s Flow libdefs.

The suite then has two distinct, honest examples: one where an original Error stack is reused, and one where the original Error has no stack and GraphQLError creates its own. No production refactor is warranted by this diff.

## Verification status

The changed test and constructor were inspected statically. Tests and Flow checks were not run. The patch whitespace check completed with no output.

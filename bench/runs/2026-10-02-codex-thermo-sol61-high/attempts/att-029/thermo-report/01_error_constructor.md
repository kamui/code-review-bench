# Error constructor and constructor tests

## Scope and judgment

Reviewed `src/error/GraphQLError.js` and `src/error/__tests__/GraphQLError-test.js` in the committed range. The constructor declaration gains `null` for `nodes`. The tests gain Flow strict mode, typed constructor messages, a shared parsed fixture, and one altered original-error fixture. The stackless fixture change is the single actionable issue; the production implementation remains unchanged.

## Finding: the regression test no longer supplies a stackless original error

At `src/error/__tests__/GraphQLError-test.js:57–64`, the test name still promises stack generation for an original error with no stack. At line 58, however, the head creates `new Error('original')`. In the permitted Node test environment, that object already has a nonempty stack. Passing it to the constructor at line 59 reaches the truthy `originalError && originalError.stack` condition at `src/error/GraphQLError.js:195`. Lines 196–200 copy that existing stack. Lines 201–208, which generate a stack when copying is unavailable, are no longer exercised by this case.

The preceding test at lines 41–55 already covers copying an original error's stack. The new input therefore removes a distinct regression case while making the suite appear to retain it. The base fixture was `{ message: 'original' }`, which had no stack and forced generation even though an original error was present. The separate no-original-error test does not protect the same boundary: it would not catch an erroneous assumption that every present original error has a usable stack.

This is a test coverage regression, not a demonstrated runtime regression in this PR. It is actionable because the PR's purpose is to type these tests without changing what they protect, and a small typed fixture correction restores that guarantee.

## Source evidence and measurements

The source was inspected with `git diff main...review-head`, `nl -ba src/error/GraphQLError.js`, `nl -ba src/error/__tests__/GraphQLError-test.js`, and `git show main:src/error/__tests__/GraphQLError-test.js`. The latter establishes the missing-stack precondition in the base rather than inferring it from the test name.

`wc -l` and matching `git show main:<path> | wc -l` measurements give 220 → 220 lines for the constructor and 156 → 150 lines for its test file. The constructor has +1/−1 changed lines, entirely in the declaration. The test file has +27/−33 changed lines. Neither file crosses the skill's 1,000-line decomposition threshold. The diff introduces no production conditional and no new production dependency.

The declaration at line 25 now accepts `null`, matching the existing falsy-node normalization at lines 101–108. The implementation's separate function annotation at line 94 is pre-existing and remains unchanged. This review does not treat that longstanding dual-declaration arrangement as a new regression or demand a constructor rewrite to resolve a one-line public typing fix.

The shared source at test lines 17–26 parses one immutable-by-use document and narrows the first definition through `Kind.OPERATION_DEFINITION`. Existing cases continue to verify array nodes, a single node, offset zero, explicit positions, and serialization. Dedenting moves the field's position from 8 to 4 and its column from 7 to 3; the updated assertions correspond to the new source and pass. No test mutates the shared fixture.

## Verification

The focused target command was:

```sh
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha --require @babel/register --require @babel/polyfill src/error/__tests__/GraphQLError-test.js src/error/__tests__/locatedError-test.js src/error/__tests__/printError-test.js src/jsutils/__tests__/inspect-test.js
```

It completed successfully with 27 passing tests, including the misleadingly named stack-generation case. It was executed once with that flag set. The installed dependencies emitted an outdated Browserslist data warning; no dependency update or network request was attempted.

Scratch verification is retained outside the checkout at `../stack-review-test.js`. It loads the actual constructor and checks that the head fixture's resulting stack equals `original.stack`. It next clears a typed Error's stack and verifies that the actual constructor produces a nonempty, different string while retaining the originalError identity.

The third scratch check reads the constructor source and changes only `if (originalError && originalError.stack)` to `if (originalError)` before compiling it in memory with the clone's Babel. With this faulty constructor, all four assertions from the changed stack-generation test still pass for its current fixture. A cleared-stack fixture instead produces an empty stack and fails a nonempty-stack assertion. The exact old object fixture produces an undefined stack and fails the original string assertion. This distinguishes a verified loss of mutation sensitivity from a merely hypothetical test-quality concern.

The first scratch run had two passing checks and one failure because an added assertion assumed a `GraphQLError: msg` stack header. That assumption is outside the behavior being reviewed. It was removed in favor of a nonempty-stack assertion. The corrected selection ran once with a different reporter flag:

```sh
BABEL_DISABLE_CACHE=1 ./node_modules/.bin/mocha --reporter dot --require @babel/register --require @babel/polyfill /home/jack/.t3/bench-runs/2026-10-02-codex-thermo-sol61-high/att-029/clone-work/stack-review-test.js
```

Result: 3 passing. No target file was rewritten or mutation installed in the checkout. Babel caching was disabled for both selections.

## Worked code-judo remedy

Keep the nominal Error input that Flow expects and explicitly represent the constructor's falsy-stack boundary. This deletes the need to choose between an untyped structural fixture and weakened behavioral coverage. The proposed body preserves the existing assertions and adds the missing precondition and output distinction:

```js
it('creates new stack if original error has no stack', () => {
  const original = new Error('original');
  original.stack = '';
  expect(original.stack).to.equal('');

  const e = new GraphQLError('msg', null, null, null, null, original);
  expect(e.name).to.equal('GraphQLError');
  expect(e.stack).to.be.a('string');
  expect(e.stack).to.not.equal(original.stack);
  expect(e.message).to.equal('msg');
  expect(e.originalError).to.equal(original);
});
```

Clearing the stack to an empty string retains its string type and exercises the same falsy-stack branch as an absent property. There is no `any`, cast, new helper, or production special case. The runtime behavior of this fixture and the stronger stack assertion was verified in scratch. The exact proposed Flow typing was not checked; run the repository's normal Flow check after implementing the correction in an authorized development environment.

The larger structural simplification in this subsystem is already in the diff: a shared parsed source replaces several repeated fixtures. A generic AST extraction helper or table-driven rewrite would create extra concepts without deleting substantial complexity in this 150-line suite. No such refactor is needed for approval.

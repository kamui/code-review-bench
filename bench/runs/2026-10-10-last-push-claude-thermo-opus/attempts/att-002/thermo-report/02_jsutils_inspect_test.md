# 02. jsutils: `src/jsutils/__tests__/inspect-test.js`

Covers finding F3 from `summary.md`.

File at the head (`7e39a122e`): 78 lines, +2 / -1. The change flips the header from `@noflow` to `@flow strict` and adds one suppression comment.

## Verification performed

The file's nine tests pass at the head as part of the 27-test run recorded in `01_error_module.md`.

The scratch probe (`clone-work/scratch/stack-branch.js`) also compares the tagged-template expectation with a plain literal:

```
raw === literal: true "\"\\\"\""
```

That is, ``String.raw`"\""`` and `'"\\""'` are the same four-character string: quote, backslash, quote, quote.

Not performed: `flow check` (outside the execution allowance). The statement that Flow rejects the `String.raw` tagged call is inferred from the fact that the PR had to add a suppression on that line to turn `@flow strict` on, and from the identical suppression already sitting above the other typed `String.raw` use in the repository. Flow: not executed.

## F3. Unexplained `$FlowFixMe` where a literal would do

### Evidence

Head, lines 28-33:

```js
  it('string', () => {
    expect(inspect('')).to.equal('""');
    expect(inspect('abc')).to.equal('"abc"');
    // $FlowFixMe
    expect(inspect('"')).to.equal(String.raw`"\""`);
  });
```

The repository's suppression config (`.flowconfig`) matches `$FlowFixMe` against the whole following line, so the comment hides any Flow error on line 32, not only the `String.raw` one. That includes errors in `inspect('"')`, the call the test is about.

How other suppressions in the tree are written, from a search of `src` for `FlowFixMe|FlowIssue|DisableFlowOnNegativeTest`:

```
src/jsutils/objectValues.js:15:// $FlowFixMe workaround for: https://github.com/facebook/flow/issues/2221
src/jsutils/isInteger.js:14:// $FlowFixMe workaround for: https://github.com/facebook/flow/issues/4441
src/jsutils/isFinite.js:14:// $FlowFixMe workaround for: https://github.com/facebook/flow/issues/4441
src/subscription/mapAsyncIterator.js:25:  // $FlowFixMe(>=0.68.0)
src/utilities/__tests__/schemaPrinter-test.js:178:      // $FlowFixMe
src/jsutils/__tests__/inspect-test.js:31:    // $FlowFixMe
```

The `jsutils` production suppressions each say what they work around. The new one says nothing. The one bare precedent, `schemaPrinter-test.js` line 178, was added two commits earlier in the same conversion series and wraps a multi-line SDL block containing `\t` and `\f` escapes, where rewriting with doubled backslashes would hurt readability. That reasoning does not carry over to a four-character expectation.

Status: runtime equivalence verified by execution; the Flow side is inferred, not executed.

### Why it matters

The PR exists to bring these files under type checking. Adding a line-wide hole in order to keep a clever spelling of a four-character string works against that goal for no gain. `String.raw` here is not making anything clearer: the two neighbouring assertions on lines 29 and 30 use ordinary single-quoted literals, and the reader has to stop and decode a tagged template to learn that the third expectation is the same kind of value.

### Worked proposal

```js
  it('string', () => {
    expect(inspect('')).to.equal('""');
    expect(inspect('abc')).to.equal('"abc"');
    expect(inspect('"')).to.equal('"\\""');
  });
```

One line changed, one line deleted. The suppression goes away, `String.raw` goes away, all three assertions read the same way, and line 32 is fully checked. If the maintainers would rather keep `String.raw` for some reason not visible in the diff, the minimum acceptable form is a suppression that states its cause, in the style of the `jsutils` production files, so a later Flow upgrade can retire it.

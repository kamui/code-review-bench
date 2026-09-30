# 02 — `src/jsutils/__tests__/inspect-test.js`

Range: `5384d218..7e39a122`. The change adds `@flow strict` and a bare `// $FlowFixMe` above
``expect(inspect('"')).to.equal(String.raw`"\""`);`` (line 31–32 at head).

## Measurements and commands

- `grep -rn "FlowFixMe" src` finds existing precedent for exactly this pattern:
  `src/utilities/__tests__/schemaPrinter-test.js:178` puts a bare `// $FlowFixMe` above a
  `String.raw` template. The other suppressions in the repo name either a Flow version
  (`$FlowFixMe(>=0.68.0)`) or an upstream issue URL (`objectValues.js`, `isInteger.js`, `isFinite.js`).
- `.flowconfig` suppression regex: `$FlowFixMe` with or without a version matches, so the bare form
  suppresses **every** error on the next line.
- The mocha run of this file passes (all 9 `inspect` cases).

---

## Finding 2.1 — A blanket `$FlowFixMe` is used where the suppressed construct could simply be deleted

**Evidence.** The only reason for the suppression is that Flow 0.86 cannot type the `String.raw`
tagged template. The template produces a four-character string: quote, backslash, quote, quote.
A plain literal expresses that directly. The bare `$FlowFixMe` also silences any future type error
on that whole line, including errors in the `inspect('"')` call it is supposed to be testing. It
also carries no explanation, unlike the documented suppressions elsewhere in `src/jsutils`.

**Why it matters.** The PR's purpose is to turn type-checking on for these tests, and this line
switches it back off for one of the assertions. The precedent in `schemaPrinter-test.js` makes
this low severity, but in this case, unlike the multi-line schema fixture there, the construct is
trivial to avoid.

**Verification status:** Confirmed by reading the code. The literal equivalence
(``String.raw`"\""` === '"\\""'``) follows from JavaScript string semantics. It was not executed
separately.

**Remedy (code-judo: delete the suppression instead of documenting it).**

```js
expect(inspect('"')).to.equal('"\\""');
```

If the author prefers `String.raw` for readability, the suppression should at least say why, in
the same style as the rest of `src/jsutils`
(`// $FlowFixMe(>=0.86.0) String.raw tagged templates are untyped`).

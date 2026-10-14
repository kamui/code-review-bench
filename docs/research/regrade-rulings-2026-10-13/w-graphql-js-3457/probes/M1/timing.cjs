'use strict';
// Times validation of a query that repeats one field N times, at one checkout of graphql-js.
//
//   node timing.cjs <src|dist> <checkout> <label> <out.json> [--sizes 100,200] [--cases a,b] [--rules overlap|all] [--reps 5]
//
// `src` loads the TypeScript source through the project's own loader (resources/ts-register.js), as the
// project's tests do. `dist` loads the package built by the project's resources/build-npm.js (npmDist).
const path = require('path');
const fs = require('fs');

const [mode, checkoutArg, label, outPath, ...rest] = process.argv.slice(2);
const options = { sizes: '100,200,300,1000,2000', cases: '', rules: 'overlap', reps: '5' };
for (let i = 0; i < rest.length; i += 2) {
  options[rest[i].replace(/^--/, '')] = rest[i + 1];
}
const checkout = path.resolve(checkoutArg);

let root;
if (mode === 'src') {
  process.chdir(checkout);
  require(path.join(checkout, 'resources/ts-register.js'));
  root = path.join(checkout, 'src');
} else if (mode === 'dist') {
  root = path.join(checkout, 'npmDist');
} else {
  throw new Error('mode must be src or dist');
}
const { buildSchema } = require(path.join(root, 'utilities/buildASTSchema'));
const { parse } = require(path.join(root, 'language/parser'));
const { validate } = require(path.join(root, 'validation/validate'));
const { OverlappingFieldsCanBeMergedRule } = require(
  path.join(root, 'validation/rules/OverlappingFieldsCanBeMergedRule'),
);

const schema = buildSchema(`
  input Inner { c: Int, d: String }
  input In { a: Int, b: Inner, tags: [String] }
  type Query {
    hello: String
    one(a: Int): String
    two(a: Int, b: Int): String
    nested(input: In): String
    five(a: Int, b: Int, c: String, d: Boolean, e: Float): String
  }
`);

const CASES = {
  'no-arguments': 'hello',
  'one-scalar': 'one(a: 1)',
  'two-scalars': 'two(a: 1, b: 2)',
  'nested-input-object': 'nested(input: {a: 1, b: {c: 2, d: "x"}, tags: ["p", "q"]})',
  'five-scalars': 'five(a: 1, b: 2, c: "x", d: true, e: 1.5)',
};

const rules = options.rules === 'all' ? undefined : [OverlappingFieldsCanBeMergedRule];
const sizes = options.sizes.split(',').map(Number);
const reps = Number(options.reps);
const names = options.cases ? options.cases.split(',') : Object.keys(CASES).slice(0, 4);

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = sorted.length >> 1;
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}

const rows = [];
for (const name of names) {
  const selection = CASES[name];
  const warm = parse(`{ ${(selection + ' ').repeat(60)} }`);
  for (let i = 0; i < 20; i++) {
    validate(schema, warm, rules);
  }
  for (const n of sizes) {
    const text = `{ ${(selection + ' ').repeat(n)} }`;
    const doc = parse(text);
    const samples = [];
    let errors = -1;
    const count = n >= 1000 ? Math.min(reps, 3) : reps;
    for (let i = 0; i < count; i++) {
      const start = process.hrtime.bigint();
      errors = validate(schema, doc, rules).length;
      samples.push(Number(process.hrtime.bigint() - start) / 1e6);
    }
    const row = {
      label,
      mode,
      rules: options.rules,
      case: name,
      fields: n,
      pairs: (n * (n - 1)) / 2,
      query_bytes: Buffer.byteLength(text),
      errors,
      samples_ms: samples.map((s) => Number(s.toFixed(3))),
      median_ms: Number(median(samples).toFixed(3)),
      min_ms: Number(Math.min(...samples).toFixed(3)),
    };
    rows.push(row);
    console.log(JSON.stringify(row));
  }
}
fs.writeFileSync(
  outPath,
  JSON.stringify(
    { label, mode, rules: options.rules, node: process.version, node_env: process.env.NODE_ENV ?? null, rows },
    null,
    1,
  ) + '\n',
);

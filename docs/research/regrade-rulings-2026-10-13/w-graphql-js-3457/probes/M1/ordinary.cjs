'use strict';
// Times validation of small queries, where one field is selected a few times, at one checkout of graphql-js.
//
//   node ordinary.cjs <checkout> <label> <out.json> [fragments]
//
// With the fourth argument `fragments` it runs only the fragment workloads, in two forms: fragments that
// select fields with and without arguments, and fragments that select only fields with arguments.
//
// Loads the built package (npmDist). Each figure is microseconds for one validation, the median of five
// batches of 200 validations, with the overlap rule alone and with the default rule set.
const path = require('path');
const fs = require('fs');

const [checkoutArg, label, outPath, only] = process.argv.slice(2);
const root = path.join(path.resolve(checkoutArg), 'npmDist');
const { buildSchema } = require(path.join(root, 'utilities/buildASTSchema'));
const { parse } = require(path.join(root, 'language/parser'));
const { validate } = require(path.join(root, 'validation/validate'));
const { OverlappingFieldsCanBeMergedRule } = require(
  path.join(root, 'validation/rules/OverlappingFieldsCanBeMergedRule'),
);

const schema = buildSchema(`
  input Inner { c: Int, d: String }
  input In { a: Int, b: Inner, tags: [String] }
  type User {
    id: ID
    name: String
    avatar(size: Int): String
    friends(first: Int): [User]
  }
  type Query {
    hello: String
    one(a: Int): String
    two(a: Int, b: Int): String
    nested(input: In): String
    user: User
  }
`);

function repeated(selection, n) {
  return `{ ${(selection + ' ').repeat(n)} }`;
}
function fragments(n, body = 'id name avatar(size: 64) friends(first: 10) { id name avatar(size: 64) }') {
  const spreads = Array.from({ length: n }, (_, i) => `...F${i}`).join(' ');
  const definitions = Array.from({ length: n }, (_, i) => `fragment F${i} on User { ${body} }`).join('\n');
  return `{ user { ${spreads} } }\n${definitions}`;
}

const workloads = [];
for (const n of only === 'fragments' ? [] : [2, 5, 10, 20, 50]) {
  workloads.push([`no-arguments x${n}`, repeated('hello', n)]);
  workloads.push([`one-scalar x${n}`, repeated('one(a: 1)', n)]);
  workloads.push([`two-scalars x${n}`, repeated('two(a: 1, b: 2)', n)]);
  workloads.push([`nested-input-object x${n}`, repeated('nested(input: {a: 1, b: {c: 2, d: "x"}, tags: ["p", "q"]})', n)]);
}
for (const n of [5, 20, 50]) {
  workloads.push([`${n} fragments on one object, each selecting two fields with an argument`, fragments(n)]);
  if (only === 'fragments') {
    workloads.push([
      `${n} fragments on one object, fields with arguments only`,
      fragments(n, 'avatar(size: 64) friends(first: 10) { avatar(size: 64) }'),
    ]);
  }
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[sorted.length >> 1];
}
function measure(doc, rules) {
  for (let i = 0; i < 200; i++) {
    validate(schema, doc, rules);
  }
  const batches = [];
  for (let b = 0; b < 5; b++) {
    const start = process.hrtime.bigint();
    for (let i = 0; i < 200; i++) {
      validate(schema, doc, rules);
    }
    batches.push(Number(process.hrtime.bigint() - start) / 1e3 / 200);
  }
  return Number(median(batches).toFixed(1));
}

const rows = [];
for (const [name, text] of workloads) {
  const doc = parse(text);
  const errors = validate(schema, doc).length;
  const row = {
    label,
    workload: name,
    query_bytes: Buffer.byteLength(text),
    errors,
    overlap_rule_alone_us: measure(doc, [OverlappingFieldsCanBeMergedRule]),
    all_default_rules_us: measure(doc, undefined),
  };
  rows.push(row);
  console.log(JSON.stringify(row));
}
fs.writeFileSync(outPath, JSON.stringify({ label, node: process.version, rows }, null, 1) + '\n');

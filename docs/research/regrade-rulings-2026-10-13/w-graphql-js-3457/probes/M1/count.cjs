'use strict';
// Counts the work the overlap rule asks of the printer for one validation, at one checkout of graphql-js.
//
//   node count.cjs <checkout> <label> <out.json>
//
// It loads the built package (npmDist), replaces the printer's `print` and the utility `sortValueNode` with
// counting wrappers before the rule is loaded, and validates a query that repeats one field 100 times.
const path = require('path');
const fs = require('fs');

const [checkoutArg, label, outPath] = process.argv.slice(2);
const root = path.join(path.resolve(checkoutArg), 'npmDist');

const counts = { print: 0, printedCharacters: 0, printedNodes: 0, sortValueNode: 0 };
const printer = require(path.join(root, 'language/printer'));
const sorter = require(path.join(root, 'utilities/sortValueNode'));
const realPrint = printer.print;
const realSort = sorter.sortValueNode;

function nodes(value) {
  if (Array.isArray(value)) {
    return value.reduce((sum, item) => sum + nodes(item), 0);
  }
  if (value === null || typeof value !== 'object' || typeof value.kind !== 'string') {
    return 0;
  }
  return 1 + Object.keys(value).reduce((sum, key) => (key === 'loc' ? sum : sum + nodes(value[key])), 0);
}

printer.print = function countedPrint(ast) {
  counts.print += 1;
  counts.printedNodes += nodes(ast);
  const text = realPrint(ast);
  counts.printedCharacters += text.length;
  return text;
};
sorter.sortValueNode = function countedSort(node) {
  counts.sortValueNode += 1;
  return realSort(node);
};

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
  }
`);
const CASES = {
  'no-arguments': 'hello',
  'one-scalar': 'one(a: 1)',
  'two-scalars': 'two(a: 1, b: 2)',
  'nested-input-object': 'nested(input: {a: 1, b: {c: 2, d: "x"}, tags: ["p", "q"]})',
};

const rows = [];
const n = 100;
for (const [name, selection] of Object.entries(CASES)) {
  for (const key of Object.keys(counts)) {
    counts[key] = 0;
  }
  const errors = validate(schema, parse(`{ ${(selection + ' ').repeat(n)} }`), [OverlappingFieldsCanBeMergedRule]).length;
  const pairs = (n * (n - 1)) / 2;
  const row = {
    label,
    case: name,
    fields: n,
    pairs,
    errors,
    print_calls: counts.print,
    print_calls_per_pair: counts.print / pairs,
    nodes_handed_to_print_per_pair: counts.printedNodes / pairs,
    characters_printed_per_pair: counts.printedCharacters / pairs,
    sortValueNode_calls_from_the_rule: counts.sortValueNode,
  };
  rows.push(row);
  console.log(JSON.stringify(row));
}
fs.writeFileSync(outPath, JSON.stringify({ label, node: process.version, rows }, null, 1) + '\n');

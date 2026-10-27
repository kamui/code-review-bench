'use strict';

const path = require('path');
const root = process.argv[2];
require(path.join(root, 'resources/ts-register.js'));
const printer = require(path.join(root, 'src/language/printer.ts'));
const { visit } = require(path.join(root, 'src/language/visitor.ts'));
const originalPrint = printer.print;
let prints = 0;
let printedNodes = 0;
printer.print = function (node) {
  prints++;
  visit(node, { enter() { printedNodes++; } });
  return originalPrint(node);
};
const {
  buildSchema, parse, validate, executeSync, valueFromASTUntyped,
  OverlappingFieldsCanBeMergedRule,
} = require(path.join(root, 'src/index.ts'));
const schema = buildSchema(`
  input Input { s: String }
  type Query {
    f(s: String): String
    nested(input: Input): String
    hello: String
    one(a: Int): String
    collision(a9007199254740992: Int, a9007199254740993: Int): String
  }
`);
const queries = {
  'mixed-abc': '{ f(s: "abc") f(s: """abc""") }',
  'mixed-x': '{ f(s: "x") f(s: """x""") }',
  'mixed-empty': '{ f(s: "") f(s: """""") }',
  'mixed-input-object': '{ nested(input: {s: "abc"}) nested(input: {s: """abc"""}) }',
  'mixed-fragments': '{ ...A ...B } fragment A on Query { f(s: "abc") } fragment B on Query { f(s: """abc""") }',
  'quoted-control': '{ f(s: "abc") f(s: "abc") }',
  'block-control': '{ f(s: """abc""") f(s: """abc""") }',
  'escaped-control': '{ f(s: "abc") f(s: "\\u0061bc") }',
  'different-control': '{ f(s: "abc") f(s: """def""") }',
  'alias-workaround': '{ a: f(s: "abc") b: f(s: """abc""") }',
  'variable-workaround': 'query($s: String) { f(s: $s) f(s: $s) }',
  'single-quoted': '{ f(s: "abc") }',
  'single-block': '{ f(s: """abc""") }',
  'GT-w1': '{ collision(a9007199254740992: 1, a9007199254740993: 2) collision(a9007199254740993: 2, a9007199254740992: 1) }',
  'GT-w2-noargs': '{ ' + 'hello '.repeat(100) + '}',
  'GT-w2-withargs': '{ ' + 'one(a: 1) '.repeat(100) + '}',
};
const results = [];
for (const [name, query] of Object.entries(queries)) {
  const document = parse(query);
  prints = 0;
  printedNodes = 0;
  const errors = validate(schema, document);
  const printCalls = prints;
  const printedNodeCount = printedNodes;
  let resolverCalls = 0;
  const rootValue = {
    f: ({ s }) => { resolverCalls++; return s; },
    nested: ({ input }) => { resolverCalls++; return input.s; },
    hello: () => 'hello', one: () => 'one', collision: () => 'collision',
  };
  const result = errors.length ? null : executeSync({ schema, document, rootValue, variableValues: { s: 'abc' } });
  const overlap = validate(schema, document, [OverlappingFieldsCanBeMergedRule]);
  results.push({name, query, errors: errors.map(e => e.message), overlapErrors: overlap.map(e => e.message), printCalls, printedNodeCount, result, resolverCalls});
}
const mixed = parse(queries['mixed-abc']).definitions[0].selectionSet.selections;
const values = mixed.map(field => field.arguments[0].value);
const nodes = values.map(node => ({kind:node.kind, value:node.value, block:node.block, printed:originalPrint(node), coerced:valueFromASTUntyped(node)}));
console.log(JSON.stringify({revision:process.env.PROBE_REVISION, nodes, results}, null, 2));

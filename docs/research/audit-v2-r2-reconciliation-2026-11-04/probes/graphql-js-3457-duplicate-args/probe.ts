// Runs OverlappingFieldsCanBeMergedRule alone on queries with a repeated argument name.
// Usage: bun probe.ts <dir holding the commit's src/>
const dir = process.argv[2];
const g = await import(`${dir}/src/index.ts`);
const schema = g.buildSchema('type Query { f(a: Int): Int }');
const queries = [
  '{ f(a: 1, a: 2) f(a: 2, a: 1) }',
  '{ f(a: 1, a: 1) f(a: 1, a: 2) }',
  '{ f(a: 1, a: 2) f(a: 1, a: 2) }',
];
for (const q of queries) {
  const errors = g.validate(schema, g.parse(q), [g.OverlappingFieldsCanBeMergedRule]);
  console.log(JSON.stringify({ query: q, errors: errors.map((e: { message: string }) => e.message) }));
}
const full = g.validate(schema, g.parse(queries[0]));
console.log(JSON.stringify({ query: queries[0], all_specified_rules: full.map((e: { message: string }) => e.message) }));

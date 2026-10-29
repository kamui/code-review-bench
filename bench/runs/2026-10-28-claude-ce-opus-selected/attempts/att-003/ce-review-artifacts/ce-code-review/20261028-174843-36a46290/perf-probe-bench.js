require('@babel/register')({ extensions: ['.ts'] });
const { buildSchema } = require('./src/utilities/buildASTSchema');
const { parse } = require('./src/language/parser');
const { validate } = require('./src/validation/validate');
const { OverlappingFieldsCanBeMergedRule } = require('./src/validation/rules/OverlappingFieldsCanBeMergedRule');
const schema = buildSchema('type Query { a: String  b(x: Int, y: Int, z: Int): String }');
const run = (label, q) => {
  const doc = parse(q);
  validate(schema, doc, [OverlappingFieldsCanBeMergedRule]);
  const t = process.hrtime.bigint();
  let errs;
  for (let i = 0; i < 3; i++) errs = validate(schema, doc, [OverlappingFieldsCanBeMergedRule]);
  console.log(label, Number(process.hrtime.bigint() - t) / 3e6, 'ms', errs.length, 'errors');
};
const N = 1500;
run('noargs', '{' + ' a'.repeat(N) + '}');
run('args', '{' + ' b(x: 1, y: 2, z: 3)'.repeat(N) + '}');
run('argsReordered', '{' + ' b(x: 1, y: 2, z: 3) b(z: 3, y: 2, x: 1)'.repeat(N / 2) + '}');

'use strict';
// The failing case of known problem GT-w1, against the package built at one checkout.
//
//   node w1.cjs <checkout> <label>
//
// Two selections of one field give the same two arguments in opposite orders. The two argument names differ
// only in digits that round to the same JavaScript number.
const path = require('path');

const [checkoutArg, label] = process.argv.slice(2);
const root = path.join(path.resolve(checkoutArg), 'npmDist');
const { buildSchema } = require(path.join(root, 'utilities/buildASTSchema'));
const { parse } = require(path.join(root, 'language/parser'));
const { validate } = require(path.join(root, 'validation/validate'));

const [a, b] = ['a9007199254740992', 'a9007199254740993'];
const schema = buildSchema(`type Query { f(${a}: Int, ${b}: Int): String }`);
const errors = validate(schema, parse(`{ f(${a}: 1, ${b}: 2) f(${b}: 2, ${a}: 1) }`));
console.log(JSON.stringify({ label, case: 'GT-w1 reordered arguments', errors: errors.map((e) => e.message) }));

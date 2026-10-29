// Scratch verification: does the rewritten "no stack" test still reach the
// captureStackTrace fallback in GraphQLError? Run from the clone root with
// @babel/register so the clone's .babelrc applies to the required source.
const path = require('path');
const clone = process.cwd();
const { GraphQLError } = require(path.join(clone, 'src/error/GraphQLError.js'));

// Post-diff fixture
const realErr = new Error('original');
const e1 = new GraphQLError('msg', null, null, null, null, realErr);
console.log('post-diff: original has stack =', typeof realErr.stack === 'string');
console.log('post-diff: e.stack === original.stack =', e1.stack === realErr.stack);

// Pre-diff fixture
const plain = { message: 'original' };
const e2 = new GraphQLError('msg', null, null, null, null, plain);
console.log('pre-diff: original has stack =', typeof plain.stack === 'string');
console.log('pre-diff: e.stack is own fresh string =', typeof e2.stack === 'string' && e2.stack !== plain.stack);

// Does the widened declaration's null `nodes` behave at runtime?
const e3 = new GraphQLError('msg', null);
console.log('null nodes -> nodes/positions/locations undefined =',
  e3.nodes === undefined && e3.positions === undefined && e3.locations === undefined);

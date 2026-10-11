const clone = process.cwd();
const { GraphQLError } = require(clone + '/src/error/GraphQLError');
const real = new Error('original');
const e1 = new GraphQLError('msg', null, null, null, null, real);
console.log('head-version original: typeof original.stack =', typeof real.stack, '| e.stack === original.stack:', e1.stack === real.stack);
const fake = { message: 'original' };
const e2 = new GraphQLError('msg', null, null, null, null, fake);
console.log('base-version original: typeof original.stack =', typeof fake.stack, '| e.stack is own fresh string:', typeof e2.stack === 'string' && e2.stack !== fake.stack);

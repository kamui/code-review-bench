const { GraphQLError } = require(process.argv[2] + '/src/error/GraphQLError');
// New test input (head)
const withStack = new Error('original');
const e1 = new GraphQLError('msg', null, null, null, null, withStack);
console.log('head input: e.stack === original.stack ->', e1.stack === withStack.stack);
// Old test input (base)
const noStack = { message: 'original' };
const e2 = new GraphQLError('msg', null, null, null, null, noStack);
console.log('base input: typeof original.stack ->', typeof noStack.stack, '| typeof e.stack ->', typeof e2.stack, '| starts with ->', JSON.stringify(String(e2.stack).split('\n')[0]));
// Flow-typed stack-less Error alternative
const stackless = new Error('original');
Object.defineProperty(stackless, 'stack', { value: undefined });
const e3 = new GraphQLError('msg', null, null, null, null, stackless);
console.log('stackless Error: typeof original.stack ->', typeof stackless.stack, '| typeof e.stack ->', typeof e3.stack, '| reused ->', e3.stack === stackless.stack);

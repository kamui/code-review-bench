const path = require('path');
require(path.join(process.cwd(), 'node_modules/@babel/register'))({ cache: false });
const { parse, Source, GraphQLError } = require(path.join(process.cwd(), 'src'));
for (const [name, input] of [
  ['single-line-string', '{ field }'],
  ['single-line-Source', new Source('{ field }')],
  ['multiline-Source', new Source('{\n  field\n}')],
]) {
  const node = parse(input).definitions[0].selectionSet.selections[0];
  const error = new GraphQLError('msg', [node]);
  console.log(JSON.stringify({ name, start: node.loc.start, serialized: JSON.stringify(error) }));
}
const original = new Error('original');
delete original.stack;
const error = new GraphQLError('msg', null, null, null, null, original);
console.log(JSON.stringify({ name: 'no-stack-original', originalStack: typeof original.stack, stack: typeof error.stack }));

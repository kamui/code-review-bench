const path = require('path');
require(path.join(process.cwd(), 'node_modules/@babel/register'))({ cache: false });
const { graphql, buildSchema } = require(path.join(process.cwd(), 'src'));
const schema = buildSchema('type Query { hello: String }');
graphql(schema, '{ boyhowdy }').then(result => {
  console.log(JSON.stringify(result));
  if (JSON.stringify(result.errors[0].locations) !== '[{"line":1,"column":3}]') {
    process.exitCode = 1;
  }
});

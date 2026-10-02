require('./resources/ts-register');
const {buildSchema} = require('./src/utilities/buildASTSchema');
const {parse} = require('./src/language/parser');
const {validate} = require('./src/validation/validate');
const {OverlappingFieldsCanBeMergedRule: rule} = require('./src/validation/rules/OverlappingFieldsCanBeMergedRule');
const {naturalCompare} = require('./src/jsutils/naturalCompare');
for (const [a,b] of [['a01a','a1aa'],['a9007199254740992','a9007199254740993']]) {
 const schema=buildSchema(`type Query { f(${a}: Int, ${b}: Int): String }`);
 const doc=parse(`{ f(${a}: 1, ${b}: 2) f(${b}: 2, ${a}: 1) }`);
 console.log(JSON.stringify({case:'ordering',a,b,compare:naturalCompare(a,b),errors:validate(schema,doc,[rule]).map(e=>e.message)}));
}
const schema=buildSchema('type Query { hello: String }');
for (const n of [1,1000]) {
 const doc=parse(`{ ${'hello '.repeat(n)} }`);
 validate(schema,parse('{ hello hello }'),[rule]);
 const samples=[];let errors;
 for(let i=0;i<3;i++){const start=process.hrtime.bigint();errors=validate(schema,doc,[rule]);samples.push(Number(process.hrtime.bigint()-start)/1e6)}
 console.log(JSON.stringify({case:'performance',fields:n,samples_ms:samples,errors:errors.length}));
}

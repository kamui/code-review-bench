// Usage: node print-types.cjs <checkout-dir> <probe.ts>
// Type-checks <probe.ts> against <checkout-dir>/packages/server/src with the
// project's pinned TypeScript, prints every diagnostic in the probe file and the
// fully expanded type of every `type Show_*` alias.
const path = require('path');
const ts = require('typescript');

const [checkout, probe] = process.argv.slice(2).map((p) => path.resolve(p));
const options = {
  strict: true,
  noUncheckedIndexedAccess: true,
  noImplicitReturns: true,
  noFallthroughCasesInSwitch: true,
  module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Node10,
  target: ts.ScriptTarget.ES2020,
  lib: ['lib.esnext.d.ts', 'lib.dom.d.ts', 'lib.dom.iterable.d.ts'],
  esModuleInterop: true,
  skipLibCheck: true,
  noEmit: true,
  types: ['node'],
  baseUrl: checkout,
  paths: {
    '@trpc/server': ['packages/server/src'],
    '@trpc/server/*': ['packages/server/src/*'],
  },
};
const program = ts.createProgram([probe], options);
const checker = program.getTypeChecker();
const source = program.getSourceFile(probe);

console.log(`typescript ${ts.version}`);
console.log(`checkout ${path.basename(checkout)}`);
console.log('');
console.log('== inferred types ==');
const flags =
  ts.TypeFormatFlags.NoTruncation |
  ts.TypeFormatFlags.InTypeAlias |
  ts.TypeFormatFlags.UseFullyQualifiedType;
ts.forEachChild(source, function visit(node) {
  if (ts.isTypeAliasDeclaration(node) && node.name.text.startsWith('Show_')) {
    const type = checker.getTypeFromTypeNode(node.type);
    let text = checker.typeToString(type, undefined, flags);
    if (text.length > 700) {
      text = `${text.slice(0, 700)} ... [${text.length} characters in total]`;
    }
    console.log(`${node.name.text.slice(5)} = ${text}`);
  }
  ts.forEachChild(node, visit);
});

const all = ts.getPreEmitDiagnostics(program);
const inProbe = all.filter((d) => d.file && d.file.fileName === probe);
console.log('');
console.log(`== compiler errors in the probe: ${inProbe.length} ==`);
for (const d of inProbe) {
  const { line } = source.getLineAndCharacterOfPosition(d.start);
  const lineText = source.text.split('\n')[line].trim();
  console.log(
    `line ${line + 1} TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, ' | ').slice(0, 900)}`,
  );
  console.log(`    > ${lineText}`);
}
console.log('');
console.log(`compiler errors outside the probe: ${all.length - inProbe.length}`);

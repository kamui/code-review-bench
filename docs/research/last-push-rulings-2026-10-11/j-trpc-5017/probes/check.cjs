// Usage: node check.cjs <tree-dir> <probe.ts>...
// Type-checks each probe against <tree-dir>/packages/server/src with the
// TypeScript found from the probe's directory. One program per probe file.
// Prints every diagnostic and the expanded type of every `Show_*` alias.
const path = require('path');
// PROBE_LIB=esnext: use lib.esnext.d.ts and target esnext (needed by the 2026 default branch).
// PROBE_TS: path of another TypeScript module to use. PROBE_NO_STRICT_NULL=1: strictNullChecks off.
const ts = require(process.env.PROBE_TS || 'typescript');

const [tree, ...probes] = process.argv.slice(2).map((p) => path.resolve(p));
const options = {
  strict: true,
  noUncheckedIndexedAccess: true,
  noImplicitReturns: true,
  noFallthroughCasesInSwitch: true,
  module: ts.ModuleKind.ESNext,
  moduleResolution: ts.ModuleResolutionKind.Node10,
  target: ts.ScriptTarget.ES2020,
  lib: ['lib.es2020.d.ts', 'lib.dom.d.ts', 'lib.dom.iterable.d.ts'],
  esModuleInterop: true,
  skipLibCheck: true,
  noEmit: true,
  types: ['node'],
  baseUrl: tree,
  paths: {
    '@trpc/server': ['packages/server/src'],
    '@trpc/server/*': ['packages/server/src/*'],
  },
};
if (process.env.PROBE_NO_STRICT_NULL === '1') options.strictNullChecks = false;
if (process.env.PROBE_LIB === 'esnext') {
  options.lib = ['lib.esnext.d.ts', 'lib.dom.d.ts', 'lib.dom.iterable.d.ts'];
  options.target = ts.ScriptTarget.ESNext;
}
const flags =
  ts.TypeFormatFlags.NoTruncation |
  ts.TypeFormatFlags.InTypeAlias |
  ts.TypeFormatFlags.UseFullyQualifiedType;
const clip = (text, n) =>
  text.length > n ? `${text.slice(0, n)} ... [${text.length} characters in total]` : text;

console.log(`typescript ${ts.version}${options.strictNullChecks === false ? ' (strictNullChecks off)' : ''}`);
console.log(`tree ${path.basename(tree)}`);
let failed = 0;
for (const probe of probes) {
  const program = ts.createProgram([probe], options);
  const checker = program.getTypeChecker();
  const source = program.getSourceFile(probe);
  const all = ts.getPreEmitDiagnostics(program);
  const inProbe = all.filter((d) => d.file && d.file.fileName === probe);
  console.log('');
  console.log(`## ${path.basename(probe)}: ${all.length === 0 ? 'compiles' : `${inProbe.length} error(s)`}`);
  for (const d of inProbe) {
    const { line } = source.getLineAndCharacterOfPosition(d.start);
    console.log(
      `line ${line + 1} TS${d.code}: ${clip(ts.flattenDiagnosticMessageText(d.messageText, ' | '), 6000)}`,
    );
    console.log(`    > ${source.text.split('\n')[line].trim()}`);
  }
  if (all.length !== inProbe.length) {
    console.log(`errors outside the probe: ${all.length - inProbe.length}`);
    for (const d of all.filter((x) => !inProbe.includes(x)).slice(0, 5)) {
      console.log(`  ${d.file ? path.relative(tree, d.file.fileName) : ''} TS${d.code}: ${clip(ts.flattenDiagnosticMessageText(d.messageText, ' | '), 300)}`);
    }
  }
  ts.forEachChild(source, function visit(node) {
    if (ts.isTypeAliasDeclaration(node) && node.name.text.startsWith('Show_')) {
      const type = checker.getTypeFromTypeNode(node.type);
      console.log(`type ${node.name.text.slice(5)} = ${clip(checker.typeToString(type, undefined, flags), 700)}`);
    }
    ts.forEachChild(node, visit);
  });
  if (all.length) failed += 1;
}
console.log('');
console.log(`probes with errors: ${failed} of ${probes.length}`);

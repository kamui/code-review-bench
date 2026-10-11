// Usage: node check.cjs <tree-dir> <probe.ts>...
// Type-checks each probe against <tree-dir>/packages/server/src and packages/client/src with the
// TypeScript found from the probe's directory (or PROBE_TS). One program per probe file.
// Prints every diagnostic as the command-line compiler words it (messages cut at 1500 characters here),
// the expanded type of every `Show_*` alias, and, for every comment line holding `^?`, what the
// language service returns as the hover text for the token on the line above (`^? label @token`).
const path = require('path');
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
    '@trpc/client': ['packages/client/src'],
    '@trpc/client/*': ['packages/client/src/*'],
  },
};
const full =
  ts.TypeFormatFlags.NoTruncation |
  ts.TypeFormatFlags.InTypeAlias |
  ts.TypeFormatFlags.UseFullyQualifiedType;
const clip = (text, n) =>
  text.length > n ? `${text.slice(0, n)} ... [${text.length} characters in total]` : text;

console.log(`typescript ${ts.version}`);
console.log(`tree ${path.basename(tree)}`);
let failed = 0;
for (const probe of probes) {
  const host = {
    getScriptFileNames: () => [probe],
    getScriptVersion: () => '1',
    getScriptSnapshot: (f) =>
      ts.sys.fileExists(f) ? ts.ScriptSnapshot.fromString(ts.sys.readFile(f)) : undefined,
    getCurrentDirectory: () => path.dirname(probe),
    getCompilationSettings: () => options,
    getDefaultLibFileName: (o) => ts.getDefaultLibFilePath(o),
    fileExists: ts.sys.fileExists,
    readFile: ts.sys.readFile,
    readDirectory: ts.sys.readDirectory,
    directoryExists: ts.sys.directoryExists,
    getDirectories: ts.sys.getDirectories,
  };
  const service = ts.createLanguageService(host, ts.createDocumentRegistry());
  const program = service.getProgram();
  const checker = program.getTypeChecker();
  const source = program.getSourceFile(probe);
  const all = ts.getPreEmitDiagnostics(program);
  const inProbe = all.filter((d) => d.file && d.file.fileName === probe);
  console.log('');
  console.log(`## ${path.basename(probe)}: ${all.length === 0 ? 'compiles' : `${inProbe.length} error(s)`}`);
  const lines = source.text.split('\n');
  for (const d of inProbe) {
    const { line } = source.getLineAndCharacterOfPosition(d.start);
    console.log(`line ${line + 1} TS${d.code}: ${clip(ts.flattenDiagnosticMessageText(d.messageText, ' | '), 1500)}`);
    console.log(`    > ${lines[line].trim()}`);
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
      const text = checker.typeToString(type, undefined, full);
      console.log(`type ${node.name.text.slice(5)} = ${clip(text, 400)}`);
    }
    ts.forEachChild(node, visit);
  });
  lines.forEach((text, i) => {
    const m = /^(\s*\/\/\s*)\^\?\s*(.*)$/.exec(text);
    if (!m || i === 0) return;
    // `^? label @token` hovers the first `token` on the line above; without `@token`, the column of the caret.
    const at = /^(.*?)\s*@(.+)$/.exec(m[2]);
    const column = at ? lines[i - 1].indexOf(at[2]) : m[1].length;
    const pos = source.getPositionOfLineAndCharacter(i - 1, column);
    const info = service.getQuickInfoAtPosition(probe, pos);
    const shown = info ? ts.displayPartsToString(info.displayParts) : '(no hover text)';
    console.log(`hover, ${at ? at[1] : m[2] || `line ${i}`} [${shown.length} characters]: ${shown}`);
  });
  if (all.length) failed += 1;
}
console.log('');
console.log(`probes with errors: ${failed} of ${probes.length}`);

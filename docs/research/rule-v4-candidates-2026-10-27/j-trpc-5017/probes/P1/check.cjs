const path = require('node:path');
const [clone, deps] = process.argv.slice(2);
const ts = require(path.join(deps, 'node_modules/typescript'));
const files = ['literal.ts', 'pe-j1.ts', 'neighbours.ts', 'growth.ts'].map(n => path.join(clone, 'ruling-probes', n));
const program = ts.createProgram(files, {
  strict: true, noEmit: true, skipLibCheck: true,
  target: ts.ScriptTarget.ES2020, module: ts.ModuleKind.CommonJS,
  moduleResolution: ts.ModuleResolutionKind.NodeJs,
  esModuleInterop: true, types: ['node'],
  typeRoots: [path.join(deps, 'node_modules/@types')],
});
const diagnostics = ts.getPreEmitDiagnostics(program);
console.log('TypeScript', ts.version);
for (const d of diagnostics) {
  const at = d.file && d.start !== undefined ? d.file.getLineAndCharacterOfPosition(d.start) : null;
  console.log(`${d.file ? path.relative(clone, d.file.fileName) : '<config>'}:${at ? at.line + 1 : 0}:${at ? at.character + 1 : 0} TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, ' | ')}`);
}
const checker = program.getTypeChecker();
for (const f of files) {
  const source = program.getSourceFile(f);
  function walk(n) {
    if ((ts.isVariableDeclaration(n) || ts.isTypeAliasDeclaration(n)) && n.name.getText().startsWith('inspect') || ts.isTypeAliasDeclaration(n) && n.name.getText().startsWith('Inspect')) {
      console.log('TYPE', n.name.getText(), checker.typeToString(checker.getTypeAtLocation(n.name), undefined, ts.TypeFormatFlags.NoTruncation));
    }
    ts.forEachChild(n, walk);
  }
  walk(source);
}
console.log('DIAGNOSTIC_COUNT', diagnostics.length);
console.log('PER_FILE', JSON.stringify(Object.fromEntries(files.map(f => [path.basename(f), diagnostics.filter(d => d.file?.fileName === f).map(d => d.code)]))));
process.exitCode = diagnostics.some(d => !d.file || !d.file.fileName.includes('/ruling-probes/')) ? 2 : 0;

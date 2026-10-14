// Usage: node count.cjs <tree-dir> <probe.ts>...
// Type-checks each probe like check.cjs and prints, for every `Show_*` alias, how many
// members its union has, with the time the check took and the diagnostics.
const path = require('path');
const ts = require(process.env.PROBE_TS || 'typescript');
const [tree, ...probes] = process.argv.slice(2).map((p) => path.resolve(p));
const options = {
  strict: true, noUncheckedIndexedAccess: true, noImplicitReturns: true, noFallthroughCasesInSwitch: true,
  module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Node10, target: ts.ScriptTarget.ES2020,
  lib: ['lib.es2020.d.ts', 'lib.dom.d.ts', 'lib.dom.iterable.d.ts'], esModuleInterop: true, skipLibCheck: true,
  noEmit: true, types: ['node'], baseUrl: tree,
  paths: { '@trpc/server': ['packages/server/src'], '@trpc/server/*': ['packages/server/src/*'] },
};
console.log(`typescript ${ts.version}`);
console.log(`tree ${path.basename(tree)}`);
for (const probe of probes) {
  const started = Date.now();
  const program = ts.createProgram([probe], options);
  const checker = program.getTypeChecker();
  const source = program.getSourceFile(probe);
  const all = ts.getPreEmitDiagnostics(program);
  const seconds = ((Date.now() - started) / 1000).toFixed(1);
  const lines = [];
  ts.forEachChild(source, function visit(node) {
    if (ts.isTypeAliasDeclaration(node) && node.name.text.startsWith('Show_')) {
      const type = checker.getTypeFromTypeNode(node.type);
      lines.push(`${node.name.text.slice(5)}: ${type.isUnion() ? type.types.length : 1} member(s)`);
    }
    ts.forEachChild(node, visit);
  });
  const errors = all.map((d) => `TS${d.code}: ${ts.flattenDiagnosticMessageText(d.messageText, ' | ').slice(0, 200)}`);
  console.log(`${path.basename(probe)}: ${all.length === 0 ? 'compiles' : `${all.length} error(s)`}; ${lines.join('; ')}; checked in ${seconds}s`);
  for (const e of errors) console.log(`    ${e}`);
}

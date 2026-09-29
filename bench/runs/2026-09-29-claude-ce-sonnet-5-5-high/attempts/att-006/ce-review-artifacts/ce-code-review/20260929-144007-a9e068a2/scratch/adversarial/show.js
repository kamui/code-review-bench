const ts = require('/home/jack/.t3/bench-runs/2026-09-29-claude-ce-sonnet-5-5-high/att-006/clone/packages/tests/node_modules/typescript');
const file = process.argv[2], aliasName = process.argv[3];
const cfg = process.argv[4];
let opts = {strict:true, skipLibCheck:true, target: ts.ScriptTarget.ES2020, noEmit:true, lib:['lib.es2020.d.ts']};
if (cfg) { const c = ts.readConfigFile(cfg, ts.sys.readFile); opts = ts.parseJsonConfigFileContent(c.config, ts.sys, require('path').dirname(cfg)).options; }
const p = ts.createProgram([file], opts);
const ch = p.getTypeChecker();
const sf = p.getSourceFile(file);
ts.forEachChild(sf, n => {
  if (ts.isTypeAliasDeclaration(n) && n.name.text === aliasName) {
    const t = ch.getTypeAtLocation(n.name);
    for (const prop of ch.getPropertiesOfType(t)) {
      const pt = ch.getTypeOfSymbolAtLocation(prop, n);
      const inner = ch.getPropertiesOfType(pt);
      if (inner.length && inner.some(i=>i.name==='old')) {
        console.log(prop.name);
        for (const i of inner) console.log('   ', i.name, '=', ch.typeToString(ch.getTypeOfSymbolAtLocation(i, n), undefined, ts.TypeFormatFlags.NoTruncation|ts.TypeFormatFlags.InTypeAlias).slice(0,200));
      } else console.log(prop.name, '=', ch.typeToString(pt, undefined, ts.TypeFormatFlags.NoTruncation|ts.TypeFormatFlags.InTypeAlias).slice(0,300));
    }
  }
});

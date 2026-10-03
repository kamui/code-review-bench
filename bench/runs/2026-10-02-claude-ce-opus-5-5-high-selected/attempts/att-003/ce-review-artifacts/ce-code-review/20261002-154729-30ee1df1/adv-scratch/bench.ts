import { parse } from '/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone/src/language/parser';
import { buildSchema } from '/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone/src/utilities/buildASTSchema';
import { validate } from '/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone/src/validation/validate';
import { OverlappingFieldsCanBeMergedRule as NewRule } from '/home/jack/.t3/bench-runs/2026-10-02-claude-ce-opus-5-5-high-selected/att-003/clone/src/validation/rules/OverlappingFieldsCanBeMergedRule';
import { OverlappingFieldsCanBeMergedRule as OldRule } from './OldRule';

const schema = buildSchema(`
  type Query { f(a: Int, b: Int, a2: Int): String, g: String, o(i: In): String }
  input In { x: Int, y: Int }
`);

function time(label: string, rule: any, src: string, opts?: any) {
  const doc = parse(src, opts);
  // warmup
  validate(schema, doc, [rule]);
  const t = process.hrtime.bigint();
  const errs = validate(schema, doc, [rule]);
  const ms = Number(process.hrtime.bigint() - t) / 1e6;
  console.log(label, ms.toFixed(1) + 'ms', 'errors=' + errs.length);
}

function verdict(src: string) {
  const doc = parse(src);
  const o = validate(schema, doc, [OldRule]).map((e) => e.message);
  const n = validate(schema, doc, [NewRule]).map((e) => e.message);
  console.log(JSON.stringify(src), '\n  old:', JSON.stringify(o), '\n  new:', JSON.stringify(n));
}

for (const n of [100, 300]) {
  const noArgs = '{ ' + 'g '.repeat(n) + '}';
  const withArgs = '{ ' + 'f(a: 1, b: 2) '.repeat(n) + '}';
  time('old noargs n=' + n, OldRule, noArgs);
  time('new noargs n=' + n, NewRule, noArgs);
  time('old args   n=' + n, OldRule, withArgs);
  time('new args   n=' + n, NewRule, withArgs);
}

verdict('{ f(a: 1, a: 2) f(a: 1, a: 2) }');
verdict('{ f(a: 1, a: 1) f(a: 1, b: 2) }');
verdict('{ f(a: 1, a: 2) f(a: 2, a: 1) }');
verdict('{ f(a: 1, b: 2) f(b: 2, a: 1) }');
verdict('{ f(a: 1, a2: 2) f(a2: 2, a: 1) }');
verdict('{ f(a: 1) f(a: 1, b: 2) }');
verdict('{ f f(a: 1) }');
verdict('{ f f() }'.replace('()', ''));
verdict('{ o(i: {x: 1, y: 2}) o(i: {y: 2, x: 1}) }');
verdict('{ o(i: {x: 1, x: 2}) o(i: {x: 2, x: 1}) }');
verdict('{ f(a: 01) f(a: 1) }'.replace('01', '1'));

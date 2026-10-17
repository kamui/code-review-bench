// Usage: bun m1-runtime.ts <tree-dir>
// Calls procedures with a Date, a Map and a Set input followed by a pass-through middleware, and reports what the
// resolver receives at run time; then tries the write to `size` that the head's type for a Map/Set input accepts.
// Bun does not type-check. ES modules run in strict mode.
import path from 'node:path';
import { z } from 'zod';

const tree = path.resolve(process.argv[2]!);
const trpc: any = await import(`${tree}/packages/server/src/index.ts`);
const t = trpc.initTRPC.create();
const describe = (v: any) => `${Object.prototype.toString.call(v)} instanceof Date=${v instanceof Date} instanceof Map=${v instanceof Map} instanceof Set=${v instanceof Set}`;
const router = t.router({
  date: t.procedure.input(z.date()).use((o: any) => o.next()).query(({ input }: any) => describe(input)),
  map: t.procedure.input(z.map(z.string(), z.number())).use((o: any) => o.next()).query(({ input }: any) => {
    let write = 'no error';
    try {
      input.size = 0;
    } catch (e: any) {
      write = `${e.constructor.name}: ${e.message}`;
    }
    return `${describe(input)}; size after the attempted write ${input.size}; write: ${write}`;
  }),
  set: t.procedure.input(z.set(z.string())).use((o: any) => o.next()).query(({ input }: any) => describe(input)),
});
const caller = router.createCaller({});
console.log(`tree ${path.basename(tree)}`);
console.log(`date: ${await caller.date(new Date(0))}`);
console.log(`map: ${await caller.map(new Map([['a', 1]]))}`);
console.log(`set: ${await caller.set(new Set(['a']))}`);

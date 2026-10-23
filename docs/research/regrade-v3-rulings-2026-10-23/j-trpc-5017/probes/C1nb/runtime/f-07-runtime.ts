// Usage: bun f-07-runtime.ts <tree-dir>
// Calls procedures whose input is a function, with and without a pass-through middleware,
// through a server-side caller, and through the HTTP JSON form, and reports what happens.
// Bun does not type-check.
import path from 'node:path';
import { z } from 'zod';

const tree = path.resolve(process.argv[2]!);
const trpc: any = await import(`${tree}/packages/server/src/index.ts`);
const t = trpc.initTRPC.create();
const fn = z.function().args(z.number()).returns(z.number());
const router = t.router({
  withMiddleware: t.procedure.input(fn).use((o: any) => o.next()).query(({ input }: any) => `typeof input=${typeof input}; input(2)=${input(2)}`),
  noMiddleware: t.procedure.input(fn).query(({ input }: any) => `typeof input=${typeof input}; input(2)=${input(2)}`),
});
const caller = router.createCaller({});
console.log(`tree ${path.basename(tree)}`);
for (const name of ['withMiddleware', 'noMiddleware']) {
  console.log(`${name}, caller passes (x) => x * 3: ${await caller[name]((x: number) => x * 3)}`);
  try {
    console.log(`${name}, caller passes 42: ${await caller[name](42)}`);
  } catch (e: any) {
    console.log(`${name}, caller passes 42: ${e.constructor.name} ${e.code ?? ''} ${String(e.message).split('\n').slice(0, 3).join(' ')}`);
  }
}
console.log(`JSON.stringify({ input: (x) => x * 3 }) = ${JSON.stringify({ input: (x: number) => x * 3 })}`);

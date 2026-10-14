// Usage: bun runtime.ts <tree-dir>
// Runs the joined procedure and the reusable-middleware procedure of the compile probes,
// to show what the server does with each call. Bun does not type-check.
import path from 'node:path';
import { z } from 'zod';

const tree = path.resolve(process.argv[2]!);
const trpc: any = await import(`${tree}/packages/server/src/index.ts`);
const t = trpc.initTRPC.create();

const first = t.procedure.input(z.object({ a: z.string() }).optional());
const second = t.procedure.input(z.object({ b: z.number() }).optional());
const needsB = trpc.experimental_standaloneMiddleware().create((opts: any) => opts.next());

const router = t.router({
  joined: first.unstable_concat(second).query(({ input }: any) => input ?? 'no input'),
  standalone: t.procedure
    .input(z.object({ a: z.string(), b: z.number() }).optional())
    .use(needsB)
    .query(({ input }: any) => (input ? input.a.toUpperCase() : 'no input')),
});
const caller = router.createCaller({});

async function show(label: string, call: () => Promise<unknown>) {
  try {
    console.log(`${label}: ${JSON.stringify(await call())}`);
  } catch (error: any) {
    const first = (() => {
      try {
        return JSON.parse(error.message)[0];
      } catch {
        return error.message;
      }
    })();
    console.log(`${label}: rejected, code ${error.code}, ${JSON.stringify(first)}`);
  }
}

console.log(`tree ${path.basename(tree)}`);
await show('joined, no input', () => caller.joined());
await show('joined, { a, b }', () => caller.joined({ a: 'x', b: 1 }));
await show('joined, { b } alone', () => caller.joined({ b: 1 }));
await show('joined, { a } alone', () => caller.joined({ a: 'x' }));
await show('joined, {}', () => caller.joined({}));
await show('reusable middleware, no input', () => caller.standalone());
await show('reusable middleware, { a, b }', () => caller.standalone({ a: 'x', b: 1 }));
await show('reusable middleware, { b } alone', () => caller.standalone({ b: 1 }));

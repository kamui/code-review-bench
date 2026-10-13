// Usage: bun runtime.ts <tree-dir>
// Sends the hybrid payload of the compile probes to the server-side caller, to show what
// the server does with it. Bun does not type-check.
import path from 'node:path';
import { z } from 'zod';

const tree = path.resolve(process.argv[2]!);
const trpc: any = await import(`${tree}/packages/server/src/index.ts`);
const t = trpc.initTRPC.create();

const schema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('a'), x: z.string() }),
  z.object({ type: z.literal('b'), y: z.number() }),
]);
const strictSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('a'), x: z.string() }).strict(),
  z.object({ type: z.literal('b'), y: z.number() }).strict(),
]);

const router = t.router({
  plain: t.procedure.input(schema).query(({ input }: any) => input),
  withMiddleware: t.procedure
    .input(schema)
    .use((opts: any) => opts.next())
    .query(({ input }: any) => input),
  strictWithMiddleware: t.procedure
    .input(strictSchema)
    .use((opts: any) => opts.next())
    .query(({ input }: any) => input),
  threeMiddlewares: t.procedure
    .input(schema)
    .use((opts: any) => opts.next())
    .use((opts: any) => opts.next())
    .use((opts: any) => opts.next())
    .query(({ input }: any) => input),
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
const hybrid = { type: 'b', x: 'foo', y: 1 };
await show('variant a, no middleware', () => caller.plain({ type: 'a', x: 'hello' }));
await show('variant a, middleware', () => caller.withMiddleware({ type: 'a', x: 'hello' }));
await show('variant b, three middlewares', () => caller.threeMiddlewares({ type: 'b', y: 2 }));
await show('hybrid { type: b, x, y }, no middleware', () => caller.plain(hybrid));
await show('hybrid { type: b, x, y }, middleware', () => caller.withMiddleware(hybrid));
await show('hybrid, middleware, strict variants', () => caller.strictWithMiddleware(hybrid));

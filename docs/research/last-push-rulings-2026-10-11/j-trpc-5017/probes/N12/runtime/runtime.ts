// Usage: bun runtime.ts <tree-dir>
// Runs the same procedures as the compile probes, to show what happens at run
// time. Bun does not type-check, so the compile errors do not stop it.
import path from 'node:path';
import { z } from 'zod';

const tree = path.resolve(process.argv[2]!);
const trpc: any = await import(`${tree}/packages/server/src/index.ts`);
const t = trpc.initTRPC.create();

const router = t.router({
  brand: t.procedure
    .input(z.string().brand<'id'>())
    .use((opts: any) => opts.next())
    .query(({ input }: any) => `${typeof input} ${input.toUpperCase()}`),
  brandNoMiddleware: t.procedure
    .input(z.string().brand<'id'>())
    .query(({ input }: any) => `${typeof input} ${input.toUpperCase()}`),
  optional: t.procedure
    .input(z.object({ a: z.string().optional() }))
    .use((opts: any) => opts.next())
    .query(({ input }: any) => input.a ?? 'absent'),
  optionalNoMiddleware: t.procedure
    .input(z.object({ a: z.string().optional() }))
    .query(({ input }: any) => input.a ?? 'absent'),
});

const api = router.createCaller({});
console.log(`tree ${path.basename(tree)}`);
console.log('branded string with middleware, input "hello":', await api.brand('hello'));
console.log('branded string without middleware, input "hello":', await api.brandNoMiddleware('hello'));
console.log('optional field with middleware, input {}:', await api.optional({}));
console.log('optional field with middleware, input { a: undefined }:', await api.optional({ a: undefined }));
console.log('optional field without middleware, input {}:', await api.optionalNoMiddleware({}));

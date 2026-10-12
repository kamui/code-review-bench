// Usage: bun runtime.ts <tree-dir>
// Runs the same procedures as the compile probes, to show what happens at run
// time. Bun does not type-check, so the compile errors do not stop it.
import path from 'node:path';
import { z } from 'zod';

const tree = path.resolve(process.argv[2]!);
const trpc: any = await import(`${tree}/packages/server/src/index.ts`);

function sentryTrpcMiddleware(_options: unknown) {
  return function <T>({ next }: { next: () => T }): T {
    return next();
  };
}

const t = trpc.initTRPC.context().create();
const acceptsObjectOrNothing = trpc
  .experimental_standaloneMiddleware()
  .create((opts: any) => opts.next());

const router = t.router({
  genericRead: t.procedure
    .use(t.middleware(sentryTrpcMiddleware({ foo: 'bar' })))
    .query(({ ctx }: any) => ctx),
  genericCompose: t.procedure
    .use(t.middleware(sentryTrpcMiddleware({ foo: 'bar' })))
    .use(t.middleware((opts: any) => opts.next({ ctx: { object: { a: 'b' } } })))
    .query(({ ctx }: any) => ctx),
  ctxUndefined: t.procedure
    .use((opts: any) => opts.next({ ctx: undefined }))
    .query(({ ctx }: any) => ctx),
  ctxNull: t.procedure
    .use((opts: any) => opts.next({ ctx: null }))
    .query(({ ctx }: any) => ctx),
  inputAfterMiddleware: t.procedure
    .input(z.object({ value: z.string() }))
    .use(acceptsObjectOrNothing)
    .query(({ input }: any) => input.value.toUpperCase()),
});

const api = router.createCaller({ some: 'prop', userId: 'u1' });
console.log(`tree ${path.basename(tree)}`);
console.log('generic middleware, ctx in the resolver:', JSON.stringify(await api.genericRead()));
console.log('generic middleware then a second one, ctx:', JSON.stringify(await api.genericCompose()));
console.log('next({ ctx: undefined }), ctx in the resolver:', JSON.stringify(await api.ctxUndefined()));
console.log('next({ ctx: null }), ctx in the resolver:', JSON.stringify(await api.ctxNull()));
console.log('input { value: "a" }:', JSON.stringify(await api.inputAfterMiddleware({ value: 'a' })));
try {
  console.log('no input:', JSON.stringify(await api.inputAfterMiddleware()));
} catch (error: any) {
  console.log(`no input: rejected, code ${error.code}, ${String(error.message).replace(/\s+/g, ' ')}`);
}

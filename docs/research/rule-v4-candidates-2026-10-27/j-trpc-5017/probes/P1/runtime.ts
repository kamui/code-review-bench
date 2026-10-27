import { experimental_standaloneMiddleware, initTRPC, TRPCError } from '../packages/server/src';
import { z } from 'zod';
const t = initTRPC.create();
const middleware = experimental_standaloneMiddleware<{ input: { a: string } }>()
  .create((opts) => {
    console.log('middleware input', JSON.stringify(opts.input));
    return opts.next();
  });
const schema = z.object({ a: z.literal('x') });
const router = t.router({
  literal: t.procedure.input(schema).use(middleware).query(({ input }) => input),
  plain: t.procedure.input(schema).query(({ input }) => input),
  repeatedParser: t.procedure.input(schema).use(middleware).input(schema).query(({ input }) => input),
  guarded: t.procedure.input(schema).use(middleware).query(({ input }) => {
    if (input.a !== 'x') throw new Error('unexpected input');
    return input.a;
  }),
  documented: t.procedure.input(z.object({ projectId: z.string() }))
    .use(experimental_standaloneMiddleware<{ input: { projectId: string } }>()
      .create((opts) => opts.next())).query(({ input }) => input.projectId.toUpperCase()),
  optional: t.procedure.input(z.object({ a: z.string().optional() }))
    .use((opts) => opts.next()).query(({ input }) => input),
});
const caller = router.createCaller({});
console.log('plain x', JSON.stringify(await caller.plain({ a: 'x' })));
console.log('middleware x', JSON.stringify(await caller.literal({ a: 'x' })));
console.log('repeated parser x', JSON.stringify(await caller.repeatedParser({ a: 'x' })));
console.log('guarded x', JSON.stringify(await caller.guarded({ a: 'x' })));
console.log('documented projectId', JSON.stringify(await caller.documented({ projectId: 'demo' })));
try {
  await caller.literal({ a: 'y' });
} catch (error) {
  if (!(error instanceof TRPCError)) throw error;
  console.log('middleware y', error.code, error.message);
}
console.log('optional empty', JSON.stringify(await caller.optional({ a: undefined })));

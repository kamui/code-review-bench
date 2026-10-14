// A limit of the reordering: a middleware that reads the parsed input has to come after it, and the fault is back.
import { initTRPC, inferRouterInputs } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const appRouter = t.router({
  p: t.procedure
    .input(z.object({ a: z.string().optional() }))
    .use((opts) => opts.next({ ctx: { hadA: opts.input.a !== undefined } }))
    .query(({ input, ctx }) => (ctx.hadA ? input.a ?? 'absent' : 'absent')),
});
export type In = inferRouterInputs<typeof appRouter>['p'];
export async function use() {
  const caller = appRouter.createCaller({});
  return [await caller.p({}), await caller.p({ a: 'x' })];
}

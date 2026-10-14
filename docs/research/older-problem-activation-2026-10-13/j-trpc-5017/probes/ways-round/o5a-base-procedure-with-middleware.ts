// What an ordinary user writes anyway: a base procedure that carries the middleware, and the input added to it.
import { initTRPC, inferRouterInputs } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const withMiddleware = t.procedure.use((opts) => opts.next());
export const appRouter = t.router({
  p: withMiddleware
    .input(z.object({ a: z.string().optional() }))
    .query(({ input }) => input.a ?? 'absent'),
});
export type In = inferRouterInputs<typeof appRouter>['p'];
export async function use() {
  const caller = appRouter.createCaller({});
  return [await caller.p({}), await caller.p({ a: 'x' })];
}

// Way round on the server, tried: the input is a union of the object with the field and the object without it.
import { initTRPC, inferRouterInputs } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const appRouter = t.router({
  p: t.procedure
    .input(z.object({ a: z.string() }).or(z.object({}).strict()))
    .use((opts) => opts.next())
    .query(({ input }) => ('a' in input ? input.a : 'absent')),
});
export type In = inferRouterInputs<typeof appRouter>['p'];
export async function use() {
  const caller = appRouter.createCaller({});
  return [await caller.p({}), await caller.p({ a: 'x' })];
}

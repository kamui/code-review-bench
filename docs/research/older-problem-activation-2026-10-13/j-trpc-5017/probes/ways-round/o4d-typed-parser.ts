// Way round on the server, tried: a hand-written parser with an explicit return type in place of the Zod validator.
import { initTRPC, inferRouterInputs } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
function parser(value: unknown): { a?: string } {
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    if (!('a' in value) || value.a === undefined) return {};
    if (typeof value.a === 'string') return { a: value.a };
  }
  throw new Error('Expected an object with an optional string a');
}
export const appRouter = t.router({
  p: t.procedure
    .input(parser)
    .use((opts) => opts.next())
    .query(({ input }) => input.a ?? 'absent'),
});
export type In = inferRouterInputs<typeof appRouter>['p'];
export async function use() {
  const caller = appRouter.createCaller({});
  return [await caller.p({}), await caller.p({ a: 'x' })];
}

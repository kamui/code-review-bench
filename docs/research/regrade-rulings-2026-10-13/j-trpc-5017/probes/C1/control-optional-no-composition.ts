// Control: one object input with an optional key, nothing after it.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const router = t.router({
  plain: t.procedure
    .input(z.object({ a: z.string().optional() }))
    .query(({ input }) => input),
});
export type Show_caller_plain = inferRouterInputs<typeof router>['plain'];
const caller = router.createCaller({});
export const omit = caller.plain({});

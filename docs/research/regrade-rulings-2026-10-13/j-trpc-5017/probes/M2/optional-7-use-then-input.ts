// An optional object input, a middleware that changes nothing, then a second optional object input.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const router = t.router({
  proc: t.procedure
    .input(z.object({ a: z.string() }).optional())
    .use((opts) => opts.next())
    .input(z.object({ b: z.number() }).optional())
    .query(({ input }) => input),
});
export type Show_caller = inferRouterInputs<typeof router>['proc'];
const caller = router.createCaller({});
export const nothing = caller.proc();
export const both = caller.proc({ a: 'x', b: 1 });

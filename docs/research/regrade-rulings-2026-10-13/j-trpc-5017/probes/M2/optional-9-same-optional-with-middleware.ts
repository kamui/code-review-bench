// One optional object input and a middleware that changes nothing.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const router = t.router({
  proc: t.procedure
    .input(z.object({ a: z.string() }).optional())
    .use((opts) => opts.next())
    .query(({ input }) => (input ? input.a.toUpperCase() : 'none')),
});
export type Show_caller = inferRouterInputs<typeof router>['proc'];
const caller = router.createCaller({});
export const nothing = caller.proc();
export const withField = caller.proc({ a: 'x' });

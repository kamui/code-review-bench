// The comment's first route: two .input() calls in a row, each with an optional key.
// No middleware and no concat.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const router = t.router({
  twoInputs: t.procedure
    .input(z.object({ a: z.string().optional() }))
    .input(z.object({ b: z.string().optional() }))
    .query(({ input }) => input),
  sameInputTwice: t.procedure
    .input(z.object({ a: z.string().optional() }))
    .input(z.object({ a: z.string().optional() }))
    .query(({ input }) => input),
});
type Inputs = inferRouterInputs<typeof router>;
export type Show_caller_two_inputs = Inputs['twoInputs'];
export type Show_caller_same_input_twice = Inputs['sameInputTwice'];
const caller = router.createCaller({});
export const omitBoth = caller.twoInputs({});
export const omitSame = caller.sameInputTwice({});
export const assigned: Inputs['twoInputs'] = {};

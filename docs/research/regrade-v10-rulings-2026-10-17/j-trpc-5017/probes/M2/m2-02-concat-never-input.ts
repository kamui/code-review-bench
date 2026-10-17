// A procedure with an object input joined with unstable_concat to a procedure whose input is z.never().
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const never = t.procedure.input(z.never());
export const appRouter = t.router({
  joined: t.procedure
    .input(z.object({ a: z.string() }))
    .unstable_concat(never)
    .query(({ input }) => {
      type Show_resolver_input = typeof input;
      return input.a;
    }),
});
type Inputs = inferRouterInputs<typeof appRouter>;
type Show_caller_input = Inputs['joined'];

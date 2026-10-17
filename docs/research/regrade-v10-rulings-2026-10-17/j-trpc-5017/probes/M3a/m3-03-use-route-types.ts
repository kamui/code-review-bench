// The route that does reach the helper: an array input followed by a middleware. Prints the resolver's and
// the caller's input types; each `true` line compiles only if the type is identical to string[].
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

type Identical<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
const t = initTRPC.create();
export const appRouter = t.router({
  plain: t.procedure.input(z.array(z.string())).query(({ input }) => {
    const same: Identical<typeof input, string[]> = true;
    return same;
  }),
  withMiddleware: t.procedure
    .input(z.array(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      type Show_resolver_with_middleware = typeof input;
      const same: Identical<typeof input, string[]> = true;
      return same;
    }),
});
type Inputs = inferRouterInputs<typeof appRouter>;
export const callerPlain: Identical<Inputs['plain'], string[]> = true;
export const callerWithMiddleware: Identical<Inputs['withMiddleware'], string[]> = true;

// The comment's procedure route: `.input(z.array(...))` followed by a middleware.
// Prints the resolver's and the caller's input type, and checks them against string[].
import { initTRPC, inferRouterInputs } from '@trpc/server';
import { z } from 'zod';

type Identical<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
const t = initTRPC.create();
let seen: unknown;
export const router = t.router({
  arr: t.procedure
    .input(z.array(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      seen = input;
      const same: Identical<typeof input, string[]> = true;
      return same;
    }),
});
type Show_caller_input = inferRouterInputs<typeof router>['arr'];
export const callerIdentical: Identical<inferRouterInputs<typeof router>['arr'], string[]> = true;

// A procedure whose whole input is a function (Zod's z.function()), a middleware that
// changes nothing, and a resolver that calls the input.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const fn = z.function().args(z.number()).returns(z.number());
const t = initTRPC.create();
export const router = t.router({
  withMiddleware: t.procedure
    .input(fn)
    .use((opts) => opts.next())
    .query(({ input }) => input(2)),
});

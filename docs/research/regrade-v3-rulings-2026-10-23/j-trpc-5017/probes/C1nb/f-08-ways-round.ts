// Ways round f-01 in the resolver: a cast to the function type, and a typeof check.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

type Callback = (x: number) => number;
const fn = z.function().args(z.number()).returns(z.number());
const t = initTRPC.create();
export const router = t.router({
  cast: t.procedure
    .input(fn)
    .use((opts) => opts.next())
    .query(({ input }) => (input as Callback)(2)),
  typeofCheck: t.procedure
    .input(fn)
    .use((opts) => opts.next())
    .query(({ input }) => {
      if (typeof input !== 'function') throw new Error('unreachable');
      const result: number = input(2);
      return result;
    }),
});

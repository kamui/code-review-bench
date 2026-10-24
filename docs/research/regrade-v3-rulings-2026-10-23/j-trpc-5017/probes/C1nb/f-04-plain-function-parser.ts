// The same with tRPC's own parser form, a plain function that returns the input,
// in place of a Zod schema.
import { initTRPC } from '@trpc/server';

type Callback = (x: number) => number;
const t = initTRPC.create();
export const router = t.router({
  withMiddleware: t.procedure
    .input((value: unknown) => {
      if (typeof value !== 'function') throw new Error('expected a function');
      return value as Callback;
    })
    .use((opts) => opts.next())
    .query(({ input }) => input(2)),
  noMiddleware: t.procedure
    .input((value: unknown) => {
      if (typeof value !== 'function') throw new Error('expected a function');
      return value as Callback;
    })
    .query(({ input }) => input(2)),
});

// A reusable middleware declared for an input of type never, used after an object input.
import { experimental_standaloneMiddleware, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const mw = experimental_standaloneMiddleware<{ input: never }>().create((opts) => opts.next());
export const appRouter = t.router({
  p: t.procedure
    .input(z.object({ a: z.string() }))
    .use(mw)
    .query(({ input }) => {
      type Show_resolver_input = typeof input;
      return input.a;
    }),
});

// A procedure whose only input is z.never() (a form some OpenAPI plugins use for "no request body"),
// with and without a middleware after it. Prints the resolver's input type.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const appRouter = t.router({
  plain: t.procedure.input(z.never()).query(({ input }) => {
    type Show_resolver_no_middleware = typeof input;
    return 1;
  }),
  withMiddleware: t.procedure
    .input(z.never())
    .use((opts) => opts.next())
    .query(({ input }) => {
      type Show_resolver_with_middleware = typeof input;
      return 1;
    }),
});

// Date, Map and Set inputs, with and without a middleware that passes the input on unchanged.
// Prints the resolver's input type and the caller's input type for each.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const appRouter = t.router({
  date: t.procedure.input(z.date()).query(({ input }) => {
    type Show_resolver_date_no_middleware = typeof input;
    return 1;
  }),
  dateMw: t.procedure
    .input(z.date())
    .use((opts) => opts.next())
    .query(({ input }) => {
      type Show_resolver_date_middleware = typeof input;
      return 1;
    }),
  mapMw: t.procedure
    .input(z.map(z.string(), z.number()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      type Show_resolver_map_middleware = typeof input;
      return 1;
    }),
  setMw: t.procedure
    .input(z.set(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      type Show_resolver_set_middleware = typeof input;
      return 1;
    }),
});
type Inputs = inferRouterInputs<typeof appRouter>;
type Show_caller_date_no_middleware = Inputs['date'];
type Show_caller_date_middleware = Inputs['dateMw'];
type Show_caller_map_middleware = Inputs['mapMw'];
type Show_caller_set_middleware = Inputs['setMw'];

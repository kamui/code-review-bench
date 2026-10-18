// What a developer sees on a mistake: a Date input assigned to a number, without and with the middleware.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const appRouter = t.router({
  date: t.procedure.input(z.date()).query(({ input }) => {
    const wrong: number = input; // mistake, no middleware
    return wrong;
  }),
  dateMw: t.procedure
    .input(z.date())
    .use((opts) => opts.next())
    .query(({ input }) => {
      const wrong: number = input; // mistake, Date then middleware
      return wrong;
    }),
});

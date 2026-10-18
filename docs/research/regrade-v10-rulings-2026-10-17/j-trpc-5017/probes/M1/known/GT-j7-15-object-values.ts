// Object.values() and Object.entries() on the array input.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const appRouter = t.router({
  arrMw: t.procedure
    .input(z.array(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      const values: string[] = Object.values(input);
      const entries: [string, string][] = Object.entries(input);
      return { values, entries };
    }),
});

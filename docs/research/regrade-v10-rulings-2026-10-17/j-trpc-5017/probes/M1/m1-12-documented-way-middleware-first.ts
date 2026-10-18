// The documented order: a base procedure that carries the middleware, then .input() of a Date, Map or Set.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

type Identical<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
const t = initTRPC.create();
const base = t.procedure.use((opts) => opts.next());
export const appRouter = t.router({
  date: base.input(z.date()).query(({ input }) => {
    const same: Identical<typeof input, Date> = true;
    return same;
  }),
  map: base.input(z.map(z.string(), z.number())).query(({ input }) => {
    const same: Identical<typeof input, Map<string, number>> = true;
    return same;
  }),
  set: base.input(z.set(z.string())).query(({ input }) => {
    const same: Identical<typeof input, Set<string>> = true;
    return same;
  }),
});

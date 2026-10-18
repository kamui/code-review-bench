// The documented order: a base procedure that carries the middleware, then .input() of an array.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

type Identical<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B ? 1 : 2 ? true : false;
const t = initTRPC.create();
const base = t.procedure.use((opts) => opts.next());
export const appRouter = t.router({
  list: base.input(z.array(z.string())).query(async ({ input }) => {
    const same: Identical<typeof input, string[]> = true;
    const awaited: string[] = await Promise.all(input);
    return [same, awaited];
  }),
});

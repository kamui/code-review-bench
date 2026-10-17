// As m1-02, for a Map input and a Set input.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { expectTypeOf } from 'vitest';
import { z } from 'zod';

const t = initTRPC.create();
export const appRouter = t.router({
  mapMw: t.procedure
    .input(z.map(z.string(), z.number()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      expectTypeOf(input).toEqualTypeOf<Map<string, number>>();
      return 1;
    }),
  setMw: t.procedure
    .input(z.set(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      expectTypeOf(input).toEqualTypeOf<Set<string>>();
      return 1;
    }),
});
type Inputs = inferRouterInputs<typeof appRouter>;
expectTypeOf<Inputs['mapMw']>().toEqualTypeOf<Map<string, number>>();
expectTypeOf<Inputs['setMw']>().toEqualTypeOf<Set<string>>();

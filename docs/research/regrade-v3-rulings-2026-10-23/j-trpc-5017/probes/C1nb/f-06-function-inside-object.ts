// A function held inside an object input, with a middleware after the input.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const router = t.router({
  nested: t.procedure
    .input(z.object({ cb: z.function().args(z.number()).returns(z.number()) }))
    .use((opts) => opts.next())
    .query(({ input }) => input.cb(2)),
});

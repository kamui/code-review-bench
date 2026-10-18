// The documented ways to do the same things: a procedure with no input, and a middleware that adds nothing
// to the context (opts.next() with no argument), then a resolver that reads ctx and input.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.context<{ user: string }>().create();
export const appRouter = t.router({
  noInput: t.procedure.use((opts) => opts.next()).query(({ ctx }) => ctx.user),
  objectInput: t.procedure
    .input(z.object({ a: z.string() }))
    .use((opts) => opts.next())
    .query(({ ctx, input }) => ctx.user + input.a),
});

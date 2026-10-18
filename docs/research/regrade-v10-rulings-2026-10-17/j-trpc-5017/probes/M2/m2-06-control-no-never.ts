// Controls: the same shapes with an ordinary second type.
import { experimental_standaloneMiddleware, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.context<{ user: string }>().create();
const mw = experimental_standaloneMiddleware<{ input: { a: string } }>().create((opts) => opts.next());
export const appRouter = t.router({
  joined: t.procedure
    .input(z.object({ a: z.string() }))
    .unstable_concat(t.procedure.input(z.object({ b: z.number() })))
    .query(({ input }) => input.a + input.b),
  standalone: t.procedure
    .input(z.object({ a: z.string() }))
    .use(mw)
    .query(({ input }) => input.a),
  ctx: t.procedure.use((opts) => opts.next()).query(({ ctx }) => ctx.user),
});

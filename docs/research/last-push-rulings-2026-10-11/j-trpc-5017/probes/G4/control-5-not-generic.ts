// The way the documentation shows: one instance, created once, with a known
// context type. The same middleware and the same .use().
import { initTRPC } from '@trpc/server';

type Context = { req: { headers: Record<string, string> }; myContext: string };

const t = initTRPC.context<Context>().create();
const withAuth = t.middleware((opts) => opts.next({ ctx: { auth: 'user' } }));

export const router = t.router;
export const publicProcedure = t.procedure;
export const protectedProcedure = t.procedure.use(withAuth);

export const appRouter = router({
  hello: protectedProcedure.query(
    ({ ctx }) => `${ctx.myContext} ${ctx.req.headers['a']} ${ctx.auth}`,
  ),
});

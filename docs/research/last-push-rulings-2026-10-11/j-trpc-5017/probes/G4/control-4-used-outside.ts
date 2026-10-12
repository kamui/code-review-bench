// The generic function hands back the instance. The middleware is added
// outside it, where the context type is known.
import { initTRPC } from '@trpc/server';

type Base = { req: { headers: Record<string, string> } };

export function makeTRPC<T>() {
  const t = initTRPC.context<T & Base>().create();
  return { t };
}

const { t } = makeTRPC<{ myContext: string }>();
const withAuth = t.middleware((opts) => opts.next({ ctx: { auth: 'user' } }));

export const appRouter = t.router({
  hello: t.procedure
    .use(withAuth)
    .query(({ ctx }) => `${ctx.myContext} ${ctx.req.headers['a']} ${ctx.auth}`),
});

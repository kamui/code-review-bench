// The smallest shape found: a tRPC instance created inside a generic function,
// its context type an intersection with the function's unconstrained parameter,
// and a middleware made with t.middleware() that adds to the context.
import { initTRPC } from '@trpc/server';

type Base = { req: { headers: Record<string, string> } };

export function makeTRPC<T>() {
  const t = initTRPC.context<T & Base>().create();
  const withAuth = t.middleware((opts) => opts.next({ ctx: { auth: 'user' } }));
  return { router: t.router, protectedProcedure: t.procedure.use(withAuth) };
}

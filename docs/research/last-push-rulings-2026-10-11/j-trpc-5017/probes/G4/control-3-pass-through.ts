// factory-3 with a middleware that adds nothing to the context.
import { initTRPC } from '@trpc/server';

type Base = { req: { headers: Record<string, string> } };

export function makeTRPC<T>() {
  const t = initTRPC.context<T & Base>().create();
  const log = t.middleware((opts) => opts.next());
  return { router: t.router, loggedProcedure: t.procedure.use(log) };
}

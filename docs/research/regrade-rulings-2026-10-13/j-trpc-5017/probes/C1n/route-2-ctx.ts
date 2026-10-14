// The comment's second route: the context overwrite. A context with an optional key, a
// middleware that adds a key, and every place where a context value is handed over.
import { experimental_standaloneMiddleware, initTRPC } from '@trpc/server';
import { z } from 'zod';

type Context = { requestId: string; user?: { id: string } };
const t = initTRPC.context<Context>().create();

const reads = experimental_standaloneMiddleware<{
  ctx: { requestId: string; user?: { id: string } };
}>().create((opts) => opts.next());

declare function takesContext(ctx: Context): void;

const afterMiddleware = t.procedure.use((opts) => opts.next({ ctx: { locale: 'en' } }));
export type Show_resolver_ctx_after_middleware = Parameters<
  Parameters<(typeof afterMiddleware)['query']>[0]
>[0]['ctx'];

export const router = t.router({
  proc: afterMiddleware
    // a later middleware that asks for the context with its optional key
    .use(reads)
    .input(z.object({ a: z.string().optional() }))
    .query(({ ctx, input }) => {
      // the resolver passes the context on as the declared context type
      takesContext(ctx);
      const id: string | undefined = ctx.user?.id;
      return `${ctx.requestId} ${ctx.locale} ${id} ${input.a}`;
    }),
});

// a caller creates the context without the optional key, and omits the optional input key
const caller = router.createCaller({ requestId: 'r1' });
export const result = caller.proc({});

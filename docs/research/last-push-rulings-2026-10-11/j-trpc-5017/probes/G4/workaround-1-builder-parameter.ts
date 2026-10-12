// The rewrite the fix's author suggested in issue #5037 on 2023-11-21: take the
// procedure builder as a parameter constrained to ProcedureBuilder<T>.
import { inferAsyncReturnType, initTRPC } from '@trpc/server';
import type { ProcedureBuilder, ProcedureParams } from '@trpc/server';

type Req = { headers: Record<string, string> };
type Opts = { req: Req; res: { end: () => void } };

async function createInternalContext<T>(
  createContextInner: (opts?: Partial<Opts>) => T,
  opts: Opts,
) {
  const contextInner = await createContextInner();
  return { ...contextInner, req: opts.req, res: opts.res };
}
type LocalContext<T> = inferAsyncReturnType<typeof createInternalContext<T>>;
export type Context<T> = T extends infer R ? LocalContext<R> : never;

function withAuth<T extends ProcedureParams>(
  builder: ProcedureBuilder<T>,
  role: string,
) {
  return builder.use((opts) => opts.next({ ctx: { auth: role } }));
}

export function makeTRPC<T>() {
  const t = initTRPC.context<Context<T>>().create();
  return {
    router: t.router,
    publicProcedure: t.procedure,
    createProtectedProcedure: (role = 'user') => withAuth(t.procedure, role),
  };
}

const { router, createProtectedProcedure } = makeTRPC<{ myContext: string }>();
export const appRouter = router({
  hello: createProtectedProcedure().query(
    ({ ctx }) => `${ctx.myContext} ${ctx.req.headers['a']} ${ctx.auth}`,
  ),
});

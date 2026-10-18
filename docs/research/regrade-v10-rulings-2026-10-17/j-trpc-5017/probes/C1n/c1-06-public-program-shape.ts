// The shape found in public programs: a helper type MiddlewareFunction<{ _ctx_out: Context } & ProcedureParams, ...>
// whose function reads ctx.session.user and adds a user, then used on an instance with that context.
import { initTRPC, MiddlewareFunction, ProcedureParams, TRPCError } from '@trpc/server';

type Context = { session: { user?: string } };
type AuthMiddleware = MiddlewareFunction<
  { _ctx_out: Context } & ProcedureParams,
  { _ctx_out: Context & { user: { id: string } } } & ProcedureParams
>;
const auth: AuthMiddleware = async ({ ctx, next }) => {
  const sessionUser = ctx.session.user;
  if (!sessionUser) throw new TRPCError({ code: 'UNAUTHORIZED' });
  return next({ ctx: { ...ctx, user: { id: sessionUser } } });
};
const t = initTRPC.context<Context>().create();
export const appRouter = t.router({
  p: t.procedure.use(auth).query(({ ctx }) => ctx.user.id),
});

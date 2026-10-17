// The helper shape a user posted in issue #3434 (2022-12-23): a middleware typed
// MiddlewareFunction<ProcedureParams<AnyRootConfig, C>, ProcedureParams>, reading ctx and adding to it.
import { AnyRootConfig, initTRPC, MiddlewareFunction, ProcedureParams } from '@trpc/server';

type Context = { auth: string; userId?: string; isAdmin?: boolean };
type UserContext = Context & { userId: string };
const typedMiddleware =
  <C>() =>
  <T extends MiddlewareFunction<ProcedureParams<AnyRootConfig, C>, ProcedureParams>>(fn: T) =>
    fn;
const isAdmin = typedMiddleware<UserContext>()(async ({ ctx, next }) => {
  const admin = ctx.userId === 'admin';
  const auth: string = ctx.auth;
  return next({ ctx: { ...ctx, isAdmin: admin, auth } });
});
const t = initTRPC.context<Context>().create();
export const appRouter = t.router({
  p: t.procedure
    .use(async ({ ctx, next }) => next({ ctx: { ...ctx, userId: ctx.userId ?? 'u' } }))
    .use(isAdmin)
    .query(({ ctx }) => ctx.isAdmin),
});

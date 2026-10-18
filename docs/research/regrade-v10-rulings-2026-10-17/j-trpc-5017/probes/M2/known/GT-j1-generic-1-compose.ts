// A middleware built from a generic function (the shape of @sentry/node's tRPC
// middleware), then a second middleware added on top of it.
import { initTRPC } from '@trpc/server';
import type { MiddlewareBuilder, ProcedureBuilder } from '@trpc/server';

interface TrpcMiddlewareArguments<T> {
  path: string;
  type: string;
  next: () => T;
  rawInput: unknown;
}
function sentryTrpcMiddleware(_options: unknown) {
  return function <T>({ next }: TrpcMiddlewareArguments<T>): T {
    return next();
  };
}

const t = initTRPC.context<{ some: 'prop' }>().create();
const baseMiddleware = t.middleware(sentryTrpcMiddleware({ foo: 'bar' }));
const someMiddleware = t.middleware((opts) =>
  opts.next({ ctx: { object: { a: 'b' } } }),
);
const baseProcedure = t.procedure.use(baseMiddleware);
const extended = baseProcedure.use(someMiddleware);

export const router = t.router({
  foo: extended.query(({ ctx }) => `${ctx.some} ${ctx.object.a}`),
});
export type Show_what_the_generic_middleware_adds_to_the_context =
  typeof baseMiddleware extends MiddlewareBuilder<any, infer P>
    ? P['_ctx_out']
    : never;
export type Show_context_carried_after_the_generic_middleware =
  typeof baseProcedure extends ProcedureBuilder<infer P> ? P['_ctx_out'] : never;
export type Show_ctx_a_resolver_gets_after_the_generic_middleware = Parameters<
  Parameters<typeof baseProcedure.query>[0]
>[0]['ctx'];

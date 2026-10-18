// The same generic middleware, then a resolver that reads the root context.
import { initTRPC } from '@trpc/server';

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
const baseProcedure = t.procedure.use(
  t.middleware(sentryTrpcMiddleware({ foo: 'bar' })),
);

export const router = t.router({
  foo: baseProcedure.query(({ ctx }) => ctx.some),
});

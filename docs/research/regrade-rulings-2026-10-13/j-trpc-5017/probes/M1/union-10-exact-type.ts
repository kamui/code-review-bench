// Is the caller's type after a middleware exactly the type the schema declares?
// The assignment to `true` fails when it is not.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const schema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('a'), x: z.string() }),
  z.object({ type: z.literal('b'), y: z.number() }),
]);
export const router = t.router({
  withMiddleware: t.procedure
    .input(schema)
    .use((opts) => opts.next())
    .query(({ input }) => input.type),
});
type Same<X, Y> = (<T>() => T extends X ? 1 : 2) extends <T>() => T extends Y
  ? 1
  : 2
  ? true
  : false;
export const exact: Same<
  inferRouterInputs<typeof router>['withMiddleware'],
  z.infer<typeof schema>
> = true;

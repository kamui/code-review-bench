// A discriminated-union input, with and without a middleware that changes nothing.
// Prints the caller's type and the resolver's type of each procedure.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();

const schema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('a'), x: z.string() }),
  z.object({ type: z.literal('b'), y: z.number() }),
]);

let resolverPlain!: unknown;
let resolverWithMiddleware!: unknown;

export const router = t.router({
  plain: t.procedure.input(schema).query(({ input }) => {
    const seen = input;
    resolverPlain = seen as typeof seen;
    return seen;
  }),
  withMiddleware: t.procedure
    .input(schema)
    .use((opts) => opts.next())
    .query(({ input }) => {
      const seen = input;
      resolverWithMiddleware = seen as typeof seen;
      return seen;
    }),
});

type Inputs = inferRouterInputs<typeof router>;
export type Show_caller_plain = Inputs['plain'];
export type Show_caller_with_middleware = Inputs['withMiddleware'];
export type Show_resolver_plain = Parameters<
  Parameters<ReturnType<typeof t.procedure.input<typeof schema>>['query']>[0]
>[0]['input'];
const afterUse = t.procedure.input(schema).use((opts) => opts.next());
export type Show_resolver_with_middleware = Parameters<
  Parameters<(typeof afterUse)['query']>[0]
>[0]['input'];

type Declared = z.infer<typeof schema>;
type Same<X, Y> = (<T>() => T extends X ? 1 : 2) extends <T>() => T extends Y
  ? 1
  : 2
  ? true
  : false;
export type Show_plain_is_exactly_the_declared_type = Same<Inputs['plain'], Declared>;
export type Show_with_middleware_is_exactly_the_declared_type = Same<
  Inputs['withMiddleware'],
  Declared
>;
export type Show_declared_assignable_to_with_middleware = Declared extends Inputs['withMiddleware']
  ? true
  : false;
export type Show_with_middleware_assignable_to_declared = [Inputs['withMiddleware']] extends [
  Declared,
]
  ? true
  : false;
void resolverPlain;
void resolverWithMiddleware;

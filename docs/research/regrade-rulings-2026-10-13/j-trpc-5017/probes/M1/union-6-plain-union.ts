// A union of two records with no shared discriminant, after a middleware.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const schema = z.union([
  z.object({ id: z.string() }),
  z.object({ slug: z.string() }),
]);
declare function byId(value: { id: string }): string;
declare function bySlug(value: { slug: string }): string;

export const router = t.router({
  plain: t.procedure.input(schema).query(({ input }) => input),
  withMiddleware: t.procedure
    .input(schema)
    .use((opts) => opts.next())
    .query(({ input }) => {
      if ('id' in input) {
        return byId(input);
      }
      return bySlug(input);
    }),
});
type Inputs = inferRouterInputs<typeof router>;
export type Show_caller_plain = Inputs['plain'];
export type Show_caller_with_middleware = Inputs['withMiddleware'];
const caller = router.createCaller({});
export const one = caller.withMiddleware({ id: '1' });
export const two = caller.withMiddleware({ slug: 's' });
// accepted with and without the middleware: TypeScript allows a literal that has
// the fields of both members of a union with no discriminant
export const bothPlain = caller.plain({ id: '1', slug: 's' });
export const bothWithMiddleware = caller.withMiddleware({ id: '1', slug: 's' });

// An optional object input whose one key is optional too, with a middleware.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const schema = z.object({ a: z.string().optional() }).optional();
export const router = t.router({
  plain: t.procedure.input(schema).query(({ input }) => input),
  withMiddleware: t.procedure
    .input(schema)
    .use((opts) => opts.next())
    .query(({ input }) => input),
});
type Inputs = inferRouterInputs<typeof router>;
export type Show_caller_plain = Inputs['plain'];
export type Show_caller_with_middleware = Inputs['withMiddleware'];

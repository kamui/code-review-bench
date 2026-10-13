// A limit of fix-same-input: a discriminated-union input followed by a reusable middleware
// whose declared input is one more variant wide. The procedure's input fits the middleware's.
import {
  experimental_standaloneMiddleware,
  inferRouterInputs,
  initTRPC,
} from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const schema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('a'), x: z.string() }),
  z.object({ type: z.literal('b'), y: z.number() }),
]);
const takesAnyVariant = experimental_standaloneMiddleware<{
  input: { type: 'a'; x: string } | { type: 'b'; y: number } | { type: 'c'; z: boolean };
}>().create((opts) => opts.next());
export const router = t.router({
  proc: t.procedure
    .input(schema)
    .use(takesAnyVariant)
    .query(({ input }) => input.type),
});
export type Show_caller = inferRouterInputs<typeof router>['proc'];

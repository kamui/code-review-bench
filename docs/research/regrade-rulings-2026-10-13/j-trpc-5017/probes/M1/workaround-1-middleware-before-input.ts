// The same middleware placed before the input instead of after it.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const schema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('a'), x: z.string() }),
  z.object({ type: z.literal('b'), y: z.number() }),
]);
type A = { type: 'a'; x: string };
type B = { type: 'b'; y: number };
declare function takesA(value: A): string;
declare function takesB(value: B): string;

export const router = t.router({
  middlewareFirst: t.procedure
    .use((opts) => opts.next())
    .use((opts) => opts.next())
    .use((opts) => opts.next())
    .input(schema)
    .query(({ input }) => ('x' in input ? takesA(input) : takesB(input))),
});
export type Show_caller = inferRouterInputs<typeof router>['middlewareFirst'];
const caller = router.createCaller({});
// @ts-expect-error the schema has no variant 'b' with a field x
export const hybrid = caller.middlewareFirst({ type: 'b', x: 'foo', y: 1 });

// The union held in a field of an object input, with a middleware after the input.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const schema = z.object({
  event: z.discriminatedUnion('type', [
    z.object({ type: z.literal('a'), x: z.string() }),
    z.object({ type: z.literal('b'), y: z.number() }),
  ]),
});
type A = { type: 'a'; x: string };
type B = { type: 'b'; y: number };
declare function takesA(value: A): string;
declare function takesB(value: B): string;

export const router = t.router({
  nested: t.procedure
    .input(schema)
    .use((opts) => opts.next())
    .use((opts) => opts.next())
    .use((opts) => opts.next())
    .query(({ input }) => ('x' in input.event ? takesA(input.event) : takesB(input.event))),
});
export type Show_caller = inferRouterInputs<typeof router>['nested'];
const caller = router.createCaller({});
// @ts-expect-error the schema has no variant 'b' with a field x
export const hybrid = caller.nested({ event: { type: 'b', x: 'foo', y: 1 } });

// Two procedures with the same discriminated-union input, joined with unstable_concat.
// The hybrid call is marked as an expected error, so this file compiles only when it is refused.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const schema = z.discriminatedUnion('type', [
  z.object({ type: z.literal('a'), x: z.string() }),
  z.object({ type: z.literal('b'), y: z.number() }),
]);
const first = t.procedure.input(schema);
const second = t.procedure.input(schema);
export const router = t.router({
  joined: first.unstable_concat(second).query(({ input }) => input.type),
});
export type Show_caller_joined = inferRouterInputs<typeof router>['joined'];
const caller = router.createCaller({});
// @ts-expect-error the schema has no variant 'b' with a field x
export const hybrid = caller.joined({ type: 'b', x: 'foo', y: 1 });

// Two procedures, each with an optional object input, joined with unstable_concat.
// The parsers both run, so a call must be nothing at all or an object with both fields.
// A call the parsers refuse at run time is marked as an expected error, so this file
// compiles only when the type refuses it too.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const first = t.procedure.input(z.object({ a: z.string() }).optional());
const second = t.procedure.input(z.object({ b: z.number() }).optional());
export const router = t.router({
  joined: first.unstable_concat(second).query(({ input }) => input),
});
export type Show_caller_joined = inferRouterInputs<typeof router>['joined'];
const caller = router.createCaller({});
export const nothing = caller.joined();
export const both = caller.joined({ a: 'x', b: 1 });
// @ts-expect-error the first parser requires `a` whenever an object is passed
export const onlySecond = caller.joined({ b: 1 });

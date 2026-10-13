// An optional object input followed by a reusable middleware whose declared input is
// an optional object with one of the same fields.
// A call the parsers refuse at run time is marked as an expected error, so this file
// compiles only when the type refuses it too.
import {
  experimental_standaloneMiddleware,
  inferRouterInputs,
  initTRPC,
} from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const needsB = experimental_standaloneMiddleware<{
  input: { b: number } | undefined;
}>().create((opts) => opts.next());

export const router = t.router({
  proc: t.procedure
    .input(z.object({ a: z.string(), b: z.number() }).optional())
    .use(needsB)
    .query(({ input }) => input),
});
export type Show_caller = inferRouterInputs<typeof router>['proc'];
const caller = router.createCaller({});
export const nothing = caller.proc();
export const both = caller.proc({ a: 'x', b: 1 });
// @ts-expect-error the parser requires `a` whenever an object is passed
export const onlySecond = caller.proc({ b: 1 });

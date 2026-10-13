// A limit of fix-same-input: two procedures whose inputs are different objects, each with an
// optional key, joined with unstable_concat. A caller omits both optional keys.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const first = t.procedure.input(z.object({ id: z.string(), a: z.string().optional() }));
const second = t.procedure.input(z.object({ id: z.string(), b: z.string().optional() }));
export const router = t.router({
  joined: first.unstable_concat(second).query(({ input }) => input.id),
});
export type Show_caller_joined = inferRouterInputs<typeof router>['joined'];
const caller = router.createCaller({});
export const omitted = caller.joined({ id: '1' });

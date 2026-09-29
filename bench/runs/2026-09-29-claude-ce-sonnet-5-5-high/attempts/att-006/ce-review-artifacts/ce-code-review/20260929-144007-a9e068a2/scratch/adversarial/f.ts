import { initTRPC, inferRouterInputs } from '@trpc/server';
import { z } from 'zod';
const t = initTRPC.create();
const r = t.router({
  optKey: t.procedure.input(z.object({ a: z.string().optional(), b: z.number() })).use((o) => o.next()).query(({ input }) => input),
  optKeyNoMw: t.procedure.input(z.object({ a: z.string().optional(), b: z.number() })).query(({ input }) => input),
  defKey: t.procedure.input(z.object({ a: z.string().default('x') })).use((o) => o.next()).query(({ input }) => input),
});
type I = inferRouterInputs<typeof r>;
export const ok1: I['optKeyNoMw'] = { b: 1 };
export const ok2: I['optKey'] = { b: 1 };
export const ok3: I['defKey'] = {};
export const ro: (typeof r.optKey)['_def']['_input_out'] = { b: 1 };
type S = { optKey: { old: I['optKey']; new: I['optKeyNoMw'] } };

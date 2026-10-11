// What a developer sees for the input of an array procedure, with and without a middleware.
import { inferRouterInputs, inferRouterOutputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const appRouter = t.router({
  arr: t.procedure.input(z.array(z.string())).query(({ input }) => input),
  // ^? resolver input, no middleware @input })
  arrMw: t.procedure
    .input(z.array(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => input),
  // ^? resolver input, array then middleware @input })
  strMw: t.procedure
    .input(z.string())
    .use((opts) => opts.next())
    .query(({ input }) => input),
  // ^? resolver input, string then middleware @input })
  objMw: t.procedure
    .input(z.object({ ids: z.array(z.string()) }))
    .use((opts) => opts.next())
    .query(({ input }) => input),
  // ^? resolver input, object holding an array, then middleware @input })
});
type Inputs = inferRouterInputs<typeof appRouter>;
type Outputs = inferRouterOutputs<typeof appRouter>;

export type ArrInput = Inputs['arr'];
// ^? caller input type, no middleware @ArrInput
export type ArrMwInput = Inputs['arrMw'];
// ^? caller input type, array then middleware @ArrMwInput
export type ArrMwOutput = Outputs['arrMw'];
// ^? output type, array then middleware @ArrMwOutput
export type StrMwInput = Inputs['strMw'];
// ^? caller input type, string then middleware @StrMwInput

export type Show_ArrInput = Inputs['arr'];
export type Show_ArrMwInput = Inputs['arrMw'];
export type Show_ArrMwOutput = Outputs['arrMw'];
export type Show_StrMwInput = Inputs['strMw'];
export type Show_ObjMwInput = Inputs['objMw'];

export async function caller() {
  const api = appRouter.createCaller({});
  return api.arrMw(['a']);
  // ^? the call a server-side caller writes @arrMw(
}

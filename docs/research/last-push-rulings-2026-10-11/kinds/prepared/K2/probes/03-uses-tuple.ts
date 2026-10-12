// A tuple input followed by a middleware.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
function takesPair(name: string, count: number) {
  return name.repeat(count);
}
function takesTuple(pair: [string, number]) {
  return pair[0].repeat(pair[1]);
}
export const appRouter = t.router({
  tupleMw: t.procedure
    .input(z.tuple([z.string(), z.number()]))
    .use((opts) => opts.next())
    .query(({ input }) => {
      const [name, count] = input;
      const asTuple: [string, number] = input;
      const viaTuple = takesTuple(input);
      const first: string = input[0];
      const second: number = input[1];
      const two: 2 = input.length;
      return { name, count, asTuple, viaTuple, first, second, two };
    }),
});
type Inputs = inferRouterInputs<typeof appRouter>;
export type Show_TupleMwInput_first = Inputs['tupleMw'][0];
export type Show_TupleMwInput_length = Inputs['tupleMw']['length'];

export async function caller() {
  const api = appRouter.createCaller({});
  await api.tupleMw(['a', 1]);
  const typed: Inputs['tupleMw'] = ['a', 1];
  return takesPair(typed[0], typed[1]);
}

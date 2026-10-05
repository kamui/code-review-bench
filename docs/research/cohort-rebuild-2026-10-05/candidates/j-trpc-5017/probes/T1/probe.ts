import {
  experimental_standaloneMiddleware,
  inferRouterInputs,
  inferRouterOutputs,
  initTRPC,
} from '@trpc/server';
import type { Overwrite } from '@trpc/server/core/internals/utils';
import { z } from 'zod';

type Equal<A, B> = (<T>() => T extends A ? 1 : 2) extends <T>() => T extends B
  ? 1
  : 2
  ? true
  : false;
type BothWays<A, B> = [A] extends [B] ? ([B] extends [A] ? true : false) : false;

const t = initTRPC.create();
const passThrough = experimental_standaloneMiddleware<{
  input: string[];
}>().create((opts) => opts.next());

function takesArray(value: string[]) {
  return value.length;
}
function firstOf<T>(values: T[]): T | undefined {
  return values[0];
}

const appRouter = t.router({
  // The pull request's own case, for comparison.
  strMw: t.procedure
    .input(z.string())
    .use((opts) => opts.next())
    .query(({ input }) => input),

  arr: t.procedure.input(z.array(z.string())).query(({ input }) => input),

  // T1: array input followed by a middleware that changes nothing.
  arrMw: t.procedure
    .input(z.array(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => input),

  // Ordinary resolver code that treats the input as an array.
  arrMwUsed: t.procedure
    .input(z.array(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => {
      const upper: string[] = input.map((s) => s.toUpperCase());
      const same: string[] = input;
      const copy: string[] = [...input];
      for (const s of input) {
        copy.push(s);
      }
      takesArray(input);
      const first: string | undefined = input[0];
      const viaGeneric: string | undefined = firstOf(input);
      const asSet: Set<string> = new Set(input);
      const sorted: string[] = [...input].sort();
      const joined: string = input.join(',');
      if (Array.isArray(input)) {
        copy.push(...input);
      }
      return { upper, same, copy, first, viaGeneric, asSet, sorted, joined };
    }),

  arrStandaloneMw: t.procedure
    .input(z.array(z.string()))
    .use(passThrough)
    .query(({ input }) => input),

  optionalArrMw: t.procedure
    .input(z.array(z.string()).optional())
    .use((opts) => opts.next())
    .query(({ input }) => input),

  tupleMw: t.procedure
    .input(z.tuple([z.string(), z.number()]))
    .use((opts) => opts.next())
    .query(({ input }) => input),

  dateMw: t.procedure
    .input(z.date())
    .use((opts) => opts.next())
    .query(({ input }) => input.toISOString()),

  setMw: t.procedure
    .input(z.set(z.string()))
    .use((opts) => opts.next())
    .query(({ input }) => input.size),

  objMw: t.procedure
    .input(z.object({ ids: z.array(z.string()) }))
    .use((opts) => opts.next())
    .query(({ input }) => input),

  tupleMwUsed: t.procedure
    .input(z.tuple([z.string(), z.number()]))
    .use((opts) => opts.next())
    .query(({ input }) => {
      const [name, count] = input;
      const asTuple: [string, number] = input;
      return name.toUpperCase() + count.toFixed(0) + asTuple.length;
    }),
});

// Two array inputs in a row: does the builder accept this at all?
export const twoArrayInputs = t.procedure
  .input(z.array(z.string()))
  .input(z.array(z.string()));

type Inputs = inferRouterInputs<typeof appRouter>;
type Outputs = inferRouterOutputs<typeof appRouter>;

export type Show_Overwrite_string_string = Overwrite<string, string>;
export type Show_Overwrite_stringArray_stringArray = Overwrite<string[], string[]>;
export type Show_Overwrite_stringArray_numberArray = Overwrite<string[], number[]>;
export type Show_Overwrite_Date_object = Overwrite<Date, { a: 1 }>;
export type Show_Input_strMw = Inputs['strMw'];
export type Show_Input_arr = Inputs['arr'];
export type Show_Input_arrMw = Inputs['arrMw'];
export type Show_Output_arrMw = Outputs['arrMw'];
export type Show_Input_arrStandaloneMw = Inputs['arrStandaloneMw'];
export type Show_Input_optionalArrMw = Inputs['optionalArrMw'];
export type Show_Input_tupleMw = Inputs['tupleMw'];
export type Show_Input_dateMw = Inputs['dateMw'];
export type Show_Input_setMw = Inputs['setMw'];
export type Show_Input_objMw = Inputs['objMw'];

export type Show_arrMw_input_is_exactly_string_array = Equal<Inputs['arrMw'], string[]>;
export type Show_arrMw_input_assignable_both_ways_with_string_array = BothWays<Inputs['arrMw'], string[]>;
export type Show_arrMw_output_is_exactly_string_array = Equal<Outputs['arrMw'], string[]>;
export type Show_arrMw_output_assignable_both_ways_with_string_array = BothWays<Outputs['arrMw'], string[]>;
export type Show_arr_output_is_exactly_string_array = Equal<Outputs['arr'], string[]>;
export type Show_optionalArrMw_accepts_undefined = undefined extends Inputs['optionalArrMw'] ? true : false;
export type Show_tupleMw_input_assignable_both_ways_with_tuple = BothWays<Inputs['tupleMw'], [string, number]>;
export type Show_dateMw_input_assignable_both_ways_with_Date = BothWays<Inputs['dateMw'], Date>;
export type Show_setMw_input_assignable_both_ways_with_Set = BothWays<Inputs['setMw'], Set<string>>;

// What a caller writes.
async function caller() {
  const api = appRouter.createCaller({});
  const sent: string[] = ['a', 'b'];

  const echoed = await api.arrMw(sent);
  const asArray: string[] = echoed;
  const mapped = echoed.map((s) => s.length);
  const filtered: string[] = echoed.filter(Boolean);

  const typedInput: Inputs['arrMw'] = sent;
  const typedOutput: Outputs['arrMw'] = sent;
  const outputAsArray: string[] = null as unknown as Outputs['arrMw'];

  await api.optionalArrMw();
  await api.optionalArrMw(sent);
  await api.tupleMw(['a', 1]);
  await api.dateMw(new Date());
  await api.setMw(new Set(['a']));

  return { asArray, mapped, filtered, typedInput, typedOutput, outputAsArray };
}
void caller;

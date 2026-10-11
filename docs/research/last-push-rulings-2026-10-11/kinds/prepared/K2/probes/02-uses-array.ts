// Ordinary code that treats the input of `.input(z.array(z.string())).use(mw)` as an array.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
function takesArray(value: string[]) {
  return value.length;
}
function takesReadonly(value: readonly string[]) {
  return value.length;
}
function firstOf<T>(values: T[]): T | undefined {
  return values[0];
}
function lengthOf<T extends unknown[]>(values: T): number {
  return values.length;
}
export const appRouter = t.router({
  arrMw: t.procedure
    .input(z.array(z.string()))
    .use((opts) => opts.next())
    .query(async ({ input }) => {
      void input;
      const upper: string[] = input.map((s) => s.toUpperCase());
      const same: string[] = input;
      const copy: string[] = [...input];
      for (const s of input) copy.push(s);
      takesArray(input);
      takesReadonly(input);
      const first: string | undefined = input[0];
      const viaGeneric: string | undefined = firstOf(input);
      const n: number = lengthOf(input);
      const asSet: Set<string> = new Set(input);
      const joined: string = input.join(',');
      const filtered: string[] = input.filter((s) => s.length > 1);
      const total: number = input.reduce((sum, s) => sum + s.length, 0);
      const settled: string[] = await Promise.all(input.map(async (s) => s));
      const joinedTwo: string[] = input.concat(['x']);
      const [head, ...rest] = input;
      const restTyped: string[] = rest;
      const entries: [number, string][] = [...input.entries()];
      const fromArray: string[] = Array.from(input);
      const json: string = JSON.stringify(input);
      const has: boolean = input.includes('a');
      if (Array.isArray(input)) copy.push(...input);
      return { upper, same, copy, first, viaGeneric, n, asSet, joined, filtered, total, settled, joinedTwo, head, restTyped, entries, fromArray, json, has };
    }),
});
type Inputs = inferRouterInputs<typeof appRouter>;
type ArrMwInput = Inputs['arrMw'];

type ElementOf<T> = T extends readonly (infer U)[] ? U : 'not an array';
type IsArray<T> = T extends readonly unknown[] ? true : false;
export type Show_ElementOf = ElementOf<ArrMwInput>;
export type Show_IsArray = IsArray<ArrMwInput>;
export type Show_Indexed = ArrMwInput[number];
export type Show_PartialKeepsArray = Partial<ArrMwInput> extends readonly unknown[] ? true : false;
export type Show_PartialOfPlainArray = Partial<string[]> extends readonly unknown[] ? true : false;

export async function caller() {
  const api = appRouter.createCaller({});
  const sent: string[] = ['a', 'b'];
  const result = await api.arrMw(sent);
  const echoed: string[] = result.same;
  const typed: ArrMwInput = sent;
  const back: string[] = typed;
  const element: ElementOf<ArrMwInput> = 'a';
  return { echoed, typed, back, element };
}

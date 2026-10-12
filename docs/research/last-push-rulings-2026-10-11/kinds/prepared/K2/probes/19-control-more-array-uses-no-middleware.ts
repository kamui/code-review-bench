// Control for 18: the same resolver without a middleware.
import * as path from 'path';
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const schema = z.array(z.string());
function chunk<T>(values: readonly T[], size: number): T[][] {
  return [values.slice(0, size)];
}
function isNonEmpty<T>(values: T[]): values is [T, ...T[]] {
  return values.length > 0;
}
function structuredCloneLike<T>(value: T): T {
  return value;
}
function overloaded(value: unknown[]): 'array';
function overloaded(value: object): 'object';
function overloaded(value: object): 'array' | 'object' {
  return Array.isArray(value) ? 'array' : 'object';
}
export const appRouter = t.router({
  arr: t.procedure.input(schema).query(({ input }) => {
      const out: unknown[] = [];
      out.push(...input); // spread into a rest parameter
      out.push(path.join(...input));
      out.push(Math.max(...input.map((s) => s.length)));
      out.push(input.flatMap((s) => [s, s]));
      out.push(chunk(input, 2));
      out.push(isNonEmpty(input) ? input[0] : null);
      out.push(input.length === 0);
      out.push(schema.parse(input));
      out.push(Array.from(input, (s) => s.length));
      out.push(structuredCloneLike(input).map((s) => s));
      const frozen: readonly string[] = Object.freeze(input);
      out.push(frozen);
      const asConst = [...input] as const;
      out.push(asConst);
      const where: { id: { in: string | string[] } } = { id: { in: input } };
      out.push(where);
      const narrowed: string[] = Array.isArray(input) ? input : [];
      out.push(narrowed);
      const copyOf: typeof input = ['x'];
      out.push(copyOf);
      const element: (typeof input)[number] = 'x';
      out.push(element);
      const sliced: string[] = input.slice(0, 2);
      out.push(sliced, overloaded(input));
      const kind: 'array' = overloaded(input);
      out.push(kind);
      return out.length;
  }),
});

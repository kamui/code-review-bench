import { initTRPC, inferRouterInputs, inferProcedureInput } from '@trpc/server';
import { expectTypeOf } from 'vitest';
import { z } from 'zod';

const t = initTRPC.create();

const router = t.router({
  // #1 and #2: an array input followed by a pass-through middleware.
  arr: t.procedure.input(z.array(z.string())).use((o) => o.next()).query(({ input }) => input),
  // Control for #1: the same input with no middleware.
  arr2: t.procedure.input(z.array(z.string())).query(({ input }) => input),
});
type In = inferRouterInputs<typeof router>;

// #2: the review says this exact-type assertion fails.
expectTypeOf<In['arr']>().toEqualTypeOf<string[]>();
// Control for #2: the same assertion without the middleware.
expectTypeOf<In['arr2']>().toEqualTypeOf<string[]>();

// #1: an array-like value that is not an array.
declare const arrayLike: { [k: number]: string; length: number };
export const a1: In['arr'] = arrayLike; // with middleware
export const a2: In['arr2'] = arrayLike; // without middleware

// #3: an array input followed by a later input with an object shape.
export const viaInput = t.procedure
  .input(z.array(z.string()))
  .input(z.object({ b: z.literal(2) }))
  .query(({ input }) => input);
// #3: an array input followed by a reusable middleware declared for an object input with b.
const needsB = t.middleware(({ next }) => next());
export const viaMiddleware = t.procedure
  .input(z.array(z.string()))
  .use(needsB)
  .query(({ input }) => input);
type ViaMiddleware = inferProcedureInput<typeof viaMiddleware>;
export const showMiddleware: ViaMiddleware = 0 as unknown as number; // prints the type

// #3, second form: a reusable middleware declared for an input object with b, after an array input.
import { experimental_standaloneMiddleware } from '@trpc/server';
const standaloneB = experimental_standaloneMiddleware<{ input: { b: 2 } }>().create(({ next }) => next());
export const viaStandalone = t.procedure
  .input(z.array(z.string()))
  .use(standaloneB)
  .query(({ input }) => input);
// #3, third form: the reusable middleware declared for an array input, with an object-shaped _input_in.
const standaloneArr = experimental_standaloneMiddleware<{ input: string[] }>().create(({ next }) => next());
export const viaStandaloneArr = t.procedure.input(z.array(z.string())).use(standaloneArr).query(({ input }) => input);

// Sanity checks: expectTypeOf must reject a wrong type, and a strict identity check.
expectTypeOf<In['arr']>().toEqualTypeOf<number[]>(); // must fail
type Equal<X, Y> = (<T>() => T extends X ? 1 : 2) extends (<T>() => T extends Y ? 1 : 2) ? true : false;
export const strictWithMiddleware: Equal<In['arr'], string[]> = true;
export const strictWithout: Equal<In['arr2'], string[]> = true;

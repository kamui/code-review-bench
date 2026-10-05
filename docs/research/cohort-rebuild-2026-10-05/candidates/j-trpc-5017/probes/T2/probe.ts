import {
  experimental_standaloneMiddleware,
  inferRouterInputs,
  initTRPC,
} from '@trpc/server';
import type { Overwrite } from '@trpc/server/core/internals/utils';
import { z } from 'zod';

const t = initTRPC.context<{ userId: string }>().create();

// A middleware that says: "I can run on a procedure whose input is this object,
// or on one whose input was left out." It changes nothing.
const acceptsObjectOrNothing = experimental_standaloneMiddleware<{
  input: { value: string } | undefined;
}>().create((opts) => opts.next());

const acceptsObjectOrNull = experimental_standaloneMiddleware<{
  input: { value: string } | null;
}>().create((opts) => opts.next());

// The shape upstream discussion #5051 / issue #5056 reported: one property wider.
const acceptsOptionalProperty = experimental_standaloneMiddleware<{
  input: { value?: string };
}>().create((opts) => opts.next());

const acceptsStringOrNumber = experimental_standaloneMiddleware<{
  input: number | string;
}>().create((opts) => opts.next());

const requiredObject = z.object({ value: z.string() });

const appRouter = t.router({
  // Reference point: no middleware.
  plain: t.procedure
    .input(requiredObject)
    .query(({ input }) => input.value.toUpperCase()),

  // Reference point: an inline middleware that declares no input type.
  inlineMw: t.procedure
    .input(requiredObject)
    .use((opts) => opts.next())
    .query(({ input }) => input.value.toUpperCase()),

  // T2: required object input, then the middleware that also tolerates undefined.
  requiredThenUndefinedMw: t.procedure
    .input(requiredObject)
    .use(acceptsObjectOrNothing)
    .query(({ input }) => input.value.toUpperCase()),

  requiredThenNullMw: t.procedure
    .input(requiredObject)
    .use(acceptsObjectOrNull)
    .query(({ input }) => input.value.toUpperCase()),

  // The intended use of that middleware: an input that really is optional.
  optionalThenUndefinedMw: t.procedure
    .input(requiredObject.optional())
    .use(acceptsObjectOrNothing)
    .query(({ input }) => input?.value.toUpperCase()),

  // Property-level widening (upstream #5056).
  requiredThenOptionalPropertyMw: t.procedure
    .input(requiredObject)
    .use(acceptsOptionalProperty)
    .query(({ input }) => input.value.toUpperCase()),

  // Primitive input with a middleware that tolerates a wider primitive.
  stringThenStringOrNumberMw: t.procedure
    .input(z.string())
    .use(acceptsStringOrNumber)
    .query(({ input }) => input.toUpperCase()),

  // Same merge reached through the other builder method that uses it.
  requiredThenConcatOptional: t.procedure
    .input(requiredObject)
    .unstable_concat(t.procedure.input(requiredObject.optional()))
    .query(({ input }) => input.value.toUpperCase()),

  // The context form already covered by reference bug GT-j3, for comparison.
  ctxUndefined: t.procedure
    .use((opts) => opts.next({ ctx: undefined }))
    .query(({ ctx }) => ctx.userId),
});

type Inputs = inferRouterInputs<typeof appRouter>;

export type Show_Overwrite_object_objectOrUndefined = Overwrite<
  { value: string },
  { value: string } | undefined
>;
export type Show_Input_plain = Inputs['plain'];
export type Show_Input_inlineMw = Inputs['inlineMw'];
export type Show_Input_requiredThenUndefinedMw = Inputs['requiredThenUndefinedMw'];
export type Show_Input_requiredThenNullMw = Inputs['requiredThenNullMw'];
export type Show_Input_optionalThenUndefinedMw = Inputs['optionalThenUndefinedMw'];
export type Show_Input_requiredThenOptionalPropertyMw = Inputs['requiredThenOptionalPropertyMw'];
export type Show_Input_stringThenStringOrNumberMw = Inputs['stringThenStringOrNumberMw'];
export type Show_Input_requiredThenConcatOptional = Inputs['requiredThenConcatOptional'];

export type Show_requiredThenUndefinedMw_accepts_undefined =
  undefined extends Inputs['requiredThenUndefinedMw'] ? true : false;
export type Show_requiredThenNullMw_accepts_null =
  null extends Inputs['requiredThenNullMw'] ? true : false;
export type Show_requiredThenConcatOptional_accepts_undefined =
  undefined extends Inputs['requiredThenConcatOptional'] ? true : false;
export type Show_optionalThenUndefinedMw_accepts_undefined =
  undefined extends Inputs['optionalThenUndefinedMw'] ? true : false;

// What a caller writes. Each call that omits a required input is preceded by
// a "ts-expect-error" comment: the compiler reports TS2578 "Unused
// directive" on that comment when it wrongly accepts the call.
async function caller() {
  const api = appRouter.createCaller({ userId: 'u1' });

  // @ts-expect-error the input is required
  await api.plain();
  // @ts-expect-error the input is required
  await api.inlineMw();
  // @ts-expect-error the input is required
  await api.requiredThenUndefinedMw();
  // @ts-expect-error the input is required
  await api.requiredThenNullMw(null);
  // @ts-expect-error the input is required
  await api.requiredThenConcatOptional();

  await api.optionalThenUndefinedMw();
  await api.optionalThenUndefinedMw({ value: 'a' });
}
void caller;

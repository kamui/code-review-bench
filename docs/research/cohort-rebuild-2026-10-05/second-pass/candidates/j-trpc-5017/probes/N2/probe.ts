import { initTRPC, inferRouterInputs, Overwrite } from './clone/packages/server/src';
import { z } from 'zod';

const t = initTRPC.create();
function customParser(value: unknown): { a?: string } {
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    if (!('a' in value) || value.a === undefined) return {};
    if (typeof value.a === 'string') return { a: value.a };
  }
  throw new Error('Expected an object with an optional string a');
}
const zodParser = z.object({ a: z.string().optional() });
export const router = t.router({
  customPlain: t.procedure.input(customParser).query(({ input }) => input.a ?? 'absent'),
  customMiddleware: t.procedure.input(customParser).use(o => o.next()).query(({ input }) => input.a ?? 'absent'),
  customConcat: t.procedure.input(customParser).unstable_concat(t.procedure.input(customParser)).query(({ input }) => input.a ?? 'absent'),
  zodPlain: t.procedure.input(zodParser).query(({ input }) => input.a ?? 'absent'),
  zodMiddleware: t.procedure.input(zodParser).use(o => o.next()).query(({ input }) => input.a ?? 'absent'),
  zodConcat: t.procedure.input(zodParser).unstable_concat(t.procedure.input(zodParser)).query(({ input }) => input.a ?? 'absent'),
});
export type CustomPlainInput = inferRouterInputs<typeof router>['customPlain'];
export type CustomMiddlewareInput = inferRouterInputs<typeof router>['customMiddleware'];
export type CustomConcatInput = inferRouterInputs<typeof router>['customConcat'];
export type ZodPlainInput = inferRouterInputs<typeof router>['zodPlain'];
export type ZodMiddlewareInput = inferRouterInputs<typeof router>['zodMiddleware'];
export type ZodConcatInput = inferRouterInputs<typeof router>['zodConcat'];
export type MergedOptional = Overwrite<{ a?: string }, { a?: string }>;
export const customPlainInput: CustomPlainInput = {};
export const customMiddlewareInput: CustomMiddlewareInput = {};
export const customConcatInput: CustomConcatInput = {};
export const zodPlainInput: ZodPlainInput = {};
export const zodMiddlewareInput: ZodMiddlewareInput = {};
export const zodConcatInput: ZodConcatInput = {};
export function compileCalls() {
  const caller = router.createCaller({});
  caller.customPlain({});
  caller.customMiddleware({});
  caller.customConcat({});
  caller.zodPlain({});
  caller.zodMiddleware({});
  caller.zodConcat({});
}
export async function runtime() {
  const caller = router.createCaller({});
  for (const name of ['customPlain', 'customMiddleware', 'customConcat', 'zodPlain', 'zodMiddleware', 'zodConcat'] as const) {
    console.log(name, await caller[name]({}));
  }
}

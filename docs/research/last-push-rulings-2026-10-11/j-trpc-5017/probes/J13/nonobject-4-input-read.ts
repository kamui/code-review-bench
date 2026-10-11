// A required object input, then a standalone middleware whose declared input
// also allows undefined. The resolver reads the input.
import { experimental_standaloneMiddleware, initTRPC } from '@trpc/server';
import type { ProcedureBuilder, inferRouterInputs } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const acceptsObjectOrNothing = experimental_standaloneMiddleware<{
  input: { value: string } | undefined;
}>().create((opts) => opts.next());
const withMiddleware = t.procedure
  .input(z.object({ value: z.string() }))
  .use(acceptsObjectOrNothing);

export const router = t.router({
  foo: withMiddleware.query(({ input }) => input.value.toUpperCase()),
});
export type Show_input_a_resolver_gets_after_the_middleware =
  typeof withMiddleware extends ProcedureBuilder<infer P> ? P['_input_out'] : never;
export type Show_input_a_caller_must_pass = inferRouterInputs<typeof router>['foo'];

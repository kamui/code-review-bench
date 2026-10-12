// A branded string input, a middleware that changes nothing, and a resolver
// that passes the input to a function taking a string.
import { initTRPC } from '@trpc/server';
import type { ProcedureBuilder, inferRouterInputs } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
function takesString(value: string) {
  return value.toUpperCase();
}
const withMiddleware = t.procedure
  .input(z.string().brand<'id'>())
  .use((opts) => opts.next());

export const router = t.router({
  foo: withMiddleware.query(({ input }) => takesString(input)),
});
export type Show_input_a_caller_must_pass = inferRouterInputs<typeof router>['foo'];
export type Show_input_a_resolver_gets_is_a_string =
  typeof withMiddleware extends ProcedureBuilder<infer P>
    ? P['_input_out'] extends string
      ? true
      : false
    : never;

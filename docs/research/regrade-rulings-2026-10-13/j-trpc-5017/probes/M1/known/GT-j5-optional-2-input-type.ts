// The same procedure's input type, as a client would declare a variable of it.
import { inferRouterInputs, initTRPC } from '@trpc/server';
import type { ProcedureBuilder } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
const withMiddleware = t.procedure
  .input(z.object({ a: z.string().optional() }))
  .use((opts) => opts.next());

export const router = t.router({
  foo: withMiddleware.query(({ input }) => input.a ?? 'absent'),
});

export const input: inferRouterInputs<typeof router>['foo'] = {};
export type Show_input_a_caller_must_pass = inferRouterInputs<typeof router>['foo'];
export type Show_input_a_resolver_gets =
  typeof withMiddleware extends ProcedureBuilder<infer P> ? P['_input_out'] : never;

// Usage: bun runtime.ts <checkout-dir>
// Shows what the server really does when the required input is left out.
import path from 'node:path';
import { z } from 'zod';

const checkout = path.resolve(process.argv[2]!);
const { experimental_standaloneMiddleware, initTRPC } = await import(
  path.join(checkout, 'packages/server/src/index.ts')
);

const t = initTRPC.create();
const acceptsObjectOrNothing = experimental_standaloneMiddleware().create(
  (opts: any) => opts.next(),
);
const appRouter = t.router({
  requiredThenUndefinedMw: t.procedure
    .input(z.object({ value: z.string() }))
    .use(acceptsObjectOrNothing)
    .query(({ input }: any) => `resolver ran with ${JSON.stringify(input)}`),
});
const api = appRouter.createCaller({});

console.log(`checkout ${path.basename(checkout)}`);
console.log('call with { value: "a" }:', await api.requiredThenUndefinedMw({ value: 'a' }));
try {
  console.log('call with no input:', await api.requiredThenUndefinedMw(undefined));
} catch (error: any) {
  console.log(`call with no input: rejected, code ${error.code}, ${String(error.message).replace(/\s+/g, ' ')}`);
}

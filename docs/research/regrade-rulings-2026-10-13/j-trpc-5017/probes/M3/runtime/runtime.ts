// Usage: bun runtime.ts <tree-dir>
// Runs the middlewares of the compile probes on a context typed `any`, to show what the
// context holds at run time. Bun does not type-check.
import path from 'node:path';

const tree = path.resolve(process.argv[2]!);
const trpc: any = await import(`${tree}/packages/server/src/index.ts`);
const t = trpc.initTRPC.context().create();

const seen: unknown[] = [];
const logged = t.middleware(({ ctx, next }: any) => {
  seen.push(ctx.foo);
  return next();
});
const isAuthed = t.middleware(({ ctx, next }: any) => {
  if (!ctx.user) {
    throw new trpc.TRPCError({ code: 'UNAUTHORIZED' });
  }
  return next({ ctx: { user: ctx.user } });
});

const router = t.router({
  readAfterPassThrough: t.procedure.use(logged).query(({ ctx }: any) => ctx.foo),
  readAfterAuth: t.procedure.use(isAuthed).query(({ ctx }: any) => `${ctx.user.id} ${ctx.foo}`),
});

console.log(`tree ${path.basename(tree)}`);
const caller = router.createCaller({ foo: 1, user: { id: 'u1' } });
console.log(`resolver after a pass-through t.middleware(), ctx.foo: ${JSON.stringify(await caller.readAfterPassThrough())}`);
console.log(`ctx.foo as the middleware saw it: ${JSON.stringify(seen)}`);
console.log(`resolver after the authorization middleware: ${JSON.stringify(await caller.readAfterAuth())}`);
try {
  await router.createCaller({ foo: 1 }).readAfterAuth();
} catch (error: any) {
  console.log(`no user in the context: rejected, code ${error.code}`);
}

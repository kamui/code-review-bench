// Usage: bun runtime.ts <tree-dir>
// Runs the generic factory of factory-3 with a concrete type, to show what
// happens at run time. Bun does not type-check.
import path from 'node:path';

const tree = path.resolve(process.argv[2]!);
const trpc: any = await import(`${tree}/packages/server/src/index.ts`);

function makeTRPC() {
  const t = trpc.initTRPC.context().create();
  const withAuth = t.middleware((opts: any) => opts.next({ ctx: { auth: 'user' } }));
  return { router: t.router, protectedProcedure: t.procedure.use(withAuth) };
}

const { router, protectedProcedure } = makeTRPC();
const appRouter = router({
  hello: protectedProcedure.query(({ ctx }: any) => ctx),
});
const api = appRouter.createCaller({ req: { headers: { a: 'b' } }, myContext: 'mine' });
console.log(`tree ${path.basename(tree)}`);
console.log('ctx in the resolver:', JSON.stringify(await api.hello()));

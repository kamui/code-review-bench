import { initTRPC, experimental_standaloneMiddleware, inferRouterInputs } from '../../clone/packages/server/src';
import { z } from '../../clone/node_modules/zod';
const t = initTRPC.create();
const mw = t.middleware(({next}) => next());
const smw = experimental_standaloneMiddleware().create(({next}) => next());
const r = t.router({
 a: t.procedure.input(z.string()).use(mw).query(({input}) => input.toUpperCase()),
 b: t.procedure.input(z.string()).use(smw).query(({input}) => input.toUpperCase()),
 c: t.procedure.input(z.object({foo:z.string()})).use(mw).query(({input}) => input.foo),
});
type I = inferRouterInputs<typeof r>;
const x: I['a'] = 123;
const y: I['b'] = 123;
const q: I['c'] = 123;

import { initTRPC } from '../../clone/packages/server/src';
const t = initTRPC.context<{ user: {name: string} | null; id: string }>().create();
t.procedure.use(({ctx,next}) => next({ctx: ctx.user && { name: ctx.user.name }})).query(({ctx}) => ctx.id);
t.procedure.use(({next}) => next({ctx: undefined})).query(({ctx}) => ctx.id);

import { initTRPC } from './server-src';
import { z } from 'zod';
const t = initTRPC.context<{ a: string }>().create();
const q = t.procedure.input(z.string()).use((o)=>o.next()).query(() => 1);
export const x: never = q;
const q2 = t.procedure.use((o)=>o.next()).query(() => 1);
export const y: never = q2;

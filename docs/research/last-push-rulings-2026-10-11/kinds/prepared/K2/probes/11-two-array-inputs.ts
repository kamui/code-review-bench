// Two array inputs in a row.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

const t = initTRPC.create();
export const twoArrayInputs = t.procedure.input(z.array(z.string())).input(z.array(z.string()));

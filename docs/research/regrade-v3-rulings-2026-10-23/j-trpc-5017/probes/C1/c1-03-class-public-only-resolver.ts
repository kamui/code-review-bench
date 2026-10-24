// The same with a class that has only public members.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

class Point {
  constructor(public x: number, public y: number) {}
  norm() {
    return Math.hypot(this.x, this.y);
  }
}
function takesPoint(p: Point) {
  return p.norm();
}
const t = initTRPC.create();
export const router = t.router({
  withMiddleware: t.procedure
    .input(z.instanceof(Point))
    .use((opts) => opts.next())
    .query(({ input }) => takesPoint(input)),
});

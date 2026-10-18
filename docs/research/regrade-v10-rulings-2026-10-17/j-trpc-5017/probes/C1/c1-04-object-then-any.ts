// The comment's side remark: Overwrite<{ a: 1 }, any>. Does a read of an unknown key, and a use of `a` as 1, compile?
import type { Overwrite } from '@trpc/server';

declare const merged: Overwrite<{ a: 1 }, any>;
export const one: 1 = merged.a;
export const other: string = merged.whatever;

type Old<TType, TWith> = TType extends any
  ? TWith extends any
    ? { [K in keyof TType | keyof TWith]: K extends keyof TWith ? TWith[K] : K extends keyof TType ? TType[K] : never }
    : never
  : never;
type New<TType, TWith> = TType extends object
  ? TWith extends object
    ? { [K in keyof TType | keyof TWith]: K extends keyof TWith ? TWith[K] : K extends keyof TType ? TType[K] : never }
    : TWith extends any ? TWith : never
  : TType extends any
  ? TWith extends any ? TWith : TType
  : never;
declare const sym: unique symbol;
type Sym = typeof sym;
type Show<T> = [T] extends [never] ? 'NEVER' : T;
type C<A, B> = { old: Show<Old<A,B>>; nw: Show<New<A,B>> };
export const t1: { nw: 'X' } = null as any as C<string, never>;
export const t2: { nw: 'X' } = null as any as C<{a:1}, never>;
export const t3: { nw: 'X' } = null as any as C<{a:1}, unknown>;
export const t4: { nw: 'X' } = null as any as C<{a:1}, any>;
export const t5: { nw: 'X' } = null as any as C<{a:1}, undefined>;
export const t6: { nw: 'X' } = null as any as C<{a:1}|undefined, {b:2}>;
export const t7: { nw: 'X' } = null as any as C<{a:1}, {b:2}|undefined>;
export const t8: { nw: 'X' } = null as any as C<Sym, Sym>;
export const t9: { nw: 'X' } = null as any as C<{a:1}, () => void>;
export const t10: { nw: 'X' } = null as any as C<unknown, {a:1}>;
export const t11: { nw: 'X' } = null as any as C<any, {a:1}>;
export const t12: { nw: 'X' } = null as any as C<{a:1}, string>;
export const t13: { nw: 'X' } = null as any as C<{a:1}, {a:1}[]>;
export const t14: { nw: 'X' } = null as any as C<string, string>;
export const t15: { nw: 'X' } = null as any as C<string | {a:1}, string | {a:1}>;
export const t16: { nw: 'X' } = null as any as C<Sym, {a:1}>;
export const t17: { nw: 'X' } = null as any as C<{}, {}>;

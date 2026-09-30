export type Overwrite<TType, TWith> = TType extends object
  ? TWith extends object
    ? { [K in keyof TType | keyof TWith]: K extends keyof TWith ? TWith[K] : K extends keyof TType ? TType[K] : never; }
    : TWith extends any ? TWith : never
  : TType extends any ? TWith extends any ? TWith : TType : never;
type Simplify<TType> = TType extends any[] | Date ? TType : { [K in keyof TType]: TType[K] };
type Eq<A, B> = (<T>() => T extends A ? 1 : 2) extends (<T>() => T extends B ? 1 : 2) ? true : false;
const arr: Eq<Overwrite<string[], string[]>, string[]> = true;
const date: Eq<Overwrite<Date, Date>, Date> = true;
const tup: Eq<Overwrite<[string, number], [string, number]>, [string, number]> = true;
const anyCtx: Overwrite<any, { user: number }> = null!;
anyCtx.foo;
const unk: Eq<Simplify<Overwrite<{ user: string }, unknown>>, {}> = true;
const nev: Eq<Overwrite<string, never>, never> = true;
const str: Eq<Overwrite<string, string>, string> = true;

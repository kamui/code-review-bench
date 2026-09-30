export type Overwrite<TType, TWith> = TType extends object
  ? TWith extends object
    ? { [K in keyof TType | keyof TWith]: K extends keyof TWith ? TWith[K] : K extends keyof TType ? TType[K] : never; }
    : TWith extends any ? TWith : never
  : TType extends any ? TWith extends any ? TWith : TType : never;
type Simplify<TType> = TType extends any[] | Date ? TType : { [K in keyof TType]: TType[K] };
declare function get<T>(): T;
export function f() {
  const anyCtx = get<Simplify<Overwrite<any, { user: number }>>>();
  anyCtx.foo;
  const arr = get<Simplify<Overwrite<string[], string[]>>>();
  const s: string[] = arr;
  arr.map((x) => x);
  const d = get<Simplify<Overwrite<Date, Date>>>();
  const dd: Date = d;
  return [s, dd];
}

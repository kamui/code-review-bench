type Old<T, U> = T extends any ? U extends any ? { [K in keyof T | keyof U]: K extends keyof U ? U[K] : K extends keyof T ? T[K] : never } : never : never;
type New<T,U> = T extends object ? U extends object ? {[K in keyof T | keyof U]: K extends keyof U ? U[K] : K extends keyof T ? T[K] : never} : U extends any ? U : never : T extends any ? U extends any ? U : T : never;
type C = { id:string, user: {name:string} | null};
declare const a: Old<C, null | {name:string}>;
a.id;
declare const b: New<C, null | {name:string}>;
b.id;

// The same with a class whose hidden member is an ECMAScript #private field, and
// with one whose hidden member is protected.
import { initTRPC } from '@trpc/server';
import { z } from 'zod';

class Token {
  #secret: string;
  constructor(secret: string) {
    this.#secret = secret;
  }
  masked() {
    return this.#secret.slice(0, 2) + '***';
  }
}
class Account {
  protected owner: string;
  constructor(owner: string) {
    this.owner = owner;
  }
  label() {
    return this.owner.toUpperCase();
  }
}
function takesToken(v: Token) {
  return v.masked();
}
function takesAccount(v: Account) {
  return v.label();
}
const t = initTRPC.create();
export const router = t.router({
  token: t.procedure
    .input(z.instanceof(Token))
    .use((opts) => opts.next())
    .query(({ input }) => takesToken(input)),
  account: t.procedure
    .input(z.instanceof(Account))
    .use((opts) => opts.next())
    .query(({ input }) => takesAccount(input)),
});

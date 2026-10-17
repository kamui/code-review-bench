// A resolver helper typed against the resolver options of the default ProcedureParams reads a property of ctx.
// ResolveOptions is not exported from the package entry point; it is imported from its source file here.
import type { ProcedureParams } from '@trpc/server';
import type { ResolveOptions } from '@trpc/server/core/internals/utils';

export function sessionOf(opts: ResolveOptions<ProcedureParams>) {
  return opts.ctx.session;
}

// The types the comment names: Overwrite<any, unknown>, Overwrite<{ a: 1 }, any>, the resolver's ctx for the
// default ProcedureParams, and the ctx a middleware typed MiddlewareFunction<ProcedureParams, ProcedureParams> receives.
import type { MiddlewareFunction, Overwrite, ProcedureParams } from '@trpc/server';
import type { ResolveOptions } from '@trpc/server/core/internals/utils';

type Show_overwrite_any_unknown = Overwrite<any, unknown>;
type Show_overwrite_any_empty_object = Overwrite<any, {}>;
type Show_overwrite_object_any = Overwrite<{ a: 1 }, any>;
type Show_resolve_options_ctx = ResolveOptions<ProcedureParams>['ctx'];
type Show_middleware_function_ctx = Parameters<MiddlewareFunction<ProcedureParams, ProcedureParams>>[0]['ctx'];
type Show_procedure_params_ctx_out = ProcedureParams['_ctx_out'];
type Show_procedure_params_root_ctx = ProcedureParams['_config']['$types']['ctx'];

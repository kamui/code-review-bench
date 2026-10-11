// The code posted in upstream issue #5037 (create-context.ts and make-trpc.ts), in one file.
// Only change: the reporter's private package '@mama/auth-plugin' is replaced by a declaration.
import { inferAsyncReturnType, initTRPC, TRPCError } from '@trpc/server';
import type {
  CreateFastifyContextOptions,
  FastifyTRPCPluginOptions,
} from '@trpc/server/adapters/fastify';

// Stand-in for the reporter's private package '@mama/auth-plugin'.
type AuthOptions = { roles?: string[] };
declare function authenticate(
  req: CreateFastifyContextOptions['req'],
  options?: AuthOptions,
): Promise<{ auth: { userId: string } }>;

// create-context.ts, as posted
export interface CreateInnerContextOptions
  extends Partial<CreateFastifyContextOptions> {}

export function createContext(config: Partial<FastifyTRPCPluginOptions<any>>) {
  return (outerOptions: CreateFastifyContextOptions) => {
    return createInternalContext(async (innerOptions) => {
      if (!outerOptions?.req || !outerOptions?.res) {
        return innerOptions;
      }
      return {
        ...innerOptions,
        ...(await config?.trpcOptions?.createContext?.({
          req: outerOptions.req,
          res: outerOptions.res,
        })),
      };
    }, outerOptions);
  };
}

async function createInternalContext<T>(
  createContextInner: (opts?: CreateInnerContextOptions) => T,
  opts: CreateFastifyContextOptions,
) {
  const contextInner = await createContextInner();
  return {
    ...contextInner,
    req: opts.req,
    res: opts.res,
  };
}

type LocalContext<T> = inferAsyncReturnType<typeof createInternalContext<T>>;

export type Context<T> = T extends infer R ? LocalContext<R> : never;

// make-trpc.ts, as posted
export function makeTRPC<T>(config?: Parameters<typeof initTRPC.create>[0]) {
  const t = initTRPC.context<Context<T>>().create({
    errorFormatter(error) {
      return config?.errorFormatter?.(error) ?? error.shape;
    },
  });

  function withAuth(authOptions: Parameters<typeof authenticate>[1]) {
    return t.middleware(async (opts) => {
      if (!(opts.ctx as any).req) {
        throw new TRPCError({
          code: 'INTERNAL_SERVER_ERROR',
          message: 'missing req object',
        });
      }
      try {
        const { auth } = await authenticate(
          (opts.ctx as Context<CreateFastifyContextOptions>).req,
          authOptions,
        );
        return opts.next({
          ctx: {
            ...(opts.ctx as Context<Partial<CreateFastifyContextOptions>>),
            auth,
          },
        });
      } catch (e) {
        throw new TRPCError({ code: 'UNAUTHORIZED' });
      }
    });
  }

  return {
    router: t.router,
    publicProcedure: t.procedure,
    createProtectedProcedure: (
      options: Parameters<typeof authenticate>[1] = {},
    ) => t.procedure.use(withAuth(options)),
  };
}

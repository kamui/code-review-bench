/**
 * YOU PROBABLY DON'T NEED TO EDIT THIS FILE, UNLESS:
 * 1. You want to modify request context (see Part 1).
 * 2. You want to create a new middleware or type of procedure (see Part 3).
 *
 * TL;DR - This is where all the tRPC server stuff is created and plugged in. The pieces you will
 * need to use are documented accordingly near the end.
 */
import superjson from "superjson";
import { type Span, trace } from "@opentelemetry/api";
import { ZodError } from "zod";
import { db } from "~/server/db";
import { experimental_standaloneMiddleware, initTRPC } from "@trpc/server";

/**
 * 1. CONTEXT
 *
 * This section defines the "contexts" that are available in the backend API.
 *
 * These allow you to access things when processing a request, like the database, the session, etc.
 *
 * This helper generates the "internals" for a tRPC context. The API handler and RSC clients each
 * wrap this and provides the required context.
 *
 * @see https://trpc.io/docs/server/context
 */
export const createTRPCContext = async (opts: { headers: Headers }) => {
  return {
    db,
    ...opts,
  };
};

/**
 * 2. INITIALIZATION
 *
 * This is where the tRPC API is initialized, connecting the context and transformer. We also parse
 * ZodErrors so that you get typesafety on the frontend if your procedure fails due to validation
 * errors on the backend.
 */
const t = initTRPC.context<typeof createTRPCContext>().create({
  transformer: superjson,
  errorFormatter({ shape, error }) {
    return {
      ...shape,
      data: {
        ...shape.data,
        zodError:
          error.cause instanceof ZodError ? error.cause.flatten() : null,
      },
    };
  },
});

/**
 * 3. ROUTER & PROCEDURE (THE IMPORTANT BIT)
 *
 * These are the pieces you use to build your tRPC API. You should import these a lot in the
 * "/src/server/api/routers" directory.
 */

/**
 * This is how you create new routers and sub-routers in your tRPC API.
 *
 * @see https://trpc.io/docs/router
 */
export const createTRPCRouter = t.router;

const tracer = trace.getTracer("trpc");
/**
 * Public (unauthenticated) procedure
 *
 * This is the base piece you use to build new queries and mutations on your tRPC API. It does not
 * guarantee that a user querying is authorized, but you can still access user session data if they
 * are logged in.
 */
export const publicProcedure = t.procedure.use(
  experimental_standaloneMiddleware().create((opts) =>
    tracer.startActiveSpan(
      `api/trpc/${opts.type}/${opts.path}`,
      async (span: Span) => {
        const result = await opts.next();
        const output = "data" in result ? result.data : result;
        console.log(
          `api/trpc/${opts.type}/${opts.path}`,
          JSON.stringify(
            {
              input: opts.input ? JSON.stringify(opts.input) : "undefined",
              meta: opts.meta ? JSON.stringify(opts.meta) : "undefined",
              ok: result.ok,
              output: output ? JSON.stringify(output) : "undefined",
              path: opts.path,
              type: opts.type,
            },
            null,
            2,
          ),
        );
        span.setAttributes({
          input: opts.input ? JSON.stringify(opts.input) : "undefined",
          meta: opts.meta ? JSON.stringify(opts.meta) : "undefined",
          ok: result.ok,
          output: output ? JSON.stringify(output) : "undefined",
          path: opts.path,
          type: opts.type,
        });
        span.end();
        return result;
      },
    ),
  ),
);

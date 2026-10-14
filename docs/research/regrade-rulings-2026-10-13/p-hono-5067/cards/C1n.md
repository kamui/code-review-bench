# Card: the exported `parseBody()` reads a plain `Request` body into memory before parsing it

Pull request: honojs/hono #5067. Hono is a web framework for Node, Bun, Deno and Cloudflare Workers. `c.req.parseBody()` is its documented way to read a form or a file upload. It calls a function, `parseBody(request)`, that also accepts the platform's plain `Request` and that a program can import as `hono/utils/body`. "Base" is the commit before the change and "head" the reviewed commit.

## The runs

Hono's own source at both commits, on Node 24.7.0 and Bun 1.3.10. Probes: `probes/C1/`, `probes/C1n/`. All *run*. One streamed upload per fresh process, three processes per figure. Memory is in MiB above the process's baseline.

| A plain `Request` handed to `parseBody()` | Base | Head |
| --- | --- | --- |
| 100 MB upload, peak memory, Node | 313 | 445 to 502 |
| 50 MB upload, peak memory, Node | 161 to 162 | 312 |
| 8 MB upload, peak memory, Node | 34 to 35 | 58 to 59 |
| 100 MB upload, peak memory, Bun | 214 | 313 to 314 |
| 100 MB upload, held after the parse, Node | 145 to 203 | 145 to 202 |
| 100 MB upload, held after the parse, Bun | 114 | 213 to 214 |
| 100 MB upload, time in `parseBody()`, Node | 144 ms | 350 ms |
| 100 MB upload, time in `parseBody()`, Bun | 71 ms | 120 ms |
| urlencoded form of 26 bytes, time, Node | 0.004 ms | 0.009 ms |

The fields and files returned are the same at both commits. After the parse the bytes that were read are no longer reachable on either runtime. On Bun the process lets the extra memory go when the parsed result is released.

The platform's own `request.formData()`, with no Hono code, peaks at 312 to 313 on Node and 213 to 214 on Bun for the 100 MB upload.

One patch for the plain `Request` path, which hands the body stream to the parser without collecting the bytes first, brings memory, and the time for the 100 MB upload, back to the base's, keeps mixed-case media types working on Node and Bun, and passes the project's tests for these files.

No run put the process under a memory limit, and no run shows a failure.

## What the affected person sees

Nothing in the result. A program that hands the function a large upload on a plain `Request` needs more memory at the peak and more time. For a form of a few fields the difference is 0.002 to 0.005 ms.

## Impact facts

- **Who is affected:** a program that imports `parseBody` from `hono/utils/body` and hands it a plain `Request`, and the project's own method-override middleware, which does so for urlencoded forms only.
- **How:** a higher peak and a slower parse for large bodies. Results are unchanged.
- **How stuck:** the holder of a plain `Request` can call the platform's `request.formData()` directly, which uses the base's memory and does not accept a mixed-case media type on Bun.
- **How common:** not measured. One public program written before the pull request has such a call, behind a condition that is false for a platform request with a body on Node and Bun. No program was found that reaches the call with an upload.
- **How the function is offered:** its type and its tests accept a plain `Request`. The documentation does not mention `hono/utils/body`. In 2023 a maintainer wrote of these modules: "While there is a possibility of sudden breaking changes due to its internal use, you can import it from `hono/utils/*`:".
- **Neighbours:** the same lines raise memory for `c.req.parseBody()` itself, where the request also keeps the bytes after the parse.

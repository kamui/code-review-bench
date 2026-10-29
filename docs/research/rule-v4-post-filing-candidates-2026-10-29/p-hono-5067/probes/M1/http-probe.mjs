// Serve one Hono route over a real socket on 127.0.0.1 so curl can send each header exactly as written.
// Usage: <runtime> http-probe.mjs <api.mjs> <port> [node-server entry, for Node]
// The route answers with what c.req.parseBody() returned, or lets the error reach Hono's default handler.
const args = globalThis.Deno?.args ?? process.argv.slice(2)
const [target, port, nodeServer] = args
const { Hono } = await import(new URL('file://' + target).href)

const app = new Hono()
app.post('/', async (c) => c.json(await c.req.parseBody()))

if (globalThis.Bun) {
  Bun.serve({ port: Number(port), hostname: '127.0.0.1', fetch: app.fetch })
} else if (globalThis.Deno) {
  Deno.serve({ port: Number(port), hostname: '127.0.0.1', onListen() {} }, app.fetch)
} else {
  const { serve } = await import(new URL('file://' + nodeServer).href)
  serve({ fetch: app.fetch, port: Number(port), hostname: '127.0.0.1' })
}
console.log('ready')

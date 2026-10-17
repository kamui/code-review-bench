// Run from <hono checkout>/_probe/net-server.ts (see ./net-run.sh). Serves a Hono app over HTTP on 127.0.0.1:PORT.
// Bun: Bun.serve with app.fetch. Node: node:http with a minimal adapter that hands app.fetch a Request whose body is
// the incoming stream (Readable.toWeb); @hono/node-server is not used because it is not among the project's
// dependencies. The handler awaits c.req.parseBody() once and answers with the time that call took.
import { createServer } from 'node:http'
import { Readable } from 'node:stream'
import { Hono } from '../src/index'

declare const Bun: { serve: (options: unknown) => unknown; version: string } | undefined

const PORT = Number(process.env.PORT ?? '8787')
const app = new Hono()
app.post('/upload', async (c) => {
  const start = performance.now()
  const body = await c.req.parseBody()
  const parseMs = performance.now() - start
  const file = body['file']
  return c.json({ parseMs: Math.round(parseMs * 1000) / 1000, fileBytes: file instanceof File ? file.size : -1, title: body['title'] })
})

if (typeof Bun !== 'undefined') {
  Bun.serve({ port: PORT, hostname: '127.0.0.1', fetch: app.fetch, maxRequestBodySize: 1024 * 1024 * 1024 })
} else {
  createServer(async (req, res) => {
    const headers = new Headers()
    for (const [key, value] of Object.entries(req.headers)) {
      if (typeof value === 'string') headers.set(key, value)
    }
    const request = new Request(`http://127.0.0.1:${PORT}${req.url}`, {
      method: req.method,
      headers,
      body: req.method === 'GET' || req.method === 'HEAD' ? undefined : (Readable.toWeb(req) as ReadableStream),
      // @ts-expect-error duplex is needed by node for a streamed body
      duplex: 'half',
    })
    const response = await app.fetch(request)
    res.writeHead(response.status, Object.fromEntries(response.headers))
    res.end(Buffer.from(await response.arrayBuffer()))
  }).listen(PORT, '127.0.0.1')
}
console.log(`listening ${typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`} ${PORT}`)

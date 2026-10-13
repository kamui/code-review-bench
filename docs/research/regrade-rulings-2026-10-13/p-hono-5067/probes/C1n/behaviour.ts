// Run from <hono checkout>/_probe/behaviour.ts (see ../M1/run.sh).
// What parseBody() returns for a platform Request and for a HonoRequest, on one source tree, with small bodies.
import { Hono } from '../src/index'
import { methodOverride } from '../src/middleware/method-override/index'
import { HonoRequest } from '../src/request'
import { parseBody } from '../src/utils/body'

declare const Bun: { version: string } | undefined

const multipart = async (mediaType = 'multipart/form-data') => {
  const form = new FormData()
  form.append('foo', 'bar')
  form.append('file', new File(['original'], 'a.txt', { type: 'text/plain' }))
  const source = new Request('http://localhost/', { method: 'POST', body: form })
  return new Request('http://localhost/', {
    method: 'POST',
    headers: {
      'Content-Type': source.headers.get('Content-Type')!.replace('multipart/form-data', mediaType),
    },
    body: await source.arrayBuffer(),
  })
}
const urlencoded = async (mediaType = 'application/x-www-form-urlencoded') =>
  new Request('http://localhost/', {
    method: 'POST',
    headers: { 'Content-Type': mediaType },
    body: 'foo=bar&baz=qux',
  })

const show = async (value: unknown): Promise<unknown> => {
  if (value instanceof File) {
    return `File(${value.name}:${await value.text()})`
  }
  if (value && typeof value === 'object') {
    const out: Record<string, unknown> = {}
    for (const [key, item] of Object.entries(value)) {
      out[key.length > 40 ? key.slice(0, 40) + '...' : key] = await show(item)
    }
    return out
  }
  return typeof value === 'string' && value.length > 70 ? value.slice(0, 70) + '...' : value
}
const attempt = async (name: string, run: () => Promise<unknown>) => {
  try {
    console.log(`${name}: ${JSON.stringify(await show(await run()))}`)
  } catch (error) {
    console.log(`${name}: THROWS ${(error as Error).name}: ${(error as Error).message}`)
  }
}

console.log(`runtime: ${typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`}`)

console.log('\n## a platform Request handed to the exported parseBody()')
await attempt('multipart', async () => parseBody(await multipart()))
await attempt('urlencoded', async () => parseBody(await urlencoded()))
await attempt('Multipart/Form-Data', async () => parseBody(await multipart('Multipart/Form-Data')))
await attempt('Application/X-WWW-Form-Urlencoded', async () =>
  parseBody(await urlencoded('Application/X-WWW-Form-Urlencoded'))
)
await attempt('a clone of the request, then the original read as text', async () => {
  const request = await urlencoded()
  const body = await parseBody(request.clone())
  return { body, originalText: await request.text() }
})
await attempt('the same Request parsed twice', async () => {
  const request = await urlencoded()
  await parseBody(request)
  return parseBody(request)
})
await attempt("the project's method-override middleware, urlencoded _method=DELETE", async () => {
  const app = new Hono()
  app.use('/posts', methodOverride({ app }))
  app.delete('/posts', (c) => c.text('deleted'))
  app.post('/posts', (c) => c.text('posted'))
  const response = await app.request('/posts', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: '_method=DELETE&id=1',
  })
  return { status: response.status, text: await response.text() }
})

console.log('\n## a HonoRequest: c.req.parseBody()')
await attempt('multipart', async () => new HonoRequest(await multipart()).parseBody())
await attempt('urlencoded', async () => new HonoRequest(await urlencoded()).parseBody())
await attempt('Multipart/Form-Data', async () =>
  new HonoRequest(await multipart('Multipart/Form-Data')).parseBody()
)
await attempt('Application/X-WWW-Form-Urlencoded', async () =>
  new HonoRequest(await urlencoded('Application/X-WWW-Form-Urlencoded')).parseBody()
)
await attempt('text() then parseBody()', async () => {
  const req = new HonoRequest(await urlencoded())
  await req.text()
  return req.parseBody()
})
await attempt('parseBody() then formData()', async () => {
  const req = new HonoRequest(await urlencoded())
  await req.parseBody()
  return Object.fromEntries([...(await req.formData()).entries()])
})
await attempt('parseBody() then text(), urlencoded', async () => {
  const req = new HonoRequest(await urlencoded())
  await req.parseBody()
  return { text: await req.text(), bodyCacheKeys: Object.keys(req.bodyCache).join(',') }
})
await attempt('parseBody() and formData() started together (known problem GT-p2)', async () => {
  const req = new HonoRequest(await urlencoded())
  const [body, form] = await Promise.allSettled([req.parseBody(), req.formData()])
  return {
    parseBody: body.status === 'fulfilled' ? body.value : `rejected: ${(body.reason as Error).message}`,
    formData:
      form.status === 'fulfilled'
        ? Object.fromEntries([...form.value.entries()])
        : `rejected: ${(form.reason as Error).message}`,
  }
})

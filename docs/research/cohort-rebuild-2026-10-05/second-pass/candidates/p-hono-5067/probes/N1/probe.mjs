import { pathToFileURL } from 'node:url'
const { Hono, HonoRequest } = await import(pathToFileURL(process.argv[2]))
const runtime = typeof Bun === 'undefined' ? `Node ${process.version}` : `Bun ${Bun.version}`
async function makeRequest(encoding) {
  if (encoding === 'urlencoded') return new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: 'foo=bar&removed=present' })
  const form = new FormData()
  form.set('foo', 'bar')
  form.set('removed', 'present')
  form.set('upload', new File(['original file'], 'original.txt', { type: 'text/plain' }))
  const encoded = new Request('http://localhost/', { method: 'POST', body: form })
  const contentType = encoded.headers.get('content-type')
  return new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': contentType }, body: await encoded.arrayBuffer() })
}
function view(body) {
  return Object.fromEntries(Object.entries(body).map(([key, value]) => [key, value instanceof File ? { name: value.name, size: value.size } : value]))
}
function errorText(e) { return { error: `${e.name}: ${e.message}`, cause: e.cause ? String(e.cause) : null } }
for (const encoding of ['urlencoded', 'multipart']) {
  const req = new HonoRequest(await makeRequest(encoding))
  const first = await req.parseBody()
  const form1 = await req.formData()
  const second = await req.parseBody()
  const form2 = await req.formData()
  console.log(JSON.stringify({ runtime, encoding, scenario: 'repeat unchanged', first: view(first), second: view(second), sameForm: form1 === form2, sameFileWithinForm: encoding === 'multipart' ? form1.get('upload') === form1.get('upload') : null, sameFile: encoding === 'multipart' ? first.upload === second.upload : null, cacheKeys: Object.keys(req.bodyCache) }))
  const changed = new HonoRequest(await makeRequest(encoding))
  await changed.parseBody()
  const cached = await changed.formData()
  cached.set('foo', 'normalized')
  cached.delete('removed')
  cached.set('added', 'middleware')
  if (encoding === 'multipart') cached.set('upload', new File(['replacement'], 'replacement.txt', { type: 'text/plain' }))
  const parsed = await changed.parseBody()
  const after = await changed.formData()
  console.log(JSON.stringify({ runtime, encoding, scenario: 'mutate between repeated reads', parsed: view(parsed), cachedFooAfter: after.get('foo'), cachedAddedAfter: after.get('added'), cachedRemovedAfter: after.get('removed'), sameForm: cached === after, oldFormStillMutated: cached.get('foo') === 'normalized' }))
  const app = new Hono()
  app.use('*', async (c, next) => {
    await c.req.parseBody()
    const f = await c.req.formData()
    f.set('foo', 'normalized')
    f.delete('removed')
    await next()
  })
  app.post('/', async c => c.json(view(await c.req.parseBody())))
  const response = await app.request(await makeRequest(encoding))
  console.log(JSON.stringify({ runtime, encoding, scenario: 'middleware mutation then handler', status: response.status, body: await response.json() }))
  const reverse = new HonoRequest(await makeRequest(encoding))
  await reverse.formData()
  try { console.log(JSON.stringify({runtime, encoding, scenario: 'GT-p1 control formData first', parsed: view(await reverse.parseBody())})) }
  catch(e) { console.log(JSON.stringify({runtime, encoding, scenario: 'GT-p1 control formData first', ...errorText(e)})) }
}

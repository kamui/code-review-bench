import { pathToFileURL } from 'node:url'
const { Hono, HonoRequest, parseBody, validator, prettyJSON, methodOverride } = await import(pathToFileURL(process.argv[2]))
const { Request: OtherRequest } = await import(new URL('./clone/node_modules/undici/index.js', pathToFileURL(process.argv[2])))
const selected = process.argv[3]
const multipart = '--X\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--X--\r\n'
const request = (contentType, body = 'foo=bar') => new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': contentType }, body })
const attempt = async (fn) => { try { return { ok: true, value: await fn() } } catch (error) { return { ok: false, error: String(error) } } }
const form = async (req) => Object.fromEntries(await req.formData())
const validation = async (contentType, body, prior = false, target = 'form') => {
  const app = new Hono()
  const errors = []
  app.onError((error, c) => { errors.push(String(error)); return 'getResponse' in error ? error.getResponse() : c.text('Internal Server Error', 500) })
  if (prior) app.use('*', async (c, next) => { await c.req.parseBody().catch((error) => errors.push('caught: ' + String(error))); await next() })
  app.post('/', validator(target, (value) => value), (c) => c.json(c.req.valid(target)))
  const response = await app.request(request(contentType, body))
  return { status: response.status, body: await response.text(), errors }
}
const checks = {
  M1: async () => {
    const header = 'Multipart/Form-Data; BOUNDARY=X'
    const controls = {}
    for (const control of ['multipart/form-data; BOUNDARY=X', 'Multipart/Form-Data; boundary=X', 'multipart/form-data; boundary=X']) controls[control] = { native: await attempt(() => form(request(control, multipart))), parseBody: await attempt(() => parseBody(request(control, multipart))), validator: await validation(control, multipart) }
    return { nativeRequest: await attempt(() => form(request(header, multipart))), nativeResponse: await attempt(() => form(new Response(multipart, { headers: { 'Content-Type': header } }))), comparableUndici: await attempt(() => form(new OtherRequest('http://localhost/', { method: 'POST', headers: { 'Content-Type': header }, body: multipart }))), parseBody: await attempt(() => parseBody(request(header, multipart))), validator: await validation(header, multipart), suffix: await validation('application/LD+JSON', '{"foo":"bar"}', false, 'json'), controls }
  },
  M2: async () => ({ direct: await validation('multipart/form-data; boundary=abc', 'garbage'), afterCaught: await validation('multipart/form-data; boundary=abc', 'garbage', true) }),
  M3: async () => ({ malformed: await attempt(() => parseBody(request('multipart/form-data boundary=X', multipart))), native: await attempt(() => form(request('multipart/form-data boundary=X', multipart))), duplicate: await attempt(() => parseBody(request('application/x-www-form-urlencoded, application/x-www-form-urlencoded'))) }),
  M4: async () => {
    const out = {}
    for (const header of ['multipart/form-data ; boundary=X', 'Multipart/Form-Data ; boundary=X', 'multipart/form-data; charset=utf-8; boundary=X', 'multipart/form-data; boundary=X']) out[header] = { native: await attempt(() => form(request(header, multipart))), parseBody: await attempt(() => parseBody(request(header, multipart))), validator: await validation(header, multipart) }
    return out
  },
  M5: async () => {
    let calls = 0
    class CustomRequest extends Request { async formData() { calls++; const value = new FormData(); value.set('custom', 'yes'); return value } }
    const req = new CustomRequest('http://localhost/', { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: 'foo=bar' })
    const result = await parseBody(req)
    return { result, calls, ordinary: await parseBody(request('application/x-www-form-urlencoded')) }
  },
  M6: async () => {
    const out = {}
    for (const header of ['application/x-www-form-urlencoded ; charset=utf-8', 'Application/X-WWW-Form-Urlencoded ; charset=utf-8', 'Application/X-WWW-Form-Urlencoded; charset=utf-8']) out[header] = { native: await attempt(() => form(request(header))), parseBody: await attempt(() => parseBody(request(header))), validator: await validation(header, 'foo=bar'), jsonValidator: await validation(header, 'foo=bar', false, 'json') }
    return out
  },
  P1: async () => {
    const out = {}
    for (const [header, body] of [['Application/X-WWW-Form-Urlencoded', 'foo=bar'], ['Multipart/Form-Data; boundary=X', multipart], ['application/x-www-form-urlencoded', 'foo=bar']]) out[header] = { native: await attempt(() => form(request(header, body))), comparableUndici: await attempt(() => form(new OtherRequest('http://localhost/', { method: 'POST', headers: { 'Content-Type': header }, body }))), direct: await attempt(() => form(new HonoRequest(request(header, body)))), parseBody: await attempt(() => parseBody(new HonoRequest(request(header, body)))), validator: await validation(header, body) }
    return out
  },
  P2: async () => {
    const out = {}
    for (const header of ['application/json', 'Application/JSON']) for (const mode of ['query', 'force']) {
      const app = new Hono()
      app.use('*', prettyJSON(mode === 'force' ? { force: true } : undefined))
      app.get('/', (c) => c.body('{"foo":"bar"}', 200, { 'Content-Type': header }))
      const response = await app.request('/' + (mode === 'query' ? '?pretty' : ''))
      out[header + ':' + mode] = { status: response.status, body: await response.text() }
    }
    return out
  },
  known: async () => {
    const out = {}
    for (const [type, body] of [['multipart/form-data; boundary=X', multipart], ['application/x-www-form-urlencoded', 'foo=bar']]) {
      const prior = new HonoRequest(request(type, body)); await prior.formData()
      const sequential = await attempt(() => prior.parseBody())
      const edit = new HonoRequest(request(type, body)); await edit.parseBody(); (await edit.formData()).set('foo', 'edited')
      const repeated = await attempt(() => edit.parseBody())
      const concurrent = new HonoRequest(request(type, body)); const promises = [concurrent.parseBody(), concurrent.formData()]
      const overlap = await Promise.all(promises.map((p) => attempt(() => p.then((x) => x instanceof FormData ? Object.fromEntries(x) : x))))
      const first = new HonoRequest(request(type, body)); await first.parseBody()
      let parses = 0
      const requestParser = Request.prototype.formData
      const responseParser = Response.prototype.formData
      Request.prototype.formData = function () { parses++; return requestParser.call(this) }
      Response.prototype.formData = function () { parses++; return responseParser.call(this) }
      let repeatedCalls
      try { const repeatedRequest = new HonoRequest(request(type, body)); repeatedCalls = await attempt(async () => { for (let i = 0; i < 5; i++) await repeatedRequest.parseBody(); return { parses } }) }
      finally { Request.prototype.formData = requestParser; Response.prototype.formData = responseParser }
      out[type] = { sequential, repeated, overlap, firstCache: Object.keys(first.bodyCache), retainedRawBytes: first.bodyCache.arrayBuffer ? (await first.bodyCache.arrayBuffer).byteLength : 0, repeatedCalls }
    }
    const app = new Hono(); app.use('*', methodOverride({ app })); app.post('/', (c) => c.text('POST')); app.delete('/', (c) => c.text('DELETE'))
    out.methodOverride = await (await app.request(request('Application/X-WWW-Form-Urlencoded', '_method=DELETE'))).text()
    return out
  },
}
for (const name of selected ? [selected] : Object.keys(checks)) console.log(JSON.stringify({ group: name, runtime: process.versions.bun ? 'Bun ' + process.versions.bun : 'Node ' + process.version, result: await checks[name]() }))

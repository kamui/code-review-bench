// Probe for group M1: multipart headers whose media type starts with
// `multipart/form-data` and continues with something other than `;`.
// Usage: <runtime> probe.mjs <path to api.mjs bundled from the checkout>
// Prints one JSON line per header.
const target = process.argv[2] ?? globalThis.Deno?.args?.[0]
const { Hono, HonoRequest, parseBody, validator } = await import(new URL('file://' + target).href)

const multipart = '--X\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--X--\r\n'
const urlencoded = 'foo=bar'

// [label, Content-Type, body]
const cases = [
  // the candidate's headers
  ['suffix-with-boundary', 'multipart/form-data-extra; boundary=X', multipart],
  ['suffix-no-boundary', 'multipart/form-data-extra', multipart],
  ['comma-with-boundary', 'multipart/form-data,foo; boundary=X', multipart],
  ['comma-no-boundary', 'multipart/form-data,foo', multipart],
  // other headers of the same shape
  ['letter-suffix', 'multipart/form-datax; boundary=X', multipart],
  ['comma-joined-twice', 'multipart/form-data, multipart/form-data; boundary=X', multipart],
  ['mixed-case-suffix', 'Multipart/Form-Data-Extra; boundary=X', multipart],
  // the known problem GT-p6: no `;` before boundary
  ['GT-p6-missing-semicolon', 'multipart/form-data boundary=X', multipart],
  // controls: valid headers
  ['control-valid', 'multipart/form-data; boundary=X', multipart],
  ['control-mixed-case', 'Multipart/Form-Data; boundary=X', multipart],
  ['control-space-before-semicolon', 'multipart/form-data ; boundary=X', multipart],
  ['control-urlencoded', 'application/x-www-form-urlencoded', urlencoded],
  // controls: other media types that are not forms
  ['other-type-multipart-mixed', 'multipart/mixed; boundary=X', multipart],
  ['other-type-text', 'text/plain', urlencoded],
  ['other-type-json', 'application/json', '{"foo":"bar"}'],
  // urlencoded headers of the same shape, for comparison
  ['urlencoded-suffix', 'application/x-www-form-urlencoded-extra', urlencoded],
  ['urlencoded-comma', 'application/x-www-form-urlencoded,foo', urlencoded],
]

const request = (contentType, body) =>
  new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': contentType }, body })
const attempt = async (fn) => {
  try {
    return { ok: true, value: await fn() }
  } catch (error) {
    return { ok: false, error: String(error) }
  }
}

// What an application sees: a route that reads c.req.parseBody() and answers with it.
const throughApp = async (contentType, body) => {
  const app = new Hono()
  const errors = []
  app.onError((error, c) => {
    errors.push(String(error))
    return c.text('Internal Server Error', 500)
  })
  app.post('/', async (c) => c.json(await c.req.parseBody()))
  const response = await app.request(request(contentType, body))
  return { status: response.status, body: await response.text(), errors }
}

// The same route with the validator in front of it.
const throughValidator = async (contentType, body) => {
  const app = new Hono()
  app.onError((error, c) => ('getResponse' in error ? error.getResponse() : c.text('Internal Server Error', 500)))
  app.post('/', validator('form', (value) => value), (c) => c.json(c.req.valid('form')))
  const response = await app.request(request(contentType, body))
  return { status: response.status, body: await response.text() }
}

const runtime = globalThis.Bun
  ? 'Bun ' + Bun.version
  : globalThis.Deno
    ? 'Deno ' + Deno.version.deno
    : 'Node ' + process.version

for (const [label, contentType, body] of cases) {
  const result = {
    // the platform's own reader, called directly on the same request
    platformFormData: await attempt(async () => Object.fromEntries(await request(contentType, body).formData())),
    // the exported parseBody() handed a plain Request
    parseBody: await attempt(() => parseBody(request(contentType, body))),
    // parseBody() on a HonoRequest, as c.req.parseBody() calls it
    honoRequestParseBody: await attempt(() => new HonoRequest(request(contentType, body)).parseBody()),
    app: await throughApp(contentType, body),
    validator: await throughValidator(contentType, body),
  }
  console.log(JSON.stringify({ runtime, label, contentType, result }))
}

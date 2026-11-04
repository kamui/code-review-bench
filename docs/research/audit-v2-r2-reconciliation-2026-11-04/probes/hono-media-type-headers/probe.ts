// Copied into a Hono checkout as probe-w3.ts and bundled from there, so these
// imports resolve to that checkout's own source.
import { Hono } from './src/hono'
import { parseBody } from './src/utils/body'
import { validator } from './src/validator'

const multipartBody = '--X\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--X--\r\n'
const urlencodedBody = 'foo=bar'

const cases: Array<[string, string]> = [
  ['multipart/form-data; boundary=X', multipartBody],
  ['multipart/form-data; charset=utf-8; boundary=X', multipartBody],
  ['multipart/form-data ; boundary=X', multipartBody],
  ['multipart/form-data; boundary=X; charset=utf-8', multipartBody],
  ['multipart/form-data,foo', multipartBody],
  ['multipart/form-data,foo; boundary=X', multipartBody],
  ['application/x-www-form-urlencoded', urlencodedBody],
  ['application/x-www-form-urlencoded; charset=utf-8', urlencodedBody],
  ['application/x-www-form-urlencoded ; charset=utf-8', urlencodedBody],
  ['application/x-www-form-urlencoded;', urlencodedBody],
  ['application/x-www-form-urlencoded; charset', urlencodedBody],
]

const app = new Hono()
app.post(
  '/v',
  validator('form', (value) => value),
  (c) => c.json(c.req.valid('form'))
)
app.post('/p', async (c) => c.json(await c.req.parseBody()))
app.onError((err, c) => c.text(`onError: ${String(err)}`, 500))

const show = async (res: Response) => `${res.status} ${await res.text()}`

for (const [contentType, body] of cases) {
  let direct: string
  try {
    const req = new Request('http://localhost/', {
      method: 'POST',
      headers: { 'Content-Type': contentType },
      body,
    })
    direct = JSON.stringify(await parseBody(req))
  } catch (e) {
    direct = `THROWS ${String(e)}`
  }
  const init = () => ({ method: 'POST', headers: { 'Content-Type': contentType }, body })
  const routeParse = await show(await app.request('/p', init()))
  const routeValidator = await show(await app.request('/v', init()))
  console.log(`## ${JSON.stringify(contentType)}`)
  console.log(`  exported parseBody(Request): ${direct}`)
  console.log(`  route c.req.parseBody():     ${routeParse}`)
  console.log(`  route validator('form'):     ${routeValidator}`)
}

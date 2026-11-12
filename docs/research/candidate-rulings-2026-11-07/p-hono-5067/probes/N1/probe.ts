// Copied into a Hono checkout as probe-n1.ts and bundled from there, so these
// imports resolve to that checkout's own source.
import { Hono } from './src/hono'
import { parseBody } from './src/utils/body'
import { validator } from './src/validator'

const jsonBody = JSON.stringify({ foo: 'bar' })
const badJsonBody = '{"foo": '
const urlencodedBody = 'foo=bar'
const multipartBody = '--X\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--X--\r\n'

const jsonCases: Array<[string, string]> = [
  ['application/json', jsonBody],
  ['application/json; charset=utf-8', jsonBody],
  ['Application/JSON; charset=utf-8', jsonBody],
  ['application/json ; charset=utf-8', jsonBody],
  ['Application/JSON ; charset=utf-8', jsonBody],
  ['application/json ;charset=utf-8', jsonBody],
  ['application/json\t; charset=utf-8', jsonBody],
  ['application/merge-patch+json ; charset=utf-8', jsonBody],
  ['application/json; charset=utf-8', badJsonBody],
  ['application/json ; charset=utf-8', badJsonBody],
]

const formCases: Array<[string, string]> = [
  ['application/x-www-form-urlencoded; charset=utf-8', urlencodedBody],
  ['application/x-www-form-urlencoded ; charset=utf-8', urlencodedBody],
  ['Application/X-WWW-Form-Urlencoded ; charset=utf-8', urlencodedBody],
  ['multipart/form-data; boundary=X', multipartBody],
  ['multipart/form-data ; boundary=X', multipartBody],
]

const app = new Hono()
app.post(
  '/vjson',
  validator('json', (value) => value),
  (c) => c.json(c.req.valid('json'))
)
app.post(
  '/vform',
  validator('form', (value) => value),
  (c) => c.json(c.req.valid('form'))
)
app.post(
  '/vjson-then-raw',
  validator('json', (value) => value),
  async (c) => c.json({ valid: c.req.valid('json'), raw: await c.req.json() })
)
app.post('/json', async (c) => c.json(await c.req.json()))
app.post('/parse', async (c) => c.json(await c.req.parseBody()))
app.onError((err, c) => c.text(`onError: ${String(err)}`, 'status' in err ? (err.status as 400) : 500))

const show = async (res: Response) => `${res.status} ${await res.text()}`
const init = (contentType: string, body: string) => ({
  method: 'POST',
  headers: { 'Content-Type': contentType },
  body,
})

console.log('# validator(json)')
for (const [contentType, body] of jsonCases) {
  let platform: string
  try {
    platform = JSON.stringify(await new Request('http://localhost/', init(contentType, body)).json())
  } catch (e) {
    platform = `THROWS ${String(e)}`
  }
  console.log(`## ${JSON.stringify(contentType)} body=${JSON.stringify(body)}`)
  console.log(`  header as received:           ${JSON.stringify(new Headers({ 'Content-Type': contentType }).get('Content-Type'))}`)
  console.log(`  platform Request.json():      ${platform}`)
  console.log(`  route c.req.json():           ${await show(await app.request('/json', init(contentType, body)))}`)
  console.log(`  route c.req.parseBody():      ${await show(await app.request('/parse', init(contentType, body)))}`)
  console.log(`  route validator('json'):      ${await show(await app.request('/vjson', init(contentType, body)))}`)
  console.log(`  validator then c.req.json():  ${await show(await app.request('/vjson-then-raw', init(contentType, body)))}`)
}

console.log('# validator(form)')
for (const [contentType, body] of formCases) {
  let direct: string
  try {
    direct = JSON.stringify(await parseBody(new Request('http://localhost/', init(contentType, body))))
  } catch (e) {
    direct = `THROWS ${String(e)}`
  }
  console.log(`## ${JSON.stringify(contentType)}`)
  console.log(`  exported parseBody(Request):  ${direct}`)
  console.log(`  route validator('form'):      ${await show(await app.request('/vform', init(contentType, body)))}`)
  console.log(`  route validator('json'):      ${await show(await app.request('/vjson', init(contentType, body)))}`)
}

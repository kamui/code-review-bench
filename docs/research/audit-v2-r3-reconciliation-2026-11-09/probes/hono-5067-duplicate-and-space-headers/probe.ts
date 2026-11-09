// Copied into a Hono checkout as probe-r3e.ts and bundled from there, so these
// imports resolve to that checkout's own source.
import { Hono } from './src/hono'
import { parseBody } from './src/utils/body'
import { validator } from './src/validator'

const urlencodedBody = 'foo=bar'
const multipartBody = '--X\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--X--\r\n'
const jsonBody = '{"foo":"bar"}'

const cases: Array<[string, string]> = [
  // unit 21: the comment's duplicated urlencoded header, joined as one value
  ['application/x-www-form-urlencoded, application/x-www-form-urlencoded', urlencodedBody],
  // unit 21 context: the comment's other example (GT-p6)
  ['multipart/form-data boundary=X', multipartBody],
  // unit 23: the comment's header
  ['Application/X-WWW-Form-Urlencoded ; charset=utf-8', urlencodedBody],
  // unit 23 context: the same header without the space, and a JSON header with the space (GT-p8)
  ['Application/X-WWW-Form-Urlencoded; charset=utf-8', urlencodedBody],
  ['application/json ; charset=utf-8', jsonBody],
  ['application/json; charset=utf-8', jsonBody],
]

const app = new Hono()
app.post('/form', validator('form', (value) => value), (c) => c.json(c.req.valid('form')))
app.post('/json', validator('json', (value) => value), (c) => c.json(c.req.valid('json')))
app.post('/p', async (c) => c.json(await c.req.parseBody()))
app.onError((err, c) => c.text(`onError: ${String(err)}`, 500))

const show = async (res: Response) => `${res.status} ${await res.text()}`

const nativeFormData = async (headers: Headers, body: string) => {
  try {
    const fd = await new Request('http://localhost/', { method: 'POST', headers, body }).formData()
    return JSON.stringify([...fd.entries()])
  } catch (e) {
    return `THROWS ${String(e)}`
  }
}

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
  console.log(`## ${JSON.stringify(contentType)}`)
  console.log(`  runtime Request.formData():  ${await nativeFormData(new Headers({ 'Content-Type': contentType }), body)}`)
  console.log(`  exported parseBody(Request): ${direct}`)
  console.log(`  route c.req.parseBody():     ${await show(await app.request('/p', init()))}`)
  console.log(`  route validator('form'):     ${await show(await app.request('/form', init()))}`)
  console.log(`  route validator('json'):     ${await show(await app.request('/json', init()))}`)
}

// The duplicated header as two appended lines, which Headers joins with ", "
const twice = new Headers()
twice.append('Content-Type', 'application/x-www-form-urlencoded')
twice.append('Content-Type', 'application/x-www-form-urlencoded')
console.log(`## two appended Content-Type lines -> ${JSON.stringify(twice.get('Content-Type'))}`)
console.log(`  runtime Request.formData():  ${await nativeFormData(twice, urlencodedBody)}`)
let twiceDirect: string
try {
  twiceDirect = JSON.stringify(
    await parseBody(new Request('http://localhost/', { method: 'POST', headers: twice, body: urlencodedBody }))
  )
} catch (e) {
  twiceDirect = `THROWS ${String(e)}`
}
console.log(`  exported parseBody(Request): ${twiceDirect}`)

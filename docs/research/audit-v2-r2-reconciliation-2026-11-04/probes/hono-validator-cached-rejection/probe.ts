// Copied into a Hono checkout as probe-w3.ts and bundled from there.
import { Hono } from './src/hono'
import { validator } from './src/validator'

const validMultipart = '--X\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--X--\r\n'

type Scenario = { name: string; contentType: string; body: string; before: 'none' | 'parseBody' | 'formData+parseBody' }
const scenarios: Scenario[] = [
  { name: 'malformed multipart, validator alone', contentType: 'multipart/form-data; boundary=abc', body: 'garbage', before: 'none' },
  { name: 'malformed multipart, caught parseBody() first (PE-p3 trigger)', contentType: 'multipart/form-data; boundary=abc', body: 'garbage', before: 'parseBody' },
  { name: 'valid multipart, formData() then caught parseBody() first (the comment\'s order)', contentType: 'multipart/form-data; boundary=X', body: validMultipart, before: 'formData+parseBody' },
  { name: 'valid urlencoded, formData() then caught parseBody() first (the comment\'s order)', contentType: 'application/x-www-form-urlencoded', body: 'foo=bar', before: 'formData+parseBody' },
]

for (const s of scenarios) {
  const app = new Hono()
  let earlier = ''
  app.use('*', async (c, next) => {
    if (s.before === 'formData+parseBody') {
      await c.req.formData()
    }
    if (s.before !== 'none') {
      try {
        earlier = `parseBody() returned ${JSON.stringify(await c.req.parseBody())}`
      } catch (e) {
        earlier = `parseBody() threw ${String(e)} (caught)`
      }
    }
    await next()
  })
  app.post('/', validator('form', (value) => value), (c) => c.json(c.req.valid('form')))
  app.onError((err, c) => c.text(`onError ${err.constructor.name}: ${String(err)}`, 'getResponse' in err ? (err as { status: 400 }).status : 500))
  const res = await app.request('/', { method: 'POST', headers: { 'Content-Type': s.contentType }, body: s.body })
  console.log(`## ${s.name}`)
  if (earlier) console.log(`  middleware: ${earlier}`)
  console.log(`  validator('form') response: ${res.status} ${await res.text()}`)
}

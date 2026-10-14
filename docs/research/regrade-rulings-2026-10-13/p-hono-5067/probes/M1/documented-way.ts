// Run from <hono checkout>/_probe/documented-way.ts (see ./run.sh).
// The documented ways to read a form once: c.req.parseBody() in the handler, and c.req.valid('form') after
// validator('form'). Prints what each returns and how often the platform's form parser was entered.
import { Hono } from '../src/index'
import { validator } from '../src/validator/index'

declare const Bun: { version: string } | undefined

let parses = 0
for (const proto of [Request.prototype, Response.prototype]) {
  const original = proto.formData
  proto.formData = function (this: Request | Response) {
    parses++
    return original.call(this)
  }
}

const app = new Hono()
app.post('/entry', async (c) => c.json(await c.req.parseBody()))
app.post(
  '/posts',
  validator('form', (value, c) => {
    const body = value['body']
    if (!body || typeof body !== 'string') {
      return c.text('Invalid!', 400)
    }
    return { body }
  }),
  (c) => c.json(c.req.valid('form'))
)

console.log(`runtime: ${typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`}`)
for (const path of ['/entry', '/posts']) {
  parses = 0
  const response = await app.request(path, {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: 'body=hello',
  })
  console.log(`POST ${path}: HTTP ${response.status} ${await response.text()} | platform parses: ${parses}`)
}

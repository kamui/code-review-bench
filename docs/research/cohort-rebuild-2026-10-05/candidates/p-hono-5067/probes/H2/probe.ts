// Run from <hono checkout>/_probe/probe.ts (see ../run.sh). Imports the framework source of that checkout.
import { connect } from 'node:net'
import { Hono } from '../src/index'
import { methodOverride } from '../src/middleware/method-override/index'
import { HonoRequest } from '../src/request'
import { parseBody } from '../src/utils/body'
import { validator } from '../src/validator/index'

declare const Bun:
  | { serve: (options: { port: number; fetch: (req: Request) => unknown }) => { port: number; stop: () => void } }
  | undefined

const outcome = async (run: () => Promise<unknown>) => {
  try {
    return `ok ${JSON.stringify(await run())}`
  } catch (error) {
    return error instanceof Error ? `THREW ${error.name}: ${error.message}` : `THREW ${String(error)}`
  }
}

const BOUNDARY = '----probeBoundary'
const multipartBody =
  `--${BOUNDARY}\r\n` + 'Content-Disposition: form-data; name="foo"\r\n\r\n' + 'bar\r\n' + `--${BOUNDARY}--\r\n`

const headerValues: { label: string; contentType: string; body: string }[] = [
  { label: 'control: plain urlencoded', contentType: 'application/x-www-form-urlencoded', body: 'foo=bar' },
  {
    label: 'control: urlencoded with a parameter',
    contentType: 'application/x-www-form-urlencoded; charset=UTF-8',
    body: 'foo=bar',
  },
  {
    label: 'two identical urlencoded headers, joined by Headers',
    contentType: 'application/x-www-form-urlencoded, application/x-www-form-urlencoded',
    body: 'foo=bar',
  },
  {
    label: 'urlencoded with a comma before the parameter',
    contentType: 'application/x-www-form-urlencoded,charset=UTF-8',
    body: 'foo=bar',
  },
  {
    label: 'two identical urlencoded headers that each carry a parameter',
    contentType:
      'application/x-www-form-urlencoded; charset=UTF-8, application/x-www-form-urlencoded; charset=UTF-8',
    body: 'foo=bar',
  },
  {
    label: 'two identical multipart headers',
    contentType: `multipart/form-data; boundary=${BOUNDARY}, multipart/form-data; boundary=${BOUNDARY}`,
    body: multipartBody,
  },
  {
    label: 'a different type first, urlencoded last',
    contentType: 'text/plain, application/x-www-form-urlencoded',
    body: 'foo=bar',
  },
]

console.log('=== 1. One request object per call; each line is one way of reading the same request ===')
for (const { label, contentType, body } of headerValues) {
  const make = () => new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': contentType }, body })
  console.log(`\n${label}\n  Content-Type: ${contentType}`)
  console.log(
    `  runtime Request.formData():        ${await outcome(async () => [...(await make().formData()).entries()])}`
  )
  console.log(`  hono parseBody(Request):           ${await outcome(() => parseBody(make()))}`)
  console.log(`  hono c.req.parseBody():            ${await outcome(() => new HonoRequest(make()).parseBody())}`)
  console.log(`  hono c.req.formData():             ${await outcome(async () => [...(await new HonoRequest(make()).formData()).entries()])}`)
}

console.log('\n=== 2. Through a Hono app, duplicate headers built with Headers.append ===')
const app = new Hono()
app.use('/posts', methodOverride({ app }))
app.post('/form', async (c) =>
  c.json({ contentTypeSeenByHono: c.req.header('content-type'), parseBody: await c.req.parseBody() })
)
app.post(
  '/validated',
  validator('form', (value) => value),
  (c) => c.json({ validatorForm: c.req.valid('form') })
)
app.post('/posts', (c) => c.text('handler for POST ran (method override did NOT apply)'))
app.delete('/posts', (c) => c.text('handler for DELETE ran (method override applied)'))

const duplicateHeaders = () => {
  const headers = new Headers()
  headers.append('Content-Type', 'application/x-www-form-urlencoded')
  headers.append('Content-Type', 'application/x-www-form-urlencoded')
  return headers
}
for (const [path, body] of [
  ['/form', 'foo=bar'],
  ['/validated', 'foo=bar'],
  ['/posts', '_method=DELETE'],
] as const) {
  const res = await app.request(path, { method: 'POST', headers: duplicateHeaders(), body })
  console.log(`POST ${path} body=${body}: HTTP ${res.status} ${await res.text()}`)
}

console.log('\n=== 3. Over a real socket: a client sends two Content-Type header lines ===')
const sendRaw = (port: number, path: string, body: string) =>
  new Promise<string>((resolve, reject) => {
    const socket = connect(port, '127.0.0.1', () => {
      socket.write(
        `POST ${path} HTTP/1.1\r\n` +
          'Host: localhost\r\n' +
          'Content-Type: application/x-www-form-urlencoded\r\n' +
          'Content-Type: application/x-www-form-urlencoded\r\n' +
          `Content-Length: ${body.length}\r\n` +
          'Connection: close\r\n\r\n' +
          body
      )
    })
    let response = ''
    socket.on('data', (chunk) => (response += chunk.toString()))
    socket.on('end', () => resolve(response))
    socket.on('error', reject)
  })

const summarize = (raw: string) => {
  const statusLine = raw.split('\r\n')[0]
  const bodyText = raw.split('\r\n\r\n').slice(1).join('\r\n\r\n').trim()
  return `${statusLine} | ${bodyText}`
}

if (typeof Bun !== 'undefined') {
  const server = Bun.serve({ port: 0, fetch: (req) => app.fetch(req) })
  console.log('server: Bun.serve')
  console.log(`POST /form  -> ${summarize(await sendRaw(server.port, '/form', 'foo=bar'))}`)
  console.log(`POST /posts -> ${summarize(await sendRaw(server.port, '/posts', '_method=DELETE'))}`)
  server.stop()
} else {
  const { serve } = await import('@hono/node-server')
  const port = await new Promise<number>((resolve) => {
    const server = serve({ fetch: app.fetch, port: 0 }, (info) => resolve(info.port))
    setTimeout(() => server.close(), 3000).unref()
  })
  console.log('server: @hono/node-server on node:http')
  console.log(`POST /form  -> ${summarize(await sendRaw(port, '/form', 'foo=bar'))}`)
  console.log(`POST /posts -> ${summarize(await sendRaw(port, '/posts', '_method=DELETE'))}`)
  process.exit(0)
}

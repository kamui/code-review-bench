// Run from <hono checkout>/_probe/time.ts (see ./run.sh).
// Times c.req.parseBody() on one request whose form is read more than once.
//   SCENARIO=repeat     the handler awaits c.req.parseBody() CALLS times in a row
//   SCENARIO=validator  validator('form') runs first, then the handler awaits c.req.parseBody() CALLS times
//   SCENARIO=formdata   the handler awaits c.req.formData() once, then c.req.parseBody() CALLS times
// BODY=urlencoded-small | multipart-small | multipart-file (with FILE_BYTES)
// REQUESTS requests are sent in one process after WARMUP unmeasured ones. Each line of output is one process.
// COUNT=1 also counts how often the platform's form parser is entered (Request.prototype.formData and
// Response.prototype.formData are wrapped by a counter that calls the original). Timings are taken with COUNT unset.
import { Hono } from '../src/index'
import { validator } from '../src/validator/index'

const SCENARIO = process.env.SCENARIO ?? 'repeat'
const CALLS = Number(process.env.CALLS ?? '2')
const BODY = process.env.BODY ?? 'multipart-file'
const FILE_BYTES = Number(process.env.FILE_BYTES ?? String(8 * 1024 * 1024))
const REQUESTS = Number(process.env.REQUESTS ?? '10')
const WARMUP = Number(process.env.WARMUP ?? '2')
const COUNT = process.env.COUNT === '1'
const LABEL = process.env.LABEL ?? ''
const BOUNDARY = '----probeBoundary'

let platformParses = 0
if (COUNT) {
  for (const proto of [Request.prototype, Response.prototype]) {
    const original = proto.formData
    proto.formData = function (this: Request | Response) {
      platformParses++
      return original.call(this)
    }
  }
}

const buildBody = (): { body: Uint8Array | string; contentType: string } => {
  if (BODY === 'urlencoded-small') {
    return { body: 'title=hello&message=world', contentType: 'application/x-www-form-urlencoded' }
  }
  const encoder = new TextEncoder()
  const head = encoder.encode(
    `--${BOUNDARY}\r\n` + 'Content-Disposition: form-data; name="title"\r\n\r\nhello\r\n'
  )
  const tail = encoder.encode(`--${BOUNDARY}--\r\n`)
  if (BODY === 'multipart-small') {
    const field = encoder.encode(
      `--${BOUNDARY}\r\n` + 'Content-Disposition: form-data; name="message"\r\n\r\nworld\r\n'
    )
    const bytes = new Uint8Array(head.length + field.length + tail.length)
    bytes.set(head, 0)
    bytes.set(field, head.length)
    bytes.set(tail, head.length + field.length)
    return { body: bytes, contentType: `multipart/form-data; boundary=${BOUNDARY}` }
  }
  const fileHead = encoder.encode(
    `--${BOUNDARY}\r\n` +
      'Content-Disposition: form-data; name="file"; filename="big.bin"\r\n' +
      'Content-Type: application/octet-stream\r\n\r\n'
  )
  const fileTail = encoder.encode('\r\n')
  const bytes = new Uint8Array(
    head.length + fileHead.length + FILE_BYTES + fileTail.length + tail.length
  )
  let at = 0
  bytes.set(head, at)
  at += head.length
  bytes.set(fileHead, at)
  at += fileHead.length
  bytes.fill(0x61, at, at + FILE_BYTES)
  at += FILE_BYTES
  bytes.set(fileTail, at)
  at += fileTail.length
  bytes.set(tail, at)
  return { body: bytes, contentType: `multipart/form-data; boundary=${BOUNDARY}` }
}

const { body, contentType } = buildBody()

type Sample = { calls: number[]; before: number }
let sample: Sample = { calls: [], before: 0 }
let lastKeys: string[] = []
let lastTitle = ''
let fileBytes = -1
let sameCachedForm: boolean | null = null

const readCalls = async (c: {
  req: {
    parseBody: () => Promise<Record<string, unknown>>
    bodyCache: Record<string, unknown>
  }
}) => {
  const forms: unknown[] = []
  let parsed: Record<string, unknown> = {}
  for (let i = 0; i < CALLS; i++) {
    const start = performance.now()
    parsed = await c.req.parseBody()
    sample.calls.push(performance.now() - start)
    forms.push(await c.req.bodyCache.formData)
  }
  lastKeys = Object.keys(c.req.bodyCache)
  lastTitle = String(parsed['title'])
  const file = parsed['file']
  fileBytes = file instanceof File ? file.size : -1
  sameCachedForm = forms.every((form) => form === forms[0])
}

const app = new Hono()
app.post('/repeat', async (c) => {
  await readCalls(c)
  return c.text('ok')
})
app.post(
  '/validator',
  async (c, next) => {
    const start = performance.now()
    const run = validator('form', (value) => value)
    // The validator awaits next(); stop its clock when the handler starts.
    return run(c, async () => {
      sample.before = performance.now() - start
      await next()
    })
  },
  async (c) => {
    await readCalls(c)
    return c.text('ok')
  }
)
app.post('/formdata', async (c) => {
  const start = performance.now()
  await c.req.formData()
  sample.before = performance.now() - start
  await readCalls(c)
  return c.text('ok')
})

const send = async () => {
  sample = { calls: [], before: 0 }
  const start = performance.now()
  const response = await app.request(`/${SCENARIO}`, {
    method: 'POST',
    headers: { 'Content-Type': contentType },
    body,
  })
  const text = await response.text()
  const total = performance.now() - start
  if (response.status !== 200 || text !== 'ok') {
    throw new Error(`HTTP ${response.status} ${text}`)
  }
  return { total, calls: sample.calls, before: sample.before }
}

for (let i = 0; i < WARMUP; i++) {
  await send()
}
platformParses = 0
const totals: number[] = []
const before: number[] = []
const perCall: number[][] = Array.from({ length: CALLS }, () => [])
for (let i = 0; i < REQUESTS; i++) {
  const result = await send()
  totals.push(result.total)
  before.push(result.before)
  result.calls.forEach((ms, index) => perCall[index].push(ms))
}

const stats = (values: number[]) => {
  const sorted = [...values].sort((a, b) => a - b)
  const round = (value: number) => Math.round(value * 1000) / 1000
  return {
    median: round(sorted[Math.floor(sorted.length / 2)]),
    min: round(sorted[0]),
    max: round(sorted[sorted.length - 1]),
    mean: round(values.reduce((sum, value) => sum + value, 0) / values.length),
  }
}

declare const Bun: { version: string } | undefined
console.log(
  JSON.stringify({
    label: LABEL,
    runtime: typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`,
    scenario: SCENARIO,
    body: BODY,
    fileBytes: BODY === 'multipart-file' ? FILE_BYTES : 0,
    bodyBytes: typeof body === 'string' ? body.length : body.length,
    calls: CALLS,
    requests: REQUESTS,
    requestMs: stats(totals),
    beforeMs: SCENARIO === 'repeat' ? null : stats(before),
    callMs: perCall.map((values) => stats(values)),
    platformParsesPerRequest: COUNT ? platformParses / REQUESTS : null,
    bodyCacheKeys: lastKeys,
    sameCachedFormAcrossCalls: sameCachedForm,
    title: lastTitle,
    fileBytesParsed: fileBytes,
  })
)

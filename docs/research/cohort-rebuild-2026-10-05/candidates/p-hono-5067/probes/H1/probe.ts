// Run from <hono checkout>/_probe/probe.ts (see run.sh). Imports the framework source of that checkout.
import { Hono } from '../src/index'
import { HonoRequest } from '../src/request'

const MULTIPART_BOUNDARY = '----probeBoundary'
const multipartBody =
  `--${MULTIPART_BOUNDARY}\r\n` +
  'Content-Disposition: form-data; name="foo"\r\n\r\n' +
  'bar\r\n' +
  `--${MULTIPART_BOUNDARY}--\r\n`

const kinds = {
  urlencoded: { contentType: 'application/x-www-form-urlencoded', body: 'foo=bar' },
  multipart: {
    contentType: `multipart/form-data; boundary=${MULTIPART_BOUNDARY}`,
    body: multipartBody,
  },
} as const

const newRawRequest = (kind: keyof typeof kinds) =>
  new Request('http://localhost/', {
    method: 'POST',
    headers: { 'Content-Type': kinds[kind].contentType },
    body: kinds[kind].body,
  })

const describe = (settled: PromiseSettledResult<unknown>) => {
  if (settled.status === 'rejected') {
    const reason = settled.reason as Error
    return `REJECTED ${reason?.name}: ${reason?.message}`
  }
  const value = settled.value
  if (value instanceof FormData) {
    return `ok FormData ${JSON.stringify([...value.entries()])}`
  }
  return `ok ${JSON.stringify(value)}`
}

type Scenario = {
  name: string
  run: (req: HonoRequest) => Promise<PromiseSettledResult<unknown>[]>
  labels: string[]
}

const scenarios: Scenario[] = [
  {
    name: 'A. same tick, parseBody() started first, then formData()',
    labels: ['parseBody', 'formData'],
    run: (req) => Promise.allSettled([req.parseBody(), req.formData()]),
  },
  {
    name: 'B. same tick, formData() started first, then parseBody()',
    labels: ['formData', 'parseBody'],
    run: (req) => Promise.allSettled([req.formData(), req.parseBody()]),
  },
  {
    name: 'C. same tick, two parseBody() calls',
    labels: ['parseBody#1', 'parseBody#2'],
    run: (req) => Promise.allSettled([req.parseBody(), req.parseBody()]),
  },
  {
    name: 'D. awaited in sequence, parseBody() then formData() (control)',
    labels: ['parseBody', 'formData'],
    run: async (req) => {
      const first = await Promise.allSettled([req.parseBody()])
      const second = await Promise.allSettled([req.formData()])
      return [...first, ...second]
    },
  },
  {
    name: 'E. awaited in sequence, arrayBuffer() then formData() (older weakness, control)',
    labels: ['arrayBuffer', 'formData'],
    run: async (req) => {
      const first = await Promise.allSettled([req.arrayBuffer().then((b) => `${b.byteLength} bytes`)])
      const second = await Promise.allSettled([req.formData()])
      return [...first, ...second]
    },
  },
  {
    name: 'F. same tick, parseBody() started first, then text()',
    labels: ['parseBody', 'text'],
    run: (req) => Promise.allSettled([req.parseBody(), req.text()]),
  },
]

for (const kind of Object.keys(kinds) as (keyof typeof kinds)[]) {
  console.log(`\n=== ${kind} (Content-Type: ${kinds[kind].contentType}) ===`)
  for (const scenario of scenarios) {
    const results = await scenario.run(new HonoRequest(newRawRequest(kind)))
    console.log(scenario.name)
    results.forEach((result, index) => {
      console.log(`    ${scenario.labels[index]}: ${describe(result)}`)
    })
  }
}

console.log('\n=== through a Hono app: what the HTTP client gets ===')
const app = new Hono()
app.post('/concurrent', async (c) => {
  const [body, form] = await Promise.all([c.req.parseBody(), c.req.formData()])
  return c.json({ body, formFoo: form.get('foo') })
})
// A middleware starts a body read without awaiting it before calling next(); the handler reads the other way.
app.post(
  '/middleware',
  async (c, next) => {
    const pending = c.req.parseBody()
    await next()
    await pending
  },
  async (c) => {
    const form = await c.req.formData()
    return c.json({ formFoo: form.get('foo') })
  }
)
for (const path of ['/concurrent', '/middleware']) {
  for (const kind of Object.keys(kinds) as (keyof typeof kinds)[]) {
    const res = await app.request(path, {
      method: 'POST',
      headers: { 'Content-Type': kinds[kind].contentType },
      body: kinds[kind].body,
    })
    console.log(`POST ${path} ${kind}: HTTP ${res.status} ${await res.text()}`)
  }
}

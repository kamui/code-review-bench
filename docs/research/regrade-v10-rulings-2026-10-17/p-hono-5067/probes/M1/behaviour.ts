// Run from <hono checkout>/_probe/behaviour.ts (see ./run.sh).
// What each read returns on one source tree, for the cases the patches could change: the first read with a lower-case
// and a mixed-case media type, the known problems' orders of reads, and a second parseBody(). A counter wraps the
// platform's Request.prototype.formData and Response.prototype.formData and calls the original.
import { Hono } from '../src/index'
import { parseBody } from '../src/utils/body'

declare const Bun: { version: string } | undefined

let parses = 0
for (const proto of [Request.prototype, Response.prototype]) {
  const original = proto.formData
  proto.formData = function (this: Request | Response) {
    parses++
    return original.call(this)
  }
}

const show = async (value: unknown): Promise<unknown> => {
  if (value instanceof File) {
    return `File(${value.name}:${await value.text()})`
  }
  if (value instanceof FormData) {
    return show(Object.fromEntries(value.entries()))
  }
  if (value && typeof value === 'object') {
    const out: Record<string, unknown> = {}
    for (const [key, item] of Object.entries(value)) {
      out[key.length > 40 ? key.slice(0, 40) + '...' : key] = await show(item)
    }
    return out
  }
  return typeof value === 'string' && value.length > 60 ? value.slice(0, 60) + '...' : value
}

const bodies = async (mediaCase: 'lower' | 'mixed') => {
  const form = new FormData()
  form.append('foo', 'bar')
  form.append('file', new File(['original'], 'a.txt', { type: 'text/plain' }))
  const source = new Request('http://localhost/', { method: 'POST', body: form })
  const multipartType = source.headers.get('Content-Type')!
  const multipartBytes = await source.arrayBuffer()
  return {
    multipart: {
      contentType:
        mediaCase === 'lower'
          ? multipartType
          : multipartType.replace('multipart/form-data', 'Multipart/Form-Data'),
      body: () => multipartBytes.slice(0),
    },
    urlencoded: {
      contentType:
        mediaCase === 'lower' ? 'application/x-www-form-urlencoded' : 'Application/X-WWW-Form-Urlencoded',
      body: () => 'foo=bar',
    },
  }
}

type Reader = (c: any) => Promise<unknown>
const cases: [string, Reader][] = [
  ['parseBody()', async (c) => c.req.parseBody()],
  [
    'parseBody() twice',
    async (c) => {
      await c.req.parseBody()
      return c.req.parseBody()
    },
  ],
  [
    'formData() then parseBody()  [GT-p1 first case]',
    async (c) => {
      await c.req.formData()
      return c.req.parseBody()
    },
  ],
  [
    'parseBody(), edit formData(), parseBody()  [GT-p1 widened case]',
    async (c) => {
      await c.req.parseBody()
      const form = await c.req.formData()
      form.set('foo', 'edited')
      return c.req.parseBody()
    },
  ],
  [
    'parseBody() and formData() started together  [GT-p2]',
    async (c) => {
      const [parsed, form] = await Promise.all([c.req.parseBody(), c.req.formData()])
      return { parsed, form: await show(form) }
    },
  ],
  [
    'text() then parseBody()',
    async (c) => {
      await c.req.text()
      return c.req.parseBody()
    },
  ],
]

console.log(`runtime: ${typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`}`)
for (const mediaCase of ['lower', 'mixed'] as const) {
  const made = await bodies(mediaCase)
  for (const [encoding, { contentType, body }] of Object.entries(made)) {
    console.log(`\n## ${encoding}, media type ${mediaCase} case: ${contentType.split(';')[0]}`)
    for (const [name, read] of cases) {
      if (mediaCase === 'mixed' && name !== 'parseBody()') {
        continue
      }
      const app = new Hono()
      app.post('/', async (c) => {
        parses = 0
        try {
          const result = await read(c)
          return c.text(`${JSON.stringify(await show(result))} | platform parses: ${parses}`)
        } catch (error) {
          return c.text(
            `THROWS ${(error as Error).name}: ${(error as Error).message} | platform parses: ${parses}`
          )
        }
      })
      const response = await app.request('/', {
        method: 'POST',
        headers: { 'Content-Type': contentType },
        body: body(),
      })
      console.log(`${name}: HTTP ${response.status} ${await response.text()}`)
    }
    parses = 0
    try {
      const plain = await parseBody(
        new Request('http://localhost/', {
          method: 'POST',
          headers: { 'Content-Type': contentType },
          body: body(),
        })
      )
      console.log(`exported parseBody(plain Request): ${JSON.stringify(await show(plain))} | platform parses: ${parses}`)
    } catch (error) {
      console.log(`exported parseBody(plain Request): THROWS ${(error as Error).message}`)
    }
  }
}

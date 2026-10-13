// Run from <hono checkout>/_probe/behaviour.ts (see ./run.sh).
// Every case of known problem GT-p1 and of the repeated parse, on one source tree. For each case it prints what
// parseBody() returned and how often the platform's form parser was entered (Request.prototype.formData and
// Response.prototype.formData are wrapped by a counter that calls the original; nothing else is replaced).
import { Hono } from '../src/index'
import { HonoRequest } from '../src/request'
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

const multipart = (mediaType = 'multipart/form-data') => {
  const form = new FormData()
  form.append('foo', 'bar')
  form.append('removed', 'present')
  form.append('file', new File(['original'], 'a.txt', { type: 'text/plain' }))
  const source = new Request('http://localhost/', { method: 'POST', body: form })
  return async () =>
    new Request('http://localhost/', {
      method: 'POST',
      headers: {
        'Content-Type': source.headers
          .get('Content-Type')!
          .replace('multipart/form-data', mediaType),
      },
      body: await source.clone().arrayBuffer(),
    })
}
const urlencoded = (mediaType = 'application/x-www-form-urlencoded') => {
  return async () =>
    new Request('http://localhost/', {
      method: 'POST',
      headers: { 'Content-Type': mediaType },
      body: 'foo=bar&removed=present',
    })
}

const show = async (value: unknown): Promise<unknown> => {
  if (value instanceof File) {
    return `File(${value.name}:${await value.text()})`
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

const attempt = async (name: string, run: () => Promise<unknown>) => {
  parses = 0
  try {
    const result = await run()
    console.log(`${name}: ${JSON.stringify(await show(result))} | platform parses: ${parses}`)
  } catch (error) {
    console.log(
      `${name}: THROWS ${(error as Error).name}: ${(error as Error).message} | platform parses: ${parses}`
    )
  }
}

console.log(`runtime: ${typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`}`)

for (const [encoding, make] of [
  ['multipart', multipart()],
  ['urlencoded', urlencoded()],
] as const) {
  console.log(`\n## ${encoding}`)

  await attempt('first read: parseBody()', async () => {
    const req = new HonoRequest(await make())
    return req.parseBody()
  })

  await attempt('GT-p1 first case: formData() then parseBody()', async () => {
    const req = new HonoRequest(await make())
    await req.formData()
    return req.parseBody()
  })

  await attempt('GT-p1 first case, afterwards: formData() again', async () => {
    const req = new HonoRequest(await make())
    await req.formData()
    try {
      await req.parseBody()
    } catch {
      // the next read is what is shown
    }
    const form = await req.formData()
    return Object.fromEntries([...form.entries()])
  })

  await attempt('GT-p1 widened case: parseBody(), edit the form from formData(), parseBody()', async () => {
    const req = new HonoRequest(await make())
    await req.parseBody()
    const form = await req.formData()
    form.set('foo', 'normalized')
    form.delete('removed')
    form.append('added', 'middleware')
    return req.parseBody()
  })

  await attempt('repeated parse: parseBody() three times, no edit', async () => {
    const req = new HonoRequest(await make())
    const first = await req.parseBody()
    const firstForm = await req.bodyCache.formData
    await req.parseBody()
    const third = await req.parseBody()
    const thirdForm = await req.bodyCache.formData
    return {
      third,
      sameRememberedForm: firstForm === thirdForm,
      sameFileObject: first['file'] === undefined ? 'no file' : first['file'] === third['file'],
      bodyCacheKeys: Object.keys(req.bodyCache).join(','),
    }
  })

  await attempt("repeated parse: validator('form') then parseBody() in the handler", async () => {
    const app = new Hono()
    let cacheKeys = ''
    let sameForm: boolean | null = null
    app.post(
      '/',
      validator('form', (value) => value),
      async (c) => {
        const validated = await c.req.bodyCache.formData
        const body = await c.req.parseBody()
        sameForm = validated === (await c.req.bodyCache.formData)
        cacheKeys = Object.keys(c.req.bodyCache).join(',')
        return c.json((await show(body)) as Record<string, unknown>)
      }
    )
    const response = await app.fetch(await make())
    return {
      status: response.status,
      body: await response.json(),
      sameRememberedForm: sameForm,
      bodyCacheKeys: cacheKeys,
    }
  })

  await attempt("validator('form'), edit the form from formData(), parseBody()", async () => {
    const app = new Hono()
    app.post(
      '/',
      validator('form', (value) => value),
      async (c) => {
        const form = await c.req.formData()
        form.set('foo', 'normalized')
        form.delete('removed')
        return c.json((await show(await c.req.parseBody())) as Record<string, unknown>)
      }
    )
    const response = await app.fetch(await make())
    return { status: response.status, body: await response.json() }
  })

  await attempt('control: text() then parseBody()', async () => {
    const req = new HonoRequest(await make())
    await req.text()
    return req.parseBody()
  })

  await attempt('control: parseBody() then formData()', async () => {
    const req = new HonoRequest(await make())
    await req.parseBody()
    const form = await req.formData()
    return Object.fromEntries([...form.entries()])
  })
}

console.log('\n## mixed-case media type, first read')
await attempt('Multipart/Form-Data: parseBody()', async () => {
  const req = new HonoRequest(await multipart('Multipart/Form-Data')())
  return req.parseBody()
})
await attempt('Application/X-WWW-Form-Urlencoded: parseBody()', async () => {
  const req = new HonoRequest(await urlencoded('Application/X-WWW-Form-Urlencoded')())
  return req.parseBody()
})
await attempt('Multipart/Form-Data: parseBody() twice', async () => {
  const req = new HonoRequest(await multipart('Multipart/Form-Data')())
  await req.parseBody()
  return req.parseBody()
})

type Mods = {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  Hono: any
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  methodOverride: any
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  prettyJSON: any
}

const BOUNDARY = '----probeBoundaryAbC123'

const multipartBody = (fields: Record<string, string>) =>
  Object.entries(fields)
    .map(
      ([name, value]) =>
        `--${BOUNDARY}\r\nContent-Disposition: form-data; name="${name}"\r\n\r\n${value}\r\n`
    )
    .join('') + `--${BOUNDARY}--\r\n`

const urlencodedBody = (fields: Record<string, string>) => new URLSearchParams(fields).toString()

type Case = { contentType: string; kind: 'urlencoded' | 'multipart' }

const CASES: Case[] = [
  { contentType: 'application/x-www-form-urlencoded', kind: 'urlencoded' },
  { contentType: 'Application/X-WWW-Form-Urlencoded', kind: 'urlencoded' },
  { contentType: 'APPLICATION/X-WWW-FORM-URLENCODED', kind: 'urlencoded' },
  { contentType: `multipart/form-data; boundary=${BOUNDARY}`, kind: 'multipart' },
  { contentType: `Multipart/Form-Data; boundary=${BOUNDARY}`, kind: 'multipart' },
  { contentType: `MULTIPART/FORM-DATA; boundary=${BOUNDARY}`, kind: 'multipart' },
]

const FIELD_SETS: Record<string, string>[] = [
  { _method: 'DELETE' },
  { _method: 'DELETE', title: 'hello' },
]

const describeError = (e: unknown) =>
  e instanceof Error ? `${e.name}: ${e.message}` : `non-Error thrown: ${String(e)}`

const runtimeName = () => {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const g = globalThis as any
  if (g.Bun) {
    return `Bun ${g.Bun.version}`
  }
  return `Node ${g.process?.version}`
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const bodyHandler = (name: string) => async (c: any) => {
  let parsed: unknown
  try {
    parsed = await c.req.parseBody()
  } catch (e) {
    parsed = `THROWN ${describeError(e)}`
  }
  return c.json({ handler: name, parseBody: parsed })
}

const buildApp = ({ Hono, methodOverride }: Mods, withMiddleware: boolean) => {
  const app = new Hono()
  if (withMiddleware) {
    app.use('/posts', methodOverride({ app }))
  }
  app.delete('/posts', bodyHandler('DELETE'))
  app.post('/posts', bodyHandler('POST'))
  return app
}

const bodyFor = (c: Case, fields: Record<string, string>) =>
  c.kind === 'urlencoded' ? urlencodedBody(fields) : multipartBody(fields)

const row = (cols: Record<string, unknown>) => console.log(JSON.stringify(cols))

export const run = async (mods: Mods, label: string) => {
  const { Hono, prettyJSON } = mods
  console.log(`# commit label: ${label}`)
  console.log(`# runtime: ${runtimeName()}`)

  for (const withMiddleware of [true, false]) {
    console.log(
      withMiddleware
        ? '\n## A1+A2: POST /posts through methodOverride({ app }), app.request()'
        : '\n## control: POST /posts with no method-override middleware, app.request()'
    )
    for (const fields of FIELD_SETS) {
      for (const c of CASES) {
        const app = buildApp(mods, withMiddleware)
        let status: unknown
        let text: unknown
        try {
          const res = await app.request('http://localhost/posts', {
            method: 'POST',
            headers: { 'Content-Type': c.contentType },
            body: bodyFor(c, fields),
          })
          status = res.status
          text = await res.text()
        } catch (e) {
          status = 'THROWN'
          text = describeError(e)
        }
        row({
          middleware: withMiddleware,
          sentFields: fields,
          contentType: c.contentType,
          status,
          body: text,
        })
      }
    }
  }

  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const bun = (globalThis as any).Bun
  if (bun) {
    console.log(
      '\n## A1+A2 over a socket: Bun.serve({ fetch: app.fetch }) and fetch(), through methodOverride({ app })'
    )
    const app = buildApp(mods, true)
    const server = bun.serve({ port: 0, hostname: '127.0.0.1', fetch: app.fetch })
    try {
      for (const c of CASES) {
        const fields = FIELD_SETS[0]
        const res = await fetch(`http://127.0.0.1:${server.port}/posts`, {
          method: 'POST',
          headers: { 'Content-Type': c.contentType },
          body: bodyFor(c, fields),
        })
        row({
          transport: 'socket',
          sentFields: fields,
          contentType: c.contentType,
          status: res.status,
          body: await res.text(),
        })
      }
    } finally {
      server.stop(true)
    }
  }

  console.log('\n## A4a: c.req.formData() in a POST handler, no middleware, app.request()')
  for (const c of CASES) {
    const app = new Hono()
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    app.post('/fd', async (c: any) => {
      try {
        const fd: FormData = await c.req.formData()
        return c.json({ formData: [...fd.entries()] })
      } catch (e) {
        return c.json({ formData: `THROWN ${describeError(e)}` })
      }
    })
    const res = await app.request('http://localhost/fd', {
      method: 'POST',
      headers: { 'Content-Type': c.contentType },
      body: bodyFor(c, { _method: 'DELETE', title: 'hello' }),
    })
    row({ contentType: c.contentType, status: res.status, body: await res.text() })
  }

  console.log('\n## A4a (platform only, no Hono): new Request(...).formData()')
  for (const c of CASES) {
    let out: unknown
    try {
      const fd = await new Request('http://localhost/fd', {
        method: 'POST',
        headers: { 'Content-Type': c.contentType },
        body: bodyFor(c, { _method: 'DELETE', title: 'hello' }),
      }).formData()
      out = [...fd.entries()]
    } catch (e) {
      out = `THROWN ${describeError(e)}`
    }
    row({ contentType: c.contentType, platformFormData: out })
  }

  console.log('\n## A4b: prettyJSON() on GET /?pretty, handler sets the response Content-Type')
  for (const responseContentType of ['application/json', 'Application/JSON']) {
    const app = new Hono()
    app.use(prettyJSON())
    app.get(
      '/',
      () =>
        new Response(JSON.stringify({ a: 1, b: { c: 2 } }), {
          headers: { 'Content-Type': responseContentType },
        })
    )
    const res = await app.request('http://localhost/?pretty')
    row({
      responseContentType,
      status: res.status,
      contentTypeOut: res.headers.get('Content-Type'),
      body: await res.text(),
    })
  }
}

// Run from <hono checkout>/_probe/time.ts (see ./run.sh).
// Times the FIRST read of a form on a request that has read nothing yet. One request per measurement; every request
// is new, so no form or bytes are remembered when the read starts.
//   PATH=hono   app.request() to a Hono route whose handler awaits c.req.<OP>() once
//   PATH=plain  the exported parseBody() of src/utils/body.ts handed a platform Request (OP is ignored)
//   OP=parseBody | formData   which reader the Hono handler calls (formData() is the platform-backed reader)
//   MODE=stream  the body is a ReadableStream of 64 KiB chunks made while it is read, as an upload arrives
//   MODE=bytes   the body is one Uint8Array prepared once per process
//   BODY=urlencoded | multipart-small | file (with FILE_MB, a multipart form with one text field and one file)
//   CT=lower | mixed   the media type as written in the Content-Type header
// The process first sends one unmeasured-for-the-median request and reports its time as "cold", then WARMUP
// unmeasured requests, then REQUESTS measured ones. It prints one JSON line.
import { Hono } from '../src/index'
import { parseBody } from '../src/utils/body'

declare const Bun: { version: string } | undefined

const PATH = process.env.PATH_KIND ?? 'hono'
const OP = process.env.OP ?? 'parseBody'
const MODE = process.env.MODE ?? 'stream'
const BODY = process.env.BODY ?? 'file'
const FILE_MB = Number(process.env.FILE_MB ?? '8')
const CT = process.env.CT ?? 'lower'
const REQUESTS = Number(process.env.REQUESTS ?? '10')
const WARMUP = Number(process.env.WARMUP ?? '2')
const LABEL = process.env.LABEL ?? ''
const MIB = 1024 * 1024
const CHUNK_BYTES = 64 * 1024
const BOUNDARY = '----probeBoundary'

const encoder = new TextEncoder()
const mixed = (mediaType: string) =>
  CT === 'mixed' ? mediaType.replace(/(^|[/-])([a-z])/g, (_, sep, ch) => sep + ch.toUpperCase()) : mediaType

const parts = (): { pieces: Uint8Array[]; fileBytes: number; contentType: string } => {
  if (BODY === 'urlencoded') {
    return {
      pieces: [encoder.encode('title=hello&message=world')],
      fileBytes: 0,
      contentType: mixed('application/x-www-form-urlencoded'),
    }
  }
  const contentType = `${mixed('multipart/form-data')}; boundary=${BOUNDARY}`
  const title = encoder.encode(
    `--${BOUNDARY}\r\n` + 'Content-Disposition: form-data; name="title"\r\n\r\nhello\r\n'
  )
  const tail = encoder.encode(`--${BOUNDARY}--\r\n`)
  if (BODY === 'multipart-small') {
    const message = encoder.encode(
      `--${BOUNDARY}\r\n` + 'Content-Disposition: form-data; name="message"\r\n\r\nworld\r\n'
    )
    return { pieces: [title, message, tail], fileBytes: 0, contentType }
  }
  const fileHead = encoder.encode(
    `--${BOUNDARY}\r\n` +
      'Content-Disposition: form-data; name="file"; filename="big.bin"\r\n' +
      'Content-Type: application/octet-stream\r\n\r\n'
  )
  const fileTail = encoder.encode('\r\n')
  return { pieces: [title, fileHead, fileTail, tail], fileBytes: FILE_MB * MIB, contentType }
}
const { pieces, fileBytes, contentType } = parts()

const prepared = (() => {
  if (MODE !== 'bytes') {
    return null
  }
  const total = pieces.reduce((sum, piece) => sum + piece.length, 0) + fileBytes
  const bytes = new Uint8Array(total)
  let at = 0
  for (const [index, piece] of pieces.entries()) {
    bytes.set(piece, at)
    at += piece.length
    if (fileBytes && index === 1) {
      bytes.fill(0x61, at, at + fileBytes)
      at += fileBytes
    }
  }
  return bytes
})()

const streamed = () => {
  let index = 0
  let sent = 0
  return new ReadableStream<Uint8Array>({
    pull(controller) {
      if (fileBytes && index === 2 && sent < fileBytes) {
        const size = Math.min(CHUNK_BYTES, fileBytes - sent)
        controller.enqueue(new Uint8Array(size).fill(0x61))
        sent += size
        return
      }
      if (index < pieces.length) {
        controller.enqueue(pieces[index].slice())
        index++
        return
      }
      controller.close()
    },
  })
}

const body = () => (MODE === 'bytes' ? prepared! : streamed())

let lastTitle = ''
let lastFileBytes = -1
let readMs = 0

const note = (parsed: Record<string, unknown>) => {
  lastTitle = String(parsed['title'])
  const file = parsed['file']
  lastFileBytes = file instanceof File ? file.size : -1
}

const app = new Hono()
app.post('/read', async (c) => {
  const start = performance.now()
  if (OP === 'formData') {
    const form = await c.req.formData()
    readMs = performance.now() - start
    note(Object.fromEntries(form.entries()))
  } else {
    const parsed = await c.req.parseBody()
    readMs = performance.now() - start
    note(parsed)
  }
  return c.text('ok')
})

const once = async () => {
  if (PATH === 'plain') {
    const request = new Request('http://localhost/read', {
      method: 'POST',
      headers: { 'Content-Type': contentType },
      body: body(),
      // @ts-expect-error duplex is needed by node for a streamed body
      duplex: 'half',
    })
    const start = performance.now()
    const parsed = await parseBody(request)
    readMs = performance.now() - start
    note(parsed)
    return readMs
  }
  readMs = -1
  const response = await app.request('/read', {
    method: 'POST',
    headers: { 'Content-Type': contentType },
    body: body(),
    // @ts-expect-error duplex is needed by node for a streamed body
    duplex: 'half',
  })
  const text = await response.text()
  if (response.status !== 200 || text !== 'ok') {
    throw new Error(`HTTP ${response.status} ${text.slice(0, 200)}`)
  }
  return readMs
}

let cold = -1
let error: string | null = null
const times: number[] = []
try {
  cold = await once()
  for (let i = 0; i < WARMUP; i++) {
    await once()
  }
  for (let i = 0; i < REQUESTS; i++) {
    times.push(await once())
  }
} catch (caught) {
  error = `${(caught as Error).name}: ${(caught as Error).message}`
}

const sorted = [...times].sort((a, b) => a - b)
const round = (value: number) => Math.round(value * 1000) / 1000
console.log(
  JSON.stringify({
    label: LABEL,
    runtime: typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`,
    path: PATH,
    op: PATH === 'plain' ? 'parseBody(Request)' : OP,
    mode: MODE,
    body: BODY,
    fileMb: fileBytes ? FILE_MB : 0,
    ct: CT,
    requests: times.length,
    coldMs: round(cold),
    readMs: times.length
      ? {
          median: round(sorted[Math.floor(sorted.length / 2)]),
          min: round(sorted[0]),
          max: round(sorted[sorted.length - 1]),
        }
      : null,
    title: lastTitle,
    fileBytesParsed: lastFileBytes,
    error,
  })
)

// Run from <hono checkout>/_probe/probe.ts (see ../run.sh and ./measure.sh).
// One process handles one streamed multipart upload of SIZE_MB and reports process memory:
//   baseline  - resident memory before the request, after a forced collection
//   retained  - resident memory inside the handler after the body was parsed, after a forced collection,
//               while the request object and the parsed result are still alive
//   peak      - the highest resident memory the operating system recorded for the process
// MODE=parseBody (default) reads with c.req.parseBody(); MODE=validator reads with validator('form').
import { Hono } from '../src/index'
import { validator } from '../src/validator/index'

declare const Bun: { gc: (force: boolean) => void } | undefined

const SIZE_MB = Number(process.env.SIZE_MB ?? '100')
const MODE = process.env.MODE ?? 'parseBody'
const MIB = 1024 * 1024
const CHUNK_BYTES = 64 * 1024
const BOUNDARY = '----probeBoundary'

const collect = async () => {
  for (let i = 0; i < 4; i++) {
    if (typeof Bun !== 'undefined') {
      Bun.gc(true)
    } else {
      globalThis.gc?.()
    }
    await new Promise((resolve) => setTimeout(resolve, 20))
  }
}
const residentMb = () => process.memoryUsage().rss / MIB
const peakMb = () => process.resourceUsage().maxRSS / 1024

const uploadStream = () => {
  const encoder = new TextEncoder()
  const head = encoder.encode(
    `--${BOUNDARY}\r\n` +
      'Content-Disposition: form-data; name="title"\r\n\r\nhello\r\n' +
      `--${BOUNDARY}\r\n` +
      'Content-Disposition: form-data; name="file"; filename="big.bin"\r\n' +
      'Content-Type: application/octet-stream\r\n\r\n'
  )
  const tail = encoder.encode(`\r\n--${BOUNDARY}--\r\n`)
  let sent = 0
  let stage: 'head' | 'file' | 'tail' = 'head'
  return new ReadableStream<Uint8Array>({
    pull(controller) {
      if (stage === 'head') {
        controller.enqueue(head)
        stage = 'file'
      } else if (stage === 'file') {
        controller.enqueue(new Uint8Array(CHUNK_BYTES).fill(0x61))
        sent += CHUNK_BYTES
        if (sent >= SIZE_MB * MIB) {
          stage = 'tail'
        }
      } else {
        controller.enqueue(tail)
        controller.close()
      }
    },
  })
}

const app = new Hono()

const report = async (c: { req: { bodyCache: Record<string, unknown> } }, fileBytes: number, baseline: number) => {
  await collect()
  const retained = residentMb()
  const memory = process.memoryUsage()
  return {
    mode: MODE,
    uploadMb: SIZE_MB,
    fileMbParsed: Math.round(fileBytes / MIB),
    bodyCacheKeys: Object.keys(c.req.bodyCache),
    baselineRssMb: Math.round(baseline),
    retainedRssMb: Math.round(retained),
    retainedOverBaselineMb: Math.round(retained - baseline),
    peakRssMb: Math.round(peakMb()),
    peakOverBaselineMb: Math.round(peakMb() - baseline),
    arrayBuffersMb: Math.round(memory.arrayBuffers / MIB),
    externalMb: Math.round(memory.external / MIB),
  }
}

let baseline = 0

app.post('/parseBody', async (c) => {
  const body = await c.req.parseBody()
  const file = body['file']
  const result = await report(c, file instanceof File ? file.size : -1, baseline)
  return c.json({ ...result, stillAlive: Object.keys(body).length })
})

app.post(
  '/validator',
  validator('form', (value) => value),
  async (c) => {
    const form = c.req.valid('form')
    const file = form['file']
    const result = await report(c, file instanceof File ? file.size : -1, baseline)
    return c.json({ ...result, stillAlive: Object.keys(form).length })
  }
)

await collect()
baseline = residentMb()

const response = await app.request(`/${MODE}`, {
  method: 'POST',
  headers: { 'Content-Type': `multipart/form-data; boundary=${BOUNDARY}` },
  body: uploadStream(),
  // @ts-expect-error duplex is required by node for a streamed request body and is missing from older type definitions
  duplex: 'half',
})
console.log(`HTTP ${response.status} ${await response.text()}`)

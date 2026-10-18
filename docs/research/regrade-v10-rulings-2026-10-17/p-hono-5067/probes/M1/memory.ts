// Run from <hono checkout>/_probe/memory.ts (see ./run.sh).
// One process reads one streamed multipart upload of FILE_MB once and reports process memory:
//   peak      the highest resident memory the operating system recorded for the process, above the baseline
//   held      resident memory after the read and a forced collection, while the request and the result are alive
// PATH_KIND=hono   app.request() to a handler that awaits c.req.parseBody() once
// PATH_KIND=plain  the exported parseBody() handed a platform Request
// Also prints the time of the read, which in this probe includes making the 64 KiB chunks of the upload.
import { Hono } from '../src/index'
import { parseBody } from '../src/utils/body'

declare const Bun: { gc: (force: boolean) => void; version: string } | undefined

const PATH = process.env.PATH_KIND ?? 'hono'
const FILE_MB = Number(process.env.FILE_MB ?? '100')
const LABEL = process.env.LABEL ?? ''
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
        if (sent >= FILE_MB * MIB) {
          stage = 'tail'
        }
      } else {
        controller.enqueue(tail)
        controller.close()
      }
    },
  })
}

let baseline = 0
const report = async (parsed: Record<string, unknown>, readMs: number, cacheKeys: string[] | null) => {
  const file = parsed['file']
  await collect()
  const held = residentMb()
  return {
    label: LABEL,
    runtime: typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`,
    path: PATH,
    uploadMb: FILE_MB,
    fileMbParsed: file instanceof File ? Math.round(file.size / MIB) : -1,
    bodyCacheKeys: cacheKeys,
    readMs: Math.round(readMs),
    peakOverBaselineMb: Math.round(peakMb() - baseline),
    heldOverBaselineMb: Math.round(held - baseline),
    alive: Object.keys(parsed).length,
  }
}

const contentType = `multipart/form-data; boundary=${BOUNDARY}`
const app = new Hono()
app.post('/read', async (c) => {
  const start = performance.now()
  const parsed = await c.req.parseBody()
  const readMs = performance.now() - start
  return c.json(await report(parsed, readMs, Object.keys(c.req.bodyCache)))
})

await collect()
baseline = residentMb()

if (PATH === 'plain') {
  const request = new Request('http://localhost/read', {
    method: 'POST',
    headers: { 'Content-Type': contentType },
    body: uploadStream(),
    // @ts-expect-error duplex is needed by node for a streamed body
    duplex: 'half',
  })
  const start = performance.now()
  const parsed = await parseBody(request)
  const readMs = performance.now() - start
  console.log(JSON.stringify(await report(parsed, readMs, null)))
} else {
  const response = await app.request('/read', {
    method: 'POST',
    headers: { 'Content-Type': contentType },
    body: uploadStream(),
    // @ts-expect-error duplex is needed by node for a streamed body
    duplex: 'half',
  })
  console.log(`HTTP ${response.status} ${await response.text()}`)
}

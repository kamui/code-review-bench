// Run from <hono checkout>/_probe/probe.ts (see ../M1/run.sh).
// One process parses one streamed multipart upload of FILE_MB and reports process memory:
//   baseline  resident memory before the request, after a forced collection
//   retained  resident memory after the parse, after a forced collection, while the request object and the
//             parsed result are still alive
//   peak      the highest resident memory the operating system recorded for the process
// MODE=plain   parseBody(new Request(...)) from src/utils/body: the exported function handed a platform Request
// MODE=hono    new HonoRequest(new Request(...)).parseBody(): what c.req.parseBody() runs
// MODE=native  (new Request(...)).formData(): the platform's own parser, with no Hono code
// TRACK=1 also wraps Request.prototype.arrayBuffer so that the buffer it returns is held only through a WeakRef,
// and reports after the forced collection whether that buffer is still reachable. Memory figures are taken with
// TRACK unset.
import { HonoRequest } from '../src/request'
import { parseBody } from '../src/utils/body'

declare const Bun: { gc: (force: boolean) => void; version: string } | undefined

const FILE_MB = Number(process.env.FILE_MB ?? '100')
const MODE = process.env.MODE ?? 'plain'
const TRACK = process.env.TRACK === '1'
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

let bufferRefs: WeakRef<ArrayBuffer>[] = []
let arrayBufferCalls = 0
if (TRACK) {
  const original = Request.prototype.arrayBuffer
  Request.prototype.arrayBuffer = async function (this: Request) {
    arrayBufferCalls++
    const buffer = await original.call(this)
    bufferRefs.push(new WeakRef(buffer))
    return buffer
  }
}

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

await collect()
const baseline = residentMb()

const request = new Request('http://localhost/upload', {
  method: 'POST',
  headers: { 'Content-Type': `multipart/form-data; boundary=${BOUNDARY}` },
  body: uploadStream(),
  // @ts-expect-error duplex is required by node for a streamed request body and is missing from older type definitions
  duplex: 'half',
})

let honoRequest: HonoRequest | undefined
let file: unknown
let kept: unknown
const start = performance.now()
if (MODE === 'plain') {
  const body = await parseBody(request)
  kept = body
  file = body['file']
} else if (MODE === 'hono') {
  honoRequest = new HonoRequest(request)
  const body = await honoRequest.parseBody()
  kept = body
  file = body['file']
} else {
  const form = await request.formData()
  kept = form
  file = form.get('file')
}
const parseMs = performance.now() - start

await collect()
const retained = residentMb()
const memory = process.memoryUsage()
console.log(
  JSON.stringify({
    label: LABEL,
    runtime: typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`,
    mode: MODE,
    uploadMb: FILE_MB,
    fileMbParsed: file instanceof File ? Math.round(file.size / MIB) : -1,
    parseMs: Math.round(parseMs),
    bodyCacheKeys: honoRequest ? Object.keys(honoRequest.bodyCache) : null,
    baselineRssMb: Math.round(baseline),
    retainedRssMb: Math.round(retained),
    retainedOverBaselineMb: Math.round(retained - baseline),
    peakRssMb: Math.round(peakMb()),
    peakOverBaselineMb: Math.round(peakMb() - baseline),
    arrayBuffersMb: Math.round(memory.arrayBuffers / MIB),
    externalMb: Math.round(memory.external / MIB),
    tracked: TRACK
      ? {
          requestArrayBufferCalls: arrayBufferCalls,
          bufferStillReachable: bufferRefs.map((ref) => ref.deref() !== undefined),
        }
      : null,
    requestBodyUsed: request.bodyUsed,
    stillAlive: kept !== undefined && request !== undefined,
  })
)

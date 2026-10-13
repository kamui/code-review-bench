// Run from <hono checkout>/_probe/memory.ts (see ./run.sh).
// One process handles one streamed multipart upload of FILE_MB and reports process memory, as the earlier
// peak-memory probe of this pull request did (cohort-rebuild-2026-10-05/candidates/p-hono-5067/probes/H3/probe.ts):
//   baseline  resident memory before the request, after a forced collection
//   retained  resident memory inside the handler after the last read, after a forced collection,
//             while the request object and the parsed results are still alive
//   peak      the highest resident memory the operating system recorded for the process
// SCENARIO=repeat     the handler awaits c.req.parseBody() CALLS times in a row
// SCENARIO=validator  validator('form') runs first, then the handler awaits c.req.parseBody() CALLS times
// HOLD=1 keeps every call's result alive until the report (a middleware and a handler that each keep theirs);
// HOLD=0 keeps only the last one.
import { Hono } from '../src/index'
import { validator } from '../src/validator/index'

declare const Bun: { gc: (force: boolean) => void; version: string } | undefined

const FILE_MB = Number(process.env.FILE_MB ?? '100')
const SCENARIO = process.env.SCENARIO ?? 'repeat'
const CALLS = Number(process.env.CALLS ?? '2')
const HOLD = (process.env.HOLD ?? '1') === '1'
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

const readAndReport = async (c: {
  req: {
    parseBody: () => Promise<Record<string, unknown>>
    bodyCache: Record<string, unknown>
  }
}) => {
  const kept: Record<string, unknown>[] = []
  let parsed: Record<string, unknown> = {}
  for (let i = 0; i < CALLS; i++) {
    parsed = await c.req.parseBody()
    if (HOLD) {
      kept.push(parsed)
    }
  }
  const file = parsed['file']
  await collect()
  const retained = residentMb()
  const memory = process.memoryUsage()
  return {
    label: LABEL,
    runtime: typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`,
    scenario: SCENARIO,
    calls: CALLS,
    hold: HOLD,
    uploadMb: FILE_MB,
    fileMbParsed: file instanceof File ? Math.round(file.size / MIB) : -1,
    bodyCacheKeys: Object.keys(c.req.bodyCache),
    baselineRssMb: Math.round(baseline),
    retainedRssMb: Math.round(retained),
    retainedOverBaselineMb: Math.round(retained - baseline),
    peakRssMb: Math.round(peakMb()),
    peakOverBaselineMb: Math.round(peakMb() - baseline),
    arrayBuffersMb: Math.round(memory.arrayBuffers / MIB),
    externalMb: Math.round(memory.external / MIB),
    stillAlive: kept.length + Object.keys(parsed).length,
  }
}

const app = new Hono()
app.post('/repeat', async (c) => c.json(await readAndReport(c)))
app.post(
  '/validator',
  validator('form', (value) => value),
  async (c) => c.json(await readAndReport(c))
)

await collect()
baseline = residentMb()

const response = await app.request(`/${SCENARIO}`, {
  method: 'POST',
  headers: { 'Content-Type': `multipart/form-data; boundary=${BOUNDARY}` },
  body: uploadStream(),
  // @ts-expect-error duplex is required by node for a streamed request body and is missing from older type definitions
  duplex: 'half',
})
console.log(`HTTP ${response.status} ${await response.text()}`)

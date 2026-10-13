// Run from <hono checkout>/_probe/time.ts (see ../M1/run.sh).
// Times the exported parseBody() handed a platform Request. REQUESTS requests in one process after WARMUP
// unmeasured ones; a new Request is built for each from the same prepared body.
// BODY=urlencoded-small | multipart-file (with FILE_BYTES)
import { parseBody } from '../src/utils/body'

declare const Bun: { version: string } | undefined

const BODY = process.env.BODY ?? 'urlencoded-small'
const FILE_BYTES = Number(process.env.FILE_BYTES ?? String(8 * 1024 * 1024))
const REQUESTS = Number(process.env.REQUESTS ?? '10')
const WARMUP = Number(process.env.WARMUP ?? '2')
const LABEL = process.env.LABEL ?? ''
const BOUNDARY = '----probeBoundary'

const build = (): { body: Uint8Array | string; contentType: string } => {
  if (BODY === 'urlencoded-small') {
    return { body: '_method=DELETE&title=hello', contentType: 'application/x-www-form-urlencoded' }
  }
  const encoder = new TextEncoder()
  const head = encoder.encode(
    `--${BOUNDARY}\r\n` +
      'Content-Disposition: form-data; name="title"\r\n\r\nhello\r\n' +
      `--${BOUNDARY}\r\n` +
      'Content-Disposition: form-data; name="file"; filename="big.bin"\r\n' +
      'Content-Type: application/octet-stream\r\n\r\n'
  )
  const tail = encoder.encode(`\r\n--${BOUNDARY}--\r\n`)
  const bytes = new Uint8Array(head.length + FILE_BYTES + tail.length)
  bytes.set(head, 0)
  bytes.fill(0x61, head.length, head.length + FILE_BYTES)
  bytes.set(tail, head.length + FILE_BYTES)
  return { body: bytes, contentType: `multipart/form-data; boundary=${BOUNDARY}` }
}
const { body, contentType } = build()

let title = ''
const once = async () => {
  const request = new Request('http://localhost/upload', {
    method: 'POST',
    headers: { 'Content-Type': contentType },
    body,
  })
  const start = performance.now()
  const parsed = await parseBody(request)
  const elapsed = performance.now() - start
  title = String(parsed['title'])
  return elapsed
}
for (let i = 0; i < WARMUP; i++) {
  await once()
}
const times: number[] = []
for (let i = 0; i < REQUESTS; i++) {
  times.push(await once())
}
const sorted = [...times].sort((a, b) => a - b)
const round = (value: number) => Math.round(value * 1000) / 1000
console.log(
  JSON.stringify({
    label: LABEL,
    runtime: typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`,
    body: BODY,
    fileBytes: BODY === 'multipart-file' ? FILE_BYTES : 0,
    requests: REQUESTS,
    parseBodyMs: {
      median: round(sorted[Math.floor(sorted.length / 2)]),
      min: round(sorted[0]),
      max: round(sorted[sorted.length - 1]),
    },
    title,
  })
)

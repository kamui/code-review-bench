// Run from <hono checkout>/_probe/release.ts (see ../M1/run.sh).
// One 100 MB upload through the exported parseBody() handed a platform Request. Reports resident memory above the
// baseline while the parsed result is alive, and again after the result and the request are released and collected.
import { parseBody } from '../src/utils/body'
declare const Bun: { gc: (force: boolean) => void; version: string } | undefined
const MIB = 1024 * 1024
const FILE_MB = Number(process.env.FILE_MB ?? '100')
const BOUNDARY = '----probeBoundary'
const collect = async () => {
  for (let i = 0; i < 6; i++) {
    if (typeof Bun !== 'undefined') Bun.gc(true)
    else globalThis.gc?.()
    await new Promise((r) => setTimeout(r, 50))
  }
}
const rss = () => Math.round(process.memoryUsage().rss / MIB)
const stream = () => {
  const enc = new TextEncoder()
  const head = enc.encode(`--${BOUNDARY}\r\nContent-Disposition: form-data; name="file"; filename="big.bin"\r\nContent-Type: application/octet-stream\r\n\r\n`)
  const tail = enc.encode(`\r\n--${BOUNDARY}--\r\n`)
  let sent = 0
  let stage = 0
  return new ReadableStream<Uint8Array>({
    pull(c) {
      if (stage === 0) { c.enqueue(head); stage = 1 }
      else if (stage === 1) { c.enqueue(new Uint8Array(65536).fill(0x61)); sent += 65536; if (sent >= FILE_MB * MIB) stage = 2 }
      else { c.enqueue(tail); c.close() }
    },
  })
}
await collect()
const baseline = rss()
let request: Request | undefined = new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': `multipart/form-data; boundary=${BOUNDARY}` }, body: stream(), duplex: 'half' } as RequestInit)
let body: Record<string, unknown> | undefined = await parseBody(request)
await collect()
const held = rss()
const size = (body['file'] as File).size
// read the file once to see whether touching it changes anything
const firstByte = new Uint8Array(await (body['file'] as File).slice(0, 1).arrayBuffer())[0]
body = undefined
request = undefined
await collect()
await new Promise((r) => setTimeout(r, 500))
await collect()
const released = rss()
console.log(JSON.stringify({ runtime: typeof Bun !== 'undefined' ? 'bun ' + Bun.version : 'node ' + process.version, label: process.env.LABEL, fileMb: size / MIB, firstByte, heldOverBaseline: held - baseline, afterReleaseOverBaseline: released - baseline }))

// Run with bun, or bundle for node (see ./run.sh); it imports no Hono code.
// Where the head's extra time goes, for one prepared multipart body of FILE_MB: the platform's three steps that the
// head's parseFormData performs in turn, against the one step the base performs.
//   base:  request.formData()
//   head:  request.arrayBuffer(); new Response(bytes, { headers }); response.formData()
// Each line is the median of REQUESTS rounds after WARMUP, in ms.
declare const Bun: { version: string } | undefined

const FILE_MB = Number(process.env.FILE_MB ?? '100')
const REQUESTS = Number(process.env.REQUESTS ?? '7')
const WARMUP = Number(process.env.WARMUP ?? '2')
const BOUNDARY = '----probeBoundary'
const MIB = 1024 * 1024
const encoder = new TextEncoder()
const head = encoder.encode(
  `--${BOUNDARY}\r\nContent-Disposition: form-data; name="title"\r\n\r\nhello\r\n` +
    `--${BOUNDARY}\r\nContent-Disposition: form-data; name="file"; filename="big.bin"\r\n` +
    'Content-Type: application/octet-stream\r\n\r\n'
)
const tail = encoder.encode(`\r\n--${BOUNDARY}--\r\n`)
const body = new Uint8Array(head.length + FILE_MB * MIB + tail.length)
body.set(head, 0)
body.fill(0x61, head.length, head.length + FILE_MB * MIB)
body.set(tail, head.length + FILE_MB * MIB)
const contentType = `multipart/form-data; boundary=${BOUNDARY}`
const request = () =>
  new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': contentType }, body })

const samples: Record<string, number[]> = {
  'base: request.formData()': [],
  'head: request.arrayBuffer()': [],
  'head: new Response(bytes)': [],
  'head: response.formData()': [],
  'head: all three': [],
}
for (let i = 0; i < WARMUP + REQUESTS; i++) {
  const keep = i >= WARMUP
  let start = performance.now()
  await request().formData()
  if (keep) samples['base: request.formData()'].push(performance.now() - start)

  start = performance.now()
  const bytes = await request().arrayBuffer()
  const a = performance.now()
  const response = new Response(bytes, { headers: { 'Content-Type': contentType } })
  const b = performance.now()
  await response.formData()
  const c = performance.now()
  if (keep) {
    samples['head: request.arrayBuffer()'].push(a - start)
    samples['head: new Response(bytes)'].push(b - a)
    samples['head: response.formData()'].push(c - b)
    samples['head: all three'].push(c - start)
  }
}
const median = (values: number[]) => [...values].sort((x, y) => x - y)[Math.floor(values.length / 2)]
console.log(`${typeof Bun !== 'undefined' ? `bun ${Bun.version}` : `node ${process.version}`}, ${FILE_MB} MB file, median of ${REQUESTS}`)
for (const [name, values] of Object.entries(samples)) {
  console.log(`  ${name}: ${median(values).toFixed(1)} ms [${Math.min(...values).toFixed(1)}-${Math.max(...values).toFixed(1)}]`)
}

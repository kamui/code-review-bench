// Does the platform's own form parser accept a mixed-case media type? No Hono code.
const form = new FormData()
form.append('message', 'hello')
const source = new Request('http://localhost/', { method: 'POST', body: form })
const boundaryType = source.headers.get('Content-Type')
const bytes = await source.arrayBuffer()
const cases = [
  ['multipart/form-data', boundaryType, bytes],
  ['Multipart/Form-Data', boundaryType.replace('multipart/form-data', 'Multipart/Form-Data'), bytes],
  ['application/x-www-form-urlencoded', 'application/x-www-form-urlencoded', 'message=hello'],
  ['Application/X-WWW-Form-Urlencoded', 'Application/X-WWW-Form-Urlencoded', 'message=hello'],
]
console.log(typeof Bun !== 'undefined' ? 'bun ' + Bun.version : 'node ' + process.version)
for (const [name, contentType, body] of cases) {
  for (const [kind, make] of [
    ['Request', () => new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': contentType }, body })],
    ['Response', () => new Response(body, { headers: { 'Content-Type': contentType } })],
  ]) {
    try {
      const parsed = await make().formData()
      console.log(`  ${kind}.formData() with ${name}: ${JSON.stringify(Object.fromEntries([...parsed.entries()]))}`)
    } catch (error) {
      console.log(`  ${kind}.formData() with ${name}: THROWS ${error.name}: ${error.message}`)
    }
  }
}

// Copied into a Hono checkout as probe-w3.ts and bundled from there.
import { parseBody } from './src/utils/body'

const multipartBody = '--X\r\nContent-Disposition: form-data; name="foo"\r\n\r\nbar\r\n--X--\r\n'
const cases: Array<[string, string]> = [
  ['multipart/form-data; boundary=X', multipartBody],
  ['Multipart/Form-Data; boundary=X', multipartBody],
  ['application/x-www-form-urlencoded', 'foo=bar'],
  ['Application/X-WWW-Form-Urlencoded', 'foo=bar'],
]
const make = (contentType: string, body: string) =>
  new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': contentType }, body })

for (const [contentType, body] of cases) {
  let native: string
  try {
    native = JSON.stringify(Object.fromEntries(await make(contentType, body).formData()))
  } catch (e) {
    native = `THROWS ${String(e)}`
  }
  let viaParseBody: string
  try {
    viaParseBody = JSON.stringify(await parseBody(make(contentType, body)))
  } catch (e) {
    viaParseBody = `THROWS ${String(e)}`
  }
  console.log(`## ${JSON.stringify(contentType)}`)
  console.log(`  platform request.formData(): ${native}`)
  console.log(`  exported parseBody(Request): ${viaParseBody}`)
}

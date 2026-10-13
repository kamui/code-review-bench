// Does `new Response(arrayBuffer)` copy the bytes? The buffer is changed after the Response is built; a Response
// that still reads the first value holds its own copy.
const buffer = new Uint8Array([97, 97, 97, 97]).buffer
const response = new Response(buffer)
new Uint8Array(buffer).fill(98)
const text = await response.text()
console.log(
  `${typeof Bun !== 'undefined' ? 'bun ' + Bun.version : 'node ' + process.version}: buffer now "bbbb", Response reads "${text}" -> ${text === 'aaaa' ? 'copied' : 'not copied'}`
)

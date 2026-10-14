// The one public program found that hands hono's parseBody a platform Request calls it only when
//   request.body && Object.keys(request.body).length
// is true. This prints that condition for a platform Request with a body.
const request = new Request('http://localhost/', { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: 'a=1' })
console.log(`${typeof Bun !== 'undefined' ? 'bun ' + Bun.version : 'node ' + process.version}: Object.keys(request.body).length = ${Object.keys(request.body).length}`)

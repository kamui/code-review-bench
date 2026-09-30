import { Hono } from '/home/jack/.t3/bench-runs/2026-09-29-claude-thermo-opus-5-5-high/att-023/clone/src/hono'
const app = new Hono()
app.post('/a', async (c) => {
  const fd = await c.req.formData()
  let pb: unknown
  try { pb = await c.req.parseBody() } catch (e) { pb = 'ERR ' + (e as Error).message }
  return c.json({ fd: fd.get('message'), pb })
})
app.post('/b', async (c) => {
  await c.req.text()
  let pb: unknown
  try { pb = await c.req.parseBody() } catch (e) { pb = 'ERR ' + (e as Error).message }
  return c.json({ pb })
})
app.post('/c', async (c) => {
  const pb = await c.req.parseBody()
  const fd = await c.req.formData()
  return c.json({ pb, fd: fd.get('message'), keys: Object.keys(c.req.bodyCache) })
})
const fd = new FormData(); fd.append('message', 'hello')
const params = new URLSearchParams({ message: 'hello' })
for (const path of ['/a', '/b', '/c']) {
  console.log(path, 'multipart', await (await app.request(path, { method: 'POST', body: fd })).text())
  console.log(path, 'urlenc   ', await (await app.request(path, { method: 'POST', body: params })).text())
}

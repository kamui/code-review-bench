import { Hono } from '/home/jack/.t3/bench-runs/2026-10-08-last-push-claude-sonnet/att-007/clone/src/index'
const app = new Hono()
app.post('/', async (c) => {
  await c.req.formData()
  try { return c.json(await c.req.parseBody()) } catch (e) { return c.text('ERR ' + e, 500) }
})
const fd = new FormData(); fd.append('a','b')
const r = await app.request('/', { method: 'POST', body: fd })
console.log(r.status, await r.text())

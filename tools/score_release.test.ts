import { expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

type Attempt = { id: string, admission: { state: string }, usage: unknown }
type Cell = { configuration: string, run: string, attempts: string[], state: string }
const inventory = JSON.parse(readFileSync(join(import.meta.dir, '../bench/grading/current/inventory.json'), 'utf8')) as
  { attempts: Attempt[], cells: Cell[] }

test('selected recoveries retain their failed attempts and usage', () => {
  const attempts = new Map(inventory.attempts.map(attempt => [attempt.id, attempt]))
  for (const [configuration, run, predecessor, recovered] of [
    ['codex-ce-luna-high', 'runs/2026-10-10-last-push-codex-ce-luna', 'att-023', 'att-024'],
    ['codex-thermo-luna-high', 'runs/2026-09-29-codex-thermo-high', 'att-026', 'att-038'],
  ] as const) {
    const cell = inventory.cells.find(cell => cell.configuration === configuration && cell.run === run
      && cell.attempts.some(attempt => attempt.endsWith(`/${predecessor}`)))
    expect(cell?.attempts.map(attempt => attempt.split('/')[1])).toEqual([predecessor, recovered])
    expect(cell?.state).toBe('resolved')
    const [before, after] = (cell?.attempts ?? []).map(id => attempts.get(id))
    expect(before?.admission.state).toBe('excluded')
    expect(after?.admission.state).toBe('admitted')
    expect(before?.usage ?? null).not.toBeNull()
    expect(after?.usage ?? null).not.toBeNull()
  }
})

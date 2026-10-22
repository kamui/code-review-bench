import { appendFileSync, readFileSync } from 'node:fs'
import { dirname, extname, join, resolve } from 'node:path'
import { parseArgs } from 'node:util'

const usage = `Usage: bun tools/select_grading_tests.ts --event-name NAME --event-path EVENT.json --job JOB --output GITHUB_OUTPUT

Selects one CI job's coverage from the complete PR diff, defaulting to full coverage. Appends run=true or run=false.`

const ROOT = resolve(import.meta.dir, '..')
export const JOBS: ReadonlySet<string> = new Set(['build', 'mapping-replay', 'grading-tests', 'client'])
const UI_PREFIXES = ['src/components/', 'src/routes/']
const ASSET_SUFFIXES = new Set(['.css', '.svg', '.png', '.jpg', '.jpeg', '.webp', '.ico', '.woff', '.woff2'])
const UNRELATED_FILES = new Set(['src/styles.css', '.github/workflows/rust-runtime.yml'])

const unrelated = (path: string) =>
  (UI_PREFIXES.some(prefix => path.startsWith(prefix)) && ['.tsx', '.css'].includes(extname(path)))
  || UNRELATED_FILES.has(path)
  || (path.startsWith('public/') && ASSET_SUFFIXES.has(extname(path)))

const isDoc = (path: string) => path.startsWith('docs/') || (!path.includes('/') && path.endsWith('.md'))

/** The jobs whose result a change to this path can affect: every job for an unknown path. */
function jobsReading(path: string): ReadonlySet<string> {
  if (unrelated(path)) return new Set()
  if (dirname(path) === 'tools' && path.endsWith('.test.ts')) return new Set(['build'])
  return JOBS
}

/** A reader can reach a doc through its own path or any directory that holds it, except `docs` itself. */
function names(doc: string): string[] {
  const parts = doc.split('/')
  const names: string[] = []
  for (let end = parts[0] === 'docs' ? 2 : 1; end <= parts.length; end++) names.push(parts.slice(0, end).join('/'))
  return names
}

const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/** A changed doc and another tracked file that names it, counting every file except Markdown prose as a reader. */
function namedDoc(docs: string[], root: string): [string, string] | undefined {
  const wanted = [...new Set(docs.flatMap(names))]
  const found = Bun.spawnSync(['git', 'grep', '-l', '-z', '-I', '-F', ...wanted.flatMap(name => ['-e', name]),
    '--', '.', ':(exclude,glob)**/*.md'], { cwd: root })
  if (found.exitCode === 1) return undefined
  if (found.exitCode !== 0) throw new Error(`git grep exited ${found.exitCode}: ${found.stderr.toString().trim()}`)
  const alternatives = [...wanted].sort((a, b) => b.length - a.length).map(escape).join('|')
  const mention = new RegExp(`(?<![A-Za-z0-9_.-])(${alternatives})/?(?![A-Za-z0-9_./-])`, 'g')
  for (const reader of found.stdout.toString().split('\0')) {
    if (!reader) continue
    const named = new Set([...readFileSync(join(root, reader), 'utf8').matchAll(mention)].map(match => match[1]))
    const doc = docs.find(doc => doc !== reader && names(doc).some(name => named.has(name)))
    if (doc) return [doc, reader]
  }
  return undefined
}

type Event = { pull_request?: { base?: { sha?: unknown }, head?: { sha?: unknown } } }

export function selection(eventName: string, event: Event, root = ROOT): [ReadonlySet<string>, string] {
  if (eventName !== 'pull_request') return [JOBS, 'Main and manual runs retain full coverage']
  const base = event.pull_request?.base?.sha
  const head = event.pull_request?.head?.sha
  if (typeof base !== 'string' || typeof head !== 'string') throw new Error('the event names no base and head commit')
  const diff = Bun.spawnSync(['git', 'diff', '--name-only', '--no-renames', '-z', `${base}...${head}`, '--'], { cwd: root })
  if (diff.exitCode !== 0) throw new Error(`git diff exited ${diff.exitCode}: ${diff.stderr.toString().trim()}`)
  const paths = diff.stdout.toString().split('\0').filter(Boolean)
  if (paths.length === 0) return [JOBS, 'Empty change detection retains full coverage']
  const docs = paths.filter(isDoc)
  let jobs: ReadonlySet<string> = new Set(paths.filter(path => !isDoc(path)).flatMap(path => [...jobsReading(path)]))
  let reason = `${paths.length} changed paths`
  if (docs.length > 0 && jobs.size !== JOBS.size) {
    const named = namedDoc(docs, root)
    reason += named ? `, where ${named[1]} names ${named[0]},` : `, including ${docs.length} docs no other file names,`
    if (named) jobs = JOBS
  }
  return [jobs, `${reason} select ${[...jobs].sort().join(', ') || 'no gated job'}`]
}

if (import.meta.main) {
  const { values } = parseArgs({
    options: { 'event-name': { type: 'string' }, 'event-path': { type: 'string' }, job: { type: 'string' },
      output: { type: 'string' } },
  })
  const [eventName, eventPath, job, output] = [values['event-name'], values['event-path'], values.job, values.output]
  if (!eventName || !eventPath || !job || !JOBS.has(job) || !output) {
    console.error(usage)
    process.exit(2)
  }
  let run: boolean
  let reason: string
  try {
    const [jobs, why] = selection(eventName, JSON.parse(readFileSync(eventPath, 'utf8')) as Event)
    ;[run, reason] = [jobs.has(job), why]
  } catch (error) {
    ;[run, reason] = [true, `Change detection unavailable: ${error instanceof Error ? error.message : String(error)}`]
  }
  console.log(reason)
  appendFileSync(output, `run=${run}\n`)
}

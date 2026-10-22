import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { JOBS, selection } from './select_grading_tests'

const SCRIPT = join(import.meta.dir, 'select_grading_tests.ts')
const ALL = [...JOBS].sort()

describe('CI job coverage selection', () => {
  let root = ''
  let base = ''
  const git = (...args: string[]) => {
    const done = Bun.spawnSync(['git', ...args], { cwd: root })
    if (done.exitCode !== 0) throw new Error(done.stderr.toString())
    return done.stdout.toString().trim()
  }
  const write = (name: string, content = 'changed') => {
    mkdirSync(dirname(join(root, name)), { recursive: true })
    writeFileSync(join(root, name), content)
  }
  const commit = () => {
    git('add', '-A')
    git('commit', '-qm', 'fixture')
    return git('rev-parse', 'HEAD')
  }
  const event = () => ({ pull_request: { base: { sha: base }, head: { sha: git('rev-parse', 'HEAD') } } })
  const current = () => [...selection('pull_request', event(), root)[0]].sort()
  const selected = (...paths: string[]) => {
    for (const path of paths) write(path, `changed after ${base}`)
    commit()
    const jobs = current()
    base = git('rev-parse', 'HEAD')
    return jobs
  }

  beforeEach(() => {
    root = mkdtempSync(join(tmpdir(), 'select-grading-'))
    git('init', '-q')
    git('config', 'user.email', 'test@example.org')
    git('config', 'user.name', 'Test')
    write('src/components/Chart.tsx', 'initial UI')
    write('rust/bench-tools/src/lib.rs', 'initial runtime')
    write('rust/bench-tools/tests/reader.rs',
      'root.join("docs/read.md"); root.join("docs/data"); root.join("docs/slash/"); "docs"')
    write('README.md', 'Prose that links [unread](docs/unread.md) reads nothing.')
    write('docs/unread.md', 'initial prose')
    base = commit()
  })
  afterEach(() => rmSync(root, { recursive: true, force: true }))

  test('UI and assets skip', () => {
    expect(selected('src/components/Chart.tsx', 'src/routes/index.tsx', 'src/styles.css', 'public/icon.svg',
      'src/components/newline\nchart.tsx', '.github/workflows/rust-runtime.yml')).toEqual([])
  })

  test('the Rust runtime workflow alone skips', () => {
    expect(selected('.github/workflows/rust-runtime.yml')).toEqual([])
  })

  test('a tools test selects only the web job', () => {
    expect(selected('tools/export.test.ts')).toEqual(['build'])
    expect(selected('tools/export.test.ts', 'src/routes/index.tsx')).toEqual(['build'])
  })

  test('relevant and unknown paths run every job', () => {
    for (const path of ['rust/bench-tools/src/lib.rs', 'bench/tools/bench-tools', 'tools/select_grading_tests.ts',
      'tools/without-python.sh', 'tools/behavior_contracts/grade-map.v1.json', 'tools/test_data.json',
      'tools/nested/export.test.ts', 'bench/tools/dispatch.sh', 'bench/tools/grade.py',
      'bench/grading/current/assessments/receipt.json', 'bench/regrading/evidence.json', 'package.json', 'bun.lock',
      '.github/workflows/pages.yml', '.github/workflows/rates.yml', 'src/lib/scoring.ts', 'new-file',
      'src/components/unknown.py', 'public/client-pin.json'])
      expect([path, selected(path)]).toEqual([path, ALL])
  })

  test('a doc no code reads skips', () => {
    for (const paths of [['docs/unread.md'], ['docs/new/guide.md', 'NOTES.md'], ['docs/read.md.orig'],
      ['docs/data-v2/table.md'], ['docs/other.json'], ['docs/unread.md', 'src/components/Chart.tsx']])
      expect([paths, selected(...paths)]).toEqual([paths, []])
  })

  test('a doc code reads runs every job', () => {
    for (const paths of [['docs/read.md'], ['docs/data/table.md'], ['docs/slash/deeper/table.md'],
      ['docs/unread.md', 'docs/read.md'], ['docs/read.md', 'tools/export.test.ts']])
      expect([paths, selected(...paths)]).toEqual([paths, ALL])
  })

  test('a doc that names only itself skips', () => {
    write('docs/rollback.sh', '# Usage: docs/rollback.sh SCRATCH')
    commit()
    expect(current()).toEqual([])
  })

  test('a doc alongside a tools test selects only that test\'s job', () => {
    expect(selected('docs/unread.md', 'tools/export.test.ts')).toEqual(['build'])
  })

  test('a doc named by a fixture or record runs every job', () => {
    write('bench/fixtures/case.json', '{"path": "docs/unread.md"}')
    base = commit()
    expect(selected('docs/unread.md')).toEqual(ALL)
  })

  test('a deleted doc that code still names runs', () => {
    write('docs/read.md')
    base = commit()
    git('rm', '-q', 'docs/read.md')
    commit()
    expect(current()).toEqual(ALL)
  })

  test('an earlier PR commit is not hidden by the latest UI commit', () => {
    write('rust/bench-tools/src/lib.rs')
    commit()
    write('src/components/Chart.tsx')
    commit()
    expect(current()).toEqual(ALL)
  })

  test('a rename from a relevant path runs', () => {
    mkdirSync(join(root, 'public'))
    git('mv', 'rust/bench-tools/src/lib.rs', 'public/icon.svg')
    commit()
    expect(current()).toEqual(ALL)
  })

  test('a rename to a relevant path runs', () => {
    git('mv', 'src/components/Chart.tsx', 'rust/bench-tools/src/chart.rs')
    commit()
    expect(current()).toEqual(ALL)
  })

  test('a deleted relevant path runs and a deleted UI path skips', () => {
    git('rm', '-q', 'rust/bench-tools/src/lib.rs')
    commit()
    expect(current()).toEqual(ALL)
    base = git('rev-parse', 'HEAD')
    git('rm', '-q', 'src/components/Chart.tsx')
    commit()
    expect(current()).toEqual([])
  })

  test('main and manual runs do not depend on the diff, and an empty diff runs', () => {
    for (const name of ['push', 'workflow_dispatch']) expect([...selection(name, {}, root)[0]].sort()).toEqual(ALL)
    expect(current()).toEqual(ALL)
  })

  const runCli = (eventText: string, job: string) => {
    const script = join(root, 'tools/select_grading_tests.ts')
    write('tools/select_grading_tests.ts', readFileSync(SCRIPT, 'utf8'))
    const eventPath = join(root, 'event.json')
    const output = join(root, 'output')
    writeFileSync(eventPath, eventText)
    writeFileSync(output, '')
    const done = Bun.spawnSync([process.execPath, script, '--event-name', 'pull_request', '--event-path', eventPath,
      '--job', job, '--output', output], { cwd: root })
    return { done, output: readFileSync(output, 'utf8') }
  }

  test('the command line writes each job\'s selection', () => {
    write('tools/export.test.ts')
    commit()
    const eventText = JSON.stringify(event())
    for (const job of ALL) {
      const { done, output } = runCli(eventText, job)
      expect([job, done.exitCode, output]).toEqual([job, 0, job === 'build' ? 'run=true\n' : 'run=false\n'])
    }
  })

  test('detection failures default to full coverage', () => {
    for (const text of ['invalid JSON', '{}',
      JSON.stringify({ pull_request: { base: { sha: 'missing-base' }, head: { sha: 'missing-head' } } })]) {
      const { done, output } = runCli(text, 'mapping-replay')
      expect(done.exitCode).toBe(0)
      expect(output).toBe('run=true\n')
      expect(done.stdout.toString()).toContain('Change detection unavailable')
    }
  })
})

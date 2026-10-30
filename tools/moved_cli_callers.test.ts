import { describe, expect, test } from 'bun:test'
import { lstatSync, readFileSync } from 'node:fs'
import { basename, join, resolve } from 'node:path'

const REPO = resolve(import.meta.dir, '..')
// Python program, the command that moved out of it, and the command that replaces it.
const MOVED: [string, string, string][] = [
  ['bench/tools/rates.py', 'check', 'bench-tools rates check'],
  ['bench/tools/rates.py', 'refresh', 'bench-tools rates refresh'],
  ['bench/tools/upstream.py', 'report', 'bench-tools upstream report'],
  ['bench/tools/upstream.py', 'collect', 'bench-tools upstream collect'],
  ['bench/tools/evidence_store.py', 'fetch', 'bench-tools evidence fetch'],
  ['bench/tools/evidence_store.py', 'verify', 'bench-tools evidence verify'],
  ['bench/tools/evidence_store.py', 'pack', 'bench-tools evidence pack'],
  ['bench/tools/evidence_store.py', 'publish', 'bench-tools evidence publish'],
  ['bench/tools/recover_evidence.py', '--source', 'bench-tools evidence recover'],
  ['bench/tools/evidence_inventory.py', '--root', 'bench-tools evidence inventory'],
  ['tools/collect_run.py', '--run', 'bench-tools evidence collect'],
  ['bench/tools/reconcile_evidence.py', '--inventory', 'bench-tools evidence reconcile'],
  ['bench/tools/native_artifacts.py', 'verify', 'bench-tools native-artifacts verify'],
  ['bench/tools/native_artifacts.py', 'restore', 'bench-tools native-artifacts restore'],
  ['tools/import_benchmark.py', '--check', 'bench-tools import-benchmark --check'],
  ['tools/import_benchmark.py', '--preserve', 'bench-tools import-benchmark --preserve'],
  ['bench/tools/ruling_record.py', 'route', 'bench-tools ruling route'],
  ['bench/tools/ruling_record.py', 'summary', 'bench-tools ruling summary'],
  ['bench/tools/ruling_record.py', 'precedents', 'bench-tools ruling precedents'],
  ['bench/tools/current_grading.py', 'inventory', 'bench-tools current-grading inventory'],
  ['bench/tools/current_grading.py', 'check', 'bench-tools current-grading check'],
  ['bench/tools/current_grading.py', 'status', 'bench-tools current-grading status'],
  ['bench/tools/claims.py', 'check', 'bench-tools claims check'],
  ['bench/tools/claims.py', 'inventory', 'bench-tools claims inventory'],
  ['bench/tools/claims.py', 'dossier', 'bench-tools claims dossier'],
  ['bench/tools/claims.py', 'evidence', 'bench-tools claims evidence'],
  ['bench/tools/calibration.py', 'check', 'bench-tools calibration check'],
  ['bench/tools/calibration.py', 'cards', 'bench-tools calibration cards'],
  ['bench/tools/calibration.py', 'queue', 'bench-tools calibration queue'],
  ['bench/tools/evaluator_audit.py', 'check', 'bench-tools evaluator-audit check'],
  ['bench/tools/evaluator_audit.py', 'draw', 'bench-tools evaluator-audit draw'],
  ['bench/tools/evaluator_audit.py', 'claims', 'bench-tools evaluator-audit claims'],
  ['bench/tools/evaluator_audit.py', 'second', 'bench-tools evaluator-audit second'],
  ['bench/tools/evaluator_audit.py', 'compare', 'bench-tools evaluator-audit compare'],
  ['bench/tools/evaluator_audit.py', 'conclude', 'bench-tools evaluator-audit conclude'],
]
// Python programs whose command line moved whole, including the form without a command word.
const MOVED_PROGRAMS: [string, string][] = [
  ['bench/tools/ruling_record.py', 'bench-tools ruling check'],
  ['bench/tools/ruling_dossier.py', 'bench-tools ruling dossier'],
  ['bench/tools/check_manifest.py', 'bench-tools check-manifest'],
  ['bench/tools/seal.py', 'bench-tools seal'],
  ['bench/tools/compare.py', 'bench-tools provision hash'],
  ['tools/fetch_freeze_commits.py', 'bench-tools fetch-freeze-commits'],
  ['bench/tools/methodology.py', 'bench-tools methodology'],
  ['tools/export_explorer.py', 'bench-tools export-explorer'],
  ['bench/tools/grade.py', 'bench-tools grade'],
  ['bench/tools/regrade.py', 'bench-tools regrade run'],
  ['bench/tools/grading_client_probe.py', 'bench-tools grading probe-claude'],
  ['bench/tools/test_grading_client.py', 'bench-tools grading probe-claude'],
  ['bench/tools/prune_workspace.py', 'bench-tools prune-workspace'],
  ['bench/tools/retire_workspace.py', 'bench-tools retire-workspace'],
  ['bench/tools/rust_runtime.py', 'bench/tools/bench-tools'],
]
// Frozen runs, imported originals, research records, saved rulings, saved fixture and migration records, and the
// contract decisions that describe superseded behavior keep the commands they recorded.
const HISTORICAL = ['artifacts/', 'bench/runs/', 'docs/research/', 'bench/grading/rulings/', 'bench/fixtures/',
  'docs/python-migration/evidence/', 'docs/python-migration/pilot/', 'tools/behavior_contracts/']
// The migration records quote the Python launcher's command lines from before 2026-10-22; these pages describe the
// current launcher.
const LAUNCHER = 'bench/tools/rust_runtime.py'
const LAUNCHER_HISTORY = 'docs/python-migration/'
const CURRENT_MIGRATION = ['docs/python-migration/runtime.md', 'docs/python-migration/completion.md']

const escape = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')

/** A shell command, a quoted argument list or prose that runs `module command`. */
const invocation = (module: string, command: string) =>
  new RegExp(`\\b${escape(basename(module))}["')\\],\\s\`]+${escape(command)}\\b`)

/** `python3 path/module ...`, or a quoted command line that starts with the module and has an argument. */
const programRun = (module: string) => {
  const name = escape(basename(module))
  return new RegExp(`\\bpython3?\\s+(?:\\S*/)?${name}\\b|\`${name}\\s+[^\`]+\``)
}

function currentTextFiles(): [string, string][] {
  const listed = Bun.spawnSync(['git', '-C', REPO, 'ls-files', '-z'])
  const decoder = new TextDecoder('utf-8', { fatal: true })
  const files: [string, string][] = []
  for (const name of listed.stdout.toString().split('\0')) {
    if (!name || HISTORICAL.some(prefix => name.startsWith(prefix)) || name === 'tools/moved_cli_callers.test.ts') continue
    const path = join(REPO, name)
    try {
      if (!lstatSync(path).isFile()) continue
      files.push([name, decoder.decode(readFileSync(path))])
    } catch {
      continue
    }
  }
  return files
}

describe('moved commands', () => {
  test('the pattern finds each calling form', () => {
    const pattern = invocation('bench/tools/rates.py', 'check')
    for (const text of ['"rates:check": "python3 bench/tools/rates.py check"', 'invoke `rates.py check` first',
      '[sys.executable, str(TOOLS / "rates.py"), "check", "--json"]', 'rates.py\tcheck']) expect(text).toMatch(pattern)
    for (const text of ['python3 bench/tools/rates.py refresh', "`rates.py`'s `check` command", 'pirates.py check'])
      expect(text).not.toMatch(pattern)
  })

  test('the program pattern finds a run without a command word', () => {
    const pattern = programRun('bench/tools/ruling_dossier.py')
    for (const text of ['run `python3 bench/tools/ruling_dossier.py {OUT}`', '`ruling_dossier.py DIRECTORY`',
      'python ruling_dossier.py pr']) expect(text).toMatch(pattern)
    for (const text of ['passes `ruling_dossier.py`.', 'ruling_dossier.py reads structure only',
      "`ruling_dossier.py`'s faults", 'str(tools / "ruling_dossier.py")']) expect(text).not.toMatch(pattern)
  })

  test('no current caller runs a moved command', () => {
    const checks = [
      ...MOVED.map(([module, command, replacement]) =>
        ({ module, pattern: invocation(module, command), message: `runs ${basename(module)} ${command}; use ${replacement}` })),
      ...MOVED_PROGRAMS.map(([module, replacement]) =>
        ({ module, pattern: programRun(module), message: `runs ${basename(module)}; use ${replacement}` })),
    ]
    const found: string[] = []
    for (const [name, text] of currentTextFiles()) {
      for (const { module, pattern, message } of checks) {
        if (name === module || !pattern.test(text)) continue
        if (module === LAUNCHER && name.startsWith(LAUNCHER_HISTORY) && !CURRENT_MIGRATION.includes(name)) continue
        text.split('\n').forEach((line, index) => {
          if (pattern.test(line)) found.push(`${name}:${index + 1}: ${message}`)
        })
      }
    }
    expect(found).toEqual([])
  }, 30_000)
  test('package scripts run the Rust commands through the launcher', () => {
    const scripts = JSON.parse(readFileSync(join(REPO, 'package.json'), 'utf8')).scripts as Record<string, string>
    expect(scripts['rates:check']).toBe('bench/tools/bench-tools rates check')
    expect(scripts['rates:refresh']).toBe('bench/tools/bench-tools rates refresh')
    expect(scripts['audit:maintainers']).toBe('bench/tools/bench-tools upstream report')
    expect(scripts['verify:import']).toBe('bun run evidence:fetch && bench/tools/bench-tools import-benchmark --check')
    expect(scripts['import:preserve']).toBe('bench/tools/bench-tools import-benchmark --preserve')
    expect(scripts['verify:current']).toBe(
      'bun run evidence:fetch && bench/tools/bench-tools current-grading check && bench/tools/bench-tools calibration check')
    expect(scripts.data).toBe('bun run evidence:fetch && bench/tools/bench-tools export-explorer')
    expect(scripts['plan:methodology']).toBe('bench/tools/bench-tools methodology')
    expect(scripts['test:bench']).toContain('cargo test --locked --test behavior_contracts -- check-manifest::')
    expect(scripts['evidence:fetch']).toBe(
      'bench/tools/bench-tools evidence fetch --manifest bench/evidence/manifests/repository-payloads-2026-10-08-v1.json')
  })
})

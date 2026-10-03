import { datasetSchema } from './data'
import type { Assessment, Attempt, Candidate, Configuration, Dataset, Family, Task } from './data'

type Claim = Assessment['claims'][number]
type Remedy = Assessment['recommendations'][number]
type Advice = Assessment['advice'][number]
export type Trial = { attempts: Attempt[]; pending?: true }
type Cells = Record<string, [Task, (Attempt | Trial)[]][]>

export const setup = (id: string, experimental = false, overrides: Partial<Configuration> = {}): Configuration => ({ id, label: id, short: id, version: '1',
  method: 'builtin', builtin: true, experimental, note: '', billing: 'api-dollars', models: ['model'], reasoningEffort: 'high', reasoningSource: 'explicit',
  reviewEdition: 'baseline', reviewChange: null, skillProvenanceUrl: null, skillReleases: [], conditions: [], ...overrides })

export const family = (id: string, impact: Family['impact'] = 'unknown', overrides: Partial<Family> = {}): Family => ({ id, title: id, trigger: '',
  consequence: '', requiredOutcome: '', concerns: ['Functional'], eligibility: 'approved', eligibilityReason: 'Saved human eligibility ruling',
  impact, impactReason: impact === 'unknown' ? 'Awaiting calibration' : 'Saved human impact ruling', rulings: [], manifestations: [], ...overrides })

export const task = (id: string, families: Family[], control: Task['control'] = 'known-problems', overrides: Partial<Task> = {}): Task => ({ id, repo: id,
  pr: 1, head: 'head', base: 'base', shape: 'fixture', language: 'TypeScript', profile: { changeKinds: [], areas: [], technologies: [], concerns: [] },
  families, control, controlReason: 'Fixture control state', controlRulingUrl: null, registerUrl: '', packetUrl: '', sourceUrl: '', ...overrides })

export const claim = (id: string, outcome: Claim['outcome'], overrides: Partial<Claim> = {}): Claim =>
  ({ id, itemId: id, outcome, familyId: null, canonicalId: null, duplicateGroup: null, ...overrides })

export const remedy = (id: string, addressedClaims: string[], safety: Remedy['safety'], sufficiency: Remedy['sufficiency'] = []): Remedy =>
  ({ id, addressedClaims, safety, sufficiency })

export function assessment(subject: Task, caught: string[], overrides: Partial<Assessment> = {}): Assessment {
  return { state: 'assessed', families: subject.families.filter(item => item.eligibility === 'approved').map(item => ({ familyId: item.id,
    outcome: caught.includes(item.id) ? 'caught' : 'missed', sufficiency: caught.includes(item.id) ? 'absent' : 'unassessed',
    claimIds: caught.includes(item.id) ? [`c-${item.id}`] : [] })),
  claims: caught.map(id => claim(`c-${id}`, 'eligible', { familyId: id })), recommendations: [], remedyInventory: 'complete', advice: [], ...overrides }
}

let serial = 0
export const review = (subject: Task, caught: string[] | null, overrides: Partial<Attempt> = {}): Attempt => ({ id: `run/att-${serial += 1}`, label: 'att',
  runId: 'run', taskId: subject.id, replicate: 1, disposition: 'valid completed', complete: true, admitted: true, observedItems: 0,
  assessment: caught === null ? null : assessment(subject, caught), cost: 1, outputTokens: 100, durationSeconds: 60, billing: 'api-dollars',
  predecessor: null, retryReason: null, detailUrl: '', ...overrides })

export const failed = (subject: Task, overrides: Partial<Attempt> = {}) => review(subject, null, { admitted: false, complete: false, disposition: 'harness-invalid: audit', ...overrides })
export const claims = (subject: Task, found: Claim[], overrides: Partial<Assessment> = {}) => review(subject, null, { assessment: assessment(subject, [], { claims: found, ...overrides }) })

export function build(tasks: Task[], cells: Cells, options: { audit?: string; experimental?: string[]; candidates?: Candidate[];
  configurations?: Record<string, Partial<Configuration>>; coverage?: Partial<Dataset['evidence']['coverage']> } = {}): Dataset {
  const trials = (rows: (Attempt | Trial)[]): Trial[] => rows.map(row => 'attempts' in row ? row : { attempts: [row] })
  return datasetSchema.parse({ schemaVersion: 4, release: 'fixture', revision: 'fixture', profileStatus: 'proposed',
    evidence: { datasetHash: 'fixture', coverage: { requiredReviews: 0, assessedReviews: 0, unresolvedRecoveries: 0, unresolvedClaims: 0, complete: true, reason: '', ...options.coverage },
      audit: { state: options.audit ?? 'assessed', reason: null } },
    tasks, configurations: Object.keys(cells).map(id => setup(id, options.experimental?.includes(id), options.configurations?.[id])),
    outcomes: Object.entries(cells).flatMap(([configurationId, rows]) => rows.map(([subject, scheduled]) => ({ configurationId, taskId: subject.id,
      status: 'ran', reason: '', attemptIds: trials(scheduled).flatMap(trial => trial.attempts.map(attempt => attempt.id)),
      trials: trials(scheduled).map((trial, index) => ({ replicate: index + 1, state: trial.pending ? 'pending' : 'resolved',
        reason: trial.pending ? 'Stopped attempt awaits replacement.' : 'valid completed', attemptIds: trial.attempts.map(attempt => attempt.id),
        terminal: trial.attempts.at(-1)?.id ?? null })) }))),
    attempts: Object.values(cells).flatMap(rows => rows.flatMap(([, scheduled]) => trials(scheduled).flatMap(trial => trial.attempts))),
    candidates: options.candidates ?? [], import: { files: 0, transcripts: 0, mismatches: 0 } })
}

/**
 * One dataset holding every display state the scorecard must keep distinct. It stands in for graded
 * evidence while the cohort is assessed and is never benchmark evidence itself.
 */
export function scorecardFixture(): Dataset {
  const profile = (technology: string, concern: string) => ({ changeKinds: ['Bug fix'], areas: ['Library'], technologies: [technology], concerns: [concern] })
  const named = (repo: string, pr: number, shape: string, technology: string, concern: string): Partial<Task> =>
    ({ repo, pr, shape, profile: profile(technology, concern), sourceUrl: `https://example.invalid/${repo}/pull/${pr}` })
  const problem = (id: string, title: string, impact: Family['impact'], concern: string, overrides: Partial<Family> = {}) =>
    family(id, impact, { title, concerns: [concern], trigger: `Trigger of ${id}`, consequence: `Consequence of ${id}`, requiredOutcome: `Obligation of ${id}`, ...overrides })
  const payments = task('payments', [problem('S1', 'Refund is applied twice on retry', 'serious', 'Functional'),
    problem('S2', 'Webhook signature is not verified', 'serious', 'Security'), problem('M1', 'Currency rounding drifts in reports', 'other-material', 'Functional'),
    problem('U1', 'Retry backoff ignores the configured cap', 'unknown', 'Reliability')], 'known-problems',
  named('example/payments', 101, 'Retry handling in a payment client', 'TypeScript', 'Security'))
  const queue = task('queue', [problem('S3', 'Acknowledged jobs are redelivered after restart', 'serious', 'Reliability'),
    problem('M2', 'Dead-letter count is off by one', 'other-material', 'Functional'), problem('M3', 'Metrics label cardinality grows per job', 'other-material', 'Performance'),
    problem('P1', 'Shutdown hook can drop the last batch', 'unknown', 'Reliability', { eligibility: 'pending', eligibilityReason: 'Awaiting eligibility approval' })],
  'known-problems', named('example/queue', 202, 'Durable queue restart path', 'Go', 'Reliability'))
  const docs = task('docs', [problem('U2', 'Install snippet omits a required step', 'unknown', 'Functional'),
    problem('U3', 'Example config uses a removed option', 'unknown', 'Maintainability')], 'known-problems',
  named('example/docs', 303, 'Setup guide rewrite', 'Markdown', 'Functional'))
  const audited = task('audited', [], 'audited-clean', { ...named('example/clean', 404, 'Audited change with no problem', 'Python', 'Testing'),
    controlReason: 'Independent audit found no eligible problem' })
  const empty = task('empty', [], 'unaudited', { ...named('example/empty', 505, 'Change with an empty, unaudited register', 'Rust', 'Maintainability'),
    controlReason: 'No audit has been performed' })
  const novel = task('novel', [], 'provisional', { ...named('example/novel', 606, 'Audited change with a pending novel candidate', 'Python', 'Functional'),
    controlReason: 'Independently audited; a novel candidate awaits a ruling' })
  const tasks = [payments, queue, docs, audited, empty, novel]

  const at = (cost: number, outputTokens: number, durationSeconds: number) => ({ cost, outputTokens, durationSeconds })
  const fixing = (subject: Task, caught: string[], safety: Remedy['safety'], usage: Partial<Attempt>, extra: Claim[] = [], advice: Advice[] = []) =>
    review(subject, null, { ...usage, observedItems: caught.length + extra.length, assessment: assessment(subject, caught, {
      families: subject.families.filter(item => item.eligibility === 'approved').map(item => ({ familyId: item.id, outcome: caught.includes(item.id) ? 'caught' : 'missed',
        sufficiency: caught.includes(item.id) ? 'sufficient' : 'unassessed', claimIds: caught.includes(item.id) ? [`c-${item.id}`] : [] })),
      claims: [...caught.map(id => claim(`c-${id}`, 'eligible', { familyId: id })), ...extra], advice,
      recommendations: caught.map(id => remedy(`fix-${id}`, [`c-${id}`], safety, [{ familyId: id, outcome: 'sufficient' }])) }) })
  const thrice = <T>(make: () => T): T[] => [make(), make(), make()]
  const quiet = (subject: Task, usage: Partial<Attempt>) => thrice(() => fixing(subject, [], 'safe', usage))
  const sample = { population: 'Advisory claims in admitted reviews', selection: 'Every third advisory claim', limits: 'One assessor; no outcome measured' }

  const steady = at(1.2, 9000, 240), selective = at(0.4, 3000, 90), waiting = at(0.8, 6000, 150), risky = at(2.5, 14000, 420), ungraded = at(0.6, 5000, 120)
  const stopped = failed(payments, { ...waiting, disposition: 'stopped: infrastructure interruption' })
  const cells: Cells = {
    steady: [[payments, thrice(() => fixing(payments, ['S1', 'S2', 'M1', 'U1'], 'safe', steady))], [queue, thrice(() => fixing(queue, ['S3', 'M2'], 'safe', steady))],
      [docs, thrice(() => fixing(docs, ['U2'], 'safe', steady))], [audited, quiet(audited, steady)], [empty, quiet(empty, steady)],
      [novel, [fixing(novel, [], 'safe', steady, [claim('candidate', 'unresolved')]), ...quiet(novel, steady).slice(1)]]],
    selective: [[payments, [fixing(payments, ['S1'], 'safe', selective, [claim('wrong-1', 'refuted'), claim('wrong-2', 'refuted')]), failed(payments, selective), failed(payments, selective)]],
      [queue, thrice(() => fixing(queue, ['M2', 'M3'], 'safe', selective, [claim('thin', 'unsupported')]))], [docs, quiet(docs, selective)],
      [audited, quiet(audited, selective)], [empty, quiet(empty, selective)], [novel, quiet(novel, selective)]],
    waiting: [[payments, [fixing(payments, ['S1', 'S2'], 'safe', waiting), fixing(payments, ['S1'], 'safe', waiting), { attempts: [stopped], pending: true }]],
      [queue, thrice(() => fixing(queue, ['S3'], 'safe', waiting))], [docs, thrice(() => fixing(docs, ['U2', 'U3'], 'safe', waiting))],
      [audited, quiet(audited, waiting)], [empty, quiet(empty, waiting)], [novel, quiet(novel, waiting)]],
    risky: [[payments, [fixing(payments, ['S1', 'S2', 'M1'], 'unsafe', risky), fixing(payments, ['S1', 'S2'], 'unassessed', risky), fixing(payments, ['S1', 'S2', 'U1'], 'unassessed', risky)]],
      [queue, thrice(() => fixing(queue, ['S3', 'M2', 'M3'], 'safe', risky, [claim('tip', 'advisory'), claim('note', 'advisory')], [
        { id: 'd-tip', claimIds: ['tip'], kind: 'sampled', benefit: 'supported', sample }, { id: 'd-note', claimIds: ['note'], kind: 'generic', benefit: 'unresolved', sample: null }]))],
      [docs, thrice(() => fixing(docs, ['U2', 'U3'], 'safe', risky))],
      [audited, [fixing(audited, [], 'safe', risky, [claim('alarm', 'unsupported')]), ...quiet(audited, risky).slice(1)]], [empty, quiet(empty, risky)], [novel, quiet(novel, risky)]],
    silent: tasks.map((subject): [Task, Attempt[]] => [subject, thrice(() => failed(subject, at(0.1, 200, 15)))]),
    ungraded: [[payments, [fixing(payments, ['S1', 'S2', 'M1', 'U1'], 'safe', ungraded), review(payments, null, { ...ungraded, observedItems: 2 }), fixing(payments, ['S1'], 'safe', ungraded)]],
      [queue, thrice(() => fixing(queue, ['S3'], 'safe', ungraded))], [docs, quiet(docs, ungraded)], [audited, quiet(audited, ungraded)], [empty, quiet(empty, ungraded)], [novel, quiet(novel, ungraded)]],
    sparse: [[payments, thrice(() => fixing(payments, ['S1', 'S2'], 'safe', steady))], [queue, thrice(() => fixing(queue, ['S3'], 'safe', steady))]],
  }
  const conditions = (client: string, network: string, sandbox: string) => [{ name: 'Client', values: [client] }, { name: 'Reasoning effort', values: ['high'] },
    { name: 'Network access', values: [network] }, { name: 'Sandbox', values: [sandbox] }, { name: 'Billing basis', values: ['api-dollars'] }]
  const builtin = (short: string, model: string, client: string): Partial<Configuration> =>
    ({ short, label: `${short} (fixture)`, method: 'claude-builtin', models: [model], version: client, conditions: conditions(client, 'off', 'n/a') })
  const skill = (short: string, model: string, client: string): Partial<Configuration> => ({ short, label: `${short} (fixture)`, method: 'ce-code-review', builtin: false,
    reviewEdition: 'snapshot', models: [model], version: client, conditions: conditions(client, 'on', 'workspace-write'),
    skillReleases: [{ version: '1.0.0', date: '2026-09-01', dateSource: 'release', provenanceUrl: '' }] })
  const dataset = build(tasks, cells, { audit: 'unassessed', experimental: ['sparse'],
    candidates: [{ id: 'NC-00000000f1c5', taskId: 'novel', recordedAt: '2026-09-20T12:00:00Z', claim: 'A cache key omits the tenant and can serve another tenant\'s entry.',
      limits: 'Read from the diff only; no reproduction was run.', relevance: 'Would make this control a known-problem task and add a serious candidate family.' }],
    configurations: { steady: builtin('Claude built-in / Fixture A / High', 'fixture-a', 'claude-code 2.1.284'), selective: skill('ce-code-review / Fixture B / High', 'fixture-b', 'codex-cli 0.159.0'),
      waiting: builtin('Claude built-in / Fixture B / High', 'fixture-b', 'claude-code 2.1.282'), risky: skill('ce-code-review / Fixture A / High', 'fixture-a', 'claude-code 2.1.284'),
      silent: builtin('Claude built-in / Fixture C / High', 'fixture-c', 'claude-code 2.1.284'), ungraded: skill('ce-code-review / Fixture C / High', 'fixture-c', 'codex-cli 0.159.0'),
      sparse: skill('ce-code-review / Fixture D / High', 'fixture-d', 'codex-cli 0.160.0') } })
  const admitted = dataset.attempts.filter(attempt => attempt.admitted)
  return { ...dataset, release: 'Fixture', evidence: { ...dataset.evidence, audit: { state: 'unassessed', reason: 'The fixture declares no evaluator audit.' },
    coverage: { ...dataset.evidence.coverage, requiredReviews: admitted.length, assessedReviews: admitted.filter(attempt => attempt.assessment?.state === 'assessed').length,
      unresolvedClaims: 1, complete: false, reason: 'Fixture data for interface checks, not benchmark evidence.' } },
    attempts: dataset.attempts.map(attempt => ({ ...attempt, label: attempt.id.replace('run/', ''), detailUrl: `/data/attempts/${attempt.id}.json` })) }
}

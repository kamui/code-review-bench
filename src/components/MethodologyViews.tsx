import { useState } from 'react'
import { Group, Paper, Select, Stack, Switch, Table, Tabs, Text, Title } from '@mantine/core'
import type { Configuration, Dataset } from '../lib/data'
import { bandLabels, compact, conditionDifferences, detectionLabel, duration, estimatorLabels, lowerFirst, measured, money, percent, perReview, points, reading, scaled, share } from '../lib/metrics'
import type { DetectionView, Reading } from '../lib/metrics'
import { bands, candidates as pendingFamilies, conditionalMetrics, estimators, matched, orderingConflicts, pairwise, pendingCandidates, recommend, referenceCoverage } from '../lib/scoring'
import type { ConditionalMetric, Estimator, Scorecard, Selection } from '../lib/scoring'
import type { Inspection } from './EvidenceDrawer'
import { DataTable, Value } from './Reading'
import type { Column } from './Reading'

const signed = (value: number) => `${value > 0 ? '+' : ''}${value.toFixed(2)} pp`
const span = (range: { low: number; high: number }) => `${percent(range.low * 100)} to ${percent(range.high * 100)}`
const reasons = (counts: Record<string, number>) => Object.entries(counts).map(([reason, count]) => `${count} ${reason}`).join('; ') || 'None'
const averages: Record<Estimator, string> = { equalProblem: 'equal problems', equalPr: 'equal PRs' }
const matchedLabels: Record<ConditionalMetric, string> = {
  refuted: 'Refuted claims per admitted review', unsupported: 'Unsupported claims per admitted review', unresolved: 'Unresolved claims per admitted review',
  harmful: 'Unsafe recommendations per admitted review', clean: 'Correctly silent share of admitted control reviews',
}

function harm(card: Scorecard): Reading {
  const { harm: bound, safe, unsafe } = card.remedies
  if (bound.kind === 'lower-bound') return { text: `At least ${bound.perAdmittedReview.toFixed(2)}`, reason: null }
  return { text: 'Unavailable', reason: bound.kind === 'observed' ? `${unsafe} unsafe among ${safe + unsafe} assessed remedies so far. ${bound.reason}` : bound.reason }
}

export const scorecardTabs = ['detection', 'delivery', 'reliability', 'remedies', 'controls', 'advice', 'usage', 'candidates', 'comparison'] as const

export function MethodologyViews({ dataset, configurations, cards, selection, detection, now, onInspect, initialTab = 'detection' }: {
  dataset: Dataset; configurations: Configuration[]; cards: Scorecard[]; selection: Selection; detection: DetectionView; now: Date
  onInspect: (inspection: Inspection) => void; initialTab?: (typeof scorecardTabs)[number]
}) {
  const [left, setLeft] = useState<string | null>(null)
  const [right, setRight] = useState<string | null>(null)
  const [metric, setMetric] = useState<ConditionalMetric>('refuted')
  const [showTables, setShowTables] = useState(false)
  const rows = configurations.flatMap(configuration => {
    const card = cards.find(item => item.configurationId === configuration.id)
    return card ? [{ configuration, card }] : []
  })
  type Row = (typeof rows)[number]
  const { band, estimator } = detection
  const key = (row: Row) => row.configuration.id
  const setup: Column<Row> = { label: 'Setup', value: row => row.configuration.short }
  const short = (id: string) => configurations.find(configuration => configuration.id === id)?.short ?? id
  const task = (taskId: string) => dataset.tasks.find(item => item.id === taskId)
  const name = (taskId: string) => {
    const found = task(taskId)
    return found ? `${found.repo} #${found.pr}` : taskId
  }
  const coverage = referenceCoverage(dataset, selection)
  const conflicts = orderingConflicts(rows.map(row => row.card), band)
  const misses = rows.flatMap(row => row.card.seriousMisses.families.map(miss => ({ ...miss, configuration: row.configuration })))
  const common = matched(dataset, rows.map(key), selection, metric)
  const novel = pendingCandidates(dataset, selection.taskIds, now)
  const awaiting = dataset.tasks.filter(item => selection.taskIds.includes(item.id))
    .flatMap(item => pendingFamilies(item, selection.concern).map(family => ({ taskId: item.id, family })))
  const samples = rows.flatMap(row => row.card.advice.dossiers).flatMap(dossier => dossier.sample ?? [])
    .filter((sample, index, all) => all.findIndex(other => JSON.stringify(other) === JSON.stringify(sample)) === index)
  const a = rows.find(row => row.configuration.id === left) ?? rows[0]
  const b = rows.find(row => row.configuration.id === right && row.configuration.id !== a?.configuration.id) ?? rows.find(row => row.configuration.id !== a?.configuration.id)
  const comparison = a && b ? pairwise(a.card, b.card, band) : null
  const omissionRange = comparison ? measured(comparison.omissionRange[estimator]) : null
  const differences = a && b ? conditionDifferences([a.configuration, b.configuration]) : []
  const advice = a && b ? recommend(dataset, a.card, b.card, selection) : null
  const recommendation = (label: string, result: NonNullable<typeof advice>['impact']) => <Text size="sm">{label}: {result.kind === 'none' ? 'none'
    : `${result.kind}, ${result.prefer ? `prefers ${short(result.prefer)}` : 'no preference'}`}{result.reasons.length > 0 && `. ${result.reasons.join(' ')}`}</Text>
  return <Paper withBorder radius="lg" p="lg" mt="xl" className="methodology-panel">
    <Title order={3}>The scorecard, dimension by dimension</Title>
    <Text size="sm" c="dimmed" mt="xs">Every value comes from the shared scoring kernel for the selected setups and PRs. Dimensions stay separate: there is no blended score. A numbered mark explains an unavailable value below its table.</Text>
    <Tabs defaultValue={initialTab} mt="md">
      <Tabs.List><Tabs.Tab value="detection">Detection</Tabs.Tab><Tabs.Tab value="delivery">Delivery</Tabs.Tab><Tabs.Tab value="reliability">Claim reliability</Tabs.Tab>
        <Tabs.Tab value="remedies">Remedies</Tabs.Tab><Tabs.Tab value="controls">Controls</Tabs.Tab><Tabs.Tab value="advice">Advice benefit</Tabs.Tab>
        <Tabs.Tab value="usage">Cost and time</Tabs.Tab><Tabs.Tab value="candidates">Pending candidates ({novel.length + awaiting.length})</Tabs.Tab><Tabs.Tab value="comparison">Compare two setups</Tabs.Tab></Tabs.List>

      <Tabs.Panel value="detection" pt="md"><Stack>
        <Text size="sm" className="footnote">Recovery of each reference problem is averaged over the PR's scheduled trials; a resolved trial without an admitted review counts as not caught. Both averages are shown. Problems weighted equally is the primary average for the serious and other-material bands; the unknown and all-reference bands have no preferred average. A serious reference is one the implementer had to be made aware of before release. An other-material reference earns credit when a review raises it but did not have to be raised.</Text>
        <Text size="sm">Approved references on the selected PRs: {bands.map(item => `${coverage.bands[item]} ${bandLabels[item].toLowerCase()}`).join(' · ')}. {coverage.pendingFamilies} more {coverage.pendingFamilies === 1 ? 'awaits' : 'await'} an eligibility ruling.</Text>
        <DataTable label="Detection by impact band and average" rows={rows} rowKey={key} minWidth={1250} columns={[setup,
          ...bands.flatMap(item => estimators.map((average): Column<Row> => ({ label: `${bandLabels[item]}, ${averages[average]}`, value: row => share(row.card.detection[item][average]) }))),
          { label: 'All labelled serious caught', value: row => share(row.card.seriousCaught.equalPr) },
          { label: 'Repeated serious misses', value: row => reading(row.card.seriousMisses.repeated, String) }]} />
        <Text size="xs" c="dimmed" className="footnote">All labelled serious caught is the share of scheduled trials that caught every serious reference of a PR, averaged over PRs with one. A repeated serious miss is a serious reference not caught in two or more scheduled trials.</Text>
        {conflicts.length > 0 && <Text size="sm">In the {bandLabels[band].toLowerCase()} band the two averages order {conflicts.length === 1 ? 'this pair' : 'these pairs'} differently: {conflicts.map(pair => `${short(pair.a)} and ${short(pair.b)}`).join('; ')}. That ordering depends on the average chosen.</Text>}
        <Title order={4}>{bandLabels[band]} band counts</Title>
        <DataTable label={`Counts behind ${bandLabels[band].toLowerCase()} detection`} rows={rows} rowKey={key} minWidth={1000} columns={[setup,
          { label: 'Problems / PRs', value: row => `${row.card.detection[band].problems} / ${row.card.detection[band].prs}` },
          { label: 'Observed caught / determined', value: row => `${row.card.detection[band].observed.caught} / ${row.card.detection[band].observed.determined}` },
          { label: 'Admitted reviews / scheduled trials', value: row => `${row.card.detection[band].admittedOnly.admittedReviews} / ${row.card.detection[band].admittedOnly.scheduledTrials}` },
          { label: `Admitted reviews only, ${averages[estimator]}`, value: row => share(row.card.detection[band].admittedOnly[estimator]) },
          { label: 'Range with one PR left out', value: row => reading(row.card.detection[band].omissions[estimator], span) },
          { label: 'Each linked claim as its own unit', value: row => reading(row.card.detection[band].grouping, grouping => `${percent(grouping[estimator] * 100)} over ${grouping.units} units`) }]} />
        <Text size="xs" c="dimmed" className="footnote">Observed counts are outcomes determined so far and are not a final rate. The admitted-only figure excludes failed trials, so it describes delivered reviews rather than the setup. The left-out range shows sensitivity to these PRs, not a confidence interval.</Text>
        <Title order={4}>Serious references not caught</Title>
        {misses.length ? <Table.ScrollContainer minWidth={700}><Table aria-label="Serious references not caught" className="data-table">
          <Table.Thead><Table.Tr><Table.Th scope="col">Setup</Table.Th><Table.Th scope="col">PR</Table.Th><Table.Th scope="col">Serious reference</Table.Th><Table.Th scope="col">Observed not caught / scheduled trials</Table.Th><Table.Th scope="col">Repeated</Table.Th></Table.Tr></Table.Thead>
          <Table.Tbody>{misses.map(miss => <Table.Tr key={`${miss.configuration.id}/${miss.familyId}`}><Table.Th scope="row">{miss.configuration.short}</Table.Th>
            <Table.Td><button className="text-button" onClick={() => onInspect({ kind: 'task', id: miss.taskId })}>{name(miss.taskId)}</button></Table.Td>
            <Table.Td>{task(miss.taskId)?.families.find(family => family.id === miss.familyId)?.title ?? miss.familyId}</Table.Td>
            <Table.Td>{miss.notCaught} / {miss.scheduled}{miss.undetermined > 0 && ` (${miss.undetermined} undetermined)`}</Table.Td>
            <Table.Td><Value cell={reading(miss.repeated, repeated => repeated ? 'Yes' : 'No')} /></Table.Td></Table.Tr>)}</Table.Tbody>
        </Table></Table.ScrollContainer>
          : <Text size="sm" c="dimmed">{coverage.bands.serious ? 'No determined miss of a serious reference for the selected setups.' : 'No reference on the selected PRs is labelled serious, so serious misses cannot be listed.'}</Text>}
      </Stack></Tabs.Panel>

      <Tabs.Panel value="delivery" pt="md"><Stack>
        <Text size="sm" className="footnote">A scheduled trial is one planned repetition on a PR. It is admitted when its last attempt delivered a usable review, failed when it resolved without one, and pending while a replacement is owed. Replaced attempts stay in the counts and in the cost.</Text>
        <DataTable label="Delivery and completion" rows={rows} rowKey={key} minWidth={1150} columns={[setup,
          { label: 'PRs run / selected', value: row => `${row.card.coverage.ran} / ${row.card.coverage.selected}` },
          { label: 'Scheduled trials', value: row => String(row.card.delivery.scheduled) }, { label: 'Admitted', value: row => String(row.card.delivery.admitted) },
          { label: 'Reported complete', value: row => String(row.card.delivery.complete) }, { label: 'Failed', value: row => String(row.card.delivery.unadmitted) },
          { label: 'Pending', value: row => String(row.card.delivery.pending) }, { label: 'Attempts', value: row => String(row.card.delivery.attempts) },
          { label: 'Replacements', value: row => String(row.card.delivery.replacements) },
          { label: 'Failure reasons', value: row => reasons(row.card.delivery.failureReasons) }, { label: 'Pending reasons', value: row => reasons(row.card.delivery.pendingReasons) }]} />
      </Stack></Tabs.Panel>

      <Tabs.Panel value="reliability" pt="md"><Stack>
        <Text size="sm" className="footnote">Rates divide distinct claims by admitted reviews, the denominator shown beside them. Unsupported claims are not proven false. A setup with no admitted review has no rate: silence from a failed trial is not reliability. Rates stay unavailable while a trial is pending or an admitted review awaits assessment.</Text>
        <DataTable label="Claim outcomes per admitted review" rows={rows} rowKey={key} minWidth={1300} columns={[setup,
          { label: 'Admitted / scheduled', value: row => `${row.card.delivery.admitted} / ${row.card.delivery.scheduled}` },
          { label: 'Assessed / admitted', value: row => `${row.card.reliability.assessed} / ${row.card.reliability.admitted}` },
          { label: 'Items per admitted review', value: row => perReview(row.card.reliability.itemsPerAdmittedReview) },
          { label: 'Refuted per admitted review', value: row => perReview(row.card.reliability.outcomes.refuted.perAdmittedReview) },
          { label: 'Unsupported per admitted review', value: row => perReview(row.card.reliability.outcomes.unsupported.perAdmittedReview) },
          { label: 'Unresolved per admitted review', value: row => perReview(row.card.reliability.outcomes.unresolved.perAdmittedReview) },
          { label: 'Admitted reviews with a refuted claim', value: row => share(row.card.reliability.outcomes.refuted.reviewFraction) },
          { label: 'Admitted reviews with an unsupported claim', value: row => share(row.card.reliability.outcomes.unsupported.reviewFraction) },
          { label: 'Advisory claims', value: row => String(row.card.reliability.outcomes.advisory.distinct) },
          { label: 'Non-material observations', value: row => String(row.card.reliability.outcomes.inconsequential.distinct) },
          { label: 'Scope exclusions', value: row => String(row.card.reliability.outcomes['scope-excluded'].distinct) },
          { label: 'Repeated claims', value: row => String(row.card.reliability.duplicates) }, { label: 'Mixed items', value: row => String(row.card.reliability.mixedItems) }]} />
        <Text size="xs" c="dimmed" className="footnote">Claim counts are distinct claims in assessed admitted reviews. Repeated claims count extra occurrences; mixed items hold claims with different outcomes. Item counts measure volume, not reading time.</Text>
        <Title order={4}>Matched on commonly admitted PRs</Title>
        <Group align="end"><Select label="Matched measure" aria-label="Matched measure" value={metric} w={360} allowDeselect={false}
          data={conditionalMetrics.map(item => ({ value: item, label: matchedLabels[item] }))} onChange={value => { const next = conditionalMetrics.find(item => item === value); if (next) setMetric(next) }} /></Group>
        <Text size="sm">{common.included.length} of {common.included.length + common.excluded.length} selected PRs are included: those where every selected setup has admitted reviews assessed well enough for this measure. Each included PR has equal weight.</Text>
        <DataTable label={`${matchedLabels[metric]} on matched PRs`} rows={common.rows} rowKey={row => row.configurationId} minWidth={760} columns={[
          { label: 'Setup', value: row => short(row.configurationId) },
          { label: `${matchedLabels[metric]}, matched PRs`, value: row => metric === 'clean' ? share(row.equalPr) : perReview(row.equalPr) },
          { label: 'Admitted / scheduled, all selected PRs', value: row => `${row.delivery.admitted} / ${row.delivery.scheduled}` },
          { label: 'Matched PRs admitted only in part', value: row => String(row.selectivelyAdmitted) }]} />
        {common.excluded.length > 0 && <div><Text size="sm" fw={600}>Excluded PRs</Text><ul className="reason-list">{common.excluded.map(row => <li key={row.taskId}>{name(row.taskId)}: {row.reason}</li>)}</ul></div>}
        <Text size="xs" c="dimmed" className="footnote">Matching removes PRs a setup did not deliver on. A setup that admitted only some trials of a matched PR is still measured on the reviews it delivered, so that selection remains.</Text>
      </Stack></Tabs.Panel>

      <Tabs.Panel value="remedies" pt="md"><Stack>
        <Text size="sm" className="footnote">Sufficiency asks whether a recommendation resolves the problem it addresses; safety asks, independently, whether applying it does harm. A recommendation without a safety assessment is unassessed, never safe. Only when admission is final and no trial is pending are unsafe recommendations per admitted review a lower bound, because unassessed ones could add to it. Pending cohorts show observed counts and assessed exposure without a final-cohort bound.</Text>
        <DataTable label="Remedy sufficiency and safety" rows={rows} rowKey={key} minWidth={1250} columns={[setup,
          { label: 'Inventoried / admitted reviews', value: row => `${row.card.remedies.inventoried} / ${row.card.remedies.admitted}` },
          { label: 'Recommendations', value: row => String(row.card.remedies.recommendations) }, { label: 'Assessed safe', value: row => String(row.card.remedies.safe) },
          { label: 'Assessed unsafe', value: row => String(row.card.remedies.unsafe) }, { label: 'Safety unassessed', value: row => String(row.card.remedies.unassessed) },
          { label: 'Unsafe per admitted review', value: row => harm(row.card) },
          { label: 'Unsafe share of assessed remedies', value: row => share(row.card.remedies.unsafePerAssessedRemedy) },
          { label: 'Caught problems: fix sufficient', value: row => String(row.card.remedies.sufficiency.sufficient) },
          { label: 'Partial', value: row => String(row.card.remedies.sufficiency.partial) }, { label: 'No fix offered', value: row => String(row.card.remedies.sufficiency.absent) },
          { label: 'Sufficiency unassessed', value: row => String(row.card.remedies.sufficiency.unassessed) }]} />
      </Stack></Tabs.Panel>

      <Tabs.Panel value="controls" pt="md"><Stack>
        <Text size="sm" className="footnote">Only an independently audited clean control gets a correct-silence percentage, over the reviews that were delivered. An empty reference list is not an audit, and a failed trial is missing output, not silence.</Text>
        <DataTable label="Audited controls" rows={rows} rowKey={key} minWidth={1150} columns={[setup,
          { label: 'Audited controls', value: row => String(row.card.controls.audited) }, { label: 'Admitted control reviews', value: row => String(row.card.controls.admitted) },
          { label: 'Correctly silent', value: row => share(row.card.controls.cleanFraction) },
          { label: 'With a refuted or unsupported claim', value: row => String(row.card.controls.alarmed) }, { label: 'With an unresolved claim', value: row => String(row.card.controls.unresolved) },
          { label: 'Awaiting assessment', value: row => String(row.card.controls.unassessed) }, { label: 'Missing output', value: row => String(row.card.controls.missingOutput) },
          { label: 'Pending trials', value: row => String(row.card.controls.pending) }, { label: 'Unaudited or provisional controls', value: row => String(row.card.controls.unaudited.length) }]} />
        {[...coverage.controls.unaudited, ...coverage.controls.provisional].length > 0 && <div><Text size="sm" fw={600}>Controls without a clean percentage</Text>
          <ul className="reason-list">{[...coverage.controls.unaudited, ...coverage.controls.provisional].map(id => <li key={id}>
            <button className="text-button" onClick={() => onInspect({ kind: 'task', id })}>{name(id)}</button>: {task(id)?.control}. {task(id)?.controlReason}</li>)}</ul></div>}
      </Stack></Tabs.Panel>

      <Tabs.Panel value="advice" pt="md"><Stack>
        <Text size="sm" className="footnote">Advice benefit is a description of an audited sample. It earns no points, and a count of advisory claims does not establish benefit. Read each figure with its sample.</Text>
        <DataTable label="Supported advice benefit, sampled" rows={rows} rowKey={key} minWidth={900} columns={[setup,
          { label: 'Sampled: benefit supported', value: row => String(row.card.advice.supported) }, { label: 'Sampled: benefit unsupported', value: row => String(row.card.advice.unsupported) },
          { label: 'Sampled: unresolved', value: row => String(row.card.advice.unresolved) },
          { label: 'Reviews sampled / admitted', value: row => `${row.card.advice.sampledReviews} / ${row.card.advice.admitted}` },
          { label: 'Generic advisory dossiers, benefit unmeasured', value: row => String(row.card.advice.generic) }]} />
        {samples.length ? <ul className="reason-list">{samples.map(sample => <li key={JSON.stringify(sample)}>Population: {sample.population}. Selection: {sample.selection}. Limits: {sample.limits}.</li>)}</ul>
          : <Text size="sm" c="dimmed">No advice sample has been assessed for the selected setups, so no benefit is reported.</Text>}
      </Stack></Tabs.Panel>

      <Tabs.Panel value="usage" pt="md"><Stack>
        <Text size="sm" className="footnote">Cost and output tokens add every attempt, retries included, and divide by scheduled trials. Time covers completed trials only and includes their replaced attempts; it excludes gaps between attempts, provisioning and grading.</Text>
        <DataTable label="Cost and time" rows={rows} rowKey={key} minWidth={1250} columns={[setup,
          { label: 'Cost per scheduled trial', value: row => reading(row.card.cost.perTrial, value => `${money(value)}${row.configuration.billing === 'list-price-equivalent' ? '*' : ''}`) },
          { label: 'Output tokens per scheduled trial', value: row => reading(row.card.tokens.perTrial, compact) },
          { label: 'Attempts with usage / attempts', value: row => `${row.card.cost.measuredAttempts} / ${row.card.cost.attempts}` },
          { label: 'Median completed-trial time', value: row => reading(row.card.time.summary, summary => duration(summary.median)) },
          { label: 'Middle 50%', value: row => reading(row.card.time.summary, summary => `${duration(summary.q1)} to ${duration(summary.q3)}`) },
          { label: 'Completed / scheduled trials', value: row => `${row.card.time.completed} / ${row.card.time.scheduled}` },
          { label: 'Failed', value: row => String(row.card.time.failed) }, { label: 'Incomplete', value: row => String(row.card.time.incomplete) },
          { label: 'Pending', value: row => String(row.card.time.pending) }, { label: 'Completed without a duration', value: row => String(row.card.time.missing) }]} />
        <Text size="xs" c="dimmed" className="footnote">* Subscription usage valued at token list prices, not a bill or quota measurement. Output tokens include reasoning and subagents.</Text>
      </Stack></Tabs.Panel>

      <Tabs.Panel value="candidates" pt="md"><Stack>
        <Text size="sm" className="footnote">A candidate is a reported problem that is not yet a reference. Only a saved human ruling makes it one, and then every selected review of that PR is graded again. Until then it earns no credit, impact and reliability recommendations on its PR are withheld, and an audited control on its PR is provisional.</Text>
        <Title order={4}>Novel candidates awaiting a ruling</Title>
        {novel.length ? <Table.ScrollContainer minWidth={900}><Table aria-label="Novel candidates awaiting a ruling" className="data-table">
          <Table.Thead><Table.Tr><Table.Th scope="col">Candidate</Table.Th><Table.Th scope="col">PR</Table.Th><Table.Th scope="col">Age</Table.Th><Table.Th scope="col">Claim</Table.Th><Table.Th scope="col">Evidence limits</Table.Th><Table.Th scope="col">Decision relevance</Table.Th></Table.Tr></Table.Thead>
          <Table.Tbody>{novel.map(candidate => <Table.Tr key={candidate.id}><Table.Th scope="row">{candidate.id}</Table.Th>
            <Table.Td><button className="text-button" onClick={() => onInspect({ kind: 'task', id: candidate.taskId })}>{name(candidate.taskId)}</button></Table.Td>
            <Table.Td>{candidate.ageDays} {candidate.ageDays === 1 ? 'day' : 'days'}<Text size="xs" c="dimmed">since {candidate.recordedAt.slice(0, 10)}</Text></Table.Td>
            <Table.Td>{candidate.claim}</Table.Td><Table.Td>{candidate.limits}</Table.Td><Table.Td>{candidate.relevance}</Table.Td></Table.Tr>)}</Table.Tbody>
        </Table></Table.ScrollContainer> : <Text size="sm" c="dimmed">No novel candidate awaits a ruling on the selected PRs.</Text>}
        <Title order={4}>Reference families awaiting eligibility</Title>
        {awaiting.length ? <Table.ScrollContainer minWidth={700}><Table aria-label="Reference families awaiting eligibility" className="data-table">
          <Table.Thead><Table.Tr><Table.Th scope="col">Family</Table.Th><Table.Th scope="col">PR</Table.Th><Table.Th scope="col">Problem</Table.Th><Table.Th scope="col">Why it is pending</Table.Th></Table.Tr></Table.Thead>
          <Table.Tbody>{awaiting.map(({ taskId, family }) => <Table.Tr key={`${taskId}/${family.id}`}><Table.Th scope="row">{family.id}</Table.Th>
            <Table.Td><button className="text-button" onClick={() => onInspect({ kind: 'task', id: taskId })}>{name(taskId)}</button></Table.Td>
            <Table.Td>{family.title}</Table.Td><Table.Td>{family.eligibilityReason}</Table.Td></Table.Tr>)}</Table.Tbody>
        </Table></Table.ScrollContainer> : <Text size="sm" c="dimmed">Every reference family on the selected PRs has an eligibility ruling.</Text>}
      </Stack></Tabs.Panel>

      <Tabs.Panel value="comparison" pt="md"><Stack>
        <Group><Select label="Comparison A" aria-label="Comparison A" value={a?.configuration.id ?? null} data={configurations.map(c => ({ value: c.id, label: c.short }))} onChange={setLeft} w={330} />
          <Select label="Comparison B" aria-label="Comparison B" value={b?.configuration.id ?? null} data={configurations.filter(c => c.id !== a?.configuration.id).map(c => ({ value: c.id, label: c.short }))} onChange={setRight} w={330} /></Group>
        {!comparison || !a || !b ? <Text>Select at least two setups.</Text> : <>
          <div><Text size="sm" fw={600}>Comparison conditions</Text>
            {differences.length ? <ul className="reason-list">{differences.map(difference => <li key={difference.name}>{difference.name}: {difference.values.map(row => `${row.value} (${row.configuration.short})`).join(' against ')}</li>)}</ul>
              : <Text size="sm">The recorded client, effort, permissions and billing basis match.</Text>}
            <Text size="xs" c="dimmed" className="footnote">A difference between two setups includes these conditions; it does not isolate the review method or the model.</Text></div>
          <Text size="sm">A minus B in {lowerFirst(detectionLabel(detection))}: <Value cell={reading(comparison.full[estimator].delta, value => signed(value * 100))} />.
            With {estimatorLabels[estimator === 'equalPr' ? 'equalProblem' : 'equalPr']}: <Value cell={reading(comparison.full[estimator === 'equalPr' ? 'equalProblem' : 'equalPr'].delta, value => signed(value * 100))} />.
            {' '}Across {comparison.rows.length} shared PRs with {bandLabels[band].toLowerCase()} references, A is higher on {comparison.wins}, tied on {comparison.ties} and lower on {comparison.losses}; {comparison.pending} have no final comparison.</Text>
          {comparison.aggregationSensitive.kind === 'available' && comparison.aggregationSensitive.value && <Text size="sm">The two averages order these setups differently, so this comparison depends on the average chosen.</Text>}
          {omissionRange && <Text size="sm">Leaving any one whole PR out, A minus B ranges from {signed(omissionRange.low * 100)} to {signed(omissionRange.high * 100)} with {estimatorLabels[estimator]}. This is sensitivity to these PRs, not a confidence interval.</Text>}
          {advice && <div><Text size="sm" fw={600}>What the evidence supports</Text>{recommendation('Serious-impact preference', advice.impact)}{recommendation('Claim-reliability preference', advice.reliability)}
            <Text size="xs" c="dimmed" className="footnote">A preference is limited to its own dimension. Nothing here names a best setup overall.</Text></div>}
          <DeltaPlot rows={comparison.rows.flatMap(row => {
            const value = points(row.delta)
            return value === null ? [] : [{ id: row.taskId, label: name(row.taskId), delta: value }]
          })} a={a.configuration.short} b={b.configuration.short} />
          <Group justify="space-between"><Text size="xs" c="dimmed" className="footnote">Per-PR differences weight that PR's problems equally. Ties use a 0.0000001 percentage-point tolerance for floating-point arithmetic. This is not a significance threshold.</Text>
            <Switch size="xs" checked={showTables} onChange={event => setShowTables(event.currentTarget.checked)} label="Show per-PR tables" className="nowrap-switch" /></Group>
          {showTables && <><Table.ScrollContainer minWidth={800}><Table aria-label="Per-PR detection and repetition variation">
            <Table.Thead><Table.Tr><Table.Th>PR</Table.Th><Table.Th>References</Table.Th><Table.Th>A mean</Table.Th><Table.Th>B mean</Table.Th><Table.Th>A minus B</Table.Th><Table.Th>Individual repetitions</Table.Th></Table.Tr></Table.Thead>
            <Table.Tbody>{comparison.rows.map(row => {
              const sides = [a, b].flatMap(side => side.card.tasks.find(item => item.taskId === row.taskId) ?? [])
              return <Table.Tr key={row.taskId}>
                <Table.Td>{name(row.taskId)}</Table.Td><Table.Td>{sides[0]?.bands[band].problems ?? 'Unavailable'}</Table.Td>
                {sides.map((side, index) => <Table.Td key={index}><Value cell={share(side.bands[band].recall)} /><Text size="xs" c="dimmed">Observed {percent(scaled(side.bands[band].low))} to {percent(scaled(side.bands[band].high))}</Text></Table.Td>)}
                <Table.Td><Value cell={reading(row.delta, value => signed(value * 100))} /></Table.Td><Table.Td>{sides.map((side, index) => <Text size="xs" key={index}>{index === 0 ? 'A' : 'B'}: {side.trials.map((trial, position) => {
                  const value = side.bands[band].repetitions[position] ?? null
                  return `#${trial.replicate} ${value === null ? 'undetermined' : percent(value * 100)}${trial.state === 'unadmitted' ? ' failed' : trial.state === 'admitted' && !trial.complete ? ' incomplete' : ''}`
                }).join(', ')}</Text>)}</Table.Td>
              </Table.Tr>
            })}</Table.Tbody>
          </Table></Table.ScrollContainer>
          <Table.ScrollContainer minWidth={700}><Table aria-label="Leave-one-PR-out detection comparison">
            <Table.Thead><Table.Tr><Table.Th>Omitted PR</Table.Th><Table.Th>Remaining PRs</Table.Th><Table.Th>A detection</Table.Th><Table.Th>B detection</Table.Th><Table.Th>A minus B, equal PRs</Table.Th><Table.Th>A minus B, equal problems</Table.Th></Table.Tr></Table.Thead>
            <Table.Tbody>{comparison.omissions.map(row => <Table.Tr key={row.taskId}><Table.Td>{name(row.taskId)}</Table.Td><Table.Td>{row.remaining}</Table.Td>
              <Table.Td><Value cell={share(row[estimator].a)} /></Table.Td><Table.Td><Value cell={share(row[estimator].b)} /></Table.Td>
              <Table.Td><Value cell={reading(row.equalPr.delta, value => signed(value * 100))} /></Table.Td><Table.Td><Value cell={reading(row.equalProblem.delta, value => signed(value * 100))} /></Table.Td></Table.Tr>)}</Table.Tbody>
          </Table></Table.ScrollContainer></>}
          {!comparison.rows.length && <Text>No shared PR has {bandLabels[band].toLowerCase()} references under these filters.</Text>}
        </>}
        <Text size="xs" c="dimmed">Each omission removes the entire PR and all its repetitions. Repetition indices are not matched experimental seeds.</Text>
      </Stack></Tabs.Panel>
    </Tabs>
  </Paper>
}

function DeltaPlot({ rows, a, b }: { rows: { id: string; label: string; delta: number }[]; a: string; b: string }) {
  if (!rows.length) return null
  const limit = Math.max(25, ...rows.map(row => Math.abs(row.delta)))
  const at = (value: number) => 50 + value / limit * 50
  return <div className="delta-plot" role="list" aria-label="A minus B detection difference per PR">
    <div className="delta-head" aria-hidden="true"><span /><span className="delta-sides"><span>{b} higher</span><span>{a} higher</span></span></div>
    {[...rows].sort((left, right) => right.delta - left.delta).map(row => <div role="listitem" key={row.id} className="delta-row"
      aria-label={`${row.label}: ${signed(row.delta)}`}>
      <span className="delta-label">{row.label}</span>
      <span className="delta-track" aria-hidden="true"><span className="delta-zero" />
        <span className="delta-bar" style={{ left: `${Math.min(50, at(row.delta))}%`, width: `${Math.abs(at(row.delta) - 50)}%` }} />
        <span className={row.delta > 1e-9 ? 'delta-dot positive' : row.delta < -1e-9 ? 'delta-dot negative' : 'delta-dot'} style={{ left: `${at(row.delta)}%` }} /></span>
      <span className="delta-value">{signed(row.delta)}</span>
    </div>)}
  </div>
}

import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import { ArrowLeftRight } from 'lucide-react'
import { Group, Paper, Select, Stack, Switch, Table, Tabs, Text, Title } from '@mantine/core'
import type { Configuration, Dataset } from '../lib/data'
import { bandLabels, compact, conditionDifferences, detectionLabel, foundLabels, duration, estimatorLabels, lowerFirst, measured, percent, perReview, points, reading, scaled, share } from '../lib/metrics'
import type { DetectionView, Reading } from '../lib/metrics'
import { bands, candidates as pendingFamilies, conditionalMetrics, estimators, matched, orderingConflicts, overviewOrder, pairwise, pendingCandidates, preExistingLabels, recommend, referenceCoverage } from '../lib/scoring'
import type { ConditionalMetric, Estimator, PreExistingLabel, Scorecard, Selection } from '../lib/scoring'
import { scorecardTabs } from '../lib/view'
import type { Inspection, ScorecardTab, Section } from '../lib/view'
import { ClaimLabels } from './ClaimLabels'
import { reviewColor, SetupName, setupLabel, setupParts } from './Chart'
import { costNote, costText, DataTable, Disclosure, Referenced, Rich, Value } from './Reading'
import { SeriousMisses } from './SeriousMisses'
import type { Column } from './Reading'
import { VendorMark } from './VendorMark'

const signed = (value: number) => `${value > 0 ? '+' : value < 0 ? '−' : ''}${Math.abs(value).toFixed(1)} points`
const span = (range: { low: number; high: number }) => `${percent(range.low * 100)} to ${percent(range.high * 100)}`
const reasons = (counts: Record<string, number>) => Object.entries(counts).map(([reason, count]) => `${count} ${reason}`).join('; ') || 'None'
const averages: Record<Estimator, string> = { equalProblem: 'per problem', equalPr: 'per PR' }
const matchedLabels: Record<ConditionalMetric, string> = {
  refuted: 'Refuted claims per review', unsupported: 'Unproven claims per review', unresolved: 'Unresolved claims per review',
  harmful: 'Unsafe fixes per review', clean: 'Correctly silent share of admitted control reviews',
}

function harm(card: Scorecard): Reading {
  const { harm: bound, safe, unsafe } = card.remedies
  if (bound.kind === 'lower-bound') return { text: `≥ ${bound.perAdmittedReview.toFixed(2)}`, reason: null }
  return { text: 'Unavailable', reason: bound.kind === 'observed' ? `${unsafe} unsafe among ${safe + unsafe} assessed remedies so far. ${bound.reason}` : bound.reason }
}

export function MethodologyViews({ fullTable, dataset, configurations, cards, selection, detection, now, onInspect, tab, onTab }: {
  fullTable?: ReactNode; dataset: Dataset; configurations: Configuration[]; cards: Scorecard[]; selection: Selection; detection: DetectionView; now: Date
  onInspect: (inspection: Inspection, section?: Section) => void; tab: ScorecardTab; onTab: (tab: ScorecardTab) => void
}) {
  const tabList = useRef<HTMLDivElement>(null)
  // A deep link to a later tab must not leave it scrolled out of the strip; scrolling the strip alone keeps the page still.
  useEffect(() => {
    const list = tabList.current, active = list?.querySelector<HTMLElement>('[data-active]')
    if (list && active) list.scrollLeft = Math.max(0, active.offsetLeft - (list.clientWidth - active.offsetWidth) / 2)
  }, [tab])
  const [left, setLeft] = useState<string | null>(null)
  const [right, setRight] = useState<string | null>(null)
  const [metric, setMetric] = useState<ConditionalMetric>('refuted')
  const [showTables, setShowTables] = useState(false)
  const { band, estimator } = detection
  const byName = configurations.flatMap(configuration => {
    const card = cards.find(item => item.configurationId === configuration.id)
    return card ? [{ configuration, card }] : []
  }).sort((left, right) => left.configuration.short.localeCompare(right.configuration.short))
  const rows = overviewOrder(byName, row => ({ name: row.configuration.short, card: row.card }), { column: 'detection', direction: 'descending' }, band, estimator)
  type Row = (typeof rows)[number]
  const key = (row: Row) => row.configuration.id
  const tint = (row: Row) => reviewColor(row.configuration)
  const setup: Column<Row> = { label: 'Setup', value: row => row.configuration.short, render: row => <SetupName configuration={row.configuration} /> }
  const short = (id: string) => {
    const configuration = configurations.find(item => item.id === id)
    return configuration ? setupLabel(configuration) : id
  }
  const task = (taskId: string) => dataset.tasks.find(item => item.id === taskId)
  const name = (taskId: string) => {
    const found = task(taskId)
    return found ? `${found.repo} #${found.pr}` : taskId
  }
  const coverage = referenceCoverage(dataset, selection)
  const conflicts = orderingConflicts(rows.map(row => row.card), band)
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
  const scope = band === 'all' ? 'any' : bandLabels[band].toLowerCase()
  const recommendation = (label: string, result: NonNullable<typeof advice>['impact']) => <Text size="sm">{label}: {result.kind === 'none' ? 'none'
    : `${result.kind}, ${result.prefer ? `prefers ${short(result.prefer)}` : 'no preference'}`}{result.reasons.length > 0 && `. ${result.reasons.join(' ')}`}</Text>
  const adviceTable = <DataTable label="Supported advice benefit, sampled" rows={rows} rowKey={key} tint={tint} minWidth={900} columns={[setup,
    { label: 'Sampled: benefit supported', value: row => String(row.card.advice.supported) }, { label: 'Sampled: benefit unsupported', value: row => String(row.card.advice.unsupported) },
    { label: 'Sampled: unresolved', value: row => String(row.card.advice.unresolved) },
    { label: 'Reviews sampled / usable', value: row => `${row.card.advice.sampledReviews} / ${row.card.advice.admitted}` },
    { label: 'Generic advisory dossiers, benefit unmeasured', value: row => String(row.card.advice.generic) }]} />
  const groups: { group: string; tabs: { value: ScorecardTab; label: string }[] }[] = [
    { group: 'What it finds', tabs: [{ value: 'detection', label: 'Detection' }, { value: 'misses', label: 'Serious misses' }, { value: 'pre-existing', label: 'Pre-existing problems' }] },
    { group: 'How right it is', tabs: [{ value: 'reliability', label: 'Claim reliability' }, { value: 'remedies', label: 'Remedies' }, ...samples.length > 0 || tab === 'advice' ? [{ value: 'advice' as const, label: 'Advice benefit' }] : [], { value: 'controls', label: 'Controls' }] },
    { group: 'What it takes', tabs: [{ value: 'delivery', label: 'Delivery' }, { value: 'usage', label: 'Cost and time' }] },
    { group: 'Tools', tabs: [...novel.length + awaiting.length > 0 || tab === 'candidates' ? [{ value: 'candidates' as const, label: `Pending candidates (${novel.length + awaiting.length})` }] : [], { value: 'comparison', label: 'Compare setups' }, { value: 'table', label: 'Full table' }] },
  ]
  return <Paper withBorder radius="lg" className="methodology-panel" id="dimensions">
    <div className="dimension-head"><Title order={3}>Detail tables</Title>
      <Text className="subsection-lede">Each measure stands on its own, for the selected setups and PRs; no single score combines them. Rows start with the setup that finds the most known problems. A small number beside a value points to a note under its table.</Text></div>
    <Tabs value={tab} onChange={value => { const next = scorecardTabs.find(item => item === value); if (next) onTab(next) }} className="dimension-tabs">
      <Select className="dimension-select" aria-label="Measure" size="sm" allowDeselect={false} value={tab} onChange={value => { const next = scorecardTabs.find(item => item === value); if (next) onTab(next) }}
        data={groups.map(group => ({ group: group.group, items: group.tabs }))} />
      <Tabs.List ref={tabList}>
        {groups.flatMap((group, groupIndex) => group.tabs.map((item, index) => <Tabs.Tab key={item.value} value={item.value} data-group={index === 0 ? group.group : undefined}
          className={index > 0 || groupIndex === 0 ? undefined : group.group === 'Tools' ? 'tab-group-start tab-tools-start' : 'tab-group-start'}>{item.label}</Tabs.Tab>))}</Tabs.List>

      <Tabs.Panel value="table">{fullTable}</Tabs.Panel>
      <Tabs.Panel value="detection" ><Stack>
        <TabIntro lede="How many known problems each setup caught, averaged over its repeated runs on each PR.">Recovery of each reference problem is averaged over the PR's scheduled trials; a resolved trial without an admitted review counts as not caught. Unresolved recoveries also receive zero credit and stay in the denominator, with an annotation counting affected problems and detection decisions. Both averages are shown. Problems weighted equally is the primary average for the serious and other-material bands; the unknown and all-reference bands have no preferred average. A serious reference is one the implementer had to be made aware of before release. An other-material reference earns credit when a review raises it but did not have to be raised.</TabIntro>
        <Text size="sm">Approved references on the selected PRs: {bands.map(item => `${coverage.bands[item]} ${bandLabels[item].toLowerCase()}`).join(' · ')}. {coverage.pendingFamilies} more {coverage.pendingFamilies === 1 ? 'awaits' : 'await'} an eligibility ruling.</Text>
        <Title order={4}>Detection by impact band and average</Title>
        <DataTable label="Detection by impact band and average" rows={rows} rowKey={key} tint={tint} minWidth={1120} columns={[setup,
          ...bands.flatMap(item => estimators.map((average): Column<Row> => ({ label: `${bandLabels[item]}, ${averages[average]}`, value: row => share(row.card.detection[item][average], row.card.detection[item].zeroCredit.reason) }))),
          { label: 'All labelled serious caught', value: row => share(row.card.seriousCaught.equalPr, row.card.detection.serious.zeroCredit.reason) },
          { label: 'Repeated serious misses', value: row => reading(row.card.seriousMisses.repeated, String) }]} />
        <Text size="xs" c="dimmed" className="footnote">All labelled serious caught is the share of scheduled trials that caught every serious reference of a PR, averaged over PRs with one. A repeated serious miss is a serious reference not caught in two or more scheduled trials.</Text>
        {conflicts.length > 0 && <Disclosure title={`The two averages order ${conflicts.length === 1 ? 'one pair' : `${conflicts.length} pairs`} of setups differently`}>
          <Text size="sm" mb={8}>In the {bandLabels[band].toLowerCase()} band, which of these setups finds more depends on the average chosen.</Text>
          <ul className="pair-list">{conflicts.map(pair => <li key={`${pair.a}/${pair.b}`}>{short(pair.a)}<ArrowLeftRight size={14} aria-label="and" />{short(pair.b)}</li>)}</ul></Disclosure>}
        <Disclosure title={`Counts behind ${lowerFirst(foundLabels[band])}`}><DataTable label={`Counts behind ${lowerFirst(foundLabels[band])}`} rows={rows} rowKey={key} tint={tint} minWidth={1000} columns={[setup,
          { label: 'Problems / PRs with one', value: row => `${row.card.detection[band].problems} / ${row.card.detection[band].prs}` },
          { label: 'Observed caught / determined', value: row => `${row.card.detection[band].observed.caught} / ${row.card.detection[band].observed.determined}` },
          { label: 'Usable reviews / trials', value: row => `${row.card.detection[band].admittedOnly.admittedReviews} / ${row.card.detection[band].admittedOnly.scheduledTrials}` },
          { label: `Admitted reviews only, ${averages[estimator]}`, value: row => share(row.card.detection[band].admittedOnly[estimator], row.card.detection[band].zeroCredit.reason) },
          { label: 'Range with one PR left out', value: row => reading(row.card.detection[band].omissions[estimator], span) },
          { label: 'Each linked claim as its own unit', value: row => reading(row.card.detection[band].grouping, grouping => `${percent(grouping[estimator] * 100)} over ${grouping.units} units`) }]} />
        <Text size="xs" c="dimmed" className="footnote">Observed counts are outcomes determined so far and are not a final rate. The admitted-only figure excludes failed trials, so it describes delivered reviews rather than the setup. The left-out range shows sensitivity to these PRs, not a confidence interval.</Text></Disclosure>
      </Stack></Tabs.Panel>

      <Tabs.Panel value="misses" ><Stack>
        <TabIntro lede="Which serious known problems each setup did not catch, trial by trial." />
        <Title order={4} id="serious-misses" className="visually-hidden">Serious references not caught</Title>
        <SeriousMisses dataset={dataset} configurations={configurations} cards={cards} selection={selection} onInspect={inspection => onInspect(inspection, 'serious-misses')} />
      </Stack></Tabs.Panel>

      <Tabs.Panel value="delivery" ><Stack>
        <TabIntro lede="Whether each planned run produced a usable review, and how many attempts that took.">A scheduled trial is one planned repetition on a PR. It is admitted when its last attempt delivered a usable review, failed when it resolved without one, and pending while a replacement is owed. Replaced attempts stay in the counts and in the cost.</TabIntro>
        <DataTable label="Delivery and completion" rows={rows} rowKey={key} tint={tint} minWidth={1150} columns={[setup,
          { label: 'PRs run / selected', value: row => `${row.card.coverage.ran} / ${row.card.coverage.selected}` },
          { label: 'Scheduled trials', value: row => String(row.card.delivery.scheduled) }, { label: 'Admitted', value: row => String(row.card.delivery.admitted) },
          { label: 'Reported complete', value: row => String(row.card.delivery.complete) }, { label: 'Failed', value: row => String(row.card.delivery.unadmitted) },
          { label: 'Pending', value: row => String(row.card.delivery.pending) }, { label: 'Attempts', value: row => String(row.card.delivery.attempts) },
          { label: 'Replacements', value: row => String(row.card.delivery.replacements) },
          { label: 'Failure reasons', value: row => reasons(row.card.delivery.failureReasons) }, { label: 'Pending reasons', value: row => reasons(row.card.delivery.pendingReasons) }]} />
      </Stack></Tabs.Panel>

      <Tabs.Panel value="reliability" ><Stack>
        <TabIntro lede="How often a delivered review made a claim that was wrong or not shown to be right.">Rates divide distinct claims by admitted reviews, the denominator shown beside them. Unsupported claims are not proven false. A setup with no admitted review has no rate: silence from a failed trial is not reliability. Rates stay unavailable while a trial is pending or an admitted review awaits assessment.</TabIntro>
        <Title order={4}>Claim outcomes per review</Title>
        <DataTable label="Claim outcomes per review" rows={rows} rowKey={key} tint={tint} minWidth={1300} columns={[setup,
          { label: 'Usable reviews / trials', value: row => `${row.card.delivery.admitted} / ${row.card.delivery.scheduled}` },
          { label: 'Graded / usable reviews', value: row => `${row.card.reliability.assessed} / ${row.card.reliability.admitted}` },
          { label: 'Per review, items', value: row => perReview(row.card.reliability.itemsPerAdmittedReview) },
          { label: 'Per review, refuted', value: row => perReview(row.card.reliability.outcomes.refuted.perAdmittedReview) },
          { label: 'Per review, unproven', value: row => perReview(row.card.reliability.outcomes.unsupported.perAdmittedReview) },
          { label: 'Per review, unresolved', value: row => perReview(row.card.reliability.outcomes.unresolved.perAdmittedReview) },
          { label: 'Reviews with a claim, refuted', value: row => share(row.card.reliability.outcomes.refuted.reviewFraction) },
          { label: 'Reviews with a claim, unproven', value: row => share(row.card.reliability.outcomes.unsupported.reviewFraction) },
          { label: 'Claims, out of scope', value: row => String(row.card.reliability.outcomes['scope-excluded'].distinct) },
          { label: 'Claims, repeated', value: row => String(row.card.reliability.duplicates) }, { label: 'Mixed-outcome items', value: row => String(row.card.reliability.mixedItems) }]} />
        <Text size="xs" c="dimmed" className="footnote">Claim counts are distinct claims in assessed admitted reviews. Repeated claims count extra occurrences; mixed items hold claims with different outcomes. Item counts measure volume, not reading time.</Text>
        <Title order={4}>Claim labels per review</Title>
        <ClaimLabels rows={rows.map(row => ({ id: key(row), configuration: row.configuration, rates: row.card.reliability.labelsPerAdmittedReview }))} />
        <Title order={4}>Matched on commonly admitted PRs</Title>
        <Group align="end"><Select label="Matched measure" aria-label="Matched measure" value={metric} w={360} allowDeselect={false}
          data={conditionalMetrics.map(item => ({ value: item, label: matchedLabels[item] }))} onChange={value => { const next = conditionalMetrics.find(item => item === value); if (next) setMetric(next) }} /></Group>
        <Text size="sm">{common.included.length} of {common.included.length + common.excluded.length} selected PRs are included: those where every selected setup has admitted reviews assessed well enough for this measure. Each included PR has equal weight.</Text>
        <DataTable label={`${matchedLabels[metric]} on matched PRs`} rows={common.rows} rowKey={row => row.configurationId} minWidth={760} columns={[
          { label: 'Setup', value: row => short(row.configurationId), render: row => {
            const found = configurations.find(configuration => configuration.id === row.configurationId)
            return found ? <SetupName configuration={found} /> : short(row.configurationId)
          } },
          { label: `${matchedLabels[metric]}, matched PRs`, value: row => metric === 'clean' ? share(row.equalPr) : perReview(row.equalPr) },
          { label: 'Usable reviews / trials, all selected PRs', value: row => `${row.delivery.admitted} / ${row.delivery.scheduled}` },
          { label: 'Matched PRs admitted only in part', value: row => String(row.selectivelyAdmitted) }]} />
        {common.excluded.length > 0 && <div><Text size="sm" fw={700}>Excluded PRs</Text><ul className="reason-list">{common.excluded.map(row => <li key={row.taskId}>{name(row.taskId)}: {row.reason}</li>)}</ul></div>}
        <Text size="xs" c="dimmed" className="footnote">Matching removes PRs a setup did not deliver on. A setup that admitted only some trials of a matched PR is still measured on the reviews it delivered, so that selection remains.</Text>
      </Stack></Tabs.Panel>

      <Tabs.Panel value="remedies" ><Stack>
        <TabIntro lede="Whether proposed fixes would solve their problem, and whether applying them would do harm.">Sufficiency asks whether a recommendation resolves the problem it addresses; safety asks, independently, whether applying it does harm. New first-grader assessments check safety only for sufficient or partial fixes; the second grader omits safety. Historical assessments retain their original coverage. A recommendation without a safety assessment is unassessed, never safe. The assessed subset is selected for usefulness, so its unsafe share does not describe all advice. Only when admission is final and no trial is pending are unsafe recommendations per admitted review a lower bound, because unassessed ones could add to it. Pending cohorts show observed counts and assessed exposure without a final-cohort bound.</TabIntro>
        <DataTable label="Remedy sufficiency and safety" rows={rows} rowKey={key} tint={tint} minWidth={1250} columns={[setup,
          { label: 'Reviews listed / usable', value: row => `${row.card.remedies.inventoried} / ${row.card.remedies.admitted}` },
          { label: 'Recommendations', value: row => String(row.card.remedies.recommendations) }, { label: 'Safety, assessed safe', value: row => String(row.card.remedies.safe) },
          { label: 'Safety, assessed unsafe', value: row => String(row.card.remedies.unsafe) }, { label: 'Safety, unassessed', value: row => String(row.card.remedies.unassessed) },
          { label: 'Unsafe fixes per review', value: row => harm(row.card) },
          { label: 'Unsafe share of assessed remedies', value: row => share(row.card.remedies.unsafePerAssessedRemedy) },
          { label: 'Fix for a caught problem, sufficient', value: row => String(row.card.remedies.sufficiency.sufficient) },
          { label: 'Fix for a caught problem, partial', value: row => String(row.card.remedies.sufficiency.partial) },
          { label: 'Fix for a caught problem, ineffective', value: row => String(row.card.remedies.sufficiency.ineffective) }, { label: 'Fix for a caught problem, none offered', value: row => String(row.card.remedies.sufficiency.absent) },
          { label: 'Fix for a caught problem, unassessed', value: row => String(row.card.remedies.sufficiency.unassessed) }]} />
      </Stack></Tabs.Panel>

      <Tabs.Panel value="pre-existing" ><Stack>
        <TabIntro lede="How often reviews raised known problems that were already in the code before the PR.">An approved pre-existing problem counts as raised once for each admitted review that says what goes wrong. Silence creates no miss. Each count is given separately for serious and for other-material pre-existing problems. These counts do not change recall or recommendations.</TabIntro>
        {preExistingLabels.map(label => <Stack key={label}>
          <Title order={4}>{bandLabels[label]} pre-existing problems</Title>
          <DataTable label={`${bandLabels[label]} pre-existing problems raised`} rows={rows} rowKey={key} tint={tint} minWidth={1150} foldShared={false} columns={[setup,
            { label: 'Approved catalog', value: row => String(row.card.preExisting.labels[label].catalog) },
            { label: 'Times raised', value: row => reading(row.card.preExisting.labels[label].raised, String) },
            { label: 'Distinct problems raised', value: row => reading(row.card.preExisting.labels[label].problems, String) },
            { label: 'Reviews that raised one', value: row => reading(row.card.preExisting.labels[label].reviews, String) },
            { label: 'Observed raised', value: row => String(row.card.preExisting.labels[label].observed.raised) },
            { label: 'Reviews graded / usable', value: row => `${row.card.preExisting.labels[label].assessed} / ${row.card.preExisting.labels[label].admitted}` },
            { label: 'Unresolved', value: row => String(row.card.preExisting.labels[label].unresolved) },
            { label: 'Pending candidates', value: row => String(row.card.preExisting.candidates.length) }]} />
          <PreExistingMatrix label={label} rows={byName} name={name} />
        </Stack>)}
        <Text size="xs" c="dimmed">The catalog contains discovered cases, not every pre-existing fault. Assessment coverage does not establish historical audit coverage.</Text>
      </Stack></Tabs.Panel>

      <Tabs.Panel value="controls" ><Stack>
        <TabIntro lede="How often reviews stayed quiet on PRs audited to have no problems.">Only an independently audited clean control gets a correct-silence percentage, over the reviews that were delivered. An empty reference list is not an audit, and a failed trial is missing output, not silence.</TabIntro>
        <DataTable label="Audited controls" rows={rows} rowKey={key} tint={tint} minWidth={1150} columns={[setup,
          { label: 'Audited controls', value: row => String(row.card.controls.audited) }, { label: 'Usable control reviews', value: row => String(row.card.controls.admitted) },
          { label: 'Without a refuted or unproven claim', value: row => share(row.card.controls.cleanFraction) },
          { label: 'With a refuted or unproven claim', value: row => String(row.card.controls.alarmed) }, { label: 'With an unresolved claim', value: row => String(row.card.controls.unresolved) },
          { label: 'Awaiting assessment', value: row => String(row.card.controls.unassessed) }, { label: 'Missing output', value: row => String(row.card.controls.missingOutput) },
          { label: 'Pending trials', value: row => String(row.card.controls.pending) }, { label: 'Unaudited or provisional controls', value: row => String(row.card.controls.unaudited.length) }]} />
        {[...coverage.controls.unaudited, ...coverage.controls.provisional].length > 0 && <div><Title order={4} mb={8}>Controls without a clean percentage</Title>
          <ul className="reason-list">{[...coverage.controls.unaudited, ...coverage.controls.provisional].map(id => <li key={id}>
            <button className="text-button" onClick={() => onInspect({ kind: 'task', id })}>{name(id)}</button>: {task(id)?.control}. {task(id)?.controlReason}</li>)}</ul></div>}
      </Stack></Tabs.Panel>

      <Tabs.Panel value="advice" ><Stack>
        <TabIntro lede="What an audited sample found about the benefit of review advice. It earns no points.">Advice benefit is a description of an audited sample. It earns no points, and a count of advisory claims does not establish benefit. Read each figure with its sample.</TabIntro>
        {samples.length ? <>{adviceTable}
          <ul className="reason-list">{samples.map(sample => <li key={JSON.stringify(sample)}>Population: {sample.population}. Selection: {sample.selection}. Limits: {sample.limits}.</li>)}</ul></>
          : <><div className="empty-state" role="status">No advice sample has been assessed for the selected setups, so no benefit is reported.</div>
            <Disclosure title="Admitted reviews per setup">{adviceTable}</Disclosure></>}
      </Stack></Tabs.Panel>

      <Tabs.Panel value="usage" ><Stack>
        <TabIntro lede="What each scheduled trial cost in money, output tokens and time.">Cost and output tokens add every attempt, retries included, and divide by scheduled trials. Time covers completed trials only and includes their replaced attempts; it excludes gaps between attempts, provisioning and grading.</TabIntro>
        <DataTable label="Cost and time" rows={rows} rowKey={key} tint={tint} minWidth={1250} columns={[setup,
          { label: 'Cost per scheduled trial', value: row => reading(row.card.cost.perTrial, costText(row.configuration, rows.map(item => item.configuration))) },
          { label: 'Output tokens per scheduled trial', value: row => reading(row.card.tokens.perTrial, compact) },
          { label: 'Attempts with usage / attempts', value: row => `${row.card.cost.measuredAttempts} / ${row.card.cost.attempts}` },
          { label: 'Completed-trial time, median', value: row => reading(row.card.time.summary, summary => duration(summary.median)) },
          { label: 'Completed-trial time, middle 50%', value: row => reading(row.card.time.summary, summary => `${duration(summary.q1)} to ${duration(summary.q3)}`.replace(/ /g, '\u00a0')) },
          { label: 'Completed / scheduled trials', value: row => `${row.card.time.completed} / ${row.card.time.scheduled}` },
          { label: 'Failed', value: row => String(row.card.time.failed) }, { label: 'Incomplete', value: row => String(row.card.time.incomplete) },
          { label: 'Pending', value: row => String(row.card.time.pending) }, { label: 'Completed without a duration', value: row => String(row.card.time.missing) }]} />
        <Text size="xs" c="dimmed" className="footnote">{costNote(rows.map(row => row.configuration))} Output tokens include reasoning and subagents.</Text>
      </Stack></Tabs.Panel>

      <Tabs.Panel value="candidates" ><Stack>
        <TabIntro lede="Problems that reviews reported but that are not yet references, waiting for a human ruling.">A candidate is a reported problem that is not yet a reference. Only a saved human ruling makes it one, and then every selected review of that PR is graded again. Until then it earns no credit, impact and reliability recommendations on its PR are withheld, and an audited control on its PR is provisional.</TabIntro>
        <Title order={4}>Novel candidates awaiting a ruling</Title>
        {novel.length ? <Table.ScrollContainer minWidth={900}><Table aria-label="Novel candidates awaiting a ruling" className="data-table prose-table">
          <Table.Thead><Table.Tr><Table.Th scope="col">PR</Table.Th><Table.Th scope="col">Age</Table.Th><Table.Th scope="col">Claim</Table.Th><Table.Th scope="col">Evidence limits</Table.Th><Table.Th scope="col">Decision relevance</Table.Th></Table.Tr></Table.Thead>
          <Table.Tbody>{novel.map(candidate => <Table.Tr key={candidate.id}><Table.Th scope="row"><button className="text-button" onClick={() => onInspect({ kind: 'task', id: candidate.taskId })}>{name(candidate.taskId)}</button></Table.Th>
            <Table.Td>{candidate.ageDays} {candidate.ageDays === 1 ? 'day' : 'days'}<Text size="xs" c="dimmed" className="nowrap">since {candidate.recordedAt.slice(0, 10)}</Text></Table.Td>
            <Table.Td title={`Record ${candidate.id}`}>{candidate.claim}</Table.Td><Table.Td><Referenced text={candidate.limits} task={task(candidate.taskId)} /></Table.Td><Table.Td><Referenced text={candidate.relevance} task={task(candidate.taskId)} /></Table.Td></Table.Tr>)}</Table.Tbody>
        </Table></Table.ScrollContainer> : <div className="empty-state" role="status">No novel candidate awaits a ruling on the selected PRs.</div>}
        <Title order={4}>Reference families awaiting eligibility</Title>
        {awaiting.length ? <Table.ScrollContainer minWidth={700}><Table aria-label="Reference families awaiting eligibility" className="data-table">
          <Table.Thead><Table.Tr><Table.Th scope="col">PR</Table.Th><Table.Th scope="col">Problem</Table.Th><Table.Th scope="col">Why it is pending</Table.Th></Table.Tr></Table.Thead>
          <Table.Tbody>{awaiting.map(({ taskId, family }) => <Table.Tr key={`${taskId}/${family.id}`}><Table.Th scope="row"><button className="text-button" onClick={() => onInspect({ kind: 'task', id: taskId })}>{name(taskId)}</button></Table.Th>
            <Table.Td title={`Known problem ${family.id}`}>{family.title}</Table.Td><Table.Td>{family.eligibilityReason}</Table.Td></Table.Tr>)}</Table.Tbody>
        </Table></Table.ScrollContainer> : <div className="empty-state" role="status">Every reference family on the selected PRs has an eligibility ruling.</div>}
      </Stack></Tabs.Panel>

      <Tabs.Panel value="comparison" ><Stack>
        <TabIntro lede="Pick two setups to see how their detection differs, PR by PR." />
        <Group><Select label="Comparison A" aria-label="Comparison A" value={a?.configuration.id ?? null} data={configurations.map(c => ({ value: c.id, label: setupLabel(c) }))} onChange={setLeft} w={330} />
          <Select label="Comparison B" aria-label="Comparison B" value={b?.configuration.id ?? null} data={configurations.filter(c => c.id !== a?.configuration.id).map(c => ({ value: c.id, label: setupLabel(c) }))} onChange={setRight} w={330} /></Group>
        {!comparison || !a || !b ? <Text>Select at least two setups.</Text> : <>
          <div className="compare-figures">
            <div className="compare-figure"><span className="compare-label">A minus B, {estimatorLabels[estimator]}</span><span className="compare-value"><Value cell={reading(comparison.full[estimator].delta, value => signed(value * 100))} /></span></div>
            <div className="compare-figure"><span className="compare-label">A minus B, {estimatorLabels[estimator === 'equalPr' ? 'equalProblem' : 'equalPr']}</span><span className="compare-value"><Value cell={reading(comparison.full[estimator === 'equalPr' ? 'equalProblem' : 'equalPr'].delta, value => signed(value * 100))} /></span></div>
            <div className="compare-figure"><span className="compare-label">Shared PRs with {scope} references, A against B</span><span className="compare-value">{comparison.wins} higher · {comparison.ties} tied · {comparison.losses} lower</span>
              {comparison.pending > 0 && <span className="compare-note">{comparison.pending} without a final comparison</span>}</div>
          </div>
          <Text size="sm" c="dimmed">{lowerFirst(detectionLabel(detection)).replace(/^./, first => first.toUpperCase())}, in percentage points.{omissionRange && ` Leaving any one whole PR out, A minus B ranges from ${signed(omissionRange.low * 100)} to ${signed(omissionRange.high * 100)}; this is sensitivity to these PRs, not a confidence interval.`}
            {comparison.aggregationSensitive.kind === 'available' && comparison.aggregationSensitive.value && ' The two averages order these setups differently, so this comparison depends on the average chosen.'}</Text>
          {[a, b].map(side => side.card.detection[band].zeroCredit.reason && <Text size="sm" key={side.configuration.id}>{side.configuration.short}: {side.card.detection[band].zeroCredit.reason}</Text>)}
          <DeltaPlot rows={comparison.rows.flatMap(row => {
            const value = points(row.delta)
            return value === null ? [] : [{ id: row.taskId, label: name(row.taskId), delta: value }]
          })} a={setupLabel(a.configuration)} b={setupLabel(b.configuration)} />
          {advice && <div className="compare-block"><Text size="sm" fw={700}>What the evidence supports</Text>{recommendation('Serious-impact preference', advice.impact)}{recommendation('Claim-reliability preference', advice.reliability)}
            <Text size="xs" c="dimmed" className="footnote">A preference is limited to its own dimension. Nothing here names a best setup overall.</Text></div>}
          <div className="compare-block"><Text size="sm" fw={700}>Comparison conditions</Text>
            {differences.length ? <ul className="reason-list">{differences.map(difference => <li key={difference.name}>{difference.name}: {difference.values.map(row => `${row.value} (${row.configuration.short})`).join(' against ')}</li>)}</ul>
              : <Text size="sm">The recorded client, effort, permissions and billing basis match.</Text>}
            <Text size="xs" c="dimmed" className="footnote">A difference between two setups includes these conditions; it does not isolate the review method or the model.</Text></div>
          <Group justify="space-between"><Text size="xs" c="dimmed" className="footnote">Per-PR differences weight that PR's problems equally. A tie means the two values are equal up to rounding; it is not a significance test.</Text>
            <Switch size="xs" checked={showTables} onChange={event => setShowTables(event.currentTarget.checked)} label="Show per-PR tables" className="nowrap-switch" /></Group>
          {showTables && <><Table.ScrollContainer minWidth={800}><Table aria-label="Per-PR detection and repetition variation">
            <Table.Thead><Table.Tr><Table.Th>PR</Table.Th><Table.Th>References</Table.Th><Table.Th>A mean</Table.Th><Table.Th>B mean</Table.Th><Table.Th>A minus B</Table.Th><Table.Th>Individual repetitions</Table.Th></Table.Tr></Table.Thead>
            <Table.Tbody>{comparison.rows.map(row => {
              const sides = [a, b].flatMap(side => side.card.tasks.find(item => item.taskId === row.taskId) ?? [])
              return <Table.Tr key={row.taskId}>
                <Table.Td>{name(row.taskId)}</Table.Td><Table.Td>{sides[0]?.bands[band].problems ?? 'Unavailable'}</Table.Td>
                {sides.map((side, index) => <Table.Td key={index}><Value cell={share(side.bands[band].recall)} />{side.bands[band].zeroCredit.reason && <Text size="xs" c="dimmed">{side.bands[band].zeroCredit.reason}</Text>}<Text size="xs" c="dimmed">Observed {percent(scaled(side.bands[band].low))} to {percent(scaled(side.bands[band].high))}</Text></Table.Td>)}
                <Table.Td><Value cell={reading(row.delta, value => signed(value * 100))} /></Table.Td><Table.Td>{sides.map((side, index) => <Text size="xs" key={index}>{index === 0 ? 'A' : 'B'}: {side.trials.map((trial, position) => {
                  const value = side.bands[band].repetitions[position] ?? null
                  return `#${trial.replicate} ${value === null ? 'undetermined' : percent(value * 100)}${trial.state === 'unadmitted' ? ' failed' : trial.state === 'admitted' && !trial.complete ? ' incomplete' : ''}`
                }).join(', ')}</Text>)}</Table.Td>
              </Table.Tr>
            })}</Table.Tbody>
          </Table></Table.ScrollContainer>
          <Table.ScrollContainer minWidth={700}><Table aria-label="Leave-one-PR-out detection comparison">
            <Table.Thead><Table.Tr><Table.Th>Omitted PR</Table.Th><Table.Th>Remaining PRs</Table.Th><Table.Th>A detection</Table.Th><Table.Th>B detection</Table.Th><Table.Th>A minus B, per PR</Table.Th><Table.Th>A minus B, per problem</Table.Th></Table.Tr></Table.Thead>
            <Table.Tbody>{comparison.omissions.map(row => <Table.Tr key={row.taskId}><Table.Td>{name(row.taskId)}</Table.Td><Table.Td>{row.remaining}</Table.Td>
              <Table.Td><Value cell={share(row[estimator].a)} /></Table.Td><Table.Td><Value cell={share(row[estimator].b)} /></Table.Td>
              <Table.Td><Value cell={reading(row.equalPr.delta, value => signed(value * 100))} /></Table.Td><Table.Td><Value cell={reading(row.equalProblem.delta, value => signed(value * 100))} /></Table.Td></Table.Tr>)}</Table.Tbody>
          </Table></Table.ScrollContainer></>}
          {!comparison.rows.length && <Text>No shared PR has {scope} references under these filters.</Text>}
        </>}
        <Text size="xs" c="dimmed" className="footnote">Each omission removes the entire PR and all its repetitions. Repetition indices are not matched experimental seeds.</Text>
      </Stack></Tabs.Panel>
    </Tabs>
  </Paper>
}

function DeltaPlot({ rows, a, b }: { rows: { id: string; label: string; delta: number }[]; a: string; b: string }) {
  if (!rows.length) return null
  const limit = Math.max(25, ...rows.map(row => Math.abs(row.delta)))
  const at = (value: number) => 50 + value / limit * 50
  return <div className="delta-plot" role="list" aria-label="A minus B detection difference per PR">
    <div className="delta-head" aria-hidden="true"><span /><span className="delta-sides"><span><b>← B higher</b>{b}</span><span><b>A higher →</b>{a}</span></span></div>
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

function TabIntro({ lede, children }: { lede: string; children?: ReactNode }) {
  return <div className="tab-intro"><Text className="tab-lede">{lede}</Text>
    {children && <Disclosure title="How this is measured"><Text size="sm" className="disclosure-body">{children}</Text></Disclosure>}</div>
}

function PreExistingMatrix({ label, rows, name }: { label: PreExistingLabel; rows: { configuration: Configuration; card: Scorecard }[]; name: (taskId: string) => string }) {
  const problems = rows[0]?.card.preExisting.labels[label].rows ?? []
  if (!problems.length) return null
  const groups = rows.reduce<{ method: string; span: number }[]>((all, row) => {
    const { method } = setupParts(row.configuration)
    const last = all.at(-1)
    return last?.method === method ? [...all.slice(0, -1), { method, span: last.span + 1 }] : [...all, { method, span: 1 }]
  }, [])
  return <>
    <Text size="sm">Each cell is how many assessed admitted reviews raised the problem, out of those assessed.</Text>
    <Table.ScrollContainer className="matrix-scroll" minWidth={320 + rows.length * 104}><Table aria-label={`${bandLabels[label]} pre-existing problems raised, by setup`} className="data-table pre-matrix">
      <Table.Thead>
        <Table.Tr><Table.Th scope="col" rowSpan={2} className="pre-sticky">Problem</Table.Th>
          {groups.map((group, index) => <Table.Th key={`${group.method}-${index}`} scope="colgroup" colSpan={group.span} className="pre-method">{group.method}</Table.Th>)}</Table.Tr>
        <Table.Tr>{rows.map(row => <Table.Th key={row.configuration.id} scope="col" data-numeric><VendorMark configuration={row.configuration} />{[setupParts(row.configuration).model, setupParts(row.configuration).effort].filter(Boolean).join(' · ')}</Table.Th>)}</Table.Tr>
      </Table.Thead>
      <Table.Tbody>{problems.map(problem => <Table.Tr key={`${problem.taskId}/${problem.problemId}`}>
        <Table.Th scope="row" className="pre-sticky"><Rich text={problem.title} /><Text size="xs" c="dimmed">{name(problem.taskId)}</Text></Table.Th>
        {rows.map(row => {
          const cell = row.card.preExisting.labels[label].rows.find(item => item.problemId === problem.problemId && item.taskId === problem.taskId)
          return <Table.Td key={row.configuration.id} data-numeric data-raised={cell && cell.raised > 0 ? true : undefined}>
            {cell ? <>{cell.raised}<span className="pre-of"> / {cell.assessed}</span>{cell.unresolved > 0 && <span className="pre-open">{cell.unresolved} open</span>}</> : '—'}</Table.Td>
        })}
      </Table.Tr>)}</Table.Tbody>
    </Table></Table.ScrollContainer>
  </>
}

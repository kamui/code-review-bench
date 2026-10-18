import { useState } from 'react'
import { Tooltip, VisuallyHidden } from '@mantine/core'
import type { CSSProperties, FocusEvent, MouseEvent, ReactNode } from 'react'
import { ArrowDown, ArrowUp, ArrowUpDown } from 'lucide-react'
import { linearScale, position, ticks } from '../lib/chart'
import type { Scale } from '../lib/chart'
import { bandLabels, detectionLabel, duration, lowerFirst, measured, money, perReview, reading, share } from '../lib/metrics'
import type { DetectionView, Reading, Summary } from '../lib/metrics'
import { labelSegments, overviewColumns, overviewFirstDirection, overviewKey, overviewOrder } from '../lib/scoring'
import type { LabelSegment, OverviewColumn, OverviewSort, Scorecard } from '../lib/scoring'
import { Mark, reviewColor, setupParts } from './Chart'
import { segmentClass, segmentLabels, swatch, trueSegments } from './ClaimLabels'
import { Notes, notesFor, Value } from './Reading'

const costScale: Scale = { kind: 'log', min: 0.001, max: 10 }
const costTicks = [0.001, 0.01, 0.1, 1, 10]
const timeScale: Scale = { kind: 'log', min: 20, max: 1800 }
const timeTicks = [20, 60, 300, 1800]
const shareTicks = [0, 25, 50, 75, 100]
const at = (scale: Scale, value: number) => position(scale, value, 0, 100)

const problemBands: Partial<Record<LabelSegment, 'serious' | 'other-material' | 'unknown'>> = { 'problem:serious': 'serious', 'problem:other-material': 'other-material', 'problem:unknown': 'unknown' }

function segmentTip(card: Scorecard, segment: LabelSegment, value: number) {
  const text = `${segmentLabels[segment]} · ${value.toFixed(2)} claims per review`
  const band = problemBands[segment]
  if (!band) return text
  const caught = card.problemsCaught[band], known = `${bandLabels[band].toLowerCase()} known problems`
  return caught.kind === 'available' ? `${text} · caught ${caught.value.caught} of ${caught.value.known} ${known}` : `${text} · ${known} caught: unavailable. ${caught.reason}`
}

type Tip = { text: string; left: number; top: number }
const doubtMeaning = 'Share of admitted reviews with at least one refuted or unproven claim. Hatched: reviews whose only doubt is an unresolved claim.'

type Heading = { title: string; short: string; hint?: string; note?: string; measure: string; ascending: string; descending: string; ends?: [string, string] }

function headings(detection: DetectionView, claimsMax: number): Record<OverviewColumn, Heading> {
  return {
    setup: { title: 'Review setup', short: 'Setup', measure: 'setup name', ascending: 'A to Z', descending: 'Z to A' },
    detection: { title: detectionLabel(detection), short: 'Detection', measure: lowerFirst(detectionLabel(detection)), ascending: 'least first', descending: 'most first', ends: ['0%', '100%'] },
    claims: { title: 'True claims per review', note: 'number: problem claims', short: 'Problem claims per review', measure: 'problem claims per review', ascending: 'fewest first', descending: 'most first', ends: ['0', String(claimsMax)] },
    doubt: { title: 'Refuted or unproven', short: 'Refuted or unproven', hint: doubtMeaning, measure: 'share of reviews with a refuted or unproven claim', ascending: 'fewest first', descending: 'most first', ends: ['0%', '100%'] },
    cost: { title: 'Cost per scheduled trial', short: 'Cost per trial', measure: 'cost per scheduled trial', ascending: 'lowest first', descending: 'highest first', note: 'log scale', ends: ['$0.001', '$10'] },
    time: { title: 'Median completed-trial time', short: 'Median time', measure: 'median completed-trial time', ascending: 'shortest first', descending: 'longest first', note: 'log scale', ends: ['20 s', '30 min'] },
  }
}

export function Overview({ summaries, detection, sort, onSort, onSelect }: {
  summaries: Summary[]; detection: DetectionView; sort: OverviewSort; onSort: (sort: OverviewSort) => void; onSelect: (id: string) => void
}) {
  const [tip, setTip] = useState<Tip | null>(null)
  const show = (text: string) => (event: MouseEvent<Element> | FocusEvent<Element>) => {
    const shell = event.currentTarget.closest('.overview')?.getBoundingClientRect(), box = event.currentTarget.getBoundingClientRect()
    if (shell) setTip({ text, left: box.left + box.width / 2 - shell.left, top: box.top - shell.top })
  }
  const rows = overviewOrder(summaries, summary => ({ name: summary.configuration.short, card: summary.card }), sort, detection.band, detection.estimator)
  const claimScale = linearScale(rows.map(summary => trueSegments.reduce((total, segment) => total + (measured(summary.card.reliability.labelsPerAdmittedReview[segment]) ?? 0), 0)))
  const heads = headings(detection, claimScale.max)
  const readings = rows.map(summary => {
    const { card, configuration } = summary
    const recall = card.detection[detection.band][detection.estimator]
    const labels = labelSegments.map(segment => card.reliability.labelsPerAdmittedReview[segment]).find(rate => rate.kind === 'unavailable')
    return {
      detection: share(recall, summary.detectionNote),
      claims: labels?.kind === 'unavailable' ? reading(labels, String) : perReview(card.reliability.problemClaimsPerReview),
      doubt: share(card.reliability.reviewsWithDoubt.refutedOrUnproven), unresolved: share(card.reliability.reviewsWithDoubt.onlyUnresolved),
      cost: reading(card.cost.perTrial, value => `${money(value)}${configuration.billing === 'list-price-equivalent' ? '*' : ''}`),
      time: reading(card.time.summary, value => duration(value.median)),
    }
  })
  const notes = notesFor(readings.flatMap(cells => [cells.detection, cells.claims, cells.doubt, cells.cost, cells.time]))
  const active = heads[sort.column]
  const missing = sort.column !== 'setup' && rows.some(summary => overviewKey(summary.card, sort.column as Exclude<OverviewColumn, 'setup'>, detection.band, detection.estimator) === null)
  const rule = `Sorted by ${active.measure}, ${active[sort.direction]}.${missing ? ' Setups without a value come last.' : ''}`
  const toggle = (column: OverviewColumn) => onSort(column === sort.column
    ? { column, direction: sort.direction === 'ascending' ? 'descending' : 'ascending' } : { column, direction: overviewFirstDirection[column] })
  return <div className="overview" onMouseLeave={() => setTip(null)}>
    <p className="overview-rule" aria-live="polite">{rule}</p>
    <div role="table" aria-label={`Overview of the selected setups. ${rule}`} className="overview-table">
      <div role="row" className="overview-head">{overviewColumns.map(column => {
        const heading = heads[column], current = sort.column === column
        return <div role="columnheader" key={column} className="overview-axis" aria-sort={current ? sort.direction : 'none'}>
          <Tooltip label={heading.hint} disabled={!heading.hint} multiline w={280} withArrow>
            <button type="button" className="text-button sort-heading overview-sort" onClick={() => toggle(column)} aria-description={heading.hint}>
              <span className="overview-title">{heading.title}{heading.note && <em>{heading.note}</em>}</span><span className="overview-short">{heading.short}</span>
              {current ? sort.direction === 'ascending' ? <ArrowUp size={13} aria-hidden="true" /> : <ArrowDown size={13} aria-hidden="true" /> : <ArrowUpDown size={13} aria-hidden="true" />}
            </button>
          </Tooltip>
          {heading.ends && <span className="overview-ends" aria-hidden="true"><span>{heading.ends[0]}</span><span>{heading.ends[1]}</span></span>}
        </div>
      })}</div>
      {rows.map((summary, index) => {
        const { configuration, card } = summary
        const cells = readings[index]!
        const { method, model, effort } = setupParts(configuration)
        const rates = card.reliability.labelsPerAdmittedReview
        const doubt = measured(card.reliability.reviewsWithDoubt.refutedOrUnproven), unresolved = measured(card.reliability.reviewsWithDoubt.onlyUnresolved)
        const cost = card.cost.perTrial, time = measured(card.time.summary)
        let offset = 0
        return <div role="row" key={configuration.id} className="overview-row" style={{ '--mark-color': reviewColor(configuration) } as CSSProperties} onClick={() => onSelect(configuration.id)}>
          <div role="rowheader" className="overview-name">
            <button type="button" className="text-button rank-name" onClick={event => { event.stopPropagation(); onSelect(configuration.id) }}
              aria-label={`${configuration.short}. Inspect evidence`}>
              <Mark configuration={configuration} /><span><strong>{method}</strong><span className="rank-model">{model}{effort && ` · ${effort}`}</span></span>
            </button>
          </div>
          <Cell label={heads.detection.short} cell={cells.detection} notes={notes} grid={shareTicks}>
            {summary.detection !== null && <span className="overview-bar" style={{ width: `${summary.detection}%` }} />}</Cell>
          <Cell label={heads.claims.short} cell={cells.claims} notes={notes} grid={ticks(claimScale).map(tick => at(claimScale, tick))} interactive>
            {!cells.claims.reason && trueSegments.map(segment => {
              const value = measured(rates[segment]) ?? 0
              if (value <= 0) return null
              const start = offset
              offset += value
              const text = segmentTip(card, segment, value)
              return <span key={segment} className={segmentClass('overview-segment', segment)} role="img" tabIndex={0} aria-label={text}
                onMouseEnter={show(text)} onFocus={show(text)} onMouseLeave={() => setTip(null)} onBlur={() => setTip(null)}
                style={{ ...swatch(segment), left: `${at(claimScale, start)}%`, width: `${at(claimScale, value)}%` }} />
            })}</Cell>
          <Cell label={heads.doubt.short} cell={cells.doubt} notes={notes} grid={shareTicks} description={unresolved ? `, and ${cells.unresolved.text} whose only doubt is unresolved` : undefined}>
            {doubt !== null && <span className="overview-segment" style={{ ...swatch('refuted'), left: 0, width: `${doubt * 100}%` }} title={`Refuted or unproven, ${cells.doubt.text} of reviews`} />}
            {doubt !== null && unresolved !== null && unresolved > 0 && <span className={segmentClass('overview-segment', 'unresolved')} title={`Only doubt is unresolved, ${cells.unresolved.text} of reviews`}
              style={{ ...swatch('unresolved'), left: `${doubt * 100}%`, width: `${unresolved * 100}%` }} />}</Cell>
          <Cell label={heads.cost.short} cell={cells.cost} notes={notes} grid={costTicks.map(tick => at(costScale, tick))} floor>
            {cost.kind !== 'unavailable' && <Stem left={at(costScale, cost.value)} hollow={cost.kind === 'atLeast'} />}</Cell>
          <Cell label={heads.time.short} cell={cells.time} notes={notes} grid={timeTicks.map(tick => at(timeScale, tick))} floor>
            {time && <span className="overview-spread" style={{ left: `${at(timeScale, time.q1)}%`, width: `${at(timeScale, time.q3) - at(timeScale, time.q1)}%` }} />}
            {time && <Stem left={at(timeScale, time.median)} />}</Cell>
        </div>
      })}
    </div>
    {tip && <div className="chart-tip" role="presentation" style={{ left: tip.left, top: tip.top }}>{tip.text}</div>}
    <ul className="label-legend overview-legend" aria-label="Overview key">
      {trueSegments.map(segment => <li key={segment}><span className={segmentClass('label-swatch', segment)} style={swatch(segment)} aria-hidden="true" />{segmentLabels[segment]}</li>)}
      <li><span className="label-swatch" style={swatch('refuted')} aria-hidden="true" />Refuted or unproven</li>
      <li><span className="label-swatch hatched" style={swatch('unresolved')} aria-hidden="true" />Only doubt is unresolved</li>
      <li><span className="overview-key-dot" aria-hidden="true" />Lower-bound cost</li>
      <li><span className="overview-key-spread" aria-hidden="true" />Middle 50% of completed trials</li>
    </ul>
    <Notes notes={notes} />
    <div className="plot-caption">
      <span>Each column has its own scale; a header sorts by that column. Detection uses the impact band and average chosen above. True claims stack every true label from 0, and the number is problem claims per review. Refuted, unproven and unresolved claims are not drawn there.</span>
      <span>Refuted or unproven: {doubtMeaning} An unproven claim is not shown to be false.</span>
      <span>Cost and time use log scales from a marked floor; a value below the floor sits on it. Cost adds every attempt, retries included, per scheduled trial. Time covers completed trials only.</span>
      {rows.some(summary => summary.configuration.billing === 'list-price-equivalent') && <span>* Subscription usage valued at token list prices, not a bill or quota measurement.</span>}
    </div>
  </div>
}

function Cell({ label, cell, notes, grid, floor, interactive, description, children }: {
  label: string; cell: Reading; notes: ReturnType<typeof notesFor>; grid: number[]; floor?: true; interactive?: true; description?: string; children: ReactNode
}) {
  return <div role="cell" className={cell.text === 'Unavailable' ? 'overview-cell missing' : 'overview-cell'}><span className="overview-label" aria-hidden="true">{label}</span>
    <span className="overview-value"><Value cell={cell} notes={notes} />{description && <VisuallyHidden>{description}</VisuallyHidden>}</span>
    <span className="overview-track" aria-hidden={interactive ? undefined : true}>{grid.map(left => <span key={left} className="rank-grid" style={{ left: `${left}%` }} />)}
      {floor && <span className="overview-floor" />}{children}</span></div>
}

function Stem({ left, hollow = false }: { left: number; hollow?: boolean }) {
  return <><span className="overview-stem" style={{ width: `${left}%` }} /><span className={hollow ? 'overview-dot hollow' : 'overview-dot'} style={{ left: `${left}%` }} /></>
}

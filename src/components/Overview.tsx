import { useState } from 'react'
import { VisuallyHidden } from '@mantine/core'
import type { CSSProperties, FocusEvent, MouseEvent, ReactNode } from 'react'
import { ArrowDown, ArrowUp } from 'lucide-react'
import { linearScale, logScale, position, ticks, tipPlace } from '../lib/chart'
import type { Scale, TipPlace } from '../lib/chart'
import { bandLabels, duration, estimatorLabels, foundLabels, lowerFirst, measured, perReview, reading, share } from '../lib/metrics'
import type { DetectionView, Reading, Summary } from '../lib/metrics'
import type { Configuration } from '../lib/data'
import { labelSegments, overviewFirstDirection, overviewOrder } from '../lib/scoring'
import type { LabelSegment, Measure, OverviewColumn, Scorecard } from '../lib/scoring'
import { Chart, Mark, reviewColor, SetupName, setupParts } from './Chart'
import { segmentLabels, trueSegments } from './ClaimLabels'
import { VendorMark } from './VendorMark'
import { costScaleNote, costText, Notes, notesFor, PeerBadge, Value } from './Reading'

type Column = Exclude<OverviewColumn, 'setup'>
const columns: Column[] = ['detection', 'claims', 'doubt', 'cost', 'time']

const money = (value: number) => value >= 1 ? `$${value}` : `$${value.toFixed(Math.max(2, -Math.floor(Math.log10(value))))}`
const timeMarks: [number, string][] = [[10, '10 s'], [30, '30 s'], [60, '1 min'], [300, '5 min'], [600, '10 min'], [1800, '30 min'], [3600, '1 h'], [7200, '2 h']]
function timeScale(values: number[]): { scale: Scale; marks: [number, string][] } {
  const low = Math.min(...values, 60), high = Math.max(...values, 300)
  const min = [...timeMarks].reverse().find(([seconds]) => seconds <= low)?.[0] ?? timeMarks[0]![0]
  const max = timeMarks.find(([seconds]) => seconds >= high)?.[0] ?? timeMarks.at(-1)![0]
  return { scale: { kind: 'log', min, max }, marks: timeMarks.filter(([seconds]) => seconds >= min && seconds <= max) }
}
const at = (scale: Scale, value: number) => position(scale, value, 0, 100)
const spaced = (axis: Axis, gap: number): Axis => axis.reduce<{ kept: Axis; last: number }>(({ kept, last }, tick) =>
  tick.label && tick.at - last >= gap ? { kept: [...kept, tick], last: tick.at } : { kept: [...kept, { at: tick.at }], last }, { kept: [], last: -Infinity }).kept

const problemBands: Partial<Record<LabelSegment, 'serious' | 'other-material' | 'unknown'>> = { 'problem:serious': 'serious', 'problem:other-material': 'other-material', 'problem:unknown': 'unknown' }
const problemSegments = trueSegments.filter(segment => segment in problemBands)
const sum = (rates: Record<LabelSegment, Measure>, segments: LabelSegment[]) => segments.reduce((total, segment) => total + (measured(rates[segment]) ?? 0), 0)

function segmentLines(card: Scorecard, segment: LabelSegment, value: number): TipLine[] {
  const lines = [{ label: segmentLabels[segment], value: `${value.toFixed(2)} per review` }]
  const band = problemBands[segment]
  if (!band) return lines
  const caught = card.problemsCaught[band], label = `${bandLabels[band]} known problems caught`
  return [...lines, caught.kind === 'available' ? { label, value: `${caught.value.caught} of ${caught.value.known}` } : { label, value: 'Unavailable', note: caught.reason }]
}

type TipLine = { label: string; value: string; note?: string }
type Tip = { configuration: Configuration; lines: TipLine[] } & TipPlace
type Key = { label: string; className: string }
type Axis = { at: number; label?: string }[]
type Panel = { title: string; about: string; order: string; keys?: Key[]; axis: Axis }

const KeyList = ({ items }: { items: Key[] }) => <span className="column-keys">{items.map(item =>
  <span key={item.label}><span className={`column-key ${item.className}`} aria-hidden="true" />{item.label}</span>)}</span>

const weight = ({ band, estimator }: DetectionView, { problems, prs }: { problems: number; prs: number }) => !problems || !prs ? ''
  : estimator === 'equalProblem' ? `The ${problems} ${band === 'all' ? 'known' : lowerFirst(bandLabels[band])} problems weigh the same, so one problem caught in every review is ${(100 / problems).toFixed(1)} points.`
    : `The ${prs} PRs weigh the same, so each PR is ${(100 / prs).toFixed(0)} points, shared among its problems.`

const orderText: Record<Column, string> = { detection: 'Most first', claims: 'Most first', doubt: 'Fewest first', cost: 'Lowest first', time: 'Shortest first' }

export function Overview({ summaries, detection, matching, basis, onSelect }: {
  summaries: Summary[]; detection: DetectionView; matching: { included: number; excluded: number }; basis: { problems: number; prs: number }; onSelect: (id: string) => void
}) {
  const [tipState, setTip] = useState<Tip | null>(null)
  const [focus, setFocus] = useState<string | null>(null)
  if (!summaries.length) return <div className="plot-empty" role="status"><span>Select at least one review method and model.</span></div>
  const shown = summaries.map(summary => summary.configuration)
  const ordered = (column: Column) => overviewOrder(summaries, summary => ({ name: summary.configuration.short, card: summary.card }),
    { column, direction: overviewFirstDirection[column] }, detection.band, detection.estimator)
  const claimScale = linearScale(summaries.map(summary => sum(summary.card.reliability.labelsPerAdmittedReview, trueSegments)))
  const costs = summaries.flatMap(summary => summary.card.cost.perTrial.kind === 'unavailable' ? [] : [summary.card.cost.perTrial.value])
  const costScale = logScale(costs.length ? costs : [0.1, 1])
  const times = summaries.flatMap(summary => { const time = measured(summary.card.time.summary); return time ? [time.q1, time.q3] : [] })
  const time = timeScale(times)
  const shareAxis: Axis = [0, 25, 50, 75, 100].map(value => ({ at: value, label: `${value}%` }))
  const halfAxis: Axis = shareAxis.map(tick => tick.at % 50 ? { at: tick.at } : tick)
  const panels: Record<Column, Panel> = {
    detection: { title: foundLabels[detection.band], about: `Share of the band’s known problems that reviews caught, ${estimatorLabels[detection.estimator]}.`, order: orderText.detection, axis: shareAxis },
    claims: { title: 'Problem claims per review', about: 'True claims that name a problem. The bar continues with the review’s other true claims.', order: orderText.claims,
      keys: [{ label: 'Problem', className: 'key-solid' }, { label: 'Other true', className: 'key-rest' }], axis: ticks(claimScale).map(value => ({ at: at(claimScale, value), label: String(value) })) },
    doubt: { title: 'Refuted or unproven', about: 'Share of admitted reviews with at least one refuted or unproven claim. An unproven claim is not shown to be false.', order: orderText.doubt,
      keys: [{ label: 'Refuted or unproven', className: 'key-solid' }, { label: 'Only doubt unresolved', className: 'key-hatched' }], axis: halfAxis },
    cost: { title: 'Cost per trial', about: `Every attempt, retries included, per scheduled trial; ${costScaleNote(shown)}. A hollow dot is a lower bound.`, order: orderText.cost,
      axis: spaced(ticks(costScale).map(value => ({ at: at(costScale, value), ...Number.isInteger(Math.log10(value)) ? { label: money(value) } : {} })), 26) },
    time: { title: 'Median time', about: 'Completed trials only; the band spans the middle half of trials. Log scale.', order: orderText.time,
      axis: spaced(time.marks.map(([seconds, label]) => ({ at: at(time.scale, seconds), label })), 22) },
  }
  const readings = new Map(summaries.map(summary => {
    const { card, configuration } = summary
    const labels = labelSegments.map(segment => card.reliability.labelsPerAdmittedReview[segment]).find(rate => rate.kind === 'unavailable')
    const cells: Record<Column, Reading> & { unresolved: Reading } = {
      detection: share(card.detection[detection.band][detection.estimator], summary.detectionNote),
      claims: labels?.kind === 'unavailable' ? reading(labels, String) : perReview(card.reliability.problemClaimsPerReview),
      doubt: share(card.reliability.reviewsWithDoubt.refutedOrUnproven), unresolved: share(card.reliability.reviewsWithDoubt.onlyUnresolved),
      cost: reading(card.cost.perTrial, costText(configuration, shown)),
      time: reading(card.time.summary, value => duration(value.median)),
    }
    return [configuration.id, cells]
  }))
  const notes = notesFor([...readings.values()].flatMap(cells => columns.map(column => cells[column])))
  const show = (configuration: Configuration, lines: TipLine[]) => (event: MouseEvent<Element> | FocusEvent<Element>) => {
    const shell = event.currentTarget.closest('.overview')?.getBoundingClientRect(), box = event.currentTarget.getBoundingClientRect()
    if (shell) setTip({ configuration, lines, ...tipPlace(shell, box, 160, Infinity) })
  }

  function marks(column: Column, summary: Summary): { draw: ReactNode; more: TipLine[]; description?: string } {
    const { card } = summary
    const cells = readings.get(summary.configuration.id)!
    switch (column) {
      case 'detection': return { draw: summary.detection !== null && <span className="measure-bar" style={{ width: `${summary.detection}%` }} />, more: [] }
      case 'claims': {
        if (cells.claims.reason) return { draw: null, more: [] }
        const rates = card.reliability.labelsPerAdmittedReview, problems = sum(rates, problemSegments), others = sum(rates, trueSegments) - problems
        const more = trueSegments.flatMap(segment => { const value = measured(rates[segment]) ?? 0; return value > 0 ? segmentLines(card, segment, value) : [] })
        return { more, description: more.length ? `. By label: ${more.map(line => `${line.label} ${line.value}`).join(', ')}` : undefined,
          draw: <><span className="measure-bar" style={{ width: `${at(claimScale, problems)}%` }} />
            {others > 0 && <span className="measure-bar measure-rest" style={{ left: `${at(claimScale, problems)}%`, width: `${at(claimScale, others)}%` }} />}</> }
      }
      case 'doubt': {
        const doubt = measured(card.reliability.reviewsWithDoubt.refutedOrUnproven), unresolved = measured(card.reliability.reviewsWithDoubt.onlyUnresolved)
        return { more: unresolved ? [{ label: 'Only doubt unresolved', value: cells.unresolved.text }] : [], description: unresolved ? `, and ${cells.unresolved.text} whose only doubt is unresolved` : undefined,
          draw: doubt !== null && <><span className="measure-bar" style={{ width: `${doubt * 100}%` }} />
            {unresolved !== null && unresolved > 0 && <span className="measure-bar measure-unresolved" style={{ left: `${doubt * 100}%`, width: `${unresolved * 100}%` }} />}</> }
      }
      case 'cost': {
        const cost = card.cost.perTrial
        return { more: [], draw: cost.kind !== 'unavailable' && <Stem left={at(costScale, cost.value)} hollow={cost.kind === 'atLeast'} /> }
      }
      case 'time': {
        const spread = measured(card.time.summary)
        return { more: spread ? [{ label: 'Middle half', value: `${duration(spread.q1)} to ${duration(spread.q3)}` }] : [],
          draw: spread && <><span className="measure-spread" style={{ left: `${at(time.scale, spread.q1)}%`, width: `${at(time.scale, spread.q3) - at(time.scale, spread.q1)}%` }} />
            <Stem left={at(time.scale, spread.median)} /></> }
      }
    }
  }

  return <div className="overview" data-focusing={focus ? '' : undefined} onMouseLeave={() => { setTip(null); setFocus(null) }}>
    <div className="measure-grid">
      <section className="measure-card lead-card" data-lead aria-labelledby="measure-lead">
        <header className="measure-head"><div><h3 id="measure-lead">{foundLabels[detection.band]} against cost and refuted or unproven claims</h3>
          <p className="lead-about">Each point is one setup; up and to the left is better on both. {weight(detection, basis)}</p></div></header>
        <Chart summaries={summaries} detection={detection} matching={matching} axis="cost" view="pair" showAllLabels={false} onSelect={onSelect}
          guide={<><span>Below the scatters, each chart ranks the setups by one measure, on its own scale and in its own order; a setup without a value comes last. Point at a setup to find it in every chart, and select it to open its evidence.</span>
            <span>The charts stay separate on purpose: no single number ranks the setups.</span></>} />
      </section>
      {columns.map(column => {
        const panel = panels[column], id = `measure-${column}`
        return <section key={column} className="measure-card" data-lead={column === 'detection' || undefined} aria-labelledby={id}>
          <header className="measure-head">
            <div><h3 id={id}>{panel.title}</h3><p>{panel.about}</p></div>
            <span className="measure-order">{overviewFirstDirection[column] === 'ascending' ? <ArrowUp size={13} aria-hidden="true" /> : <ArrowDown size={13} aria-hidden="true" />}{panel.order}</span>
          </header>
          {panel.keys && <KeyList items={panel.keys} />}
          <div role="table" aria-label={`${panel.title}, ${panel.order.toLowerCase()}`} className="measure-rows">
            <div role="row" className="measure-axis">
              <VisuallyHidden role="columnheader">Setup</VisuallyHidden><VisuallyHidden role="columnheader">{panel.title}</VisuallyHidden>
              <span className="measure-ticks" aria-hidden="true">{panel.axis.map(tick => tick.label && <span key={tick.at} data-edge={tick.at <= 0 ? 'start' : tick.at >= 100 ? 'end' : undefined} style={{ left: `${tick.at}%` }}>{tick.label}</span>)}</span>
            </div>
            {ordered(column).map(summary => {
              const { configuration } = summary
              const cell = readings.get(configuration.id)![column]
              const { draw, more, description } = marks(column, summary)
              const lines = [{ label: panel.title, value: cell.text, note: cell.reason ?? cell.annotation }, ...more]
              return <div role="row" key={configuration.id} className="measure-row" data-focus={focus === configuration.id || undefined}
                style={{ '--mark-color': reviewColor(configuration) } as CSSProperties} onMouseEnter={() => setFocus(configuration.id)} onClick={() => onSelect(configuration.id)}>
                <div role="rowheader" className="measure-name"><SetupButton configuration={configuration} badges={column === 'detection'} onSelect={onSelect} onFocus={() => setFocus(configuration.id)} /></div>
                <div role="cell" className={cell.text === 'Unavailable' ? 'measure-value missing' : 'measure-value'}><Value cell={cell} notes={notes} />{description && <VisuallyHidden>{description}</VisuallyHidden>}</div>
                <span className="measure-track" aria-hidden="true" onMouseEnter={show(configuration, lines)} onMouseLeave={() => setTip(null)}>
                  {panel.axis.map(tick => <span key={tick.at} className="rank-grid" style={{ left: `${tick.at}%` }} />)}{draw}</span>
              </div>
            })}
          </div>
        </section>
      })}
    </div>
    <Notes notes={notes} />
    {tipState && <div className={tipState.below ? 'chart-tip below' : 'chart-tip'} role="presentation" style={{ left: tipState.left, top: tipState.top }}>
      <strong><SetupName configuration={tipState.configuration} mark={false} /></strong>
      {tipState.lines.map(line => <span key={line.label} className="tip-row"><span className="tip-label">{line.label}</span><b>{line.value}</b>{line.note && <span className="tip-note">{line.note}</span>}</span>)}</div>}
  </div>
}

function SetupButton({ configuration, badges, onSelect, onFocus }: { configuration: Configuration; badges: boolean; onSelect: (id: string) => void; onFocus: () => void }) {
  const { method, model, effort } = setupParts(configuration)
  return <button type="button" className="text-button measure-setup" onFocus={onFocus} onClick={event => { event.stopPropagation(); onSelect(configuration.id) }}
    aria-label={`${configuration.short}. Inspect evidence`}>
    <Mark configuration={configuration} size={10} /><strong>{method}</strong>
    <span className="measure-model"><VendorMark configuration={configuration} />{model}{effort && ` · ${effort}`}</span>
    {badges && <PeerBadge configuration={configuration} />}{configuration.experimental && <span className="row-tag">Experiment</span>}
  </button>
}

function Stem({ left, hollow = false }: { left: number; hollow?: boolean }) {
  return <><span className="measure-stem" style={{ width: `${left}%` }} /><span className={hollow ? 'measure-dot hollow' : 'measure-dot'} style={{ left: `${left}%` }} /></>
}

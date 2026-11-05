import { useState } from 'react'
import type { CSSProperties, DOMAttributes, FocusEvent, MouseEvent, ReactNode } from 'react'
import { useElementSize } from '@mantine/hooks'
import { ArrowUpLeft } from 'lucide-react'
import { groupPaths, logScale, paretoFrontier, placeLabels, position, sqrtScale, ticks, tipPlace } from '../lib/chart'
import type { Scale, TipPlace } from '../lib/chart'
import { compact, detectionLabel, foundLabels, duration, lowerFirst, money, percent, reasonCounts } from '../lib/metrics'
import type { Axis, DetectionView, Summary } from '../lib/metrics'
import type { Configuration } from '../lib/data'
import { skillReleaseLabel } from '../lib/data'
import { ChartGuide, costNote, costStarred, Notes } from './Reading'
import type { Note } from './Reading'
import { VendorMark } from './VendorMark'

export type ChartView = 'skills' | 'setups' | 'models' | 'tradeoff' | 'pair'
type Range = { low: number; high: number } | null
type Point = { summary: Summary; range: Range; metric: number | null; bounded: boolean; starred: boolean }

const seriesKeys: Record<string, string> = {
  'claude-builtin': 'claude', codex: 'codex', 'ce-code-review': 'ce', 'thermo-nuclear-review': 'thermo-nuclear', 'thermo-nuclear-code-quality-review': 'thermo',
  'review-code/v5b-30-x382': 'review-a',
}

export function reviewColor(configuration: Pick<Configuration, 'method' | 'reviewEdition'>) {
  return `var(--series-${seriesKeys[`${configuration.method}/${configuration.reviewEdition}`] ?? seriesKeys[configuration.method] ?? 'other'})`
}

export function Mark({ configuration, size = 12, hollow = false }: { configuration: Pick<Configuration, 'method' | 'reviewEdition' | 'builtin'>; size?: number; hollow?: boolean }) {
  return <span className={`mark ${configuration.builtin ? 'mark-builtin' : 'mark-skill'}${hollow ? ' mark-hollow' : ''}`} aria-hidden="true"
    style={{ '--mark-color': reviewColor(configuration), '--mark-size': `${size}px` } as CSSProperties} />
}

export function SetupName({ configuration, mark = true }: { configuration: Configuration; mark?: boolean }) {
  const { method, model, effort } = setupParts(configuration)
  return <span className="setup-name">{mark && <Mark configuration={configuration} size={10} />}
    <span className="setup-name-text"><span className="setup-method">{method}</span>{model && <><span className="setup-sep"> / </span><span className="setup-model"><VendorMark configuration={configuration} />{model}
      {effort && <><span className="setup-sep setup-dot"> / </span>{effort}</>}</span></>}</span></span>
}

export const axisLabels: Record<Axis, string> = {
  cost: 'Cost per trial', tokens: 'Output tokens per trial', refuted: 'Refuted claims per review on matched PRs', time: 'Median time',
}
const setups = (count: number) => `${count} ${count === 1 ? 'setup' : 'setups'}`

export const setupParts = (configuration: Configuration) => {
  const [method = configuration.short, model = '', effort = ''] = configuration.short.split(' / ')
  return { method, model, effort }
}

export const setupLabel = (configuration: Configuration) => Object.values(setupParts(configuration)).filter(Boolean).join(' · ')

const metricOf = (summary: Summary, axis: Axis) => axis === 'time' ? summary.time ? summary.time.median / 60 : null : summary[axis]
const boundedOn = (summary: Summary, axis: Axis) => (axis === 'cost' || axis === 'tokens') && summary.card[axis].perTrial.kind === 'atLeast'

function formatMetric(axis: Axis, value: number | null, starred: boolean, bounded = false) {
  if (value === null) return 'Unavailable'
  const text = `${bounded ? '≥ ' : ''}${axis === 'cost' ? money(value) : axis === 'tokens' ? compact(value) : axis === 'time' ? duration(value * 60) : value.toFixed(2)}`
  return axis === 'cost' && starred ? `${text}*` : text
}

function tickLabel(axis: Axis, value: number) {
  if (axis === 'cost') return value >= 1 ? `$${value}` : value >= 0.01 ? `$${value.toFixed(2)}` : `$${value}`
  if (axis === 'tokens') return compact(value)
  if (axis === 'time') return value < 1 ? `${Math.round(value * 60)} s` : `${value} min`
  return String(value)
}

const scaleNames: Record<Scale['kind'], string> = { log: 'log scale', sqrt: 'square-root scale', linear: 'linear scale' }
const scaleFor = (axis: Axis, values: number[]): Scale => axis === 'refuted' ? sqrtScale(values) : logScale(values)
const fraction = (scale: Scale, value: number) => position(scale, value, 0, 100)
const scoreTicks = [0, 25, 50, 75, 100]
const pairAxes: Axis[] = ['cost', 'refuted']

export function Chart({ summaries, detection, matching, axis, view, showAllLabels, onSelect }: {
  summaries: Summary[]; detection: DetectionView; matching: { included: number; excluded: number }; axis: Axis; view: ChartView; showAllLabels: boolean; onSelect: (id: string) => void
}) {
  const measure = detectionLabel(detection), dimensions = `${lowerFirst(measure)} against ${lowerFirst(axisLabels[axis])}`
  const [hovered, setHovered] = useState<({ id: string } & TipPlace) | null>(null)
  const shown = summaries.map(summary => summary.configuration)
  const plotted = (axis: Axis) => {
    const points: Point[] = summaries.filter(summary => summary.detection !== null).map(summary => ({
      summary, range: summary.range, metric: metricOf(summary, axis), bounded: boundedOn(summary, axis), starred: costStarred(summary.configuration, shown),
    }))
    const measured = points.flatMap(point => point.metric === null ? [] : [{ ...point, x: point.metric, y: point.summary.detection ?? 0 }])
    const frontier = new Set(paretoFrontier(measured.filter(point => !point.bounded)).map(point => point.summary.configuration.id))
    return { axis, points, measured, frontier, scale: scaleFor(axis, measured.map(point => point.x)), unplotted: points.filter(point => point.metric === null) }
  }
  const axes: Axis[] = view === 'pair' ? pairAxes : view === 'models' ? [] : [axis]
  const sides = (view === 'pair' ? pairAxes : [axis]).map(plotted)
  const { points, measured, frontier, scale } = sides[0]!
  const withheld = reasonCounts(summaries.map(summary => summary.reasons.detection))
  const hoveredPoint = points.find(point => point.summary.configuration.id === hovered?.id)
  const hover = (id: string, element: Element) => {
    const shell = element.closest('.plot-shell')?.getBoundingClientRect()
    const box = element.getBoundingClientRect()
    if (shell) setHovered({ id, ...tipPlace(shell, box, 160, 260) })
  }
  const interactions = (id: string) => ({
    onMouseEnter: (event: MouseEvent<Element>) => hover(id, event.currentTarget),
    onFocus: (event: FocusEvent<Element>) => hover(id, event.currentTarget),
    onMouseLeave: () => setHovered(null), onBlur: () => setHovered(null),
  })
  const scatter = view === 'tradeoff' || view === 'pair'
  const bounded = sides.some(side => side.measured.some(point => point.bounded))
  const notes = reasonCounts(points.map(point => point.summary.detectionNote ?? null))
  return <div className="plot-shell" onMouseLeave={() => setHovered(null)}>
    {!points.length ? <div className="plot-empty" role="status"><strong>{measure} is unavailable</strong>
      {summaries.length ? <ul className="reason-list">{withheld.map(row => <li key={row.reason}>{row.reason} ({setups(row.count)})</li>)}</ul>
        : <span>Select at least one review method and model.</span>}
      <span>No other impact band or average is substituted. Choose them above.</span></div>
      : view === 'pair' ? <div className="scatter-pair">{sides.map(side => <figure key={side.axis} className="scatter-panel">
        <figcaption>Against {lowerFirst(axisLabels[side.axis])}</figcaption>
        <Scatter points={side.measured} dimensions={`${lowerFirst(measure)} against ${lowerFirst(axisLabels[side.axis])}`} axis={side.axis} scale={side.scale} frontier={side.frontier}
          showAllLabels={showAllLabels} hovered={hovered?.id ?? null} onSelect={onSelect} interactions={interactions} yTitle={foundLabels[detection.band]} /></figure>)}</div>
      : view === 'setups' ? <SetupRows points={points} notes={notes} measure={measure} axis={axis} scale={scale} frontier={frontier} onSelect={onSelect} interactions={interactions} />
        : view === 'tradeoff' ? <Scatter points={measured} dimensions={dimensions} axis={axis} scale={scale} frontier={frontier} showAllLabels={showAllLabels}
          hovered={hovered?.id ?? null} onSelect={onSelect} interactions={interactions} yTitle={foundLabels[detection.band]} />
        : view === 'skills' ? <MethodPanels points={measured} axis={axis} scale={scale} onSelect={onSelect} interactions={interactions} />
          : <Models points={points} notes={notes} measure={measure} onSelect={onSelect} interactions={interactions} />}
    {hoveredPoint && hovered && <Tooltip point={hoveredPoint} measure={measure} axes={axes} place={hovered} />}
    {points.length > 0 && view !== 'models' && <ul className="chart-keys" aria-label="Chart key">
      {view === 'setups' && <li><span className="whisker-key" aria-hidden="true" />Range with any one PR left out</li>}
      {view === 'skills' && <li><span className="context-key" aria-hidden="true" />Other selected setups</li>}
      {scatter && [...new Map(measured.map(point => [setupParts(point.summary.configuration).method, point.summary.configuration])).entries()]
        .sort(([left, one], [right, other]) => Number(other.builtin) - Number(one.builtin) || left.localeCompare(right)).map(([method, configuration]) => <li key={method}><Mark configuration={configuration} size={10} />{method}</li>)}
      {scatter && <li className="key-break"><span className="frontier-key" aria-hidden="true" />Frontier</li>}
      {scatter && <li><span className="faded-key" aria-hidden="true"><span /><span /></span>Not on the frontier, paler</li>}
      {bounded && <li><span className="bound-key" aria-hidden="true" />Lower bound{view === 'skills' ? '' : ', left out of the frontier'}</li>}
    </ul>}
    {points.length > 0 && <ChartGuide><div className="plot-caption">
      {view === 'setups' && <span><span className="whisker-key" aria-hidden="true" />Rows are ordered by {lowerFirst(measure)}. The whisker spans this average with any one whole PR left out: sensitivity to these PRs, not a confidence interval.</span>}
      {view === 'skills' && <span>Each panel shows one review method on every model it ran on, with detection against {lowerFirst(axisLabels[axis])} on shared axes. Gray dots are the other selected setups, for context.</span>}
      {view === 'pair' && <span>Each panel plots {lowerFirst(measure)} against one cost of a review: what a trial costs, and how many of its claims were shown to be wrong. Up and to the left is better on both.</span>}
      {scatter && <span><span className="frontier-key" aria-hidden="true" />Frontier of {view === 'pair' ? 'each panel' : dimensions}: no other selected setup is at least as good on both measures and strictly better on one. Identical results share the frontier. It says nothing about any other measure.</span>}
      {view === 'models' && <span>Each row is one model, in name order. Marks show {lowerFirst(measure)} for each review method that ran on it.</span>}
      {withheld.length > 0 && <span>Not plotted, {lowerFirst(measure)} unavailable: {withheld.map(row => `${row.reason} (${setups(row.count)})`).join(' ')}</span>}
      {view !== 'models' && bounded && <span><span className="bound-key" aria-hidden="true" />Hollow marks show a lower bound of {lowerFirst(sides.filter(side => side.measured.some(point => point.bounded)).map(side => axisLabels[side.axis]).join(' or '))}: some attempts recorded only part of their usage, so the true value may be higher.{view !== 'skills' && ' They are left out of the frontier.'}</span>}
      {sides.map(side => side.unplotted.length > 0 && <span key={side.axis}>No {lowerFirst(axisLabels[side.axis])} for {side.unplotted.map(point => `${setupLabel(point.summary.configuration)} (${point.summary.reasons[side.axis]})`).join('; ')}.</span>)}
      {axes.includes('refuted') && <span>Refuted claims are compared on the {matching.included} of {matching.included + matching.excluded} selected PRs where every selected setup has admitted, assessed reviews, with PRs weighted equally. A setup that admitted only some trials of a matched PR is measured on the reviews it delivered; the tooltip gives its admitted trials.</span>}
      {axes.includes('time') && <span>Completed trials only. Includes replacement attempts; excludes gaps between attempts, provisioning, and grading.</span>}
      {axes.includes('cost') && costNote(shown) && <span>{costNote(shown)}</span>}
    </div></ChartGuide>}
    {view !== 'pair' && <Notes notes={notes} />}
  </div>
}

function NoteMark({ summary, notes }: { summary: Summary; notes: Note[] }) {
  const index = notes.findIndex(note => note.reason === summary.detectionNote)
  return index >= 0 && <sup aria-hidden="true">{index + 1}</sup>
}

type Interactions = (id: string) => Pick<DOMAttributes<Element>, 'onMouseEnter' | 'onFocus' | 'onMouseLeave' | 'onBlur'>

function SetupRows({ points, notes, measure, axis, scale, frontier, onSelect, interactions }: {
  points: Point[]; notes: Note[]; measure: string; axis: Axis; scale: Scale; frontier: Set<string>; onSelect: (id: string) => void; interactions: Interactions
}) {
  const metricTicks = ticks(scale)
  return <div role="list" aria-label={`Setups ordered by ${lowerFirst(measure)}, with ${lowerFirst(axisLabels[axis])}`}>
    <div className="rank-head" aria-hidden="true">
      <span>Review setup</span>
      <span className="rank-axis"><span className="rank-axis-title">{measure}</span>
        <span className="rank-ticks score-ticks">{scoreTicks.map(tick => <span key={tick} style={{ left: `${tick}%` }}>{tick}%</span>)}</span></span>
      <span className="rank-axis"><span className="rank-axis-title">{axisLabels[axis]}{scale.kind !== 'linear' && <em>{scaleNames[scale.kind]}</em>}</span>
        <span className={metricTicks.length > 4 ? 'rank-ticks sparse' : 'rank-ticks'}>{metricTicks.map(tick => <span key={tick} style={{ left: `${fraction(scale, tick)}%` }}>{tickLabel(axis, tick)}</span>)}</span></span>
    </div>
    {points.map(({ summary, range, metric, bounded, starred }) => {
      const { configuration } = summary
      const { method, model, effort } = setupParts(configuration)
      const score = summary.detection ?? 0
      return <div role="listitem" key={configuration.id}>
        <button type="button" className="rank-row" onClick={() => onSelect(configuration.id)} {...interactions(configuration.id)}
          aria-label={`${configuration.short}: ${measure} ${percent(score)}${range ? `, ${percent(range.low)} to ${percent(range.high)} leaving one PR out` : ''}; ${axisLabels[axis]} ${formatMetric(axis, metric, starred, bounded)}${frontier.has(configuration.id) ? '; on the frontier of these two measures' : ''}. Inspect evidence`}>
          <span className="rank-name"><Mark configuration={configuration} />
            <span><strong>{method}</strong><span className="rank-model"><VendorMark configuration={configuration} />{model}{effort && ` · ${effort}`}
              {frontier.has(configuration.id) && <span className="frontier-tag">On frontier</span>}</span></span></span>
          <span className="rank-cell"><span className="rank-value">{percent(score)}<NoteMark summary={summary} notes={notes} /></span>
            <span className="rank-track">{scoreTicks.map(tick => <span key={tick} className="rank-grid" style={{ left: `${tick}%` }} />)}
              {range && <span className="rank-whisker" style={{ left: `${range.low}%`, width: `${range.high - range.low}%` }} />}
              <span className="rank-dot" style={{ left: `${score}%` }}><Mark configuration={configuration} /></span></span></span>
          <span className="rank-cell"><span className="rank-value">{formatMetric(axis, metric, starred, bounded)}</span>
            <span className="rank-track">{metricTicks.map(tick => <span key={tick} className="rank-grid" style={{ left: `${fraction(scale, tick)}%` }} />)}
              {metric === null ? <span className="rank-missing">unavailable</span>
                : <span className="rank-dot" style={{ left: `${fraction(scale, metric)}%` }}><Mark configuration={configuration} hollow={bounded} /></span>}</span></span>
        </button>
      </div>
    })}
  </div>
}

let measureContext: CanvasRenderingContext2D | null = null
function textWidth(text: string) {
  measureContext ??= document.createElement('canvas').getContext('2d')
  if (!measureContext) return text.length * 6.5
  measureContext.font = `560 12px 'Geist Variable', system-ui, sans-serif`
  return measureContext.measureText(text).width
}

const methodKey = (configuration: Configuration) => `${configuration.method}/${configuration.reviewEdition}`

const labelOf = (points: Point[], withMethod = false) => {
  const models = new Map<string, number>()
  for (const point of points) {
    const { method, model } = setupParts(point.summary.configuration)
    models.set(`${method}/${model}`, (models.get(`${method}/${model}`) ?? 0) + 1)
  }
  return (configuration: Configuration) => {
    const { method, model, effort } = setupParts(configuration)
    const label = (models.get(`${method}/${model}`) ?? 0) > 1 ? `${model} · ${effort}` : model
    return withMethod ? `${method} · ${label}` : label
  }
}

type Plotted = Point & { x: number; y: number }

function Scatter({ points, dimensions, axis, scale, frontier, showAllLabels, hovered, onSelect, interactions, yTitle }: {
  dimensions: string; points: Plotted[]; axis: Axis; scale: Scale; frontier: Set<string>; showAllLabels: boolean
  hovered: string | null; onSelect: (id: string) => void; interactions: Interactions; yTitle: string
}) {
  const { ref, width } = useElementSize()
  const height = 400, margin = { top: 18, right: 22, bottom: 50, left: 62 }
  const plot = { left: margin.left, right: Math.max(margin.left + 1, width - margin.right), top: margin.top, bottom: height - margin.bottom }
  const px = (value: number) => position(scale, value, plot.left, plot.right)
  const py = (value: number) => plot.bottom - value / 100 * (plot.bottom - plot.top)
  const labelText = labelOf(points, true)
  const ordered = [...points].sort((left, right) => right.y - left.y)
  const steps = paretoFrontier(points.filter(point => !point.bounded))
  const segment = (x1: number, y1: number, x2: number, y2: number) =>
    ({ left: Math.min(x1, x2) - 2, top: Math.min(y1, y2) - 2, right: Math.max(x1, x2) + 2, bottom: Math.max(y1, y2) + 2 })
  const obstacles = [
    ...ordered.map(point => ({ left: px(point.x) - 10, top: py(point.y) - 10, right: px(point.x) + 10, bottom: py(point.y) + 10 })),
    ...steps.slice(1).flatMap((point, index) => {
      const previous = steps[index]!
      return [segment(px(previous.x), py(previous.y), px(point.x), py(previous.y)), segment(px(point.x), py(previous.y), px(point.x), py(point.y))]
    }),
  ]
  const prominent = (point: Point) => frontier.has(point.summary.configuration.id) || point.bounded
  const requested = ordered.filter(point => showAllLabels || prominent(point) || hovered === point.summary.configuration.id)
    .sort((left, right) => Number(frontier.has(right.summary.configuration.id)) - Number(frontier.has(left.summary.configuration.id)))
  const dimmed = (point: Point) => !prominent(point) && hovered !== point.summary.configuration.id
  const labels = width ? placeLabels(requested.map(point => ({
    id: point.summary.configuration.id, x: px(point.x), y: py(point.y), width: textWidth(labelText(point.summary.configuration)), height: 14,
  })), obstacles, { left: plot.left + 2, right: plot.right, top: 2, bottom: plot.bottom - 2 }) : []
  const stepPath = steps.map((point, index) => {
    const previous = steps[index - 1]
    return previous ? `H${px(point.x)}V${py(point.y)}` : `M${px(point.x)},${py(point.y)}`
  }).join('')
  const xTicks = ticks(scale)
  return <div ref={ref} className="tradeoff">
    {width > 0 && <svg width={width} height={height} role="group" aria-label={`Plot of ${dimensions}. ${frontier.size} setups on the frontier of these two measures are labelled.`}>
      <defs><linearGradient id="better-corner" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="var(--brand-accent)" stopOpacity="0.09" /><stop offset="0.45" stopColor="var(--brand-accent)" stopOpacity="0" />
      </linearGradient></defs>
      <rect x={plot.left} y={plot.top} width={plot.right - plot.left} height={plot.bottom - plot.top} fill="url(#better-corner)" />
      <Axes plot={plot} axis={axis} scale={scale} xTicks={xTicks} px={px} py={py} height={height} yTitle={yTitle} />
      {steps.length > 1 && <path d={stepPath} className="frontier-line" />}
      {labels.filter(label => label.leader).map(label => {
        const point = points.find(item => item.summary.configuration.id === label.id)
        return point && <line key={`leader-${label.id}`} x1={px(point.x)} y1={py(point.y)} x2={label.anchor === 'middle' ? label.x : label.x + (label.anchor === 'start' ? -3 : 3)} y2={label.y} className="leader-line" />
      })}
      {ordered.map(point => <ChartPoint key={point.summary.configuration.id} point={point} axis={axis} x={px(point.x)} y={py(point.y)} faded={dimmed(point)}
        frontier={frontier.has(point.summary.configuration.id)} onSelect={onSelect} interactions={interactions} />)}
      {labels.map(label => {
        const point = points.find(item => item.summary.configuration.id === label.id)
        return point && <text key={`label-${label.id}`} x={label.x} y={label.y} textAnchor={label.anchor} dominantBaseline="middle"
          className={frontier.has(label.id) ? 'point-label' : 'point-label secondary'}>{labelText(point.summary.configuration)}</text>
      })}
    </svg>}
    <span className="better-hint" style={{ left: plot.left + 10, top: plot.top + 8 }}><ArrowUpLeft size={13} aria-hidden="true" />Higher detection, lower {lowerFirst(axisLabels[axis])}</span>
  </div>
}

function Axes({ plot, axis, scale, xTicks, px, py, height, compact = false, yTitle = 'Detection' }: {
  plot: { left: number; right: number; top: number; bottom: number }; axis: Axis; scale: Scale; xTicks: number[]
  px: (value: number) => number; py: (value: number) => number; height: number; compact?: boolean; yTitle?: string
}) {
  const yTicks = compact ? [0, 50, 100] : scoreTicks
  return <>
    {yTicks.map(tick => <g key={tick}><line x1={plot.left} x2={plot.right} y1={py(tick)} y2={py(tick)} className="grid-line" />
      <text x={plot.left - 8} y={py(tick)} className="tick-label" textAnchor="end" dominantBaseline="middle">{tick}%</text></g>)}
    {xTicks.map(tick => <g key={tick}><line x1={px(tick)} x2={px(tick)} y1={plot.top} y2={plot.bottom} className="grid-line" />
      <text x={px(tick)} y={plot.bottom + 17} className="tick-label" textAnchor="middle">{tickLabel(axis, tick)}</text></g>)}
    <text x={14} y={(plot.top + plot.bottom) / 2} className="axis-title" textAnchor="middle" transform={`rotate(-90 14 ${(plot.top + plot.bottom) / 2})`}>{yTitle}</text>
    <text x={(plot.left + plot.right) / 2} y={height - 8} className="axis-title" textAnchor="middle">{axisLabels[axis]}{scale.kind === 'linear' ? '' : ` (${scaleNames[scale.kind]})`}</text>
  </>
}

function ChartPoint({ point, axis, x, y, faded, frontier, onSelect, interactions }: {
  point: Plotted; axis: Axis; x: number; y: number; faded: boolean; frontier: boolean; onSelect: (id: string) => void; interactions: Interactions
}) {
  const { configuration } = point.summary
  return <g role="button" tabIndex={0} className={faded ? 'chart-point faded' : 'chart-point'}
    aria-label={`${configuration.short}: ${percent(point.y)}, ${formatMetric(axis, point.x, point.starred, point.bounded)}${frontier ? ', on the frontier of these two measures' : ''}. Inspect evidence`}
    onClick={() => onSelect(configuration.id)} {...interactions(configuration.id)}
    onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(configuration.id) } }}>
    <circle cx={x} cy={y} r={14} fill="transparent" />
    <PointMark configuration={configuration} x={x} y={y} hollow={point.bounded} />
  </g>
}

function MethodPanels({ points, axis, scale, onSelect, interactions }: {
  points: Plotted[]; axis: Axis; scale: Scale; onSelect: (id: string) => void; interactions: Interactions
}) {
  const order = ['claude-builtin', 'codex']
  const rank = (key: string) => { const found = order.findIndex(method => key.startsWith(`${method}/`)); return found < 0 ? order.length : found }
  const groups = groupPaths(points, point => methodKey(point.summary.configuration)).sort((left, right) => rank(left.key) - rank(right.key) || left.key.localeCompare(right.key))
  const labelText = labelOf(points)
  return <div className="multiples" role="list" aria-label={`Detection against ${lowerFirst(axisLabels[axis])}, one panel per review method`}>
    {groups.map(group => <MethodPanel key={group.key} members={group.points} context={points.filter(point => !group.points.includes(point))}
      axis={axis} scale={scale} labelText={labelText} onSelect={onSelect} interactions={interactions} />)}
  </div>
}

function MethodPanel({ members, context, axis, scale, labelText, onSelect, interactions }: {
  members: Plotted[]; context: Plotted[]; axis: Axis; scale: Scale; labelText: (configuration: Configuration) => string; onSelect: (id: string) => void; interactions: Interactions
}) {
  const { ref, width } = useElementSize()
  const height = 262, plot = { left: 60, right: Math.max(61, width - 16), top: 12, bottom: height - 46 }
  const px = (value: number) => position(scale, value, plot.left, plot.right)
  const py = (value: number) => plot.bottom - value / 100 * (plot.bottom - plot.top)
  const first = members[0]!.summary.configuration
  const labels = width ? placeLabels([...members].sort((left, right) => right.y - left.y).map(point => ({
    id: point.summary.configuration.id, x: px(point.x), y: py(point.y), width: textWidth(labelText(point.summary.configuration)), height: 14,
  })), [...members, ...context].map(point => ({ left: px(point.x) - 10, top: py(point.y) - 10, right: px(point.x) + 10, bottom: py(point.y) + 10 })),
  { left: plot.left + 2, right: plot.right, top: 2, bottom: plot.bottom - 2 }) : []
  return <div role="listitem" className="method-panel">
    <div className="panel-head"><span className="configuration-name"><Mark configuration={first} size={10} />{setupParts(first).method}</span>
      <span className="panel-meta">{members.length} {members.length === 1 ? 'model' : 'models'}</span></div>
    <div ref={ref} className="panel-plot">{width > 0 && <svg width={width} height={height} role="group" aria-label={`${setupParts(first).method}: ${members.map(point => `${labelText(point.summary.configuration)} ${percent(point.y)}`).join(', ')}`}>
      <Axes plot={plot} axis={axis} scale={scale} xTicks={ticks(scale)} px={px} py={py} height={height} compact />
      {context.map(point => <circle key={point.summary.configuration.id} cx={px(point.x)} cy={py(point.y)} r={3.5} className="context-dot" />)}
      {labels.filter(label => label.leader).map(label => {
        const point = members.find(item => item.summary.configuration.id === label.id)
        return point && <line key={`leader-${label.id}`} x1={px(point.x)} y1={py(point.y)} x2={label.anchor === 'middle' ? label.x : label.x + (label.anchor === 'start' ? -3 : 3)} y2={label.y} className="leader-line" />
      })}
      {members.map(point => <ChartPoint key={point.summary.configuration.id} point={point} axis={axis} x={px(point.x)} y={py(point.y)} faded={false} frontier={false} onSelect={onSelect} interactions={interactions} />)}
      {labels.map(label => {
        const point = members.find(item => item.summary.configuration.id === label.id)
        return point && <text key={`label-${label.id}`} x={label.x} y={label.y} textAnchor={label.anchor} dominantBaseline="middle" className="point-label">{labelText(point.summary.configuration)}</text>
      })}
    </svg>}</div>
  </div>
}

function PointMark({ configuration, x, y, hollow }: { configuration: Configuration; x: number; y: number; hollow: boolean }) {
  const color = reviewColor(configuration)
  const props = { fill: color, className: hollow ? 'point-mark hollow' : 'point-mark', style: { '--mark-color': color } as CSSProperties }
  return configuration.builtin
    ? <circle cx={x} cy={y} r={6.5} {...props} />
    : <path d={`M${x},${y - 8}L${x + 8},${y}L${x},${y + 8}L${x - 8},${y}Z`} {...props} />
}

function Models({ points, notes, measure, onSelect, interactions }: { points: Point[]; notes: Note[]; measure: string; onSelect: (id: string) => void; interactions: Interactions }) {
  const groups = new Map<string, Point[]>()
  for (const point of points) {
    const { model, effort } = setupParts(point.summary.configuration)
    const key = `${model}${effort && effort !== 'High' ? ` · ${effort}` : ''}`
    groups.set(key, [...groups.get(key) ?? [], point])
  }
  const rows = Array.from(groups, ([model, members]) => ({ model, members: [...members].sort((a, b) => (b.summary.detection ?? 0) - (a.summary.detection ?? 0)) }))
    .sort((left, right) => left.model.localeCompare(right.model))
  return <div className="models" role="list" aria-label={`${measure} by model and review method`}>
    <div className="model-head" aria-hidden="true"><span>Model</span>
      <span className="rank-ticks">{scoreTicks.map(tick => <span key={tick} style={{ left: `${tick}%` }}>{tick}%</span>)}</span></div>
    {rows.map(({ model, members }) => {
      const scores = members.map(point => point.summary.detection ?? 0)
      const low = Math.min(...scores), high = Math.max(...scores)
      return <div role="listitem" key={model} className="model-row">
        <span className="model-name"><VendorMark configuration={members[0]!.summary.configuration} />{model}</span>
        <span className="rank-track">{scoreTicks.map(tick => <span key={tick} className="rank-grid" style={{ left: `${tick}%` }} />)}
          {members.length > 1 && <span className="model-span" style={{ left: `${low}%`, width: `${high - low}%` }} />}
          {members.map((point, index) => {
            const score = point.summary.detection ?? 0
            const stacked = members.slice(0, index).filter(other => Math.abs((other.summary.detection ?? 0) - score) < 1.5).length
            return <span key={point.summary.configuration.id} className="rank-dot" style={{ left: `${score}%`, marginTop: stacked * 9 - (stacked ? 2 : 0) }}>
              <Mark configuration={point.summary.configuration} size={13} /></span>
          })}</span>
        <span className="model-entries">{[...members].sort((left, right) => left.summary.configuration.short.localeCompare(right.summary.configuration.short)).map(point => {
          const { configuration } = point.summary
          return <button type="button" key={configuration.id} className="model-entry" onClick={() => onSelect(configuration.id)} {...interactions(configuration.id)}
            aria-label={`${configuration.short}: ${percent(point.summary.detection)}. Inspect evidence`}>
            <Mark configuration={configuration} size={10} /><span>{setupParts(configuration).method}</span>
            <strong>{percent(point.summary.detection)}<NoteMark summary={point.summary} notes={notes} /></strong>
          </button>
        })}</span>
      </div>
    })}
  </div>
}

function Tooltip({ point, measure, axes, place }: { point: Point; measure: string; axes: Axis[]; place: TipPlace }) {
  const { summary, range } = point
  const row = (key: string, label: string, value: ReactNode, note?: ReactNode) =>
    <span key={key} className="tip-row"><span className="tip-label">{label}</span><b>{value}</b>{note && <span className="tip-note">{note}</span>}</span>
  const lines: ReactNode[] = [
    row('score', measure, percent(summary.detection), range && `${percent(range.low)} to ${percent(range.high)} leaving one PR out; sensitivity, not a confidence interval`),
    summary.detectionNote && <span key="zero-credit" className="tip-note">{summary.detectionNote}</span>,
    ...axes.map(axis => {
      const metric = metricOf(summary, axis), bounded = boundedOn(summary, axis)
      return row(axis, axisLabels[axis], formatMetric(axis, metric, point.starred, bounded), (metric === null || bounded) && summary.reasons[axis])
    }),
    axes.includes('time') && summary.time && <span key="time" className="tip-note">Mean {duration(summary.time.mean)} · middle 50% {duration(summary.time.q1)} to {duration(summary.time.q3)} · {summary.time.reviews} timed trials</span>,
    row('count', 'Trials admitted', `${summary.admitted} of ${summary.trials}`, `${summary.tasks} PRs · ${summary.completed} complete`),
    !summary.configuration.builtin && row('skill', 'Skill', skillReleaseLabel([summary.configuration])),
  ]
  return <div className={place.below ? 'chart-tip below' : 'chart-tip'} role="presentation" style={{ left: place.left, top: place.top }}><strong><SetupName configuration={summary.configuration} mark={false} /></strong>{lines}</div>
}

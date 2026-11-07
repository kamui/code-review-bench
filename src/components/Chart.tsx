import { useId, useState } from 'react'
import type { CSSProperties, DOMAttributes, FocusEvent, MouseEvent, ReactNode } from 'react'
import { useElementSize } from '@mantine/hooks'
import { ArrowUpLeft } from 'lucide-react'
import { groupPaths, logScale, paretoFrontier, placeLabels, position, sideTip, sqrtScale, ticks, tipPlace } from '../lib/chart'
import type { Scale, TipPlace } from '../lib/chart'
import { compact, detectionLabel, foundLabels, duration, lowerFirst, money, percent, reasonCounts, share } from '../lib/metrics'
import type { Axis, DetectionView, Summary } from '../lib/metrics'
import type { Configuration } from '../lib/data'
import { skillReleaseLabel } from '../lib/data'
import { ChartGuide, costNote, costStarred, Glossed, GuideItem } from './Reading'
import { VendorMark } from './VendorMark'

export type ChartView = 'skills' | 'setups' | 'models' | 'tradeoff'
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

/** The second Overview panel: the refuted or unproven share, or cost when the first panel already measures refuted claims. */
export const pairedAxis = (axis: Axis): Axis => axis === 'doubt' || axis === 'refuted' ? 'cost' : 'doubt'
const shortAxis: Record<Axis, string> = { cost: 'Cost per trial', doubt: 'Refuted or unproven', refuted: 'Refuted per review', tokens: 'Output tokens', time: 'Median time' }
export const axisLabels: Record<Axis, string> = {
  cost: 'Cost per trial', tokens: 'Output tokens per trial', refuted: 'Refuted claims per review on matched PRs', doubt: 'Reviews with a refuted or unproven claim', time: 'Median time',
}
const setups = (count: number) => `${count} ${count === 1 ? 'setup' : 'setups'}`

export const setupParts = (configuration: Configuration) => {
  const [method = configuration.short, model = '', effort = ''] = configuration.short.split(' / ')
  return { method, model, effort }
}

export const setupLabel = (configuration: Configuration) => Object.values(setupParts(configuration)).filter(Boolean).join(' · ')

const metricOf = (summary: Summary, axis: Axis) => axis === 'time' ? summary.time ? summary.time.median / 60 : null : summary[axis]
const boundedOn = (summary: Summary, axis: Axis) => (axis === 'cost' || axis === 'tokens') && summary.card[axis].perTrial.kind === 'atLeast'

const stripValue = (text: string) => text === 'Unavailable' ? 'strip-value missing' : 'strip-value'

function formatMetric(axis: Axis, value: number | null, starred: boolean, bounded = false, digits = 1) {
  if (value === null) return 'Unavailable'
  const text = `${bounded ? '≥ ' : ''}${axis === 'cost' ? money(value) : axis === 'tokens' ? compact(value) : axis === 'time' ? duration(value * 60) : axis === 'doubt' ? percent(value, digits) : value.toFixed(2)}`
  return axis === 'cost' && starred ? `${text}*` : text
}

function tickLabel(axis: Axis, value: number) {
  if (axis === 'cost') return value >= 1 ? `$${value}` : value >= 0.01 ? `$${value.toFixed(2)}` : `$${value}`
  if (axis === 'tokens') return compact(value)
  if (axis === 'doubt') return `${value}%`
  if (axis === 'time') return value < 1 ? `${Math.round(value * 60)} s` : `${value} min`
  return String(value)
}

const scaleNames: Record<Scale['kind'], string> = { log: 'log scale', sqrt: 'square-root scale', linear: 'linear scale' }
const scaleFor = (axis: Axis, values: number[]): Scale => axis === 'refuted' || axis === 'doubt' ? sqrtScale(values) : logScale(values)
const fraction = (scale: Scale, value: number) => position(scale, value, 0, 100)
const scoreTicks = [0, 25, 50, 75, 100]

export function Chart({ summaries, detection, matching, axis, view, onSelect, guide, height, linked }: {
  summaries: Summary[]; detection: DetectionView; matching: { included: number; excluded: number }; axis: Axis; view: ChartView; onSelect: (id: string) => void
  guide?: ReactNode; height?: number; linked?: { id: string | null; point: (id: string | null) => void; pin: (id: string) => void }
}) {
  const measure = detectionLabel(detection), dimensions = `${lowerFirst(measure)} against ${lowerFirst(axisLabels[axis])}`
  const [hovered, setHovered] = useState<({ id: string } & TipPlace) | null>(null)
  const [spotlight, setSpotlight] = useState<{ method: string; pinned: boolean } | null>(null)
  const preview = (method: string | null) => () => setSpotlight(current => current?.pinned ? current : method === null ? null : { method, pinned: false })
  const shown = summaries.map(summary => summary.configuration)
  const points: Point[] = summaries.filter(summary => summary.detection !== null).map(summary => ({
    summary, range: summary.range, metric: metricOf(summary, axis), bounded: boundedOn(summary, axis), starred: costStarred(summary.configuration, shown),
  }))
  const measured = points.flatMap(point => point.metric === null ? [] : [{ ...point, x: point.metric, y: point.summary.detection ?? 0 }])
  const exact = measured.filter(point => !point.bounded)
  const frontier = new Set(paretoFrontier(exact).map(point => point.summary.configuration.id))
  const undecided = new Set(measured.filter(point => point.bounded && paretoFrontier([...exact, point]).includes(point)).map(point => point.summary.configuration.id))
  const scale = scaleFor(axis, measured.map(point => point.x))
  const withheld = reasonCounts(summaries.map(summary => summary.reasons.detection))
  const hoveredPoint = points.find(point => point.summary.configuration.id === hovered?.id)
  const hover = (id: string, element: Element) => {
    const shell = element.closest('.plot-shell')?.getBoundingClientRect()
    const box = element.getBoundingClientRect()
    const frame = element.closest('.leaderboard-paper, .measure-card')?.getBoundingClientRect()
    if (shell) setHovered({ id, ...view === 'tradeoff' ? sideTip(shell, box, window.innerHeight, 175, element.closest('.twin')?.getBoundingClientRect().top) : tipPlace(shell, box, 160, 320, window.innerHeight, frame?.top ?? shell.top) })
    linked?.point(id)
  }
  const unhover = () => { setHovered(null); linked?.point(null) }
  const interactions = (id: string) => ({
    onMouseEnter: (event: MouseEvent<Element>) => hover(id, event.currentTarget),
    onFocus: (event: FocusEvent<Element>) => hover(id, event.currentTarget),
    onMouseLeave: unhover, onBlur: unhover,
  })
  const unplotted = points.filter(point => point.metric === null)
  const methodTops = new Map<string, (typeof measured)[number]>()
  for (const point of [...measured].sort((left, right) => right.y - left.y)) {
    const { method } = setupParts(point.summary.configuration)
    if (!methodTops.has(method)) methodTops.set(method, point)
  }
  const lowest = (on: (point: (typeof measured)[number]) => number | null) => {
    const lows = new Map<string, (typeof measured)[number]>()
    for (const point of measured.filter(point => on(point) !== null).sort((left, right) => on(left)! - on(right)! || right.y - left.y)) {
      const { method } = setupParts(point.summary.configuration)
      if (!lows.has(method)) lows.set(method, point)
    }
    return lows
  }
  const methodLows = lowest(point => point.x)
  const named = new Set([...methodTops.values(), ...lowest(point => metricOf(point.summary, 'cost')).values()].map(point => point.summary.configuration.id))
  const pair = pairedAxis(axis)
  const plotted = (on: Axis): Plotted[] => points.flatMap(point => {
    const metric = metricOf(point.summary, on)
    return metric === null ? [] : [{ ...point, metric, bounded: boundedOn(point.summary, on), x: metric, y: point.summary.detection ?? 0 }]
  })
  const panels = view === 'tradeoff' ? [axis, pair].map(on => { const panel = plotted(on); return { on, points: panel, scale: scaleFor(on, panel.map(point => point.x)) } }) : []
  const bounded = (view === 'tradeoff' ? panels.flatMap(panel => panel.points) : measured).some(point => point.bounded)
  const boundedAxis = view === 'tradeoff' && !measured.some(point => point.bounded) ? pair : axis
  const pinAndSelect = (id: string) => { linked?.pin(id); onSelect(id) }
  return <div className="plot-shell" onMouseLeave={() => setHovered(null)}>
    {!points.length ? <div className="plot-empty" role="status"><strong>{measure} is unavailable</strong>
      {summaries.length ? <ul className="reason-list">{withheld.map(row => <li key={row.reason}>{row.reason} ({setups(row.count)})</li>)}</ul>
        : <span>Select at least one review method and model.</span>}
      <span>No other impact band or average is substituted. Choose them above.</span></div>
      : view === 'setups' ? <SetupRows points={points} measure={measure} axis={axis} scale={scale} frontier={frontier} undecided={undecided} onSelect={onSelect} interactions={interactions} />
        : view === 'tradeoff' ? <div className="twin">{panels.map(panel => <Scatter key={panel.on} points={panel.points} dimensions={`${lowerFirst(measure)} against ${lowerFirst(axisLabels[panel.on])}`}
          axis={panel.on} scale={panel.scale} spotlight={spotlight?.method ?? null} marked={hovered?.id ?? linked?.id ?? null} named={named} onSelect={pinAndSelect} interactions={interactions} yTitle={foundLabels[detection.band]} height={height} />)}</div>
        : view === 'skills' ? <MethodPanels points={measured} axis={axis} scale={scale} yTitle={foundLabels[detection.band]} onSelect={onSelect} interactions={interactions} />
          : <Models points={points} measure={measure} onSelect={onSelect} interactions={interactions} />}
    {hoveredPoint && hovered && <Tooltip point={hoveredPoint} measure={measure} axes={view === 'models' ? [] : view === 'tradeoff' ? [axis, pair] : [axis]} place={hovered} />}
    {view === 'tradeoff' && methodTops.size > 0 && <div className="method-strip">
      <div className="strip-row strip-head"><span>Review method</span><span>Top setup (most found)</span><span>Found</span><span><Glossed text={shortAxis[axis]} /></span><span><Glossed text={shortAxis[pair]} /></span><span className="strip-low strip-low-head"><span className="strip-low-title">Lowest {lowerFirst(shortAxis[axis])} in the method</span>
        <span className="strip-low-model">Model</span><span className="strip-low-value">{axis === 'cost' ? 'Cost' : 'Value'}</span><span className="strip-low-found">Found</span></span></div>
      <ul aria-label="Each review method's top setup, from the method that finds the most">{[...methodTops].map(([method, top]) => {
        const { configuration } = top.summary, { model, effort } = setupParts(configuration)
        const second = metricOf(top.summary, pair), secondText = formatMetric(pair, second, top.starred, boundedOn(top.summary, pair), 0)
        const first = formatMetric(axis, top.x, top.starred, top.bounded, 0)
        const low = methodLows.get(method), lowText = low ? formatMetric(axis, low.x, low.starred, low.bounded, 0) : null
        const lowModel = low ? setupParts(low.summary.configuration).model : null
        return <li key={method}><button type="button" className="strip-row" aria-pressed={spotlight?.method === method && spotlight.pinned}
          aria-label={`${method}: top setup ${model}${effort ? ` ${effort}` : ''}, ${percent(top.y, 0)} found, ${axisLabels[axis]} ${first}, ${axisLabels[pair]} ${secondText}${low ? `, lowest ${lowerFirst(axisLabels[axis])} ${lowText} with ${lowModel}, ${percent(low.y, 0)} found` : ''}. Select to keep this method picked out`}
          onMouseEnter={() => { preview(method)(); linked?.point(configuration.id) }} onMouseLeave={() => { preview(null)(); linked?.point(null) }}
          onFocus={() => { preview(method)(); linked?.point(configuration.id) }} onBlur={() => { preview(null)(); linked?.point(null) }}
          onClick={() => setSpotlight(current => current?.pinned && current.method === method ? null : { method, pinned: true })}>
          <span className="strip-method"><Mark configuration={configuration} size={11} />{method}{configuration.experimental && <span className="row-tag">Experiment</span>}</span>
          <span className="strip-model"><VendorMark configuration={configuration} />{model}{effort && <span className="measure-effort"> · {effort}</span>}</span>
          <span className="strip-value" data-label="Found"><Score summary={top.summary} /></span><span className={stripValue(first)} data-label={shortAxis[axis]}>{first}</span>
          <span className={stripValue(secondText)} data-label={shortAxis[pair]}>{secondText}</span>
          <span className="strip-low" data-label={axis === 'cost' ? 'Lowest cost' : `Lowest ${lowerFirst(shortAxis[axis])}`}>{low && <><span className="strip-low-model">{lowModel}</span><span className="strip-low-value">{lowText}</span><span className="strip-low-found">{percent(low.y, 0)}<span className="found-word"> found</span></span></>}</span></button></li>
      })}</ul></div>}
    {view === 'tradeoff' && points.length > 0 && <ul className="chart-keys twin-key" aria-label="Chart key">
      <li><Mark configuration={{ method: '', reviewEdition: '', builtin: true }} size={10} />Built-in reviewer</li>
      <li><Mark configuration={{ method: '', reviewEdition: '', builtin: false }} size={10} />Skill</li>
      {bounded && <li><span className="bound-key" aria-hidden="true" />Lower bound</li>}
      <li><span className="line-key" aria-hidden="true" />One method across models</li>
      {summaries.some(summary => summary.configuration.experimental) && <li><span className="line-key dashed" aria-hidden="true" />Skill experiment</li>}
    </ul>}
    {points.length > 0 && view !== 'models' && view !== 'tradeoff' && <ul className="chart-keys" aria-label="Chart key">
      {view === 'setups' && <li><span className="whisker-key" aria-hidden="true" />Range with any one PR left out</li>}
      {view === 'setups' && frontier.size > 0 && <li><span className="frontier-tag">On frontier</span>No other setup finds more at a lower {lowerFirst(axisLabels[axis])}</li>}
      {view === 'setups' && undecided.size > 0 && <li><span className="frontier-tag unknown">Frontier unknown</span>Its {lowerFirst(axisLabels[axis])} is a lower bound that no other setup beats</li>}
      {view === 'skills' && <li><span className="context-key" aria-hidden="true" />Other selected setups</li>}
      {bounded && <li><span className="bound-key" aria-hidden="true" />Lower bound</li>}
    </ul>}
    {points.length > 0 && <ChartGuide title={guide ? 'How to read these charts' : undefined}><div className="plot-caption">
      {view === 'setups' && <span><span className="whisker-key" aria-hidden="true" />Rows are ordered by {lowerFirst(measure)}. The whisker spans this average with any one whole PR left out: sensitivity to these PRs, not a confidence interval.</span>}
      {view === 'skills' && <span>Each panel shows one review method on every model it ran on, with detection against {lowerFirst(axisLabels[axis])} on shared axes. Gray dots are the other selected setups, for context.</span>}
      {view === 'tradeoff' && <GuideItem label="Table">Each review method's setup that finds the most. Point at a method to pick out its setups; select it to keep them picked out.</GuideItem>}
      {view === 'tradeoff' && <GuideItem label="Panels">The same setups at the same height in both. Point at or select a setup to name it in both and in every chart below.</GuideItem>}
      {view === 'tradeoff' && <GuideItem label="Lines">In each panel, a faint line joins one review method's setups across models, from left to right; the points between are not measured.{summaries.some(summary => summary.configuration.experimental) && ' A dashed line joins a skill experiment.'}</GuideItem>}
      {(view === 'tradeoff' || axis === 'doubt') && <GuideItem label="Unproven">An unproven claim is not shown to be false.</GuideItem>}
      {view === 'models' && <span>Each row is one model; rows start with the model whose setup finds the most. Marks show {lowerFirst(measure)} for each review method that ran on it, listed from most found to least.</span>}
      {withheld.length > 0 && <span>Not plotted, {lowerFirst(measure)} unavailable: {withheld.map(row => `${row.reason} (${setups(row.count)})`).join(' ')}</span>}
      {view !== 'models' && bounded && <GuideItem label="Hollow marks">A lower bound of {lowerFirst(axisLabels[boundedAxis])}: some attempts recorded only part of their usage, so the true value may be higher.{view === 'setups' && ' They are left out of the frontier.'}</GuideItem>}
      {unplotted.length > 0 && <span>No {lowerFirst(axisLabels[axis])} for {unplotted.map(point => `${setupLabel(point.summary.configuration)} (${point.summary.reasons[axis]})`).join('; ')}.</span>}
      {axis === 'refuted' && <span>Refuted claims are compared on the {matching.included} of {matching.included + matching.excluded} selected PRs where every selected setup has admitted, assessed reviews, with PRs weighted equally. A setup that admitted only some trials of a matched PR is measured on the reviews it delivered; the tooltip gives its admitted trials.</span>}
      {axis === 'time' && <span>Completed trials only. Includes replacement attempts; excludes gaps between attempts, provisioning, and grading.</span>}
      {(axis === 'cost' || view === 'tradeoff') && costNote(shown) && <GuideItem label="Cost">{costNote(shown)}</GuideItem>}
      {points.some(point => point.summary.detectionNote) && <GuideItem label="Dotted underline">The value counts unresolved detection decisions as zero credit; point at it for the count.</GuideItem>}
      {guide}
    </div></ChartGuide>}
  </div>
}

const noted = (summary: Summary) => summary.detectionNote ? ` (${summary.detectionNote.replace(/\.$/, '')})` : ''
const Score = ({ summary }: { summary: Summary }) => <span className={summary.detectionNote ? 'annotated' : undefined}>{percent(summary.detection, 0)}</span>

type Interactions = (id: string) => Pick<DOMAttributes<Element>, 'onMouseEnter' | 'onFocus' | 'onMouseLeave' | 'onBlur'>

function SetupRows({ points, measure, axis, scale, frontier, undecided, onSelect, interactions }: {
  points: Point[]; measure: string; axis: Axis; scale: Scale; frontier: Set<string>; undecided: Set<string>; onSelect: (id: string) => void; interactions: Interactions
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
          aria-label={`${configuration.short}: ${measure} ${percent(score)}${noted(summary)}${range ? `, ${percent(range.low)} to ${percent(range.high)} leaving one PR out` : ''}; ${axisLabels[axis]} ${formatMetric(axis, metric, starred, bounded)}${frontier.has(configuration.id) ? '; on the frontier of these two measures' : ''}. Inspect evidence`}>
          <span className="rank-name"><Mark configuration={configuration} />
            <span><strong>{method}</strong><span className="rank-model"><VendorMark configuration={configuration} />{model}{effort && ` · ${effort}`}
              {frontier.has(configuration.id) ? <span className="frontier-tag">On frontier</span> : undecided.has(configuration.id) && <span className="frontier-tag unknown">Frontier unknown</span>}</span></span></span>
          <span className="rank-cell"><span className="rank-value"><Score summary={summary} /></span>
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
  measureContext.font = `560 13px 'Geist Variable', system-ui, sans-serif`
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

function Scatter({ points, dimensions, axis, scale, spotlight, marked, named, onSelect, interactions, yTitle, height = 400 }: {
  dimensions: string; points: Plotted[]; axis: Axis; scale: Scale; spotlight: string | null; marked: string | null; named?: Set<string>; onSelect: (id: string) => void; interactions: Interactions; yTitle: string; height?: number
}) {
  const { ref, width } = useElementSize()
  const corner = useId()
  const margin = { top: 18, right: 22, bottom: 50, left: 62 }
  const plot = { left: margin.left, right: Math.max(margin.left + 1, width - margin.right), top: margin.top, bottom: height - margin.bottom }
  const px = (value: number) => position(scale, value, plot.left, plot.right)
  const py = (value: number) => plot.bottom - value / 100 * (plot.bottom - plot.top)
  const labelText = labelOf(points)
  const ordered = [...points].sort((left, right) => right.y - left.y)
  const nudge = new Map<string, number>()
  const drawn: { x: number; y: number }[] = []
  for (const point of ordered) {
    const y = py(point.y)
    let x = px(point.x)
    while (drawn.some(other => Math.hypot(other.x - x, other.y - y) < 8)) x += 9
    drawn.push({ x, y })
    nudge.set(point.summary.configuration.id, x - px(point.x))
  }
  const sx = (point: Plotted) => px(point.x) + (nudge.get(point.summary.configuration.id) ?? 0)
  const outside = (point: Point) => spotlight !== null && setupParts(point.summary.configuration).method !== spotlight
  const markedPoint = points.find(point => point.summary.configuration.id === marked)
  const active = spotlight ?? (markedPoint ? setupParts(markedPoint.summary.configuration).method : null)
  const lines = groupPaths(points, point => methodKey(point.summary.configuration)).filter(group => group.points.length > 1)
  const along = (from: Plotted, to: Plotted) => {
    const steps = Math.ceil(Math.hypot(sx(to) - sx(from), py(to.y) - py(from.y)) / 6)
    return Array.from({ length: steps }, (_, index) => {
      const x = sx(from) + (sx(to) - sx(from)) * index / steps, y = py(from.y) + (py(to.y) - py(from.y)) * index / steps
      return { left: x - 2, top: y - 2, right: x + 2, bottom: y + 2 }
    })
  }
  const obstacles = [
    ...ordered.map(point => ({ left: sx(point) - 10, top: py(point.y) - 10, right: sx(point) + 10, bottom: py(point.y) + 10 })),
    ...lines.flatMap(group => group.points.slice(1).flatMap((point, index) => along(group.points[index]!, point))),
  ]
  const request = (point: Plotted) => ({ id: point.summary.configuration.id, x: sx(point), y: py(point.y), width: textWidth(labelText(point.summary.configuration)), height: 16 })
  const bounds = { left: plot.left + 2, right: plot.right, top: 2, bottom: plot.bottom - 2 }
  const first = (point: Plotted) => point === markedPoint || spotlight !== null || !named || named.has(point.summary.configuration.id)
  const shown = ordered.filter(point => point !== markedPoint && !outside(point))
  const leading = width ? placeLabels([...(markedPoint ? [markedPoint] : []), ...shown.filter(first)].map(request), obstacles, bounds, width < 600 ? [12, 22] : [12, 24, 36, 48]) : []
  const placed = width ? [...leading, ...placeLabels(shown.filter(point => !first(point)).map(request), [...obstacles, ...leading.map(label => label.box)], bounds, width < 600 ? [12, 20] : [12, 22, 34, 46])] : []
  const labels = markedPoint && width && !placed.some(label => label.id === marked)
    ? [{ id: markedPoint.summary.configuration.id, x: sx(markedPoint) + 12, y: py(markedPoint.y), anchor: 'start' as const, leader: false }, ...placed] : placed
  const xTicks = ticks(scale, width > 700)
  return <div ref={ref} className="tradeoff">
    {width > 0 && <svg width={width} height={height} role="group" aria-label={`Plot of ${dimensions}. A line joins the setups of each review method.`}>
      <defs><linearGradient id={corner} x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="var(--brand-accent)" stopOpacity="0.09" /><stop offset="0.45" stopColor="var(--brand-accent)" stopOpacity="0" />
      </linearGradient></defs>
      <rect x={plot.left} y={plot.top} width={plot.right - plot.left} height={plot.bottom - plot.top} fill={`url(#${corner})`} />
      <Axes plot={plot} axis={axis} scale={scale} xTicks={xTicks} px={px} py={py} height={height} yTitle={yTitle} scaleNote={width > 480} />
      {lines.map(group => <polyline key={group.key} points={group.points.map(point => `${sx(point)},${py(point.y)}`).join(' ')}
        className={[outside(group.points[0]!) ? 'method-line muted' : setupParts(group.points[0]!.summary.configuration).method === active ? 'method-line active' : 'method-line', group.points[0]!.summary.configuration.experimental ? 'experiment' : ''].join(' ').trim()} style={{ stroke: reviewColor(group.points[0]!.summary.configuration) }} />)}
      {labels.filter(label => label.leader).map(label => {
        const point = points.find(item => item.summary.configuration.id === label.id)
        return point && <line key={`leader-${label.id}`} x1={sx(point)} y1={py(point.y)} x2={label.anchor === 'middle' ? label.x : label.x + (label.anchor === 'start' ? -3 : 3)} y2={label.y} className="leader-line" />
      })}
      {markedPoint && <circle cx={sx(markedPoint)} cy={py(markedPoint.y)} r={12} className="mark-ring" style={{ stroke: reviewColor(markedPoint.summary.configuration) }} />}
      {ordered.map(point => <ChartPoint key={point.summary.configuration.id} point={point} axis={axis} x={sx(point)} y={py(point.y)} muted={outside(point)} onSelect={onSelect} interactions={interactions} />)}
      {labels.map(label => {
        const point = points.find(item => item.summary.configuration.id === label.id)
        return point && <text key={`label-${label.id}`} x={label.x} y={label.y} textAnchor={label.anchor} dominantBaseline="middle"
          className={label.id === marked ? 'point-label marked' : 'point-label'}>{labelText(point.summary.configuration)}</text>
      })}
    </svg>}
    <span className="better-hint" style={{ left: plot.left + 10, top: plot.top + 8 }}><ArrowUpLeft size={13} aria-hidden="true" />Higher detection, {axis === 'doubt' ? 'fewer' : 'lower'} {lowerFirst(axisLabels[axis])}</span>
  </div>
}

function Axes({ plot, axis, scale, xTicks, px, py, height, compact = false, yTitle = 'Detection', scaleNote = true }: {
  plot: { left: number; right: number; top: number; bottom: number }; axis: Axis; scale: Scale; xTicks: number[]
  px: (value: number) => number; py: (value: number) => number; height: number; compact?: boolean; yTitle?: string; scaleNote?: boolean
}) {
  const yTicks = compact ? [0, 50, 100] : scoreTicks
  return <>
    {yTicks.map(tick => <g key={tick}><line x1={plot.left} x2={plot.right} y1={py(tick)} y2={py(tick)} className="grid-line" />
      <text x={plot.left - 8} y={py(tick)} className="tick-label" textAnchor="end" dominantBaseline="middle">{tick}%</text></g>)}
    {xTicks.map(tick => <g key={tick}><line x1={px(tick)} x2={px(tick)} y1={plot.top} y2={plot.bottom} className="grid-line" />
      <text x={px(tick)} y={plot.bottom + 17} className="tick-label" textAnchor="middle">{tickLabel(axis, tick)}</text></g>)}
    <text x={14} y={(plot.top + plot.bottom) / 2} className="axis-title" textAnchor="middle" transform={`rotate(-90 14 ${(plot.top + plot.bottom) / 2})`}>{yTitle}</text>
    <text x={(plot.left + plot.right) / 2} y={height - 8} className="axis-title" textAnchor="middle">{axisLabels[axis]}{scale.kind === 'linear' || !scaleNote ? '' : ` (${scaleNames[scale.kind]})`}</text>
  </>
}

function ChartPoint({ point, axis, x, y, muted = false, onSelect, interactions }: {
  point: Plotted; axis: Axis; x: number; y: number; muted?: boolean; onSelect: (id: string) => void; interactions: Interactions
}) {
  const { configuration } = point.summary
  return <g role="button" tabIndex={0} className={muted ? 'chart-point muted' : 'chart-point'}
    aria-label={`${configuration.short}: ${percent(point.y)}${noted(point.summary)}, ${formatMetric(axis, point.x, point.starred, point.bounded)}. Inspect evidence`}
    onClick={() => onSelect(configuration.id)} {...interactions(configuration.id)}
    onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(configuration.id) } }}>
    <circle cx={x} cy={y} r={14} fill="transparent" />
    <PointMark configuration={configuration} x={x} y={y} hollow={point.bounded} />
  </g>
}

function MethodPanels({ points, axis, scale, yTitle, onSelect, interactions }: {
  points: Plotted[]; axis: Axis; scale: Scale; yTitle: string; onSelect: (id: string) => void; interactions: Interactions
}) {
  const order = ['claude-builtin', 'codex']
  const rank = (key: string) => { const found = order.findIndex(method => key.startsWith(`${method}/`)); return found < 0 ? order.length : found }
  const groups = groupPaths(points, point => methodKey(point.summary.configuration)).sort((left, right) => rank(left.key) - rank(right.key) || left.key.localeCompare(right.key))
  const labelText = labelOf(points)
  return <div className="multiples" role="list" aria-label={`${yTitle} against ${lowerFirst(axisLabels[axis])}, one panel per review method`}>
    {groups.map(group => <MethodPanel key={group.key} members={group.points} context={points.filter(point => !group.points.includes(point))}
      axis={axis} scale={scale} yTitle={yTitle} labelText={labelText} onSelect={onSelect} interactions={interactions} />)}
  </div>
}

function MethodPanel({ members, context, axis, scale, yTitle, labelText, onSelect, interactions }: {
  members: Plotted[]; context: Plotted[]; axis: Axis; scale: Scale; yTitle: string; labelText: (configuration: Configuration) => string; onSelect: (id: string) => void; interactions: Interactions
}) {
  const { ref, width } = useElementSize()
  const height = 262, plot = { left: 60, right: Math.max(61, width - 16), top: 12, bottom: height - 46 }
  const px = (value: number) => position(scale, value, plot.left, plot.right)
  const py = (value: number) => plot.bottom - value / 100 * (plot.bottom - plot.top)
  const first = members[0]!.summary.configuration
  const labels = width ? placeLabels([...members].sort((left, right) => right.y - left.y).map(point => ({
    id: point.summary.configuration.id, x: px(point.x), y: py(point.y), width: textWidth(labelText(point.summary.configuration)), height: 16,
  })), [...members, ...context].map(point => ({ left: px(point.x) - 10, top: py(point.y) - 10, right: px(point.x) + 10, bottom: py(point.y) + 10 })),
  { left: plot.left + 2, right: plot.right, top: 2, bottom: plot.bottom - 2 }) : []
  return <div role="listitem" className="method-panel">
    <div className="panel-head"><span className="configuration-name"><Mark configuration={first} size={10} />{setupParts(first).method}</span>
      <span className="panel-meta">{members.length} {members.length === 1 ? 'model' : 'models'}</span></div>
    <div ref={ref} className="panel-plot">{width > 0 && <svg width={width} height={height} role="group" aria-label={`${setupParts(first).method}: ${members.map(point => `${labelText(point.summary.configuration)} ${percent(point.y)}`).join(', ')}`}>
      <Axes plot={plot} axis={axis} scale={scale} xTicks={ticks(scale)} px={px} py={py} height={height} compact yTitle={yTitle} />
      {context.map(point => <circle key={point.summary.configuration.id} cx={px(point.x)} cy={py(point.y)} r={3.5} className="context-dot" />)}
      {labels.filter(label => label.leader).map(label => {
        const point = members.find(item => item.summary.configuration.id === label.id)
        return point && <line key={`leader-${label.id}`} x1={px(point.x)} y1={py(point.y)} x2={label.anchor === 'middle' ? label.x : label.x + (label.anchor === 'start' ? -3 : 3)} y2={label.y} className="leader-line" />
      })}
      {members.map(point => <ChartPoint key={point.summary.configuration.id} point={point} axis={axis} x={px(point.x)} y={py(point.y)} onSelect={onSelect} interactions={interactions} />)}
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

function Models({ points, measure, onSelect, interactions }: { points: Point[]; measure: string; onSelect: (id: string) => void; interactions: Interactions }) {
  const groups = new Map<string, Point[]>()
  for (const point of points) {
    const { model, effort } = setupParts(point.summary.configuration)
    const key = `${model}${effort ? ` · ${effort}` : ''}`
    groups.set(key, [...groups.get(key) ?? [], point])
  }
  const rows = Array.from(groups, ([model, members]) => ({ model, members: [...members].sort((a, b) => (b.summary.detection ?? 0) - (a.summary.detection ?? 0)) }))
    .sort((left, right) => (right.members[0]!.summary.detection ?? 0) - (left.members[0]!.summary.detection ?? 0) || left.model.localeCompare(right.model))
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
        <span className="model-entries">{[...members].sort((left, right) => (right.summary.detection ?? -1) - (left.summary.detection ?? -1) || left.summary.configuration.short.localeCompare(right.summary.configuration.short)).map(point => {
          const { configuration } = point.summary
          return <button type="button" key={configuration.id} className="model-entry" onClick={() => onSelect(configuration.id)} {...interactions(configuration.id)}
            aria-label={`${configuration.short}: ${percent(point.summary.detection)}${noted(point.summary)}. Inspect evidence`}>
            <Mark configuration={configuration} size={10} /><span>{setupParts(configuration).method}</span>
            <strong><Score summary={point.summary} /></strong>
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
    row('score', measure, percent(summary.detection), range && `${percent(range.low)} to ${percent(range.high)} with any one PR left out`),
    summary.detectionNote && <span key="zero-credit" className="tip-note">{summary.detectionNote}</span>,
    ...axes.map(axis => {
      const metric = metricOf(summary, axis), bounded = boundedOn(summary, axis)
      return row(axis, axisLabels[axis], formatMetric(axis, metric, point.starred, bounded), (metric === null || bounded) && summary.reasons[axis])
    }),
    axes.includes('time') && summary.time && <span key="time" className="tip-note">Mean {duration(summary.time.mean)} · middle 50% {duration(summary.time.q1)} to {duration(summary.time.q3)} · {summary.time.reviews} timed trials</span>,
    row('count', 'Trials admitted', `${summary.admitted} of ${summary.trials}`, `${summary.tasks} PRs · ${summary.completed} complete`),
    !summary.configuration.builtin && row('skill', 'Skill', skillReleaseLabel([summary.configuration])),
  ]
  return <div className={place.side ? `chart-tip side-${place.side}` : place.below ? 'chart-tip below' : 'chart-tip'} role="presentation" style={{ left: place.left, top: place.top }}><strong><SetupName configuration={summary.configuration} mark={false} /></strong>{lines}</div>
}

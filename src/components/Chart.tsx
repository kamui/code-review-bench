import { useState } from 'react'
import type { CSSProperties, DOMAttributes, FocusEvent, MouseEvent, ReactNode } from 'react'
import { useElementSize } from '@mantine/hooks'
import { ArrowUpLeft } from 'lucide-react'
import { groupPaths, linearScale, logScale, paretoFrontier, placeLabels, position, segmentObstacles, ticks } from '../lib/chart'
import type { Scale } from '../lib/chart'
import { compact, detectionLabel, duration, lowerFirst, money, percent, reasonCounts } from '../lib/metrics'
import type { Axis, DetectionView, Summary } from '../lib/metrics'
import type { Configuration } from '../lib/data'
import { skillReleaseLabel } from '../lib/data'

export type ChartView = 'skills' | 'setups' | 'models' | 'tradeoff'
type Range = { low: number; high: number } | null
type Point = { summary: Summary; range: Range; metric: number | null }

const seriesKeys: Record<string, string> = {
  'claude-builtin': 'claude', codex: 'codex', 'ce-code-review': 'ce', 'thermo-nuclear-code-quality-review': 'thermo',
  'review-code/v5b-30-x382': 'review-a',
}

export function reviewColor(configuration: Pick<Configuration, 'method' | 'reviewEdition'>) {
  return `var(--series-${seriesKeys[`${configuration.method}/${configuration.reviewEdition}`] ?? seriesKeys[configuration.method] ?? 'other'})`
}

export function Mark({ configuration, size = 12 }: { configuration: Pick<Configuration, 'method' | 'reviewEdition' | 'builtin'>; size?: number }) {
  return <span className={configuration.builtin ? 'mark mark-builtin' : 'mark mark-skill'} aria-hidden="true"
    style={{ '--mark-color': reviewColor(configuration), '--mark-size': `${size}px` } as CSSProperties} />
}

export const axisLabels: Record<Axis, string> = {
  cost: 'Cost per scheduled trial', tokens: 'Output tokens per scheduled trial', refuted: 'Refuted claims per admitted review on matched PRs', time: 'Median completed-trial time',
}
const setups = (count: number) => `${count} ${count === 1 ? 'setup' : 'setups'}`

const setupParts = (configuration: Configuration) => {
  const [method = configuration.short, model = '', effort = ''] = configuration.short.split(' / ')
  return { method, model, effort }
}

const metricOf = (summary: Summary, axis: Axis) => axis === 'time' ? summary.time ? summary.time.median / 60 : null : summary[axis]

function formatMetric(axis: Axis, value: number | null, configuration?: Configuration) {
  if (value === null) return '—'
  const text = axis === 'cost' ? money(value) : axis === 'tokens' ? compact(value) : axis === 'time' ? duration(value * 60) : value.toFixed(2)
  return axis === 'cost' && configuration?.billing === 'list-price-equivalent' ? `${text}*` : text
}

function tickLabel(axis: Axis, value: number) {
  if (axis === 'cost') return value >= 1 ? `$${value}` : value >= 0.01 ? `$${value.toFixed(2)}` : `$${value}`
  if (axis === 'tokens') return compact(value)
  if (axis === 'time') return value < 1 ? `${Math.round(value * 60)} s` : `${value} min`
  return String(value)
}

const scaleFor = (axis: Axis, values: number[]): Scale => axis === 'refuted' ? linearScale(values) : logScale(values)
const fraction = (scale: Scale, value: number) => position(scale, value, 0, 100)
const scoreTicks = [0, 25, 50, 75, 100]

export function Chart({ summaries, detection, matching, axis, view, showAllLabels, onSelect }: {
  summaries: Summary[]; detection: DetectionView; matching: { included: number; excluded: number }; axis: Axis; view: ChartView; showAllLabels: boolean; onSelect: (id: string) => void
}) {
  const measure = detectionLabel(detection), dimensions = `${lowerFirst(measure)} against ${lowerFirst(axisLabels[axis])}`
  const [hovered, setHovered] = useState<{ id: string; left: number; top: number } | null>(null)
  const points: Point[] = summaries.filter(summary => summary.detection !== null).map(summary => ({
    summary, range: summary.range, metric: metricOf(summary, axis),
  }))
  const measured = points.flatMap(point => point.metric === null ? [] : [{ ...point, x: point.metric, y: point.summary.detection ?? 0 }])
  const withheld = reasonCounts(summaries.map(summary => summary.reasons.detection))
  const frontier = new Set(paretoFrontier(measured).map(point => point.summary.configuration.id))
  const scale = scaleFor(axis, measured.map(point => point.x))
  const hoveredPoint = points.find(point => point.summary.configuration.id === hovered?.id)
  const hover = (id: string, element: Element) => {
    const shell = element.closest('.plot-shell')?.getBoundingClientRect()
    const box = element.getBoundingClientRect()
    if (shell) setHovered({ id, left: box.left + box.width / 2 - shell.left, top: box.top - shell.top })
  }
  const interactions = (id: string) => ({
    onMouseEnter: (event: MouseEvent<Element>) => hover(id, event.currentTarget),
    onFocus: (event: FocusEvent<Element>) => hover(id, event.currentTarget),
    onMouseLeave: () => setHovered(null), onBlur: () => setHovered(null),
  })
  const unplotted = points.filter(point => point.metric === null)
  return <div className="plot-shell" onMouseLeave={() => setHovered(null)}>
    {!points.length ? <div className="plot-empty" role="status"><strong>{measure} is unavailable</strong>
      {summaries.length ? <ul className="reason-list">{withheld.map(row => <li key={row.reason}>{row.reason} ({setups(row.count)})</li>)}</ul>
        : <span>Select at least one review method and model.</span>}
      <span>No other impact band or average is substituted. Choose them above.</span></div>
      : view === 'setups' ? <SetupRows points={points} measure={measure} axis={axis} scale={scale} frontier={frontier} onSelect={onSelect} interactions={interactions} />
        : view === 'tradeoff' || view === 'skills' ? <Scatter mode={view} points={measured} dimensions={dimensions} axis={axis} scale={scale} frontier={frontier} showAllLabels={showAllLabels}
          hovered={hovered?.id ?? null} onSelect={onSelect} interactions={interactions} />
          : <Models points={points} measure={measure} axis={axis} onSelect={onSelect} interactions={interactions} />}
    {hoveredPoint && hovered && <Tooltip point={hoveredPoint} measure={measure} axis={axis} left={hovered.left} top={hovered.top} />}
    {points.length > 0 && <div className="plot-caption">
      {view === 'setups' && <span><span className="whisker-key" aria-hidden="true" />Rows are ordered by {lowerFirst(measure)}. The whisker spans this average with any one whole PR left out: sensitivity to these PRs, not a confidence interval.</span>}
      {view === 'skills' && <span><span className="skill-key" aria-hidden="true" />Each line joins one review method across the models it ran on, from lowest to highest {lowerFirst(axisLabels[axis])}. Hover or focus a point to follow its method.</span>}
      {view === 'tradeoff' && <span><span className="frontier-key" aria-hidden="true" />Frontier of {dimensions}: no other selected setup is higher on the first at a lower value of the second. It says nothing about any other measure.</span>}
      {view === 'models' && <span>Each row is one model. Marks show {lowerFirst(measure)} for each review method that ran on it.</span>}
      {withheld.length > 0 && <span>Not plotted, {lowerFirst(measure)} unavailable: {withheld.map(row => `${row.reason} (${setups(row.count)})`).join(' ')}</span>}
      {unplotted.length > 0 && <span>No {lowerFirst(axisLabels[axis])} for {unplotted.map(point => `${point.summary.configuration.short} (${point.summary.reasons[axis]})`).join('; ')}.</span>}
      {axis === 'refuted' && <span>Refuted claims are compared on the {matching.included} of {matching.included + matching.excluded} selected PRs where every selected setup has admitted, assessed reviews, with PRs weighted equally. A setup that admitted only some trials of a matched PR is measured on the reviews it delivered; the tooltip gives its admitted trials.</span>}
      {axis === 'time' && <span>Completed trials only. Includes replacement attempts; excludes gaps between attempts, provisioning, and grading.</span>}
      {axis === 'cost' && points.some(point => point.summary.configuration.billing === 'list-price-equivalent') && <span>* Subscription usage valued at token list prices, not a bill or quota measurement.</span>}
    </div>}
  </div>
}

type Interactions = (id: string) => Pick<DOMAttributes<Element>, 'onMouseEnter' | 'onFocus' | 'onMouseLeave' | 'onBlur'>

function SetupRows({ points, measure, axis, scale, frontier, onSelect, interactions }: {
  points: Point[]; measure: string; axis: Axis; scale: Scale; frontier: Set<string>; onSelect: (id: string) => void; interactions: Interactions
}) {
  const metricTicks = ticks(scale)
  return <div role="list" aria-label={`Setups ordered by ${lowerFirst(measure)}, with ${lowerFirst(axisLabels[axis])}`}>
    <div className="rank-head" aria-hidden="true">
      <span>Review setup</span>
      <span className="rank-axis"><span className="rank-axis-title">{measure}</span>
        <span className="rank-ticks score-ticks">{scoreTicks.map(tick => <span key={tick} style={{ left: `${tick}%` }}>{tick}%</span>)}</span></span>
      <span className="rank-axis"><span className="rank-axis-title">{axisLabels[axis]}{scale.kind === 'log' && <em>log scale</em>}</span>
        <span className={metricTicks.length > 4 ? 'rank-ticks sparse' : 'rank-ticks'}>{metricTicks.map(tick => <span key={tick} style={{ left: `${fraction(scale, tick)}%` }}>{tickLabel(axis, tick)}</span>)}</span></span>
    </div>
    {points.map(({ summary, range, metric }) => {
      const { configuration } = summary
      const { method, model, effort } = setupParts(configuration)
      const score = summary.detection ?? 0
      return <div role="listitem" key={configuration.id}>
        <button type="button" className="rank-row" onClick={() => onSelect(configuration.id)} {...interactions(configuration.id)}
          aria-label={`${configuration.short}: ${measure} ${percent(score)}${range ? `, ${percent(range.low)} to ${percent(range.high)} leaving one PR out` : ''}; ${axisLabels[axis]} ${formatMetric(axis, metric, configuration)}${frontier.has(configuration.id) ? '; on the frontier of these two measures' : ''}. Inspect evidence`}>
          <span className="rank-name"><Mark configuration={configuration} />
            <span><strong>{method}</strong><span className="rank-model">{model}{effort && ` · ${effort}`}
              {frontier.has(configuration.id) && <span className="frontier-tag">On frontier</span>}</span></span></span>
          <span className="rank-cell"><span className="rank-value">{percent(score)}</span>
            <span className="rank-track">{scoreTicks.map(tick => <span key={tick} className="rank-grid" style={{ left: `${tick}%` }} />)}
              {range && <span className="rank-whisker" style={{ left: `${range.low}%`, width: `${range.high - range.low}%` }} />}
              <span className="rank-dot" style={{ left: `${score}%` }}><Mark configuration={configuration} /></span></span></span>
          <span className="rank-cell"><span className="rank-value">{formatMetric(axis, metric, configuration)}</span>
            <span className="rank-track">{metricTicks.map(tick => <span key={tick} className="rank-grid" style={{ left: `${fraction(scale, tick)}%` }} />)}
              {metric === null ? <span className="rank-missing">unavailable</span>
                : <span className="rank-dot" style={{ left: `${fraction(scale, metric)}%` }}><Mark configuration={configuration} /></span>}</span></span>
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

function Scatter({ mode, points, dimensions, axis, scale, frontier, showAllLabels, hovered, onSelect, interactions }: {
  mode: 'skills' | 'tradeoff'; dimensions: string; points: (Point & { x: number; y: number })[]; axis: Axis; scale: Scale; frontier: Set<string>; showAllLabels: boolean
  hovered: string | null; onSelect: (id: string) => void; interactions: Interactions
}) {
  const { ref, width } = useElementSize()
  const height = 380, margin = { top: 18, right: 22, bottom: 50, left: 48 }
  const plot = { left: margin.left, right: Math.max(margin.left + 1, width - margin.right), top: margin.top, bottom: height - margin.bottom }
  const px = (value: number) => position(scale, value, plot.left, plot.right)
  const py = (value: number) => plot.bottom - value / 100 * (plot.bottom - plot.top)
  const models = new Map<string, number>()
  for (const point of points) {
    const { method, model } = setupParts(point.summary.configuration)
    models.set(`${method}/${model}`, (models.get(`${method}/${model}`) ?? 0) + 1)
  }
  const labelText = (configuration: Configuration) => {
    const { method, model, effort } = setupParts(configuration)
    return (models.get(`${method}/${model}`) ?? 0) > 1 ? `${model} · ${effort}` : model
  }
  const ordered = [...points].sort((left, right) => right.y - left.y)
  const steps = paretoFrontier(points)
  const segment = (x1: number, y1: number, x2: number, y2: number) =>
    ({ left: Math.min(x1, x2) - 2, top: Math.min(y1, y2) - 2, right: Math.max(x1, x2) + 2, bottom: Math.max(y1, y2) + 2 })
  const paths = groupPaths(points, point => methodKey(point.summary.configuration))
  const hoveredPoint = points.find(point => point.summary.configuration.id === hovered)
  const highlight = mode === 'skills' && hoveredPoint ? methodKey(hoveredPoint.summary.configuration) : null
  const obstacles = [
    ...ordered.map(point => ({ left: px(point.x) - 8, top: py(point.y) - 8, right: px(point.x) + 8, bottom: py(point.y) + 8 })),
    ...mode === 'skills' ? paths.flatMap(path => segmentObstacles(path.points.map(point => ({ x: px(point.x), y: py(point.y) }))))
      : steps.slice(1).flatMap((point, index) => {
        const previous = steps[index]!
        return [segment(px(previous.x), py(previous.y), px(point.x), py(previous.y)), segment(px(point.x), py(previous.y), px(point.x), py(point.y))]
      }),
  ]
  const requested = mode === 'skills'
    ? ordered.filter(point => !highlight || methodKey(point.summary.configuration) === highlight)
    : ordered.filter(point => showAllLabels || frontier.has(point.summary.configuration.id) || hovered === point.summary.configuration.id)
      .sort((left, right) => Number(frontier.has(right.summary.configuration.id)) - Number(frontier.has(left.summary.configuration.id)))
  const dimmed = (configuration: Configuration) => mode === 'skills'
    ? highlight !== null && methodKey(configuration) !== highlight
    : !frontier.has(configuration.id) && hovered !== configuration.id
  const labels = width ? placeLabels(requested.map(point => ({
    id: point.summary.configuration.id, x: px(point.x), y: py(point.y), width: textWidth(labelText(point.summary.configuration)), height: 14,
  })), obstacles, { left: plot.left + 2, right: plot.right, top: 2, bottom: plot.bottom - 2 }) : []
  const stepPath = steps.map((point, index) => {
    const previous = steps[index - 1]
    return previous ? `H${px(point.x)}V${py(point.y)}` : `M${px(point.x)},${py(point.y)}`
  }).join('')
  const xTicks = ticks(scale)
  return <div ref={ref} className="tradeoff">
    {width > 0 && <svg width={width} height={height} role="group" aria-label={mode === 'skills'
      ? `Plot of ${dimensions}, with a line per review method across models.`
      : `Plot of ${dimensions}. ${frontier.size} setups on the frontier of these two measures are labelled.`}>
      <defs><linearGradient id="better-corner" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stopColor="var(--brand-accent)" stopOpacity="0.09" /><stop offset="0.45" stopColor="var(--brand-accent)" stopOpacity="0" />
      </linearGradient></defs>
      <rect x={plot.left} y={plot.top} width={plot.right - plot.left} height={plot.bottom - plot.top} fill="url(#better-corner)" />
      {scoreTicks.map(tick => <g key={tick}><line x1={plot.left} x2={plot.right} y1={py(tick)} y2={py(tick)} className="grid-line" />
        <text x={plot.left - 8} y={py(tick)} className="tick-label" textAnchor="end" dominantBaseline="middle">{tick}%</text></g>)}
      {xTicks.map(tick => <g key={tick}><line x1={px(tick)} x2={px(tick)} y1={plot.top} y2={plot.bottom} className="grid-line" />
        <text x={px(tick)} y={plot.bottom + 18} className="tick-label" textAnchor="middle">{tickLabel(axis, tick)}</text></g>)}
      <text x={(plot.left + plot.right) / 2} y={height - 8} className="axis-title" textAnchor="middle">
        {axisLabels[axis]}{scale.kind === 'log' ? ' (log scale)' : ''}</text>
      {mode === 'tradeoff' && steps.length > 1 && <path d={stepPath} className="frontier-line" />}
      {mode === 'skills' && paths.filter(path => path.points.length > 1).map(path => <polyline key={path.key}
        points={path.points.map(point => `${px(point.x)},${py(point.y)}`).join(' ')} stroke={reviewColor(path.points[0]!.summary.configuration)}
        className={highlight && highlight !== path.key ? 'skill-line faded' : 'skill-line'} />)}
      {labels.filter(label => label.leader).map(label => {
        const point = points.find(item => item.summary.configuration.id === label.id)
        return point && <line key={`leader-${label.id}`} x1={px(point.x)} y1={py(point.y)} x2={label.anchor === 'middle' ? label.x : label.x + (label.anchor === 'start' ? -3 : 3)} y2={label.y} className="leader-line" />
      })}
      {ordered.map(point => {
        const { configuration } = point.summary
        const faded = dimmed(configuration)
        return <g key={configuration.id} role="button" tabIndex={0} className={faded ? 'chart-point faded' : 'chart-point'}
          aria-label={`${configuration.short}: ${percent(point.y)}, ${formatMetric(axis, point.x, configuration)}${frontier.has(configuration.id) ? ', on the frontier of these two measures' : ''}. Inspect evidence`}
          onClick={() => onSelect(configuration.id)} {...interactions(configuration.id)}
          onKeyDown={event => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(configuration.id) } }}>
          <circle cx={px(point.x)} cy={py(point.y)} r={14} fill="transparent" />
          <PointMark configuration={configuration} x={px(point.x)} y={py(point.y)} />
        </g>
      })}
      {labels.map(label => {
        const point = points.find(item => item.summary.configuration.id === label.id)
        return point && <text key={`label-${label.id}`} x={label.x} y={label.y} textAnchor={label.anchor} dominantBaseline="middle"
          className={mode === 'skills' || frontier.has(label.id) ? 'point-label' : 'point-label secondary'}>{labelText(point.summary.configuration)}</text>
      })}
    </svg>}
    <span className="better-hint" style={{ left: plot.left + 10, top: plot.top + 8 }}><ArrowUpLeft size={13} aria-hidden="true" />Higher detection, lower {lowerFirst(axisLabels[axis])}</span>
  </div>
}

function PointMark({ configuration, x, y }: { configuration: Configuration; x: number; y: number }) {
  const color = reviewColor(configuration)
  return configuration.builtin
    ? <circle cx={x} cy={y} r={6.5} fill={color} className="point-mark" />
    : <path d={`M${x},${y - 8}L${x + 8},${y}L${x},${y + 8}L${x - 8},${y}Z`} fill={color} className="point-mark" />
}

function Models({ points, measure, axis, onSelect, interactions }: { points: Point[]; measure: string; axis: Axis; onSelect: (id: string) => void; interactions: Interactions }) {
  const groups = new Map<string, Point[]>()
  for (const point of points) {
    const { model, effort } = setupParts(point.summary.configuration)
    const key = `${model}${effort && effort !== 'High' ? ` · ${effort}` : ''}`
    groups.set(key, [...groups.get(key) ?? [], point])
  }
  const rows = Array.from(groups, ([model, members]) => ({ model, members: [...members].sort((a, b) => (b.summary.detection ?? 0) - (a.summary.detection ?? 0)) }))
    .sort((left, right) => right.members.length - left.members.length || (right.members[0]?.summary.detection ?? 0) - (left.members[0]?.summary.detection ?? 0))
  return <div className="models" role="list" aria-label={`${measure} by model and review method`}>
    <div className="model-head" aria-hidden="true"><span>Model</span>
      <span className="rank-ticks">{scoreTicks.map(tick => <span key={tick} style={{ left: `${tick}%` }}>{tick}%</span>)}</span></div>
    {rows.map(({ model, members }) => {
      const scores = members.map(point => point.summary.detection ?? 0)
      const low = Math.min(...scores), high = Math.max(...scores)
      return <div role="listitem" key={model} className="model-row">
        <span className="model-name">{model}</span>
        <span className="rank-track">{scoreTicks.map(tick => <span key={tick} className="rank-grid" style={{ left: `${tick}%` }} />)}
          {members.length > 1 && <span className="model-span" style={{ left: `${low}%`, width: `${high - low}%` }} />}
          {members.map((point, index) => {
            const score = point.summary.detection ?? 0
            const stacked = members.slice(0, index).filter(other => Math.abs((other.summary.detection ?? 0) - score) < 1.5).length
            return <span key={point.summary.configuration.id} className="rank-dot" style={{ left: `${score}%`, marginTop: stacked * 9 - (stacked ? 2 : 0) }}>
              <Mark configuration={point.summary.configuration} size={13} /></span>
          })}</span>
        <span className="model-entries">{members.map(point => {
          const { configuration } = point.summary
          return <button type="button" key={configuration.id} className="model-entry" onClick={() => onSelect(configuration.id)} {...interactions(configuration.id)}
            aria-label={`${configuration.short}: ${percent(point.summary.detection)}, ${axisLabels[axis]} ${formatMetric(axis, point.metric, configuration)}. Inspect evidence`}>
            <Mark configuration={configuration} size={10} /><span>{setupParts(configuration).method}</span>
            <strong>{percent(point.summary.detection)}</strong><span className="model-metric">{formatMetric(axis, point.metric, configuration)}</span>
          </button>
        })}</span>
      </div>
    })}
  </div>
}

function Tooltip({ point, measure, axis, left, top }: { point: Point; measure: string; axis: Axis; left: number; top: number }) {
  const { summary, range } = point
  const lines: ReactNode[] = [
    !summary.configuration.builtin && <span key="skill">Skill: {skillReleaseLabel([summary.configuration])}</span>,
    <span key="score">{measure}: <b>{percent(summary.detection)}</b>{range && ` (${percent(range.low)} to ${percent(range.high)} leaving one PR out; sensitivity, not a confidence interval)`}</span>,
    <span key="metric">{axisLabels[axis]}: <b>{formatMetric(axis, point.metric, summary.configuration)}</b>{point.metric === null && ` ${summary.reasons[axis]}`}</span>,
    axis === 'time' && summary.time && <span key="time">Mean {duration(summary.time.mean)} · middle 50% {duration(summary.time.q1)} to {duration(summary.time.q3)} · {summary.time.reviews} timed trials</span>,
    <span key="count">{summary.tasks} PRs · {summary.admitted} of {summary.trials} scheduled trials admitted, {summary.completed} complete</span>,
  ]
  return <div className="chart-tip" role="presentation" style={{ left, top }}><strong>{summary.configuration.label}</strong>{lines}</div>
}

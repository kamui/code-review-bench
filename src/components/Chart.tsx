import { ScatterChart } from '@mantine/charts'
import { ArrowUpRight } from 'lucide-react'
import { z } from 'zod'
import { compact, modelComparisonSegments, money, percent } from '../lib/metrics'
import type { Axis, Summary } from '../lib/metrics'
import type { Configuration } from '../lib/data'

const colors: Record<string, string> = {
  'claude-builtin': '#8b6bd6',
  codex: '#258f9b',
  'review-code/v5b-30-x382': '#c17a32',
  'review-code/v5b-25': '#779846',
  'ce-code-review': '#b75286',
  'thermo-nuclear-code-quality-review': '#92722c',
}

export function reviewColor(configuration: Pick<Configuration, 'method' | 'reviewEdition'>) {
  return colors[`${configuration.method}/${configuration.reviewEdition}`] ?? colors[configuration.method] ?? '#607080'
}

export const axisLabels: Record<Axis, string> = {
  cost: 'Average review cost', tokens: 'Average output tokens', falseFindings: 'False findings per review',
}

const coordinateSchema = z.object({ cx: z.number(), cy: z.number(), payload: z.object({ index: z.number() }) })
const tooltipSchema = z.object({ active: z.boolean().optional(), payload: z.array(z.object({ payload: z.object({ index: z.number() }) })).optional() })

export function Chart({ summaries, axis, onSelect }: { summaries: Summary[]; axis: Axis; onSelect: (id: string) => void }) {
  const points = summaries.flatMap(summary => {
    const value = axis === 'tokens' ? summary.tokens : summary[axis]
    return value === null || summary.score === null ? [] : [{ summary, x: value, y: summary.score }]
  })
  const max = Math.max(axis === 'cost' ? 0.1 : axis === 'tokens' ? 1000 : 0.1, ...points.map(point => point.x)) * 1.22
  const connections = modelComparisonSegments(points.map(point => ({ x: point.x, y: point.y, configuration: point.summary.configuration })))
  const format = (value: number) => axis === 'cost' ? money(value) : axis === 'tokens' ? compact(value) : value.toFixed(2)
  const shape = (value: unknown) => {
    const parsed = coordinateSchema.safeParse(value)
    if (!parsed.success) return <g />
    const { cx, cy, payload } = parsed.data
    const point = points[payload.index]
    if (!point) return <g />
    const configuration = point.summary.configuration
    const [method, ...setup] = configuration.short.split(' / ')
    const right = point.x < max * 0.25
    const nearbyLabels = points.filter((other, index) => index < payload.index && Math.abs(other.x - point.x) < max * 0.2 && Math.abs(other.y - point.y) < 14).length
    const labelOffset = nearbyLabels ? 22 + (nearbyLabels - 1) * 28 : -26
    return <g role="button" tabIndex={0} className="chart-point"
      aria-label={`${configuration.short}: ${percent(point.y)}, ${format(point.x)}. Inspect configuration`}
      onClick={() => onSelect(configuration.id)} onKeyDown={event => {
        if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(configuration.id) }
      }}>
      <circle cx={cx} cy={cy} r={15} fill="transparent" />
      <circle cx={cx} cy={cy} r={6} fill={reviewColor(configuration)} stroke="var(--panel)" strokeWidth={2} />
      <text x={cx + (right ? -12 : 12)} y={cy + labelOffset} textAnchor={right ? 'end' : 'start'} className="point-label">
        <tspan>{method}</tspan>
        <tspan x={cx + (right ? -12 : 12)} dy={14}>{setup.join(' · ')}</tspan>
      </text>
    </g>
  }
  const tooltip = (value: unknown) => {
    const parsed = tooltipSchema.safeParse(value)
    const index = parsed.success && parsed.data.active ? parsed.data.payload?.[0]?.payload.index : undefined
    const point = index === undefined ? undefined : points[index]
    return point ? <div className="chart-tip"><strong>{point.summary.configuration.label}</strong><span>Review edition: {point.summary.configuration.reviewEdition}</span><span>Findings score: {percent(point.y)}</span><span>{axisLabels[axis]}: {format(point.x)}</span><span>{point.summary.tasks} tasks / {point.summary.completed} completed reviews</span></div> : null
  }
  return <div className="plot-shell">
    <div className="plot-hint"><span>Findings score</span><span>Better value <ArrowUpRight size={14} /></span></div>
    <ScatterChart h={360} className="benchmark-chart" data={points.map((point, index) => ({
      name: point.summary.configuration.short, color: reviewColor(point.summary.configuration),
      data: [{ x: point.x, y: point.y, index }],
    }))} dataKey={{ x: 'x', y: 'y' }} xAxisLabel={axisLabels[axis]}
      xAxisProps={{ reversed: true, domain: [0, max], tickCount: 5, allowDecimals: true }}
      yAxisProps={{ domain: [0, 100], ticks: [0, 20, 40, 60, 80, 100], width: 48 }}
      valueFormatter={{ x: format, y: value => `${value}%` }}
      labels={{ x: axisLabels[axis], y: 'Findings score' }}
      scatterProps={{ shape, isAnimationActive: false }} tooltipProps={{ content: tooltip }}
      scatterChartProps={{ margin: { top: 28, right: 24, bottom: 10, left: 6 } }}
      referenceLines={connections.map(({ from, to, configuration }) => ({
        segment: [from, to], color: reviewColor(configuration), strokeDasharray: '4 5', strokeWidth: 1.5,
      }))} />
    {!points.length && <div className="plot-empty"><strong>No comparable measurements</strong><span>Select a review setup and tasks with recorded results. Severity views need adjudicated labels.</span></div>}
    <div className="plot-caption"><span className="model-connection-key" /> Same review method + edition across models <span className="caption-separator" /> Hover for values. Select a point to inspect its evidence.</div>
  </div>
}

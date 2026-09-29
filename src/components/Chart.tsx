import { ScatterChart } from '@mantine/charts'
import { ArrowUpRight } from 'lucide-react'
import { z } from 'zod'
import { compact, modelComparisonSegments, money, percent } from '../lib/metrics'
import type { Axis, Summary } from '../lib/metrics'

const colors: Record<string, string> = {
  'claude-builtin-sonnet-5-5': '#5568d9', 'claude-builtin-sonnet-5': '#13887e',
  'claude-builtin-opus-5-5': '#b57a16', 'codex-builtin': '#bb526e',
  'review-code-sonnet-5-5': '#8a62b8', 'review-code-sonnet-5-5e12864': '#4d829f',
  'review-code-sonnet-5-c3c53da': '#778a37',
}

export function configurationColor(id: string) { return colors[id] ?? '#607080' }

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
    const right = point.x < max * 0.25
    const below = points.some((other, index) => index < payload.index && Math.abs(other.x - point.x) < max * 0.2 && Math.abs(other.y - point.y) < 8)
    return <g role="button" tabIndex={0} className="chart-point"
      aria-label={`${configuration.short}: ${percent(point.y)}, ${format(point.x)}. Inspect configuration`}
      onClick={() => onSelect(configuration.id)} onKeyDown={event => {
        if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); onSelect(configuration.id) }
      }}>
      <circle cx={cx} cy={cy} r={15} fill="transparent" />
      <circle cx={cx} cy={cy} r={6} fill={configurationColor(configuration.id)} stroke="var(--panel)" strokeWidth={2} />
      <text x={cx + (right ? -12 : 12)} y={cy + (below ? 22 : -12)} textAnchor={right ? 'end' : 'start'} className="point-label">
        {configuration.short.replace('Built-in / ', '').replace('Codex / ', '')}
      </text>
    </g>
  }
  const tooltip = (value: unknown) => {
    const parsed = tooltipSchema.safeParse(value)
    const index = parsed.success && parsed.data.active ? parsed.data.payload?.[0]?.payload.index : undefined
    const point = index === undefined ? undefined : points[index]
    return point ? <div className="chart-tip"><strong>{point.summary.configuration.short}</strong><span>{point.summary.configuration.version}</span><span>Findings score: {percent(point.y)}</span><span>{axisLabels[axis]}: {format(point.x)}</span><span>{point.summary.tasks} tasks / {point.summary.completed} completed reviews</span></div> : null
  }
  return <div className="plot-shell">
    <div className="plot-hint"><span>Findings score</span><span>Better value <ArrowUpRight size={14} /></span></div>
    <ScatterChart h={360} className="benchmark-chart" data={points.map((point, index) => ({
      name: point.summary.configuration.short, color: configurationColor(point.summary.configuration.id),
      data: [{ x: point.x, y: point.y, index }],
    }))} dataKey={{ x: 'x', y: 'y' }} xAxisLabel={axisLabels[axis]}
      xAxisProps={{ reversed: true, domain: [0, max], tickCount: 5, allowDecimals: true }}
      yAxisProps={{ domain: [0, 100], ticks: [0, 20, 40, 60, 80, 100], width: 48 }}
      valueFormatter={{ x: format, y: value => `${value}%` }}
      labels={{ x: axisLabels[axis], y: 'Findings score' }}
      scatterProps={{ shape, isAnimationActive: false }} tooltipProps={{ content: tooltip }}
      scatterChartProps={{ margin: { top: 28, right: 24, bottom: 10, left: 6 } }}
      referenceLines={connections.map(({ from, to }) => ({
        segment: [from, to], color: '#93a6b9', strokeDasharray: '4 5', strokeWidth: 1,
      }))} />
    {!points.length && <div className="plot-empty"><strong>No comparable measurements</strong><span>Select a review setup and tasks with recorded results. Severity views need adjudicated labels.</span></div>}
    <div className="plot-caption"><span className="model-connection-key" /> Same review method + version across models <span className="caption-separator" /> Hover for values. Select a point to inspect its evidence.</div>
  </div>
}

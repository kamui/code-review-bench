import { Text } from '@mantine/core'
import { measured, perReview } from '../lib/metrics'
import type { LabelSegment, Measure } from '../lib/scoring'
import { labelSegments } from '../lib/scoring'
import { DataTable, Notes, notesFor, Value } from './Reading'
import type { Column } from './Reading'

export const segmentLabels: Record<LabelSegment, string> = {
  'problem:serious': 'Problem, serious', 'problem:other-material': 'Problem, other material', 'problem:unknown': 'Problem, unknown impact',
  'minor-defect': 'Minor defect', 'suggestion:improvement': 'Suggestion: improvement', 'suggestion:outside-supported-use': 'Suggestion: outside supported use',
  'suggestion:relied-on': 'Suggestion: relied on, not promised', 'suggestion:known-cause': 'Suggestion: cause of a known problem',
  'outside-this-change': 'Outside this change', refuted: 'Refuted', unproven: 'Unproven', unresolved: 'Unresolved',
}
const untrueSegments: LabelSegment[] = ['refuted', 'unproven', 'unresolved']
const trueSegments = labelSegments.filter(segment => !untrueSegments.includes(segment))
const hatched: LabelSegment[] = ['outside-this-change', 'unresolved']

type Row = { id: string; label: string; rates: Record<LabelSegment, Measure> }

const swatch = (segment: LabelSegment) => ({ background: `var(--label-${segment.replace(':', '-')})` })
const segmentClass = (base: string, segment: LabelSegment) => hatched.includes(segment) ? `${base} hatched` : base
const tickStep = (extent: number) => extent <= 3 ? 0.5 : extent <= 8 ? 1 : extent <= 16 ? 2 : 5
const blocked = (row: Row) => labelSegments.map(segment => row.rates[segment]).find(rate => rate.kind === 'unavailable')
const length = (row: Row, segments: LabelSegment[]) => segments.reduce((total, segment) => total + (measured(row.rates[segment]) ?? 0), 0)

function Segments({ row, segments, origin, direction, scale }: { row: Row; segments: LabelSegment[]; origin: number; direction: 1 | -1; scale: number }) {
  let offset = 0
  return segments.flatMap(segment => {
    const value = measured(row.rates[segment]) ?? 0
    if (value <= 0) return []
    const start = offset
    offset += value
    const left = direction === 1 ? origin + start * scale : origin - offset * scale
    return [<span key={segment} className={segmentClass('label-segment', segment)} style={{ ...swatch(segment), left: `${left}%`, width: `${value * scale}%` }}
      title={`${row.label}: ${segmentLabels[segment]}, ${value.toFixed(2)} per admitted review`} />]
  })
}

export function ClaimLabels({ rows }: { rows: Row[] }) {
  const shown = rows.filter(row => !blocked(row))
  const step = tickStep(Math.max(0, ...shown.flatMap(row => [length(row, trueSegments), length(row, untrueSegments)])))
  const fit = (segments: LabelSegment[]) => Math.ceil(Math.max(0, ...shown.map(row => length(row, segments))) / step) * step
  const right = Math.max(step, fit(trueSegments)), left = fit(untrueSegments)
  const scale = 100 / (left + right), origin = left * scale
  const ticks = Array.from({ length: Math.round((left + right) / step) + 1 }, (_, index) => index * step - left)
  const unavailable = (row: Row) => {
    const rate = blocked(row)
    return rate?.kind === 'unavailable' ? { text: 'Unavailable', reason: rate.reason } : ''
  }
  const notes = notesFor(rows.map(unavailable))
  const columns: Column<Row>[] = [{ label: 'Setup', value: row => row.label },
    ...labelSegments.map((segment): Column<Row> => ({ label: segmentLabels[segment], value: row => perReview(row.rates[segment]) }))]
  return <>
    <Text size="sm" className="footnote">Each distinct claim in an assessed admitted review has one label. Only a problem matches a reference problem, so only a problem has an impact band: the most severe band among the references it says what goes wrong for. True claims extend right of the line; refuted, unproven and unresolved claims extend left. These labels have no answer key, so none of them is a miss, and detection stays in its own chart.</Text>
    <ul className="label-legend" aria-label="Claim labels">{labelSegments.map(segment => <li key={segment}>
      <span className={segmentClass('label-swatch', segment)} style={swatch(segment)} aria-hidden="true" />{segmentLabels[segment]}</li>)}</ul>
    <div className="label-plot" role="list" aria-label="Claim labels per admitted review">
      {rows.map(row => {
        const missing = unavailable(row)
        return <div role="listitem" key={row.id} className="label-row"
          aria-label={missing ? `${row.label}: unavailable. ${missing.reason}` : `${row.label}: ${labelSegments.map(segment => `${segmentLabels[segment]} ${perReview(row.rates[segment]).text}`).join(', ')}`}>
          <span className="label-name">{row.label}</span>
          {missing ? <Text size="xs" c="dimmed"><Value cell={missing} notes={notes} /></Text>
            : <span className="label-track" aria-hidden="true">
              <Segments row={row} segments={untrueSegments} origin={origin} direction={-1} scale={scale} />
              <Segments row={row} segments={trueSegments} origin={origin} direction={1} scale={scale} />
              <span className="label-zero" style={{ left: `${origin}%` }} /></span>}
        </div>
      })}
      {shown.length > 0 && <div className="label-row label-axis" aria-hidden="true"><span />
        <span className="label-ticks">{ticks.map(tick => <span key={tick} style={{ left: `${(tick + left) * scale}%` }}>{Math.abs(tick)}</span>)}</span></div>}
    </div>
    <Notes notes={notes} />
    <DataTable label="Claim labels per admitted review" rows={rows} rowKey={row => row.id} minWidth={1600} columns={columns} />
  </>
}

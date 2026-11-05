import { Table, Text, Tooltip, VisuallyHidden } from '@mantine/core'
import type { ReactNode } from 'react'
import { ChevronRight } from 'lucide-react'
import { money, peerLabel, reasonCounts } from '../lib/metrics'
import type { Configuration, Task } from '../lib/data'
import type { Reading } from '../lib/metrics'

export type Cell = string | Reading
export type Note = { reason: string; count: number; covers?: string[] }
export type Column<Row> = { label: string; value: (row: Row) => Cell; render?: (row: Row) => ReactNode }

export function PeerNote({ configurations, className }: { configurations: Configuration[]; className: string }) {
  const label = peerLabel(configurations)
  return label && <span className={className}><Glossed text={label} /></span>
}

export function PeerBadge({ configuration }: { configuration: Configuration }) {
  const label = peerLabel([configuration])
  const entry = glossary.find(item => item.term === 'Cross-model peer')!
  return label && <Tooltip multiline w={300} label={<>{label}. {entry.definition}</>} withArrow><span className="peer-badge"><span aria-hidden="true">{label.replace(/^Cross-model peer /, 'Peer ').replace(/ \(.*\)$/, '')}</span><VisuallyHidden>{label}</VisuallyHidden></span></Tooltip>
}

const listPriced = (configuration: Configuration) => configuration.billing === 'list-price-equivalent'
const listPriceMeaning = 'subscription usage valued at token list prices, not a bill or quota measurement'

// When every shown setup is list-priced, one note covers them all instead of an asterisk on each cost.
export const costStarred = (configuration: Configuration, shown: Configuration[]) => listPriced(configuration) && !shown.every(listPriced)
export const costText = (configuration: Configuration, shown: Configuration[]) => (value: number) => `${money(value)}${costStarred(configuration, shown) ? '*' : ''}`
export const costNote = (shown: Configuration[]) => !shown.some(listPriced) ? null
  : shown.every(listPriced) ? `Every cost is ${listPriceMeaning}.` : `* ${listPriceMeaning.replace(/^./, letter => letter.toUpperCase())}.`
export const costScaleNote = (shown: Configuration[]) => shown.length && shown.every(listPriced) ? 'list prices, log scale' : 'log scale'

const lowerBound = (cell: Cell) => typeof cell !== 'string' && cell.text.startsWith('≥ ')

// A lower bound already says what it is; its reason stays on the value instead of adding a numbered note.
export const notesFor = (cells: Cell[]): Note[] => {
  const groups = new Map<string, Note[]>()
  for (const note of reasonCounts(cells.map(cell => typeof cell === 'string' || lowerBound(cell) ? null : cell.reason ?? cell.annotation ?? null))) {
    const key = unresolvedDetection.test(note.reason) ? unresolvedSummary : note.reason
    groups.set(key, [...groups.get(key) ?? [], note])
  }
  return Array.from(groups, ([reason, members]) => members.length === 1 ? members[0]!
    : { reason, count: members.reduce((total, member) => total + member.count, 0), covers: members.map(member => member.reason) })
}

// Each value's own count stays in its hover text; the shared note says the rest once.
const unresolvedDetection = /^\d+ unresolved detection decisions? across \d+ reference problems? counts? as zero credit\./
const unresolvedSummary = 'Unresolved detection decisions count as zero credit; hover a marked value for its count. They remain in the denominator; confirmed detections still count.'

export function Value({ cell, notes = [] }: { cell: Cell; notes?: Note[] }) {
  if (typeof cell === 'string') return cell
  const note = cell.reason ?? cell.annotation
  if (!note) return cell.text
  const index = notes.findIndex(item => item.reason === note || item.covers?.includes(note))
  return <span className={lowerBound(cell) ? 'annotated' : cell.reason ? 'unavailable' : undefined} title={note}>{cell.text}{index >= 0 && <sup aria-hidden="true">{index + 1}</sup>}<VisuallyHidden>: {note}</VisuallyHidden></span>
}

export function Notes({ notes }: { notes: Note[] }) {
  if (!notes.length) return null
  return <div className="table-notes"><p className="notes-title">Notes on marked values</p>
    <ol aria-label="Notes on these values">
      {notes.map(note => <li key={note.reason}>{note.reason} <span className="note-count">({note.count} {note.count === 1 ? 'value' : 'values'})</span></li>)}
    </ol></div>
}

export function Disclosure({ title, children }: { title: string; children: ReactNode }) {
  return <details className="chart-guide"><summary><ChevronRight className="chevron" size={14} aria-hidden="true" />{title}</summary>{children}</details>
}

export const ChartGuide = ({ children, title = 'How to read this chart' }: { children: ReactNode; title?: string }) => <Disclosure title={title}>{children}</Disclosure>

// Saved review text marks code with Markdown backticks.
export const glossary: { term: string; definition: string; match?: RegExp }[] = [
  { term: 'Review setup', definition: 'A review method run on one model at one effort, with its client and settings.' },
  { term: 'Scheduled trial', definition: 'One planned run of a setup on a PR. Replacement attempts after an infrastructure failure stay inside it.', match: /schedul/i },
  { term: 'Admitted review', definition: 'A trial whose last attempt delivered a usable review.', match: /admitted/i },
  { term: 'Per review', definition: 'Per admitted review: only trials that delivered a usable review count.', match: /per review/i },
  { term: 'Known problem', definition: 'A reference problem: an accepted problem, backed by a saved ruling, that reviews are checked against. The set can be incomplete.' },
  { term: 'Candidate', definition: 'A problem a review reported that is not yet a reference. It earns no credit until a human rules on it.', match: /candidate/i },
  { term: 'Impact band', definition: 'Serious, other material, or unknown. Unknown is never treated as low impact.' },
  { term: 'Refuted or unproven', definition: 'Evidence contradicts a refuted claim. An unproven claim lacks the evidence to stand and is not proven false.', match: /refuted|unproven/i },
  { term: 'Problems or PRs weighted equally', definition: 'Each reference problem counts the same, or each PR counts the same however many problems it has.', match: /weighted equally/i },
  { term: 'Matched PRs', definition: 'The selected PRs on which every selected setup has admitted, assessed reviews, so each setup is measured on the same PRs.', match: /matched PRs?/i },
  { term: 'Cross-model peer', definition: 'A second review by a model from another provider, which some skills can run. Off: the run used a single provider, so the skill ran without it.', match: /cross-model peer|peer off/i },
  { term: 'pp', definition: 'Percentage points: the difference between two percentages.', match: /\bpp\b/ },
  { term: '≥ value', definition: 'A lower bound. Usage was only partly recorded, so the true value is at least this.' },
]

/** A table heading that uses a glossary term carries that term's definition on hover and focus. */
const definition = (entry: (typeof glossary)[number]) => <span className="term-tip"><b>{entry.term}</b> {entry.definition}</span>

/** Running text with the first use of each glossary term marked, its definition on hover and focus. */
const recordId = /\b(GT-[a-z]*\d+|blind-[0-9a-f]+ items? \d+)\b/g

/** Grading records cite known problems and blind-graded review items by ID; each ID shows what it names on hover. */
export function Referenced({ text, task }: { text: string; task: Task | undefined }) {
  return text.split(recordId).map((part, index) => {
    if (index % 2 === 0) return part
    const problem = task?.families.find(family => family.id === part)
    const label = problem ? `Known problem ${part}: ${problem.title}` : part.startsWith('blind-') ? 'A claim in one review, named by the anonymous ID its graders saw.' : null
    return label ? <Tooltip key={index} multiline w={320} withArrow label={label}><span className="term" tabIndex={0}>{part}</span></Tooltip> : part
  })
}

export function Glossed({ text }: { text: string }) {
  const parts: ReactNode[] = []
  const unused = glossary.filter(entry => entry.match)
  let rest = text
  for (;;) {
    const hits = unused.flatMap(entry => {
      const found = entry.match!.exec(rest)
      return found ? [{ entry, start: found.index, end: found.index + found[0].length }] : []
    }).sort((left, right) => left.start - right.start)
    const hit = hits[0]
    if (!hit) break
    let { start, end } = hit
    while (start > 0 && /\w/.test(rest[start - 1]!)) start--
    while (end < rest.length && /\w/.test(rest[end]!)) end++
    parts.push(rest.slice(0, start), <Tooltip key={parts.length} multiline w={300} withArrow label={definition(hit.entry)}>
      <span className="term" tabIndex={0}>{rest.slice(start, end)}</span></Tooltip>)
    unused.splice(unused.indexOf(hit.entry), 1)
    rest = rest.slice(end)
  }
  return <>{parts}{rest}</>
}

export function TermLabel({ label }: { label: string }) {
  const terms = glossary.filter(entry => entry.match?.test(label))
  if (!terms.length) return label
  return <Tooltip multiline w={300} withArrow label={terms.map(entry => <span key={entry.term}>{definition(entry)}</span>)}>
    <span className="term" tabIndex={0}>{label}</span></Tooltip>
}

export function Tag({ children }: { children: ReactNode }) {
  return <span className="tag">{children}</span>
}

export function Rich({ text }: { text: string }) {
  return text.split(/(`[^`]+`)/).map((part, index) => part.length > 2 && part.startsWith('`') && part.endsWith('`')
    ? <code key={index} className="inline-code">{part.slice(1, -1)}</code> : part)
}

export const humanize = (slug: string) => slug.replace(/-/g, ' ').replace(/^./, letter => letter.toUpperCase())

export function DataTable<Row>({ label, rows, columns, rowKey, minWidth, empty = 'No selected setup.', foldShared = true }: {
  label: string; rows: Row[]; columns: Column<Row>[]; rowKey: (row: Row) => string; minWidth: number; empty?: string; foldShared?: boolean
}) {
  const cells = rows.map(row => columns.map(column => column.value(row)))
  const notes = notesFor(cells.flat())
  if (!rows.length) return <Text size="sm" c="dimmed">{empty}</Text>
  const column = (position: number) => cells.map(row => row[position] ?? '')
  // Columns that hold the same value for every setup say more as one line than as a repeated column.
  const constant = columns.map((_, position) => foldShared && position > 0 && rows.length >= 5 && column(position).every(cell => sameCell(cell, cells[0]?.[position] ?? '')))
  const numeric = columns.map((_, position) => position > 0 && column(position).every(cell => /^(≥ |At least )?[$\d.,/ %KM*+-]+$|^[\d.]+% to [\d.]+%$|^[\d.]+\s(s|min)(\sto\s[\d.]+\s(s|min))?$|^Unavailable$|^None$/.test(text(cell))))
  const shown = columns.flatMap((item, position) => constant[position] ? [] : [{ item, position }])
  const shared = columns.flatMap((item, position) => constant[position] ? [{ item, cell: cells[0]?.[position] ?? '' }] : [])
  return <>
    {shared.length > 0 && <p className="shared-values"><span className="shared-title">Same for every setup</span>
      {shared.map(({ item, cell }) => <span key={item.label} className="shared-value"><span>{item.label}</span><b><Value cell={cell} notes={notes} /></b></span>)}</p>}
    <Table.ScrollContainer minWidth={Math.min(minWidth, 180 + shown.length * 84)} className="data-scroll"><Table aria-label={label} className="data-table dim-table">
      <Table.Thead><Table.Tr>{shown.map(({ item, position }) => <Table.Th key={item.label} scope="col" data-numeric={numeric[position] || undefined}><TermLabel label={item.label} /></Table.Th>)}</Table.Tr></Table.Thead>
      <Table.Tbody>{rows.map((row, index) => <Table.Tr key={rowKey(row)}>{shown.map(({ item, position }) => {
        const cell = cells[index]?.[position] ?? ''
        return position === 0 ? <Table.Th key={item.label} scope="row">{item.render ? item.render(row) : <Value cell={cell} notes={notes} />}</Table.Th>
          : <Table.Td key={item.label} data-label={item.label} data-numeric={numeric[position] || undefined}><Value cell={cell} notes={notes} />
            {typeof cell !== 'string' && cell.fraction !== undefined && <span className="cell-bar" aria-hidden="true"><span style={{ width: `${cell.fraction * 100}%` }} /></span>}</Table.Td>
      })}</Table.Tr>)}</Table.Tbody>
    </Table></Table.ScrollContainer>
    <Notes notes={notes} />
  </>
}

const text = (cell: Cell) => typeof cell === 'string' ? cell : cell.text
const sameCell = (left: Cell, right: Cell) => typeof left === 'string' || typeof right === 'string' ? left === right
  : left.text === right.text && (left.reason ?? left.annotation ?? null) === (right.reason ?? right.annotation ?? null)

import { Table, Text, VisuallyHidden } from '@mantine/core'
import type { ReactNode } from 'react'
import { ChevronRight } from 'lucide-react'
import { peerLabel, reasonCounts } from '../lib/metrics'
import type { Configuration } from '../lib/data'
import type { Reading } from '../lib/metrics'

export type Cell = string | Reading
export type Note = ReturnType<typeof reasonCounts>[number]
export type Column<Row> = { label: string; value: (row: Row) => Cell; render?: (row: Row) => ReactNode }

export function PeerNote({ configurations, className }: { configurations: Configuration[]; className: string }) {
  const label = peerLabel(configurations)
  return label && <span className={className}>{label}</span>
}

const lowerBound = (cell: Cell) => typeof cell !== 'string' && cell.text.startsWith('≥ ')

// A lower bound already says what it is; its reason stays on the value instead of adding a numbered note.
export const notesFor = (cells: Cell[]): Note[] => reasonCounts(cells.map(cell => typeof cell === 'string' || lowerBound(cell) ? null : cell.reason ?? cell.annotation ?? null))

export function Value({ cell, notes = [] }: { cell: Cell; notes?: Note[] }) {
  if (typeof cell === 'string') return cell
  const note = cell.reason ?? cell.annotation
  if (!note) return cell.text
  const index = notes.findIndex(item => item.reason === note)
  return <span className={lowerBound(cell) ? 'annotated' : cell.reason ? 'unavailable' : undefined} title={note}>{cell.text}{index >= 0 && <sup aria-hidden="true">{index + 1}</sup>}<VisuallyHidden>: {note}</VisuallyHidden></span>
}

export function Notes({ notes }: { notes: Note[] }) {
  if (!notes.length) return null
  return <details className="table-notes"><summary><ChevronRight className="chevron" size={14} aria-hidden="true" />Notes on marked values <span className="summary-count">{notes.length}</span></summary>
    <ol aria-label="Notes on these values">
      {notes.map(note => <li key={note.reason}>{note.reason} <span className="note-count">({note.count} {note.count === 1 ? 'value' : 'values'})</span></li>)}
    </ol></details>
}

export function Disclosure({ title, children }: { title: string; children: ReactNode }) {
  return <details className="chart-guide"><summary><ChevronRight className="chevron" size={14} aria-hidden="true" />{title}</summary>{children}</details>
}

export const ChartGuide = ({ children }: { children: ReactNode }) => <Disclosure title="How to read this chart">{children}</Disclosure>

// Saved review text marks code with Markdown backticks.
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
  const numeric = columns.map((_, position) => position > 0 && column(position).every(cell => /^(≥ |At least )?[$\d.,/ %KM*+-]+$|^[\d.]+% to [\d.]+%$|^Unavailable$|^None$/.test(text(cell))))
  const shown = columns.flatMap((item, position) => constant[position] ? [] : [{ item, position }])
  const shared = columns.flatMap((item, position) => constant[position] ? [{ item, cell: cells[0]?.[position] ?? '' }] : [])
  return <>
    {shared.length > 0 && <p className="shared-values"><span className="shared-title">Same for every setup</span>
      {shared.map(({ item, cell }) => <span key={item.label} className="shared-value"><span>{item.label}</span><b><Value cell={cell} notes={notes} /></b></span>)}</p>}
    <Table.ScrollContainer minWidth={Math.min(minWidth, 180 + shown.length * 84)} className="data-scroll"><Table aria-label={label} className="data-table">
      <Table.Thead><Table.Tr>{shown.map(({ item, position }) => <Table.Th key={item.label} scope="col" data-numeric={numeric[position] || undefined}>{item.label}</Table.Th>)}</Table.Tr></Table.Thead>
      <Table.Tbody>{rows.map((row, index) => <Table.Tr key={rowKey(row)}>{shown.map(({ item, position }) => {
        const cell = cells[index]?.[position] ?? ''
        return position === 0 ? <Table.Th key={item.label} scope="row">{item.render ? item.render(row) : <Value cell={cell} notes={notes} />}</Table.Th>
          : <Table.Td key={item.label} data-label={item.label} data-numeric={numeric[position] || undefined}><Value cell={cell} notes={notes} /></Table.Td>
      })}</Table.Tr>)}</Table.Tbody>
    </Table></Table.ScrollContainer>
    <Notes notes={notes} />
  </>
}

const text = (cell: Cell) => typeof cell === 'string' ? cell : cell.text
const sameCell = (left: Cell, right: Cell) => typeof left === 'string' || typeof right === 'string' ? left === right
  : left.text === right.text && (left.reason ?? left.annotation ?? null) === (right.reason ?? right.annotation ?? null)

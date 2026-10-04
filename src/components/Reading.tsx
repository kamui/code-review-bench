import { Table, Text, VisuallyHidden } from '@mantine/core'
import { reasonCounts } from '../lib/metrics'
import type { Reading } from '../lib/metrics'

export type Cell = string | Reading
export type Note = ReturnType<typeof reasonCounts>[number]
export type Column<Row> = { label: string; value: (row: Row) => Cell }

export const notesFor = (cells: Cell[]): Note[] => reasonCounts(cells.map(cell => typeof cell === 'string' ? null : cell.reason))

export function Value({ cell, notes = [] }: { cell: Cell; notes?: Note[] }) {
  if (typeof cell === 'string') return cell
  if (cell.reason === null) return cell.text
  const index = notes.findIndex(note => note.reason === cell.reason)
  return <span className="unavailable" title={cell.reason}>{cell.text}{index >= 0 && <sup aria-hidden="true">{index + 1}</sup>}<VisuallyHidden>: {cell.reason}</VisuallyHidden></span>
}

export function Notes({ notes }: { notes: Note[] }) {
  if (!notes.length) return null
  return <ol className="table-notes" aria-label="Why values are unavailable">
    {notes.map(note => <li key={note.reason}>{note.reason} <span className="note-count">({note.count} {note.count === 1 ? 'value' : 'values'})</span></li>)}
  </ol>
}

export function DataTable<Row>({ label, rows, columns, rowKey, minWidth, empty = 'No selected setup.' }: {
  label: string; rows: Row[]; columns: Column<Row>[]; rowKey: (row: Row) => string; minWidth: number; empty?: string
}) {
  const cells = rows.map(row => columns.map(column => column.value(row)))
  const notes = notesFor(cells.flat())
  if (!rows.length) return <Text size="sm" c="dimmed">{empty}</Text>
  return <>
    <Table.ScrollContainer minWidth={minWidth}><Table aria-label={label} className="data-table">
      <Table.Thead><Table.Tr>{columns.map(column => <Table.Th key={column.label} scope="col">{column.label}</Table.Th>)}</Table.Tr></Table.Thead>
      <Table.Tbody>{rows.map((row, index) => <Table.Tr key={rowKey(row)}>{columns.map((column, position) => {
        const cell = cells[index]?.[position] ?? ''
        return position === 0 ? <Table.Th key={column.label} scope="row"><Value cell={cell} notes={notes} /></Table.Th>
          : <Table.Td key={column.label}><Value cell={cell} notes={notes} /></Table.Td>
      })}</Table.Tr>)}</Table.Tbody>
    </Table></Table.ScrollContainer>
    <Notes notes={notes} />
  </>
}

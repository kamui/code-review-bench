import { useState } from 'react'
import { Badge, Button, Chip, Group, Popover, Stack, Switch, Table, Text, Tooltip } from '@mantine/core'
import type { Configuration, Dataset, Family, Task } from '../lib/data'
import { seriousMatrix, skillsPerModel } from '../lib/scoring'
import type { FamilyRow, Scorecard, Selection, TrialOutcome, Verdict } from '../lib/scoring'
import type { Inspection } from '../lib/view'
import { setupParts } from './Chart'

type Column = { configuration: Configuration; card: Scorecard; skill: string; model: string; label: string }

const outcomeText: Record<Verdict, string> = {
  caught: 'Caught.', missed: 'Not caught.', unadmitted: 'Failed: this run produced no review, which counts as not caught.',
  unresolved: 'Not determined yet: whether this review caught it is unresolved.', ungraded: 'Not determined yet: this review awaits assessment.',
  pending: 'Not determined yet: this run is pending.',
}
const legend: { outcome: TrialOutcome; text: string }[] = [
  { outcome: 'caught', text: 'Caught' }, { outcome: 'not-caught', text: 'Not caught, including a failed run that produced no review' },
  { outcome: 'undetermined', text: 'Not determined yet: ungraded, unresolved or pending' }]
const problems = (count: number) => `${count} serious ${count === 1 ? 'problem' : 'problems'}`

function columnsOf(configurations: Configuration[], cards: Scorecard[]): Column[] {
  const parts = configurations.flatMap(configuration => {
    const card = cards.find(item => item.configurationId === configuration.id)
    return card ? [{ configuration, card, ...setupParts(configuration) }] : []
  })
  return parts.map(({ configuration, card, method, model, effort }) => {
    const twin = parts.some(other => other.configuration.id !== configuration.id && other.method === method && other.model === model)
    return { configuration, card, skill: method, model, label: twin && effort ? `${model} · ${effort}` : model }
  }).sort((a, b) => a.skill.localeCompare(b.skill) || a.label.localeCompare(b.label))
}

export function SeriousMisses({ dataset, configurations, cards, selection, onInspect }: {
  dataset: Dataset; configurations: Configuration[]; cards: Scorecard[]; selection: Selection; onInspect: (inspection: Inspection) => void
}) {
  const [model, setModel] = useState('all')
  const [showAll, setShowAll] = useState(false)
  const [hovered, setHovered] = useState<number | null>(null)
  const columns = columnsOf(configurations, cards)
  const models = skillsPerModel(columns)
  const chosen = models.some(item => item.model === model) ? model : 'all'
  const shown = chosen === 'all' ? columns : columns.filter(column => column.model === chosen)
  const matrix = seriousMatrix(dataset, selection, shown.map(column => column.card))
  if (!matrix.tasks.length) return <Text size="sm" c="dimmed">No reference on the selected PRs is labelled serious, so serious misses cannot be listed.</Text>
  const groups = shown.reduce<{ skill: string; span: number }[]>((all, column) => {
    const last = all.at(-1)
    if (last?.skill === column.skill) last.span += 1
    else all.push({ skill: column.skill, span: 1 })
    return all
  }, [])
  const visible = matrix.tasks.map(row => ({ ...row, problems: row.problems.filter(problem => showAll || problem.missed) })).filter(row => row.problems.length)
  const highlight = (index: number, base = '') => `${base}${hovered === index ? ' miss-column' : ''}`.trim() || undefined
  const enter = (index: number) => () => setHovered(index)
  const runs = matrix.trialsPerCell
  return <Stack gap="sm">
    <Chip.Group value={chosen} onChange={value => setModel(typeof value === 'string' ? value : 'all')}>
      <Group gap="xs" aria-label="Model filter" role="radiogroup">
        <Chip value="all" size="xs">All models</Chip>
        {models.map(item => <Chip key={item.model} value={item.model} size="xs">{item.model} · {item.skills} {item.skills === 1 ? 'skill' : 'skills'}</Chip>)}
      </Group>
    </Chip.Group>
    <Text size="sm">{runs === null ? 'Each dot is one scheduled run of a setup on that PR, in run order.'
      : `Each setup reviewed each PR ${runs === 1 ? 'once' : `${runs} times`}; each dot is one run, in run order.`}</Text>
    <Group gap="lg" className="miss-legend" aria-label="Dot legend">
      {legend.map(item => <span key={item.outcome}><span className={`miss-dot ${item.outcome}`} aria-hidden="true" />{item.text}</span>)}
      <span><span className="miss-swatch" aria-hidden="true" />Not caught in two or more runs</span>
    </Group>
    <Switch size="xs" checked={showAll} onChange={event => setShowAll(event.currentTarget.checked)} label={`Show problems no setup missed (${matrix.unmissed})`} className="nowrap-switch" />
    {!visible.length ? <Text size="sm" c="dimmed">No determined miss of a serious reference for the selected setups.</Text>
      : <Table.ScrollContainer minWidth={280 + shown.length * 96}>
        <Table aria-label="Serious references not caught" className="data-table miss-matrix" withRowBorders={false} onMouseLeave={() => setHovered(null)}>
          <Table.Thead>
            <Table.Tr><Table.Th scope="col" rowSpan={2} className="miss-sticky">Serious problem</Table.Th>
              {groups.map((group, index) => <Table.Th key={`${group.skill}-${index}`} scope="colgroup" colSpan={group.span} className="miss-skill">{group.skill}</Table.Th>)}</Table.Tr>
            <Table.Tr>{shown.map((column, index) => <Table.Th key={column.configuration.id} scope="col" className={highlight(index, 'miss-model')}
              onMouseEnter={enter(index)}>{column.label}</Table.Th>)}</Table.Tr>
          </Table.Thead>
          <Table.Tbody>{visible.map(row => {
            const task = dataset.tasks.find(item => item.id === row.taskId)
            if (!task) return null
            return [<Table.Tr key={task.id} className="miss-pr">
              <Table.Th scope="rowgroup" className="miss-sticky">
                <button type="button" className="text-button task-name" onClick={() => onInspect({ kind: 'task', id: task.id })}>{task.repo}<span>#{task.pr}</span></button>
                <Group gap={4} mt={4}>{[...task.profile.changeKinds, ...task.profile.technologies].map(tag => <Badge key={tag} variant="outline" color="gray" size="xs">{tag}</Badge>)}</Group>
                <Text size="xs" c="dimmed" mt={4} fw={400}>{task.shape}</Text>
                <Text size="xs" mt={2} fw={500}>{problems(row.serious)}</Text>
              </Table.Th>
              {shown.map((column, index) => <Table.Td key={column.configuration.id} className={highlight(index)} onMouseEnter={enter(index)} />)}
            </Table.Tr>,
            ...row.problems.map(problem => {
              const family = task.families.find(item => item.id === problem.familyId)
              return <Table.Tr key={`${task.id}/${problem.familyId}`}>
                <Table.Th scope="row" className="miss-sticky"><Problem task={task} family={family} familyId={problem.familyId} onInspect={onInspect} /></Table.Th>
                {problem.cells.map((cell, index) => {
                  const column = shown[index]
                  if (!column) return null
                  const repeated = cell?.repeated.kind === 'available' && cell.repeated.value
                  return <Table.Td key={column.configuration.id} className={highlight(index, repeated ? 'miss-repeated' : '')} onMouseEnter={enter(index)}>
                    {cell ? <span className="miss-dots">{cell.trials.map((trial, position) => <Dot key={trial.replicate} trial={trial} position={position + 1}
                      total={cell.trials.length} setup={`${column.skill} · ${column.label}`} onOpen={attemptId => onInspect({ kind: 'task', id: task.id, attemptId })} />)}</span>
                      : <Text size="xs" c="dimmed" component="span">Not run</Text>}
                  </Table.Td>
                })}
              </Table.Tr>
            })]
          })}</Table.Tbody>
        </Table>
      </Table.ScrollContainer>}
  </Stack>
}

function Problem({ task, family, familyId, onInspect }: { task: Task; family: Family | undefined; familyId: string; onInspect: (inspection: Inspection) => void }) {
  const label = family?.shortLabel ?? familyId
  const [opened, setOpened] = useState(false)
  return <>
    <Popover width={320} position="bottom-start" withArrow shadow="md" opened={opened} onChange={setOpened}>
      <Popover.Target><button type="button" className="text-button short-label" onClick={() => setOpened(value => !value)}>{label}</button></Popover.Target>
      <Popover.Dropdown><Stack gap={6}>
        <Text size="xs" c="dimmed">{task.repo} #{task.pr}</Text>
        <Text size="sm" fw={600}>{label}</Text>
        <Text size="sm">{family?.title ?? familyId}</Text>
        <Button size="xs" variant="light" onClick={() => { setOpened(false); onInspect({ kind: 'task', id: task.id }) }}>Open PR evidence</Button>
      </Stack></Popover.Dropdown>
    </Popover>
    <Text size="xs" c="dimmed" fw={400}>{family?.concerns.length ? family.concerns.join(' · ') : 'No concern tag'}</Text>
  </>
}

function Dot({ trial, position, total, setup, onOpen }: { trial: FamilyRow['trials'][number]; position: number; total: number; setup: string; onOpen: (attemptId: string) => void }) {
  const description = `${setup}, run ${position} of ${total}: ${outcomeText[trial.verdict]}`
  const card = <Stack gap={2}><Text size="xs" fw={600}>{setup}</Text><Text size="xs">Run {position} of {total}</Text><Text size="xs">{outcomeText[trial.verdict]}</Text></Stack>
  const { attemptId } = trial
  return <Tooltip label={card} withArrow multiline w={260}>
    {attemptId ? <button type="button" className={`miss-dot ${trial.outcome}`} aria-label={`${description} Open this review.`} onClick={() => onOpen(attemptId)} />
      : <span className={`miss-dot ${trial.outcome}`} role="img" aria-label={description} />}
  </Tooltip>
}

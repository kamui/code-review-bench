import { useState } from 'react'
import { Alert, Group, Paper, Select, Stack, Table, Tabs, Text, Title } from '@mantine/core'
import type { Configuration, Dataset, Task } from '../lib/data'
import { compareTasks, feedbackSummary, percent } from '../lib/metrics'
import type { DetectionFilter, MetricVersion } from '../lib/metrics'

const count = (value: number | null) => value === null ? 'Unavailable' : String(value)
const points = (value: number | null) => value === null ? 'Unavailable' : `${value > 0 ? '+' : ''}${value.toFixed(2)} pp`

export function MethodologyViews({ dataset, configurations, tasks, filter, version }: {
  dataset: Dataset; configurations: Configuration[]; tasks: Task[]; filter: DetectionFilter; version: MetricVersion
}) {
  const [left, setLeft] = useState<string | null>(null)
  const [right, setRight] = useState<string | null>(null)
  const a = configurations.find(c => c.id === left) ?? configurations[0]
  const b = configurations.find(c => c.id === right && c.id !== a?.id) ?? configurations.find(c => c.id !== a?.id)
  const comparison = a && b ? compareTasks(dataset, a.id, b.id, tasks, filter) : null
  const rows = configurations.map(configuration => ({ configuration, ...feedbackSummary(dataset, configuration.id, tasks) }))
  return <Paper withBorder radius="lg" p="lg" mt="xl">
    <Title order={3}>Inspect reliability and task sensitivity</Title>
    <Text size="sm" c="dimmed" mt="xs">These views use the selected setups and shared task filters. Feedback volume and false allegations remain separate from detection.</Text>
    <Tabs defaultValue="feedback" mt="md">
      <Tabs.List><Tabs.Tab value="feedback">Feedback workload</Tabs.Tab><Tabs.Tab value="sensitivity">PR sensitivity</Tabs.Tab></Tabs.List>
      <Tabs.Panel value="feedback" pt="md"><Stack>
        <Text size="sm">False findings include refuted and unsupported allegations. Per-review rates use admitted terminal reviews; per-trial rates include failed trials. Missing claim-level grading is unavailable.</Text>
        <Table.ScrollContainer minWidth={1200}><Table aria-label="Feedback workload and reliability">
          <Table.Thead><Table.Tr>{['Setup', 'Admitted / trials', 'Items / review', 'False / review', 'False / trial', 'Claim-graded reviews', 'Distinct claims', 'Claim occurrences', 'Useful advice', 'Observations', 'Scope exclusions', 'Refuted', 'Unsupported', 'Unresolved', 'Repeated claims', 'Mixed items'].map(label => <Table.Th key={label}>{label}</Table.Th>)}</Table.Tr></Table.Thead>
          <Table.Tbody>{rows.map(row => <Table.Tr key={row.configuration.id}>
            <Table.Td>{row.configuration.short}</Table.Td><Table.Td>{row.admittedReviews}/{row.trials}</Table.Td>
            <Table.Td>{row.itemsPerReview?.toFixed(2) ?? 'Unavailable'}</Table.Td>
            <Table.Td>{row.falsePerAdmittedReview?.toFixed(2) ?? 'Unavailable'}</Table.Td>
            <Table.Td>{row.falsePerTrial?.toFixed(2) ?? 'Unavailable'}</Table.Td>
            <Table.Td>{row.claimGradedReviews}/{row.admittedReviews}</Table.Td>
            {[row.distinctClaims, row.claimOccurrences, row.advisory, row.inconsequential, row.scopeExcluded, row.refuted, row.unsupported, row.unresolved, row.duplicates, row.mixedItems].map((value, i) => <Table.Td key={i}>{count(value)}</Table.Td>)}
          </Table.Tr>)}</Table.Tbody>
        </Table></Table.ScrollContainer>
        <Text size="xs" c="dimmed">Outcome columns count distinct claims per review, summed across admitted reviews. Repeated claims count extra occurrences. Mixed items count entries with multiple claim outcomes. Item counts measure volume, not reading time. Historical non-material labels combine useful advice and observations.</Text>
        <Table.ScrollContainer minWidth={850}><Table aria-label="Feedback measurement and clean-task coverage">
          <Table.Thead><Table.Tr>{['Setup', 'Volume measured', 'Clean reviews', 'Clean with refuted', 'Clean with unsupported', 'Clean with unresolved', 'Unadmitted outputs', 'Unadmitted false occurrences', 'Pending trials'].map(label => <Table.Th key={label}>{label}</Table.Th>)}</Table.Tr></Table.Thead>
          <Table.Tbody>{rows.map(row => <Table.Tr key={row.configuration.id}>
            <Table.Td>{row.configuration.short}</Table.Td><Table.Td>{row.measuredReviews}/{row.admittedReviews}</Table.Td><Table.Td>{row.cleanReviews}</Table.Td>
            {[row.cleanRefutedFraction, row.cleanUnsupportedFraction, row.cleanUnresolvedFraction].map((v, i) => <Table.Td key={i}>{percent(v === null ? null : 100 * v)}</Table.Td>)}
            <Table.Td>{row.unadmittedOutputs}</Table.Td><Table.Td>{row.unadmittedFalseOccurrences}</Table.Td><Table.Td>{row.pendingTrials}</Table.Td>
          </Table.Tr>)}</Table.Tbody>
        </Table></Table.ScrollContainer>
        <Text size="xs" c="dimmed">Clean here means no currently registered eligible problem. It is not a completed human audit. Unadmitted output stays in an audit subtotal and earns no detection.</Text>
        <Table.ScrollContainer minWidth={650}><Table aria-label="Historical item categories">
          <Table.Thead><Table.Tr><Table.Th>Setup</Table.Th><Table.Th>Legacy reviews</Table.Th><Table.Th>Below-threshold items</Table.Th><Table.Th>Unresolved items</Table.Th><Table.Th>Duplicate items</Table.Th></Table.Tr></Table.Thead>
          <Table.Tbody>{rows.map(row => <Table.Tr key={row.configuration.id}><Table.Td>{row.configuration.short}</Table.Td><Table.Td>{row.legacyReviews}</Table.Td><Table.Td>{row.legacyNonMaterialItems}</Table.Td><Table.Td>{row.legacyUnresolvedItems}</Table.Td><Table.Td>{row.legacyDuplicateItems}</Table.Td></Table.Tr>)}</Table.Tbody>
        </Table></Table.ScrollContainer>
        <Text size="xs" c="dimmed">Historical category totals cover only the listed legacy reviews with measured output. Below-threshold items can include useful advice, observations and scope exclusions.</Text>
      </Stack></Tabs.Panel>
      <Tabs.Panel value="sensitivity" pt="md"><Stack>
        {version === 'historical' ? <Alert color="blue">Select trial-based metrics to inspect repetitions and PR sensitivity using the same calculation as the headline. Historical results retain their original definitions.</Alert> : <>
          <Group><Select label="Comparison A" aria-label="Sensitivity comparison A" value={a?.id ?? null} data={configurations.map(c => ({ value: c.id, label: c.short }))} onChange={setLeft} w={330} />
            <Select label="Comparison B" aria-label="Sensitivity comparison B" value={b?.id ?? null} data={configurations.filter(c => c.id !== a?.id).map(c => ({ value: c.id, label: c.short }))} onChange={setRight} w={330} /></Group>
          {!comparison ? <Text>Select at least two setups.</Text> : <>
            <Text size="sm">A minus B: {points(comparison.full.delta)} across {comparison.rows.length} shared buggy PRs. A wins {comparison.wins}, ties {comparison.ties}, loses {comparison.losses}; {comparison.pending} PRs have unavailable comparisons.</Text>
            <Text size="xs" c="dimmed">Ties use a 0.000000001 percentage-point tolerance for floating-point arithmetic. This is not a significance threshold.</Text>
            <Table.ScrollContainer minWidth={800}><Table aria-label="Per-PR detection and repetition variation">
              <Table.Thead><Table.Tr><Table.Th>PR</Table.Th><Table.Th>References</Table.Th><Table.Th>A mean</Table.Th><Table.Th>B mean</Table.Th><Table.Th>A minus B</Table.Th><Table.Th>Individual repetitions</Table.Th></Table.Tr></Table.Thead>
              <Table.Tbody>{comparison.rows.map(row => <Table.Tr key={row.task.id}>
                <Table.Td>{row.task.repo} #{row.task.pr}</Table.Td><Table.Td>{row.configurations[0]?.repetitions[0]?.references ?? 'Unavailable'}</Table.Td>
                {row.configurations.map(c => <Table.Td key={c.id}>{percent(c.mean === null ? null : c.mean * 100)}<Text size="xs" c="dimmed">Observed {percent(c.min)} to {percent(c.max)}</Text></Table.Td>)}
                <Table.Td>{points(row.delta)}</Table.Td><Table.Td>{row.configurations.map((c, index) => <Text size="xs" key={c.id}>{index === 0 ? 'A' : 'B'}: {c.repetitions.map(t => `#${t.replicate} ${t.recovered === null ? 'pending' : `${t.recovered}/${t.references}`}${!t.admitted && t.attemptId ? ' unadmitted' : t.admitted && !t.complete ? ' incomplete' : ''}`).join(', ')}</Text>)}</Table.Td>
              </Table.Tr>)}</Table.Tbody>
            </Table></Table.ScrollContainer>
            <Table.ScrollContainer minWidth={700}><Table aria-label="Leave-one-PR-out detection comparison">
              <Table.Thead><Table.Tr><Table.Th>Omitted PR</Table.Th><Table.Th>Remaining PRs</Table.Th><Table.Th>A detection</Table.Th><Table.Th>B detection</Table.Th><Table.Th>A minus B</Table.Th></Table.Tr></Table.Thead>
              <Table.Tbody>{comparison.omissions.map(row => <Table.Tr key={row.task.id}><Table.Td>{row.task.repo} #{row.task.pr}</Table.Td><Table.Td>{row.remainingTasks}</Table.Td><Table.Td>{percent(row.a)}</Table.Td><Table.Td>{percent(row.b)}</Table.Td><Table.Td>{points(row.delta)}</Table.Td></Table.Tr>)}</Table.Tbody>
            </Table></Table.ScrollContainer>
            {!comparison.rows.length && <Text>No shared eligible reference problems under these filters.</Text>}
          </>}
          <Text size="xs" c="dimmed">Each omission removes the entire PR and all its repetitions. Results describe sensitivity to these selected tasks, not confidence intervals for unseen PRs. Repetition indices are not matched experimental seeds.</Text>
        </>}
      </Stack></Tabs.Panel>
    </Tabs>
  </Paper>
}

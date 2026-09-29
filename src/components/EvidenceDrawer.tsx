import { useEffect, useState } from 'react'
import { Accordion, Alert, Badge, Button, Code, Drawer, Group, Loader, Paper, Select, Stack, Tabs, Text, Title } from '@mantine/core'
import { ExternalLink, FileJson, FileText, Download } from 'lucide-react'
import { fetchDetail } from '../lib/data'
import type { Attempt, AttemptDetail, Configuration, Dataset, Task } from '../lib/data'
import { compact, money } from '../lib/metrics'

export type Inspection = { kind: 'task'; id: string } | { kind: 'configuration'; id: string }

export function EvidenceDrawer({ dataset, inspection, onClose }: { dataset: Dataset; inspection: Inspection | null; onClose: () => void }) {
  const configuration = inspection?.kind === 'configuration' ? dataset.configurations.find(item => item.id === inspection.id) : undefined
  const task = inspection?.kind === 'task' ? dataset.tasks.find(item => item.id === inspection.id) : undefined
  return <Drawer opened={inspection !== null} onClose={onClose} position="right" size="xl"
    title={task ? `${task.repo} #${task.pr}` : configuration?.short ?? 'Evidence'}
    classNames={{ title: 'drawer-title' }}>
    {inspection && <InspectionBody key={`${inspection.kind}:${inspection.id}`} dataset={dataset} configuration={configuration} task={task} />}
  </Drawer>
}

function InspectionBody({ dataset, configuration, task }: { dataset: Dataset; configuration?: Configuration; task?: Task }) {
  const outcomes = dataset.outcomes.filter(outcome => configuration ? outcome.configurationId === configuration.id : outcome.taskId === task?.id)
  const allowed = new Set(outcomes.flatMap(outcome => outcome.attemptIds))
  const attempts = dataset.attempts.filter(attempt => allowed.has(attempt.id))
  const [attemptId, setAttemptId] = useState<string | null>(attempts[0]?.id ?? null)
  const [tab, setTab] = useState<string | null>('reviews')
  const [detail, setDetail] = useState<AttemptDetail | null>(null)
  const [error, setError] = useState('')
  const attempt = attempts.find(item => item.id === attemptId)
  const currentOutcome = outcomes.find(outcome => outcome.attemptIds.includes(attemptId ?? ''))
  useEffect(() => {
    setDetail(null); setError('')
    if (!attempt) return
    let cancelled = false
    fetchDetail(attempt.detailUrl).then(value => { if (!cancelled) setDetail(value) })
      .catch((reason: unknown) => { if (!cancelled) setError(reason instanceof Error ? reason.message : 'Unable to load review') })
    return () => { cancelled = true }
  }, [attempt])
  const currentTask = task ?? dataset.tasks.find(item => item.id === attempt?.taskId)
  return <Stack gap="lg">
    {configuration && <><Text size="sm" c="dimmed">{configuration.label}</Text><Group><Badge variant="light">{configuration.version}</Badge><Badge variant="outline">Edition: {configuration.reviewEdition}</Badge></Group><Text size="sm">{configuration.note}</Text>
      {configuration.reviewChange && <Button component="a" href={configuration.reviewChange.url} target="_blank" rel="noreferrer" variant="subtle" size="xs">Review change: {configuration.reviewChange.summary}</Button>}
      {configuration.skillProvenanceUrl && <Button component="a" href={configuration.skillProvenanceUrl} target="_blank" rel="noreferrer" variant="subtle" size="xs">Skill commits & timestamps</Button>}</>}
    {task && <><Text>{task.shape}</Text><Group gap="xs">{task.profile.concerns.map(concern => <Badge variant="light" key={concern}>{concern}</Badge>)}</Group>
      <Group><Button component="a" href={task.sourceUrl} target="_blank" rel="noreferrer" variant="light" size="xs" leftSection={<ExternalLink size={14} />}>Original PR</Button>
        <Button component="a" href={task.packetUrl} target="_blank" rel="noreferrer" variant="subtle" size="xs" leftSection={<FileText size={14} />}>Review packet</Button></Group>
      <Text size="xs" c="dimmed">Revision {task.head.slice(0, 10)} / Reference v{task.registerVersion}. Profile labels are proposed; severity is unclassified.</Text></>}
    <Tabs value={tab} onChange={setTab}>
      <Tabs.List><Tabs.Tab value="reviews">Reviews ({attempts.length})</Tabs.Tab><Tabs.Tab value="references">Reference findings</Tabs.Tab><Tabs.Tab value="coverage">Coverage & failures</Tabs.Tab></Tabs.List>
      <Tabs.Panel value="reviews" pt="lg">
        <Stack gap="md">
          <Select label="Review attempt" searchable value={attemptId} onChange={setAttemptId}
            data={attempts.map(item => {
              const owner = outcomes.find(outcome => outcome.attemptIds.includes(item.id))
              const setup = dataset.configurations.find(configuration => configuration.id === owner?.configurationId)
              return { value: item.id, label: `${item.taskId} / ${setup?.short ?? item.runId} / ${item.label}${item.complete ? '' : ' / incomplete or invalid'}` }
            })} />
          {error && <Alert color="red">{error}</Alert>}
          {currentOutcome?.status === 'not comparable' && <Alert color="yellow">This review is excluded from the current comparison. {currentOutcome.reason} Its grading record preserves the original reference version.</Alert>}
          {attempt && <ReviewSummary attempt={attempt} />}
          {attempt && !detail && !error && <Loader size="sm" />}
          {detail && <><Paper withBorder p="md" radius="md"><Stack gap="xs">
            <Text size="sm" fw={650}>Observed review harness</Text>
            <Text size="sm">{detail.record.observed.harness} {detail.record.observed.cli_version}</Text>
            <Text size="sm">Models: {detail.record.observed.models.join(', ')} / Effort: {detail.record.observed.effort ?? 'Not reported'}</Text>
            <Text size="xs" c="dimmed">Prompt identity: {detail.record.observed.prompt_registry_match ?? 'No registry match recorded'}</Text>
            <Code block>{detail.record.observed.prompt_hash ?? 'Prompt hash unavailable'}</Code>
            {detail.record.observed.skill_tree && <><Text size="xs" c="dimmed">Skill tree revision</Text><Code block>{detail.record.observed.skill_tree}</Code></>}
            <Text size="xs" c="dimmed">Subagents: {detail.record.observed.subagent_count} / Client-reported sandbox: {detail.record.observed.sandbox ?? 'Not reported'}</Text>
          </Stack></Paper><Group gap="xs">
            <Button component="a" href={detail.recordUrl} target="_blank" rel="noreferrer" variant="light" size="xs" leftSection={<FileJson size={14} />}>Attempt record</Button>
            {detail.normalizedUrl && <Button component="a" href={detail.normalizedUrl} target="_blank" rel="noreferrer" variant="subtle" size="xs">Review JSON</Button>}
            {currentOutcome?.mappingUrl && <Button component="a" href={currentOutcome.mappingUrl} target="_blank" rel="noreferrer" variant="subtle" size="xs">Grading record</Button>}
            {currentOutcome?.scorecardUrl && <Button component="a" href={currentOutcome.scorecardUrl} target="_blank" rel="noreferrer" variant="subtle" size="xs">Grading notes & candidates</Button>}
            {detail.archiveUrl && <Button component="a" href={detail.archiveUrl} download variant="subtle" size="xs" leftSection={<Download size={14} />}>Raw transcript</Button>}
          </Group>
            {!detail.archiveUrl && <Text size="xs" c="dimmed">Transcript archive: {detail.archiveStatus}. No verified download is available.</Text>}
            {!detail.items.length && <Alert color="gray">This attempt returned no usable findings. Its failure and usage remain in the evidence.</Alert>}
            <Accordion variant="separated">{detail.items.map((item, index) => <Accordion.Item key={item.id} value={item.id}>
              <Accordion.Control><Group gap="xs" mb={5}><Badge size="xs" color={item.assignment.startsWith('defect:') ? 'teal' : item.assignment === 'false-finding' ? 'red' : 'gray'} variant="light">{item.assignment.replace('defect:', 'Found ')}</Badge>
                <Text size="xs" c="dimmed">Finding {index + 1}</Text></Group><Text size="sm" fw={550}>{item.claim}</Text></Accordion.Control>
              <Accordion.Panel><Stack gap="sm"><Text size="xs" c="dimmed">{item.file}{item.line === null ? '' : `:${item.line}`}</Text><Text size="sm">{item.consequence}</Text>
                <Paper p="sm" className="evidence-note"><Text size="xs" fw={650} mb={5}>Adjudication</Text><Text size="sm">{item.notes}</Text></Paper>
                <Text size="sm"><strong>Fix suggestion: </strong>{item.proposedFix ?? 'No separate suggestion captured. The adjudication may discuss a remedy embedded in the finding.'}</Text>
                <Badge variant="outline" color="gray" size="sm">Fix sufficiency: {item.fixSufficiency}</Badge>
              </Stack></Accordion.Panel></Accordion.Item>)}</Accordion>
          </>}
        </Stack>
      </Tabs.Panel>
      <Tabs.Panel value="references" pt="lg">{currentTask && <Stack><Title order={4}>{currentTask.repo}</Title>
        <Text size="sm" c="dimmed">Current reference v{currentTask.registerVersion}, from historical model-assisted judgments. Older reviews may use an earlier version, recorded in their grading record. No severity labels have been adjudicated.</Text>
        {!currentTask.defects.length && <Alert color="teal">No accepted defects in this reference set. This task measures false alarms; it does not have a detection-score denominator.</Alert>}
        <Accordion variant="separated">{currentTask.defects.map(defect => <Accordion.Item key={defect.id} value={defect.id}>
          <Accordion.Control><Text size="xs" c="dimmed">{defect.id} / Unclassified severity</Text><Text fw={550} size="sm">{defect.title}</Text></Accordion.Control>
          <Accordion.Panel><Stack gap="sm"><Text size="sm"><strong>Trigger: </strong>{defect.trigger}</Text><Text size="sm"><strong>Consequence: </strong>{defect.consequence}</Text>
            <Text size="sm"><strong>Required outcome: </strong>{defect.requiredOutcome}</Text></Stack></Accordion.Panel>
        </Accordion.Item>)}</Accordion>
        <Button component="a" href={currentTask.registerUrl} target="_blank" rel="noreferrer" variant="light" size="xs">Open full reference register</Button>
      </Stack>}</Tabs.Panel>
      <Tabs.Panel value="coverage" pt="lg"><Stack>
        <Text size="sm" c="dimmed">All attempts remain visible. Replacements do not erase failed attempts or their usage.</Text>
        {outcomes.filter(outcome => outcome.status !== 'ran').map(outcome => <Alert key={`${outcome.configurationId}:${outcome.taskId}`} color="yellow" title={outcome.taskId}>
          {dataset.configurations.find(item => item.id === outcome.configurationId)?.short}: {outcome.status}. {outcome.reason}
        </Alert>)}
        {attempts.filter(item => !item.complete || item.predecessor).map(item => <Paper withBorder p="md" key={item.id}>
          <Text size="sm" fw={600}>{item.taskId} / {item.label}</Text><Text size="sm">{item.disposition}</Text>
          {!item.complete && item.admitted && <Text size="sm">Review returned findings but reported incomplete coverage.</Text>}
          {item.predecessor && <Text size="sm">Replaces {item.predecessor.split('/').at(-1)}. {item.retryReason}</Text>}
          <Button size="xs" variant="subtle" mt="xs" onClick={() => { setAttemptId(item.id); setTab('reviews') }}>Select this review</Button>
        </Paper>)}
        {attempts.every(item => item.complete && !item.predecessor) && <Text size="sm">No failed or replacement attempts in this selection.</Text>}
      </Stack></Tabs.Panel>
    </Tabs>
  </Stack>
}

function ReviewSummary({ attempt }: { attempt: Attempt }) {
  return <Paper withBorder p="md" radius="md"><Group justify="space-between" gap="sm">
    <Badge color={attempt.complete ? 'teal' : 'orange'} variant="light">{attempt.complete ? 'Completed' : attempt.admitted ? 'Incomplete coverage' : 'Stopped / invalid'}</Badge>
    <Text size="sm">{money(attempt.cost)} / {compact(attempt.outputTokens)} output tokens</Text>
  </Group><Text size="sm" mt="sm">{attempt.recovered.length} reference problems found / {attempt.falseFindings} distinct false findings / {attempt.noise} non-material observations</Text>
    {attempt.predecessor && <Text size="xs" c="dimmed" mt="xs">Replacement for {attempt.predecessor}. Trial costs include the original attempt.</Text>}
  </Paper>
}


import { useMemo, useState } from "react"
import { Plus, Trash2 } from "lucide-react"

import { nodeTypeMeta } from "@/components/app/sample-data"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import {
  branchOperatorValues,
  nodeConfigSchemas,
  type BranchCondition,
  type WorkflowNode,
  type WorkflowNodeType,
} from "@/lib/workflows/types"

type NodeConfigPanelProps = {
  node?: WorkflowNode
  onChange: (node: WorkflowNode) => void
}

type FieldControl = "text" | "textarea" | "number" | "boolean" | "select" | "json" | "json-array" | "conditions"

type FieldDefinition = {
  key: string
  label: string
  control: FieldControl
  options?: string[]
  placeholder?: string
  min?: number
  max?: number
  step?: number
}

const fieldDefinitions: Record<WorkflowNodeType, FieldDefinition[]> = {
  input: [
    { key: "label", label: "Label", control: "text" },
    { key: "inputType", label: "Input type", control: "select", options: ["text"] },
    { key: "placeholder", label: "Placeholder", control: "textarea" },
    { key: "required", label: "Required", control: "boolean" },
  ],
  llm: [
    { key: "label", label: "Label", control: "text" },
    { key: "systemPrompt", label: "System prompt", control: "textarea" },
    { key: "prompt", label: "Prompt", control: "textarea" },
    { key: "model", label: "Model", control: "text", placeholder: "Use workspace default" },
    { key: "temperature", label: "Temperature", control: "number", min: 0, max: 2, step: 0.1 },
    { key: "maxTokens", label: "Max tokens", control: "number", min: 1, max: 8000, step: 1 },
  ],
  planner: [
    { key: "label", label: "Label", control: "text" },
    { key: "objective", label: "Objective", control: "textarea" },
    { key: "planningStyle", label: "Planning style", control: "select", options: ["steps", "checklist", "timeline"] },
    { key: "maxSteps", label: "Max steps", control: "number", min: 1, max: 20, step: 1 },
  ],
  summarizer: [
    { key: "label", label: "Label", control: "text" },
    { key: "instruction", label: "Instruction", control: "textarea" },
    { key: "style", label: "Style", control: "select", options: ["brief", "bullets", "executive"] },
    { key: "maxLength", label: "Max length", control: "number", min: 50, max: 4000, step: 50 },
  ],
  extractor: [
    { key: "label", label: "Label", control: "text" },
    { key: "instruction", label: "Instruction", control: "textarea" },
    { key: "fields", label: "Fields", control: "json-array" },
    { key: "strictJson", label: "Strict JSON", control: "boolean" },
  ],
  textTransform: [
    { key: "label", label: "Label", control: "text" },
    { key: "operation", label: "Operation", control: "select", options: ["rewrite", "format", "classify", "map"] },
    { key: "instruction", label: "Instruction", control: "textarea" },
  ],
  branch: [
    { key: "label", label: "Label", control: "text" },
    { key: "mode", label: "Mode", control: "select", options: ["firstMatch"] },
    { key: "conditions", label: "Conditions", control: "conditions" },
    { key: "fallbackConditionId", label: "Fallback condition ID", control: "text" },
  ],
  merge: [
    { key: "label", label: "Label", control: "text" },
    { key: "strategy", label: "Strategy", control: "select", options: ["concatenate", "firstCompleted", "jsonMerge"] },
    { key: "separator", label: "Separator", control: "textarea" },
  ],
  humanCheckpoint: [
    { key: "label", label: "Label", control: "text" },
    { key: "prompt", label: "Prompt", control: "textarea" },
    { key: "approvalRequired", label: "Approval required", control: "boolean" },
  ],
  output: [
    { key: "label", label: "Label", control: "text" },
    { key: "format", label: "Format", control: "select", options: ["text", "json"] },
    { key: "includeNodeIds", label: "Include node IDs", control: "json-array" },
  ],
  form: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "formFields", label: "Form fields", control: "json-array" },
    { key: "assignees", label: "Assignees", control: "json-array" },
  ],
  questionnaire: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "questionnaire", label: "Questionnaire", control: "json" },
    { key: "assignees", label: "Assignees", control: "json-array" },
  ],
  fileRequest: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "fileRequest", label: "File request", control: "json" },
    { key: "assignees", label: "Assignees", control: "json-array" },
  ],
  manualTask: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "assignees", label: "Assignees", control: "json-array" },
  ],
  approval: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "onReject", label: "Reject behavior", control: "json" },
    { key: "assignees", label: "Assignees", control: "json-array" },
  ],
  acknowledgement: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "assignees", label: "Assignees", control: "json-array" },
  ],
  systemEmail: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "systemEmail", label: "Email", control: "json" },
    { key: "assignees", label: "Assignees", control: "json-array" },
  ],
  eSign: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "signers", label: "Signers", control: "json-array" },
    { key: "assignees", label: "Assignees", control: "json-array" },
  ],
  wait: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "waitFor", label: "Wait for", control: "json" },
  ],
  decision: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "conditions", label: "Conditions", control: "json-array" },
    { key: "paths", label: "Paths", control: "json-array" },
  ],
  parallelBranch: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "paths", label: "Paths", control: "json-array" },
  ],
  gotoMarker: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "destinationLabel", label: "Destination label", control: "text" },
  ],
  goto: [
    { key: "label", label: "Label", control: "text" },
    { key: "description", label: "Description", control: "textarea" },
    { key: "destinationLabel", label: "Destination label", control: "text" },
  ],
}

export function NodeConfigPanel({ node, onChange }: NodeConfigPanelProps) {
  const [errors, setErrors] = useState<Record<string, string>>({})
  const [draftJson, setDraftJson] = useState<Record<string, string>>({})

  if (!node) {
    return (
      <section className="rounded-md border border-border bg-card/70 p-4">
        <h2 className="text-sm font-semibold">Node Config</h2>
        <p className="mt-2 text-sm leading-6 text-muted-foreground">Select a node on the canvas to edit labels and safe configuration values.</p>
      </section>
    )
  }

  const selectedNode = node
  const meta = nodeTypeMeta[selectedNode.type]
  const Icon = meta.icon
  const fields = fieldDefinitions[selectedNode.type]

  function setFieldError(key: string, message?: string) {
    setErrors((current) => {
      const next = { ...current }
      if (message) {
        next[key] = message
      } else {
        delete next[key]
      }
      return next
    })
  }

  function updateConfigValue(key: string, value: unknown) {
    const nextConfig = { ...selectedNode.config, [key]: value }
    const parsed = nodeConfigSchemas[selectedNode.type].safeParse(nextConfig)

    if (!parsed.success) {
      setFieldError(key, parsed.error.issues[0]?.message ?? "Invalid value.")
      return false
    }

    setFieldError(key)
    onChange({ ...selectedNode, config: parsed.data } as WorkflowNode)
    return true
  }

  function updateJsonArray(key: string, rawValue: string) {
    setDraftJson((current) => ({ ...current, [`${selectedNode.id}:${key}`]: rawValue }))

    let parsed: unknown
    try {
      parsed = JSON.parse(rawValue)
    } catch {
      setFieldError(key, "Enter a valid JSON array.")
      return
    }

    if (!Array.isArray(parsed)) {
      setFieldError(key, "Enter a valid JSON array.")
      return
    }

    if (updateConfigValue(key, parsed)) {
      setDraftJson((current) => ({ ...current, [`${selectedNode.id}:${key}`]: JSON.stringify(parsed, null, 2) }))
    }
  }

  function updateJsonValue(key: string, rawValue: string) {
    setDraftJson((current) => ({ ...current, [`${selectedNode.id}:${key}`]: rawValue }))

    let parsed: unknown
    try {
      parsed = JSON.parse(rawValue)
    } catch {
      setFieldError(key, "Enter valid JSON.")
      return
    }

    if (updateConfigValue(key, parsed)) {
      setDraftJson((current) => ({ ...current, [`${selectedNode.id}:${key}`]: JSON.stringify(parsed, null, 2) }))
    }
  }

  function updateCondition(index: number, patch: Partial<BranchCondition>) {
    if (selectedNode.type !== "branch") return
    const conditions = [...selectedNode.config.conditions]
    conditions[index] = {
      ...conditions[index],
      ...patch,
    }
    updateConfigValue("conditions", conditions)
  }

  function addCondition() {
    if (selectedNode.type !== "branch") return
    const nextIndex = selectedNode.config.conditions.length + 1
    updateConfigValue("conditions", [
      ...selectedNode.config.conditions,
      {
        id: `condition-${nextIndex}`,
        label: `Condition ${nextIndex}`,
        left: "lastOutput",
        operator: "contains",
        right: "",
      },
    ])
  }

  function removeCondition(index: number) {
    if (selectedNode.type !== "branch") return
    updateConfigValue(
      "conditions",
      selectedNode.config.conditions.filter((_, conditionIndex) => conditionIndex !== index),
    )
  }

  return (
    <section className="rounded-md border border-border bg-card/70">
      <div className="border-b border-border p-4">
        <div className="flex items-center gap-3">
          <span className="flex size-9 shrink-0 items-center justify-center rounded-md border border-border bg-background">
            <Icon className="size-4" aria-hidden="true" />
          </span>
          <div className="min-w-0">
            <h2 className="truncate text-sm font-semibold">{meta.label}</h2>
            <p className="truncate text-xs text-muted-foreground">{selectedNode.id}</p>
          </div>
        </div>
      </div>
      <div className="grid gap-4 p-4">
        {fields.map((field) => (
          <ConfigFieldControl
            key={field.key}
            field={field}
            node={selectedNode}
            error={errors[field.key]}
            draftJson={draftJson[`${selectedNode.id}:${field.key}`]}
            onTextChange={updateConfigValue}
            onJsonChange={updateJsonValue}
            onJsonArrayChange={updateJsonArray}
            onConditionChange={updateCondition}
            onConditionAdd={addCondition}
            onConditionRemove={removeCondition}
          />
        ))}
      </div>
    </section>
  )
}

function ConfigFieldControl({
  field,
  node,
  error,
  draftJson,
  onTextChange,
  onJsonChange,
  onJsonArrayChange,
  onConditionChange,
  onConditionAdd,
  onConditionRemove,
}: {
  field: FieldDefinition
  node: WorkflowNode
  error?: string
  draftJson?: string
  onTextChange: (key: string, value: unknown) => void
  onJsonChange: (key: string, rawValue: string) => void
  onJsonArrayChange: (key: string, rawValue: string) => void
  onConditionChange: (index: number, patch: Partial<BranchCondition>) => void
  onConditionAdd: () => void
  onConditionRemove: (index: number) => void
}) {
  const value = (node.config as Record<string, unknown>)[field.key]
  const fieldId = `${node.id}-${field.key}`

  const errorMessage = error ? (
    <p className="text-xs text-destructive" role="alert">
      {error}
    </p>
  ) : null

  if (field.control === "conditions" && node.type === "branch") {
    return (
      <div className="grid gap-3">
        <div className="flex items-center justify-between gap-3">
          <span className="text-sm font-medium">{field.label}</span>
          <Button type="button" variant="outline" size="sm" onClick={onConditionAdd}>
            <Plus className="size-4" aria-hidden="true" />
            <span>Add</span>
          </Button>
        </div>
        <div className="grid gap-3">
          {node.config.conditions.map((condition, index) => (
            <BranchConditionEditor
              key={`${condition.id}-${index}`}
              condition={condition}
              index={index}
              onChange={onConditionChange}
              onRemove={onConditionRemove}
            />
          ))}
        </div>
        {errorMessage}
      </div>
    )
  }

  if (field.control === "boolean") {
    return (
      <label className="flex items-center justify-between gap-3 rounded-md border border-border bg-background/40 px-3 py-2">
        <span className="text-sm font-medium">{field.label}</span>
        <input
          id={fieldId}
          type="checkbox"
          checked={Boolean(value)}
          onChange={(event) => onTextChange(field.key, event.target.checked)}
          className="size-4"
        />
        {errorMessage}
      </label>
    )
  }

  if (field.control === "select") {
    return (
      <label className="grid gap-2">
        <span className="text-sm font-medium">{field.label}</span>
        <select
          id={fieldId}
          value={String(value ?? field.options?.[0] ?? "")}
          onChange={(event) => onTextChange(field.key, event.target.value)}
          className="border-input h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          {field.options?.map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
        {errorMessage}
      </label>
    )
  }

  if (field.control === "number") {
    return (
      <label className="grid gap-2">
        <span className="text-sm font-medium">{field.label}</span>
        <Input
          id={fieldId}
          type="number"
          min={field.min}
          max={field.max}
          step={field.step}
          value={typeof value === "number" ? value : ""}
          placeholder={field.placeholder}
          onChange={(event) => {
            const nextValue = event.target.valueAsNumber
            if (Number.isNaN(nextValue)) {
              onTextChange(field.key, undefined)
              return
            }
            onTextChange(field.key, nextValue)
          }}
        />
        {errorMessage}
      </label>
    )
  }

  if (field.control === "json-array") {
    const formattedValue = draftJson ?? JSON.stringify(value ?? [], null, 2)
    return (
      <label className="grid gap-2">
        <span className="text-sm font-medium">{field.label}</span>
        <Textarea
          id={fieldId}
          value={formattedValue}
          onChange={(event) => onJsonArrayChange(field.key, event.target.value)}
          className="min-h-28 font-mono text-xs"
        />
        {errorMessage}
      </label>
    )
  }

  if (field.control === "json") {
    const formattedValue = draftJson ?? JSON.stringify(value ?? {}, null, 2)
    return (
      <label className="grid gap-2">
        <span className="text-sm font-medium">{field.label}</span>
        <Textarea
          id={fieldId}
          value={formattedValue}
          onChange={(event) => onJsonChange(field.key, event.target.value)}
          className="min-h-28 font-mono text-xs"
        />
        {errorMessage}
      </label>
    )
  }

  if (field.control === "textarea") {
    return (
      <label className="grid gap-2">
        <span className="text-sm font-medium">{field.label}</span>
        <Textarea
          id={fieldId}
          value={typeof value === "string" ? value : ""}
          placeholder={field.placeholder}
          onChange={(event) => onTextChange(field.key, event.target.value)}
          className="min-h-24"
        />
        {errorMessage}
      </label>
    )
  }

  return (
    <label className="grid gap-2">
      <span className="text-sm font-medium">{field.label}</span>
      <Input
        id={fieldId}
        value={typeof value === "string" ? value : ""}
        placeholder={field.placeholder}
        onChange={(event) => onTextChange(field.key, event.target.value)}
      />
      {errorMessage}
    </label>
  )
}

function BranchConditionEditor({
  condition,
  index,
  onChange,
  onRemove,
}: {
  condition: BranchCondition
  index: number
  onChange: (index: number, patch: Partial<BranchCondition>) => void
  onRemove: (index: number) => void
}) {
  const operatorOptions = useMemo(() => [...branchOperatorValues], [])

  return (
    <div className="grid gap-3 rounded-md border border-border bg-background/40 p-3">
      <div className="flex items-center justify-between gap-3">
        <div className="font-mono text-[11px] uppercase text-muted-foreground">{condition.id}</div>
        <Button type="button" variant="outline" size="icon-sm" aria-label={`Remove ${condition.label}`} onClick={() => onRemove(index)}>
          <Trash2 className="size-4" aria-hidden="true" />
        </Button>
      </div>
      <label className="grid gap-2">
        <span className="text-xs font-medium">Condition ID</span>
        <Input value={condition.id} onChange={(event) => onChange(index, { id: event.target.value })} />
      </label>
      <label className="grid gap-2">
        <span className="text-xs font-medium">Condition label</span>
        <Input value={condition.label} onChange={(event) => onChange(index, { label: event.target.value })} />
      </label>
      <label className="grid gap-2">
        <span className="text-xs font-medium">Left value</span>
        <Input value={condition.left} onChange={(event) => onChange(index, { left: event.target.value })} />
      </label>
      <label className="grid gap-2">
        <span className="text-xs font-medium">Operator</span>
        <select
          value={condition.operator}
          onChange={(event) => onChange(index, { operator: event.target.value as BranchCondition["operator"] })}
          className="border-input h-9 w-full rounded-md border bg-transparent px-3 text-sm outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50"
        >
          {operatorOptions.map((operator) => (
            <option key={operator} value={operator}>
              {operator}
            </option>
          ))}
        </select>
      </label>
      <label className="grid gap-2">
        <span className="text-xs font-medium">Right value</span>
        <Input value={stringifyConditionRight(condition.right)} onChange={(event) => onChange(index, { right: event.target.value })} />
      </label>
      <label className="grid gap-2">
        <span className="text-xs font-medium">Target node ID</span>
        <Input value={condition.targetNodeId ?? ""} onChange={(event) => onChange(index, { targetNodeId: event.target.value || undefined })} />
      </label>
    </div>
  )
}

function stringifyConditionRight(value: unknown) {
  if (value === undefined || value === null) return ""
  if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") return String(value)
  return JSON.stringify(value)
}

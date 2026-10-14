"use client";
import type { ComponentDefinition, ComponentEntry } from "@uicast/core";
import { Evaluator } from "@uicast/expr";
import { DocumentSkeleton, EntriesRenderer, RendererProvider } from "@uicast/react";
import { ConfirmModal, RenderError } from "@uicast/shadcn-catalog";
import { impls } from "@uicast/shadcn-catalog/all/impls";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@uicast/shadcn-catalog/ui/collapsible";
import { Input } from "@uicast/shadcn-catalog/ui/input";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@uicast/shadcn-catalog/ui/select";
import { Switch } from "@uicast/shadcn-catalog/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@uicast/shadcn-catalog/ui/tabs";
import { Textarea } from "@uicast/shadcn-catalog/ui/textarea";
import { ChevronRightIcon } from "lucide-react";
import { Fragment, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { ESSENTIAL, EXAMPLES, GROUPS, type Group } from "@/lib/catalog-gallery";
import { callbacksOf, type Field, fieldsOf, type Props, readProps, writeProps } from "@/lib/catalog-gallery/props";

const evaluator = new Evaluator();
const fallbackComponents = { confirm: ConfirmModal, error: RenderError };
const TOTAL = Object.values(GROUPS).reduce((sum, defs) => sum + defs.length, 0);

// The all/essential choice, shared by the switch and every group on the page.
let essentialOnly = false;
const listeners = new Set<() => void>();
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};
const useEssentialOnly = () =>
  useSyncExternalStore(
    subscribe,
    () => essentialOnly,
    () => false,
  );
const setEssentialOnly = (value: boolean) => {
  essentialOnly = value;
  for (const listener of listeners) listener();
};

export function GallerySwitch() {
  const only = useEssentialOnly();
  return (
    <Tabs
      value={only ? "essential" : "all"}
      onValueChange={(value) => setEssentialOnly(value === "essential")}
      className="mt-6"
    >
      <TabsList>
        <TabsTrigger value="all">All ({TOTAL})</TabsTrigger>
        <TabsTrigger value="essential">Essential ({ESSENTIAL.size})</TabsTrigger>
      </TabsList>
    </Tabs>
  );
}

const ALL_DEFS = Object.values(GROUPS).flat();

// A part, such as `TableCell`, is a tab in the component whose example shows it.
export function GalleryGroup({ group }: { group: Group }) {
  const only = useEssentialOnly();
  return GROUPS[group]
    .filter((def) => typeof EXAMPLES[def.name] !== "string")
    .map((def) => ({ def, parts: ALL_DEFS.filter((part) => EXAMPLES[part.name] === def.name) }))
    .filter(({ def, parts }) => !only || [def, ...parts].some((d) => ESSENTIAL.has(d.name)))
    .toSorted((a, b) => a.def.name.localeCompare(b.def.name))
    .map(({ def, parts }) => <GalleryItem key={def.name} def={def} parts={parts} />);
}

// Descriptions mark code with backticks, as the prompt prints them.
const withCode = (text: string) => text.split("`").map((part, i) => (i % 2 ? <code key={i}>{part}</code> : part));

function GalleryItem({ def, parts }: { def: ComponentDefinition; parts: ComponentDefinition[] }) {
  const example = EXAMPLES[def.name] as ComponentEntry[];
  // Edited props by entry key; an untouched entry renders as written.
  const [edits, setEdits] = useState<Record<string, Props>>({});
  const [selected, setSelected] = useState(def);
  const entries = useMemo(
    () => example.map((entry) => (edits[entry.key] ? { ...entry, props: writeProps(edits[entry.key]) } : entry)),
    [example, edits],
  );
  // The selected component's line: its first entry in the example.
  const index = example.findIndex((entry) => entry.component === selected.name);
  const original = useMemo(() => readProps(example[index]), [example, index]);
  const entry = entries[index];
  return (
    <section id={def.name} className="mt-10 scroll-mt-24">
      {parts.map((part) => (
        <span key={part.name} id={part.name} className="block scroll-mt-24" />
      ))}
      <h3 className="text-xl font-semibold tracking-tight">
        <a href={`#${def.name}`}>{def.name}</a>
      </h3>
      <p className="mt-1 text-sm text-muted-foreground">{withCode(def.description)}</p>
      <div className="mt-3 overflow-hidden rounded-lg border">
        <ExampleBox entries={entries} />
        <Collapsible>
          <CollapsibleTrigger className="group flex w-full items-center gap-1.5 border-t px-4 py-2.5 text-sm text-muted-foreground hover:text-foreground">
            <ChevronRightIcon className="size-4 transition-transform group-data-[state=open]:rotate-90" />
            Props and source
          </CollapsibleTrigger>
          <CollapsibleContent>
            <div className="border-t p-4">
              {parts.length > 0 && (
                <Tabs
                  value={selected.name}
                  onValueChange={(name) => setSelected([def, ...parts].find((d) => d.name === name) ?? def)}
                  className="mb-4 overflow-x-auto"
                >
                  <TabsList>
                    {[def, ...parts].map((d) => (
                      <TabsTrigger key={d.name} value={d.name}>
                        {d.name}
                      </TabsTrigger>
                    ))}
                  </TabsList>
                </Tabs>
              )}
              {selected !== def && (
                <p className="mb-4 text-sm text-muted-foreground">{withCode(selected.description)}</p>
              )}
              <PropsTable
                key={selected.name}
                def={selected}
                props={edits[entry.key] ?? original}
                onChange={(props) => setEdits((prev) => ({ ...prev, [entry.key]: props }))}
              />
            </div>
            <pre className="border-t bg-muted/40 px-4 py-3 text-xs leading-relaxed break-all whitespace-pre-wrap">
              {JSON.stringify(entry)}
            </pre>
          </CollapsibleContent>
        </Collapsible>
      </div>
    </section>
  );
}

function PropsTable({
  def,
  props,
  onChange,
}: {
  def: ComponentDefinition;
  props: Props;
  onChange: (props: Props) => void;
}) {
  const fields = useMemo(() => fieldsOf(def), [def]);
  const callbacks = useMemo(() => callbacksOf(def), [def]);
  if (!fields.length && !callbacks.length) return <p className="text-sm text-muted-foreground">No props.</p>;
  const set = (name: string, value: unknown) => {
    if ("opaque" in props) return;
    const { [name]: _, ...values } = props.values;
    onChange({ ...props, values: value === undefined ? values : { ...values, [name]: value } });
  };
  return (
    <div className="grid gap-x-6 gap-y-4 text-sm sm:grid-cols-[minmax(0,14rem)_minmax(0,1fr)]">
      {fields.map((field) => (
        <Fragment key={field.name}>
          <div className="min-w-0">
            <code className="text-xs font-semibold">{field.name}</code>
            {field.required && <span className="ml-1.5 text-xs text-muted-foreground">required</span>}
            {field.description && <p className="mt-0.5 text-xs text-muted-foreground">{withCode(field.description)}</p>}
          </div>
          <div className="min-w-0">
            {"opaque" in props ? (
              <span className="text-xs text-muted-foreground">Set by the expression.</span>
            ) : field.name in props.bound ? (
              <code className="text-xs">{props.bound[field.name]}</code>
            ) : (
              <FieldControl
                field={field}
                value={props.values[field.name]}
                onChange={(value) => set(field.name, value)}
              />
            )}
          </div>
        </Fragment>
      ))}
      {callbacks.map((callback) => (
        <div key={callback.name} className="min-w-0 sm:col-span-2">
          <code className="text-xs font-semibold">{callback.name}</code>
          <span className="ml-1.5 text-xs text-muted-foreground">callback</span>
          {callback.description && (
            <p className="mt-0.5 text-xs text-muted-foreground">{withCode(callback.description)}</p>
          )}
        </div>
      ))}
    </div>
  );
}

// Radix Select takes no empty value.
const UNSET = "(unset)";
const CUSTOM = "(custom)";

// The named values, and with `custom` a JSON box for a value of another shape.
function EnumControl({
  field,
  options,
  custom,
  value,
  onChange,
}: {
  field: Field;
  options: string[];
  custom: boolean;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  const [json, setJson] = useState(value !== undefined && typeof value !== "string");
  const current = value ?? field.default;
  return (
    <div className="space-y-2">
      <Select
        value={json ? CUSTOM : typeof current === "string" ? current : UNSET}
        onValueChange={(next) => {
          setJson(next === CUSTOM);
          if (next !== CUSTOM) onChange(next === UNSET ? undefined : next);
        }}
      >
        <SelectTrigger size="sm" className="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {field.default === undefined && !field.required && <SelectItem value={UNSET}>{UNSET}</SelectItem>}
          {options.map((option) => (
            <SelectItem key={option} value={option}>
              {option}
            </SelectItem>
          ))}
          {custom && <SelectItem value={CUSTOM}>{CUSTOM}</SelectItem>}
        </SelectContent>
      </Select>
      {json && <JsonControl value={typeof value === "string" ? undefined : value} onChange={onChange} />}
    </div>
  );
}

// Inputs keep their own text, so a half-typed number or JSON value stays as typed.
function FieldControl({
  field,
  value,
  onChange,
}: {
  field: Field;
  value: unknown;
  onChange: (value: unknown) => void;
}) {
  const current = value ?? field.default;
  const placeholder = field.default === undefined ? undefined : JSON.stringify(field.default);
  switch (field.control.kind) {
    case "enum":
    case "enum-or-json":
      return (
        <EnumControl
          field={field}
          options={field.control.options}
          custom={field.control.kind === "enum-or-json"}
          value={value}
          onChange={onChange}
        />
      );
    case "boolean":
      return <Switch checked={current === true} onCheckedChange={onChange} />;
    case "number":
      return (
        <Input
          type="number"
          className="h-8"
          defaultValue={typeof value === "number" ? value : undefined}
          placeholder={placeholder}
          min={field.control.min}
          max={field.control.max}
          onChange={(event) => {
            const number = event.target.valueAsNumber;
            onChange(Number.isNaN(number) ? undefined : number);
          }}
        />
      );
    case "string": {
      const text = value === undefined ? undefined : String(value);
      const update = (next: string) => onChange(next === "" && !field.required ? undefined : next);
      return text && text.length > 60 ? (
        <Textarea defaultValue={text} rows={3} onChange={(event) => update(event.target.value)} />
      ) : (
        <Input
          className="h-8"
          defaultValue={text}
          placeholder={placeholder}
          onChange={(event) => update(event.target.value)}
        />
      );
    }
    default:
      return <JsonControl value={value} placeholder={placeholder} onChange={onChange} />;
  }
}

function JsonControl({
  value,
  placeholder,
  onChange,
}: {
  value: unknown;
  placeholder?: string;
  onChange: (value: unknown) => void;
}) {
  const [invalid, setInvalid] = useState(false);
  const text = value === undefined ? "" : JSON.stringify(value);
  return (
    <Textarea
      defaultValue={text}
      placeholder={placeholder ?? "JSON"}
      rows={Math.min(6, Math.max(1, Math.ceil(text.length / 60)))}
      aria-invalid={invalid}
      className="font-mono text-xs"
      onChange={(event) => {
        if (event.target.value.trim() === "") {
          setInvalid(false);
          onChange(undefined);
          return;
        }
        try {
          onChange(JSON.parse(event.target.value));
          setInvalid(false);
        } catch {
          setInvalid(true);
        }
      }}
    />
  );
}

// Each example gets its own provider, so its scopes are its own. It mounts near the viewport; until then, its skeleton.
function ExampleBox({ entries }: { entries: ComponentEntry[] }) {
  const ref = useRef<HTMLDivElement>(null);
  const [near, setNear] = useState(false);
  useEffect(() => {
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry.isIntersecting) return;
        setNear(true);
        observer.disconnect();
      },
      { rootMargin: "400px" },
    );
    if (ref.current) observer.observe(ref.current);
    return () => observer.disconnect();
  }, []);
  return (
    <div ref={ref} className="overflow-x-auto p-4">
      <RendererProvider implementations={impls} evaluator={evaluator} fallbackComponents={fallbackComponents}>
        {near ? <EntriesRenderer entries={entries} /> : <DocumentSkeleton entries={entries} />}
      </RendererProvider>
    </div>
  );
}

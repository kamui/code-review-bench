"use client";

import * as React from "react";
import { Select as SelectPrimitive } from "@base-ui/react/select";
import { Radio as RadioPrimitive } from "@base-ui/react/radio";
import { RadioGroup as RadioGroupPrimitive } from "@base-ui/react/radio-group";
import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { Switch as SwitchPrimitive } from "@base-ui/react/switch";
import { Progress as ProgressPrimitive } from "@base-ui/react/progress";
import { Check, ChevronDown, Minus, Plus } from "lucide-react";

import { cn, clamp } from "@/lib/utils";
import { IconButton } from "@/components/ui/button";

/* --------------------------------------------------------------- containers */

export function Field({
  label,
  hint,
  children,
  className,
  htmlFor,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
  className?: string;
  htmlFor?: string;
}) {
  const generatedId = React.useId();
  const id = htmlFor ?? generatedId;

  const control =
    React.isValidElement(children) && !(children.props as { id?: string }).id
      ? React.cloneElement(children as React.ReactElement<{ id?: string }>, {
          id,
        })
      : children;

  return (
    <div className={cn("space-y-1.5", className)}>
      <div className="flex items-center justify-between gap-2">
        <label
          htmlFor={id}
          className="text-[11px] font-medium tracking-wide text-[var(--text-secondary)]"
        >
          {label}
        </label>
        {hint ? (
          <span className="text-xs text-[var(--text-tertiary)]">{hint}</span>
        ) : null}
      </div>
      {control}
    </div>
  );
}

export function SectionLabel({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return <h3 className={cn("section-label", className)}>{children}</h3>;
}

/* ------------------------------------------------------------------ inputs */

export const TextInput = React.forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement>
>(function TextInput({ className, ...props }, ref) {
  return (
    <input
      ref={ref}
      data-slot="input"
      className={cn(
        "h-9 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--bg-raised)] px-3 text-sm text-[var(--text-primary)]",
        "transition-[border-color,background-color,box-shadow] duration-[var(--dur-fast)]",
        "placeholder:text-[var(--text-tertiary)]",
        "hover:border-[var(--border-strong)]",
        "focus-visible:border-[var(--ring)] focus-visible:bg-[var(--bg-panel)] focus-visible:ring-2 focus-visible:ring-[var(--ring)]/25",
        "disabled:cursor-not-allowed disabled:opacity-50",
        "aria-invalid:border-[var(--danger)] aria-invalid:ring-2 aria-invalid:ring-[var(--danger)]/20",
        className,
      )}
      {...props}
    />
  );
});

export const TextArea = React.forwardRef<
  HTMLTextAreaElement,
  React.TextareaHTMLAttributes<HTMLTextAreaElement>
>(function TextArea({ className, ...props }, ref) {
  return (
    <textarea
      ref={ref}
      data-slot="textarea"
      className={cn(
        "w-full resize-none rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-raised)] px-3 py-2 text-sm text-[var(--text-primary)]",
        "transition-[border-color,background-color,box-shadow] duration-[var(--dur-fast)]",
        "placeholder:text-[var(--text-tertiary)]",
        "hover:border-[var(--border-strong)]",
        "focus-visible:border-[var(--ring)] focus-visible:bg-[var(--bg-panel)] focus-visible:ring-2 focus-visible:ring-[var(--ring)]/25",
        className,
      )}
      {...props}
    />
  );
});

/* ------------------------------------------------------------------ select */

interface SelectProps extends Omit<
  React.SelectHTMLAttributes<HTMLSelectElement>,
  "children" | "onChange"
> {
  children: React.ReactNode;
  onChange: (event: { target: { value: string } }) => void;
  "aria-label"?: string;
}

interface ParsedOption {
  value: string;
  label: string;
  disabled: boolean;
  group?: string;
}

function textOf(node: React.ReactNode): string {
  if (node == null || node === false || node === true) return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (React.isValidElement(node)) {
    return textOf((node.props as { children?: React.ReactNode }).children);
  }
  return "";
}

function parseOptions(children: React.ReactNode, group?: string): ParsedOption[] {
  const options: ParsedOption[] = [];
  React.Children.forEach(children, (child) => {
    if (!React.isValidElement(child)) return;
    const props = child.props as {
      value?: string | number;
      children?: React.ReactNode;
      disabled?: boolean;
      label?: string;
    };
    if (child.type === "option") {
      const label = textOf(props.children).trim();
      options.push({
        value: String(props.value ?? ""),
        label: label || String(props.value ?? ""),
        disabled: Boolean(props.disabled),
        group,
      });
      return;
    }
    if (child.type === "optgroup") {
      options.push(...parseOptions(props.children, props.label ?? group));
      return;
    }
    if (props.children) options.push(...parseOptions(props.children, group));
  });
  return options;
}

function groupOptions(
  options: ParsedOption[],
): { group?: string; options: ParsedOption[] }[] {
  const sections: { group?: string; options: ParsedOption[] }[] = [];
  for (const option of options) {
    const last = sections.at(-1);
    if (last && last.group === option.group) last.options.push(option);
    else sections.push({ group: option.group, options: [option] });
  }
  return sections;
}

function optionsKey(options: ParsedOption[]): string {
  return JSON.stringify(options);
}

function useParsedOptions(children: React.ReactNode): ParsedOption[] {
  const parsed = parseOptions(children);
  const key = optionsKey(parsed);
  const cache = React.useRef<{ key: string; options: ParsedOption[] }>({
    key,
    options: parsed,
  });
  if (cache.current.key !== key) cache.current = { key, options: parsed };
  return cache.current.options;
}

export const Select = React.forwardRef<HTMLButtonElement, SelectProps>(
  function Select(
    {
      className,
      children,
      value,
      onChange,
      disabled,
      id,
      "aria-label": ariaLabel,
      ...props
    },
    ref,
  ) {
    const options = useParsedOptions(children);
    const current = options.find(
      (option) => option.value === String(value ?? ""),
    );
    const items = React.useMemo(
      () =>
        options.map((option) => ({ value: option.value, label: option.label })),
      [options],
    );

    return (
      <SelectPrimitive.Root
        value={String(value ?? "")}
        onValueChange={(next) => onChange({ target: { value: String(next) } })}
        disabled={disabled}
        items={items}
      >
        <SelectPrimitive.Trigger
          ref={ref}
          id={id}
          aria-label={ariaLabel}
          data-slot="select-trigger"
          className={cn(
            "flex h-9 w-full items-center justify-between gap-2 rounded-md border border-[var(--border-subtle)] bg-[var(--bg-raised)] px-3 text-sm text-[var(--text-primary)]",
            "transition-[border-color,background-color,box-shadow] duration-[var(--dur-fast)]",
            "hover:border-[var(--border-strong)]",
            "focus-visible:border-[var(--ring)] focus-visible:ring-2 focus-visible:ring-[var(--ring)]/25 focus-visible:outline-none",
            "data-popup-open:border-[var(--ring)] data-popup-open:bg-[var(--bg-panel)]",
            "disabled:cursor-not-allowed disabled:opacity-50",
            className,
          )}
          {...(props as Record<string, unknown>)}
        >
          <SelectPrimitive.Value className="truncate">
            {current?.label ?? "Select…"}
          </SelectPrimitive.Value>
          <SelectPrimitive.Icon className="shrink-0 text-[var(--text-tertiary)]">
            <ChevronDown className="size-4" aria-hidden />
          </SelectPrimitive.Icon>
        </SelectPrimitive.Trigger>

        <SelectPrimitive.Portal>
          <SelectPrimitive.Positioner
            sideOffset={6}
            alignItemWithTrigger={false}
            className="isolate z-50"
          >
            <SelectPrimitive.Popup
              data-slot="select-content"
              className={cn(
                "max-h-[min(20rem,var(--available-height))] min-w-[var(--anchor-width)] overflow-y-auto",
                "rounded-xl border border-[var(--border-subtle)] bg-[var(--bg-panel)] p-1",
                "shadow-[var(--shadow-lg)] ",
                "data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95",
                "data-closed:animate-out data-closed:fade-out-0 data-closed:zoom-out-95",
              )}
            >
              {groupOptions(options).map((section) => (
                <SelectPrimitive.Group key={section.group ?? "ungrouped"}>
                  {section.group ? (
                    <SelectPrimitive.GroupLabel className="section-label px-2.5 pt-2 pb-1">
                      {section.group}
                    </SelectPrimitive.GroupLabel>
                  ) : null}
                  {section.options.map((option) => (
                <SelectPrimitive.Item
                  key={option.value}
                  value={option.value}
                  disabled={option.disabled}
                  className={cn(
                    "flex cursor-default items-center gap-2 rounded-lg px-2.5 py-1.5 text-sm text-[var(--text-primary)] outline-none select-none",
                    "data-highlighted:bg-[var(--bg-raised)]",
                    "data-disabled:pointer-events-none data-disabled:opacity-50",
                  )}
                >
                  <SelectPrimitive.ItemIndicator className="text-[var(--accent)]">
                    <Check className="size-4" aria-hidden />
                  </SelectPrimitive.ItemIndicator>
                  <SelectPrimitive.ItemText className="flex-1">
                    {option.label}
                  </SelectPrimitive.ItemText>
                </SelectPrimitive.Item>
                  ))}
                </SelectPrimitive.Group>
              ))}
            </SelectPrimitive.Popup>
          </SelectPrimitive.Positioner>
        </SelectPrimitive.Portal>
      </SelectPrimitive.Root>
    );
  },
);

/* ------------------------------------------------------------------ slider */

export interface SliderProps {
  label: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  suffix?: string;
  onChange: (value: number) => void;
  onCommit?: () => void;
  onStart?: () => void;
  format?: (value: number) => string;
  className?: string;
  disabled?: boolean;
}

export function Slider({
  label,
  value,
  min,
  max,
  step = 1,
  suffix = "",
  onChange,
  onCommit,
  onStart,
  format,
  className,
  disabled,
}: SliderProps) {
  const id = React.useId();
  const display = format
    ? format(value)
    : `${Math.round(value * 100) / 100}${suffix}`;

  return (
    <div className={cn("space-y-1", className)}>
      <div className="flex items-center justify-between text-xs">
        <label
          htmlFor={id}
          className="font-medium text-[var(--text-secondary)]"
        >
          {label}
        </label>
        <output htmlFor={id} className="tnum text-[var(--text-tertiary)]">
          {display}
        </output>
      </div>
      <SliderPrimitive.Root
        id={id}
        aria-label={label}
        value={value}
        min={min}
        max={max}
        step={step}
        disabled={disabled}
        thumbAlignment="edge"
        onValueChange={(next) =>
          onChange(Array.isArray(next) ? next[0] : (next as number))
        }
        onValueCommitted={() => onCommit?.()}
        className="data-horizontal:w-full"
      >
        <SliderPrimitive.Control
          onPointerDown={onStart}
          className="relative flex h-5 w-full touch-none items-center select-none data-disabled:opacity-50"
        >
          <SliderPrimitive.Track className="relative h-1.5 w-full grow overflow-hidden rounded-full bg-[var(--bg-hover)] select-none">
            <SliderPrimitive.Indicator className="h-full bg-[var(--accent)] select-none" />
          </SliderPrimitive.Track>
          <SliderPrimitive.Thumb
            getAriaLabel={() => label}
            getAriaValueText={(formatted) =>
              format ? formatted : `${formatted}${suffix}`
            }
            className={cn(
              "block size-4 shrink-0 rounded-full border-2 border-[var(--accent)] bg-[var(--bg-panel)]",
              "shadow-[var(--shadow-sm)] transition-transform duration-[var(--dur-fast)]",
              "hover:scale-110 focus-visible:ring-2 focus-visible:ring-[var(--ring)] focus-visible:outline-none",
              "active:scale-110",
            )}
          />
        </SliderPrimitive.Control>
      </SliderPrimitive.Root>
    </div>
  );
}

export function NumberInput({
  value,
  onChange,
  min = -9999,
  max = 9999,
  step = 1,
  suffix,
  label,
  onStart,
  onCommit,
  steppers = false,
  scrub = true,
}: {
  value: number;
  onChange: (value: number) => void;
  min?: number;
  max?: number;
  step?: number;
  suffix?: string;
  label: string;
  onStart?: () => void;
  onCommit?: () => void;
  steppers?: boolean;
  scrub?: boolean;
}) {
  const id = React.useId();
  const scrubbing = React.useRef<{ x: number; from: number } | null>(null);
  const [active, setActive] = React.useState(false);

  const scrubHandlers = scrub
    ? {
        onPointerDown: (event: React.PointerEvent) => {
          if (event.button !== 0) return;
          scrubbing.current = { x: event.clientX, from: value };
          setActive(true);
          onStart?.();
          event.currentTarget.setPointerCapture(event.pointerId);
        },
        onPointerMove: (event: React.PointerEvent) => {
          const began = scrubbing.current;
          if (!began) return;
          event.preventDefault();
          const multiplier = event.shiftKey ? 10 : event.altKey ? 0.1 : 1;
          const delta = ((event.clientX - began.x) / 2) * step * multiplier;
          onChange(
            clamp(roundToStep(began.from + delta, step * multiplier), min, max),
          );
        },
        onPointerUp: () => {
          if (!scrubbing.current) return;
          scrubbing.current = null;
          setActive(false);
          onCommit?.();
        },
        onPointerCancel: () => {
          scrubbing.current = null;
          setActive(false);
        },
      }
    : {};

  const labelClasses = cn(
    "inline-block text-xs font-medium text-[var(--text-secondary)]",
    scrub &&
      "cursor-ew-resize touch-none select-none hover:text-[var(--text-primary)]",
    active && "text-[var(--accent)]",
  );

  const input = (
    <div className="relative flex-1">
      <input
        id={id}
        type="number"
        inputMode="decimal"
        aria-label={label}
        value={Number.isFinite(value) ? Math.round(value * 100) / 100 : 0}
        min={min}
        max={max}
        step={step}
        onFocus={onStart}
        onChange={(event) =>
          onChange(clamp(Number(event.target.value), min, max))
        }
        onBlur={onCommit}
        className={cn(
          "tnum h-8 w-full rounded-md border border-[var(--border-subtle)] bg-[var(--bg-raised)] px-2 text-sm text-[var(--text-primary)]",
          "transition-colors duration-[var(--dur-fast)] hover:border-[var(--border-strong)]",
          "focus-visible:border-[var(--ring)] focus-visible:ring-2 focus-visible:ring-[var(--ring)]/25 focus-visible:outline-none",
          suffix ? "pr-7" : undefined,
        )}
      />
      {suffix ? (
        <span className="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2 text-xs text-[var(--text-tertiary)]">
          {suffix}
        </span>
      ) : null}
    </div>
  );

  if (!steppers) {
    return (
      <div className="space-y-1">
        <label htmlFor={id} className={labelClasses} {...scrubHandlers}>
          {label}
        </label>
        {input}
      </div>
    );
  }

  return (
    <div className="space-y-1">
      <span className={labelClasses} {...scrubHandlers}>
        {label}
      </span>
      <div className="flex items-center gap-1">
        <IconButton
          label={`Decrease ${label}`}
          size="sm"
          variant="secondary"
          onClick={() => onChange(clamp(value - step, min, max))}
        >
          <Minus aria-hidden />
        </IconButton>
        {input}
        <IconButton
          label={`Increase ${label}`}
          size="sm"
          variant="secondary"
          onClick={() => onChange(clamp(value + step, min, max))}
        >
          <Plus aria-hidden />
        </IconButton>
      </div>
    </div>
  );
}

function roundToStep(value: number, step: number): number {
  if (!Number.isFinite(step) || step <= 0) return value;
  const snapped = Math.round(value / step) * step;
  const decimals = (String(step).split(".")[1] ?? "").length;
  return Number(snapped.toFixed(decimals));
}

export { NumberInput as NumberScrubber };

/* ---------------------------------------------------------------- switches */

export function Switch({
  checked,
  onChange,
  label,
  description,
  id,
  disabled,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description?: string;
  id?: string;
  disabled?: boolean;
}) {
  const generated = React.useId();
  const inputId = id ?? generated;

  return (
    <div className="flex items-center justify-between gap-3">
      <label htmlFor={inputId} className="flex-1 cursor-pointer select-none">
        <span className="block text-sm text-[var(--text-primary)]">
          {label}
        </span>
        {description ? (
          <span className="block text-xs text-[var(--text-tertiary)]">
            {description}
          </span>
        ) : null}
      </label>
      <SwitchPrimitive.Root
        id={inputId}
        checked={checked}
        onCheckedChange={(next) => onChange(next)}
        disabled={disabled}
        className={cn(
          "relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border border-transparent",
          "transition-colors duration-[var(--dur-fast)] ease-[var(--ease-out)]",
          "focus-visible:ring-2 focus-visible:ring-[var(--ring)] focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--bg-app)] focus-visible:outline-none",
          "data-checked:bg-[var(--accent)] data-unchecked:bg-[var(--bg-hover)] data-unchecked:border-[var(--border-subtle)]",
          "data-disabled:cursor-not-allowed data-disabled:opacity-50",
        )}
      >
        <SwitchPrimitive.Thumb
          className={cn(
            "pointer-events-none block size-5 rounded-full bg-white shadow-sm",
            "transition-transform duration-[var(--dur-fast)] ease-[var(--ease-out)]",
            "data-checked:translate-x-[22px] data-unchecked:translate-x-0.5",
          )}
        />
      </SwitchPrimitive.Root>
    </div>
  );
}

/* --------------------------------------------------------------- segmented */

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
  size = "md",
}: {
  value: T;
  options: {
    value: T;
    label: string;
    icon?: React.ReactNode;
    title?: string;
  }[];
  onChange: (value: T) => void;
  label: string;
  size?: "sm" | "md";
}) {
  return (
    <RadioGroupPrimitive
      aria-label={label}
      value={value}
      onValueChange={(next) => onChange(next as T)}
      className="flex rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-raised)] p-0.5"
    >
      {options.map((option) => {
        const active = option.value === value;
        return (
          <RadioPrimitive.Root
            key={option.value}
            value={option.value}
            aria-label={option.title ?? option.label}
            className={cn(
              "flex flex-1 cursor-pointer items-center justify-center gap-1.5 rounded-md font-medium",
              "transition-[background-color,color,box-shadow] duration-[var(--dur-fast)]",
              "outline-none focus-visible:ring-2 focus-visible:ring-[var(--ring)]",
              size === "sm" ? "h-7 px-2 text-xs" : "h-8 px-2.5 text-xs",
              active
                ? "bg-[var(--bg-panel)] text-[var(--text-primary)] shadow-[var(--shadow-sm)]"
                : "text-[var(--text-secondary)] hover:text-[var(--text-primary)]",
            )}
          >
            {option.icon}
            <span className={option.icon ? "hidden sm:inline" : undefined}>
              {option.label}
            </span>
          </RadioPrimitive.Root>
        );
      })}
    </RadioGroupPrimitive>
  );
}

/* ---------------------------------------------------------------- progress */

export function ProgressBar({
  value,
  label,
  indeterminate,
}: {
  value: number;
  label?: string;
  indeterminate?: boolean;
}) {
  const percent = clamp(Math.round(value * 100), 0, 100);

  return (
    <div className="space-y-1">
      <ProgressPrimitive.Root
        value={indeterminate ? null : percent}
        aria-label={label ?? "Progress"}
        className="h-2 w-full overflow-hidden rounded-full bg-[var(--bg-hover)]"
      >
        <ProgressPrimitive.Track className="h-full w-full">
          <ProgressPrimitive.Indicator
            className={cn(
              "h-full rounded-full bg-[var(--accent)] transition-[width] duration-200 ease-[var(--ease-out)]",
              indeterminate && "skeleton w-1/3",
            )}
          />
        </ProgressPrimitive.Track>
      </ProgressPrimitive.Root>
      {label ? (
        <p className="text-xs text-[var(--text-tertiary)]">
          {label}
          {indeterminate ? "…" : ` · ${percent}%`}
        </p>
      ) : null}
    </div>
  );
}

/* ------------------------------------------------------------- empty state */

export function EmptyState({
  icon,
  art,
  title,
  message,
  action,
}: {
  icon?: React.ReactNode;
  art?: React.ReactNode;
  title: string;
  message: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex flex-col items-center justify-center gap-2.5 rounded-[var(--r-md)] border border-dashed border-[var(--border-subtle)] px-6 py-12 text-center">
      {art ? <div className="mb-1">{art}</div> : null}
      {!art && icon ? (
        <div className="grid size-11 place-items-center rounded-full bg-[var(--bg-raised)] text-[var(--text-tertiary)]">
          {icon}
        </div>
      ) : null}
      <p className="text-lg font-medium text-[var(--text-primary)]">{title}</p>
      <p className="max-w-sm text-base leading-relaxed text-[var(--text-secondary)]">
        {message}
      </p>
      {action ? <div className="mt-1.5">{action}</div> : null}
    </div>
  );
}


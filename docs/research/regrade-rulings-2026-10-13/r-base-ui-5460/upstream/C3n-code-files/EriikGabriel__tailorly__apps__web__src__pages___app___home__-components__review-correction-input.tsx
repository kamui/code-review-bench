import { AutoHeight } from "@animate/primitives/effects/auto-height";
import { Select } from "@base-ui-components/react/select";
import {
  Check,
  ChevronDown,
  ChevronUp,
  List,
  ListOrdered,
  Mail,
  Text,
} from "@react-zero-ui/icon-sprite";
import { motion, useReducedMotion } from "motion/react";
import { type ReactNode, useState } from "react";
import type { CorrectionField } from "@/types/review";
import { ReviewMonthPicker } from "./review-month-picker";

const controlClass =
  "flex h-9 w-full min-w-0 items-center bg-transparent px-3 text-left text-sm font-medium text-primary-950 outline-none placeholder:text-on-surface-variant/70 focus-visible:outline-none";

type ReviewCorrectionInputProps = {
  id: string;
  field: CorrectionField;
  value: string;
  invalid: boolean;
  onChange: (value: string) => void;
};

function AnimatedField({
  children,
  icon,
  invalid,
}: {
  children: ReactNode;
  icon?: ReactNode;
  invalid: boolean;
}) {
  const reducedMotion = useReducedMotion();
  const [focused, setFocused] = useState(false);

  return (
    <div
      onFocusCapture={() => setFocused(true)}
      onBlurCapture={() => setFocused(false)}
      className={`relative mt-1.5 flex w-full max-w-64 items-center overflow-hidden rounded-lg border bg-card shadow-xs transition-[border-color,box-shadow] duration-200 motion-reduce:transition-none focus-within:ring-3 ${invalid ? "border-destructive/70 focus-within:ring-destructive/10" : "border-outline-variant/70 hover:border-primary-300 focus-within:border-primary-900 focus-within:ring-primary-900/10"}`}
    >
      {icon && (
        <span aria-hidden="true" className="pl-3 text-secondary-700">
          {icon}
        </span>
      )}
      {children}
      <motion.span
        aria-hidden="true"
        className={`pointer-events-none absolute inset-x-0 bottom-0 h-0.5 origin-center ${invalid ? "bg-destructive" : "bg-primary-900"}`}
        initial={false}
        animate={{ scaleX: focused ? 1 : 0 }}
        transition={{ duration: reducedMotion ? 0 : 0.2, ease: "easeOut" }}
      />
    </div>
  );
}

function ReviewNumberInput({
  id,
  field,
  value,
  invalid,
  onChange,
}: ReviewCorrectionInputProps) {
  const numericValue = Number(value);
  const hasValue = value.trim() !== "" && Number.isFinite(numericValue);
  const min = field.min === undefined ? undefined : Number(field.min);
  const max = field.max === undefined ? undefined : Number(field.max);

  function changeBy(direction: -1 | 1) {
    const initial = direction === 1 ? (min ?? 1) : (max ?? -1);
    const next = hasValue ? numericValue + direction : initial;
    onChange(
      String(Math.min(max ?? Infinity, Math.max(min ?? -Infinity, next))),
    );
  }

  return (
    <AnimatedField invalid={invalid} icon={<ListOrdered className="size-4" />}>
      <input
        id={id}
        type="number"
        min={field.min}
        max={field.max}
        step="any"
        placeholder={field.placeholder}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            event.currentTarget.blur();
          }
        }}
        aria-invalid={invalid}
        aria-describedby={invalid ? `${id}-error` : undefined}
        className={`${controlClass} [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none [&::-webkit-outer-spin-button]:appearance-none`}
      />
      <div className="mr-1 flex h-7 w-7 shrink-0 flex-col overflow-hidden rounded-md border border-outline-variant/60 bg-surface-container-low">
        <button
          type="button"
          aria-label={`Aumentar ${field.label}`}
          aria-controls={id}
          disabled={hasValue && max !== undefined && numericValue >= max}
          onClick={() => changeBy(1)}
          className="flex flex-1 items-center justify-center text-secondary-700 transition-colors hover:bg-primary-100 hover:text-primary-900 active:bg-primary-200 focus-visible:bg-primary-100 focus-visible:outline-2 focus-visible:outline-primary-900 disabled:cursor-not-allowed disabled:opacity-35 motion-reduce:transition-none"
        >
          <ChevronUp className="size-3" aria-hidden="true" />
        </button>
        <span className="h-px bg-outline-variant/60" aria-hidden="true" />
        <button
          type="button"
          aria-label={`Diminuir ${field.label}`}
          aria-controls={id}
          disabled={hasValue && min !== undefined && numericValue <= min}
          onClick={() => changeBy(-1)}
          className="flex flex-1 items-center justify-center text-secondary-700 transition-colors hover:bg-primary-100 hover:text-primary-900 active:bg-primary-200 focus-visible:bg-primary-100 focus-visible:outline-2 focus-visible:outline-primary-900 disabled:cursor-not-allowed disabled:opacity-35 motion-reduce:transition-none"
        >
          <ChevronDown className="size-3" aria-hidden="true" />
        </button>
      </div>
    </AnimatedField>
  );
}

function ReviewSelectInput({
  id,
  field,
  value,
  invalid,
  onChange,
}: ReviewCorrectionInputProps) {
  return (
    <AnimatedField invalid={invalid} icon={<List className="size-4" />}>
      <Select.Root
        value={value || null}
        onValueChange={(selected) => onChange(selected ?? "")}
      >
        <Select.Trigger
          id={id}
          aria-invalid={invalid}
          aria-describedby={invalid ? `${id}-error` : undefined}
          className={`${controlClass} group justify-between gap-2 data-popup-open:text-primary-900`}
        >
          <Select.Value className="truncate">
            {(selected: string | null) =>
              field.options?.find((option) => option.value === selected)
                ?.label ?? "Selecione uma opção"
            }
          </Select.Value>
          <Select.Icon className="shrink-0 text-secondary-700 transition-transform duration-200 group-data-popup-open:rotate-180 motion-reduce:transition-none">
            <ChevronDown className="size-4" aria-hidden="true" />
          </Select.Icon>
        </Select.Trigger>
        <Select.Portal>
          <Select.Positioner
            alignItemWithTrigger={false}
            sideOffset={6}
            className="z-70"
          >
            <Select.Popup className="w-(--anchor-width) min-w-64 origin-(--transform-origin) rounded-lg border border-outline-variant/60 bg-card p-1 text-primary-950 shadow-xl outline-none transition-[opacity,transform] duration-150 data-starting-style:translate-y-1 data-starting-style:opacity-0 data-ending-style:opacity-0 motion-reduce:transition-none">
              <Select.List className="max-h-[min(16rem,var(--available-height))] overflow-y-auto overscroll-contain">
                {field.options?.map((option) => (
                  <Select.Item
                    key={option.value}
                    value={option.value}
                    className="flex cursor-pointer items-center justify-between gap-3 rounded-md px-3 py-2 text-sm outline-none transition-colors data-highlighted:bg-surface-container data-selected:bg-primary-900 data-selected:text-primary-foreground data-selected:data-highlighted:bg-primary-800"
                  >
                    <Select.ItemText>{option.label}</Select.ItemText>
                    <Select.ItemIndicator>
                      <Check className="size-4" aria-hidden="true" />
                    </Select.ItemIndicator>
                  </Select.Item>
                ))}
              </Select.List>
            </Select.Popup>
          </Select.Positioner>
        </Select.Portal>
      </Select.Root>
    </AnimatedField>
  );
}

export function ReviewCorrectionInput(props: ReviewCorrectionInputProps) {
  const { id, field, value, invalid, onChange } = props;
  const describedBy = invalid ? `${id}-error` : undefined;
  const reducedMotion = useReducedMotion();

  return (
    <div className="px-1.5 pt-3 pb-1.5 text-xs font-medium">
      <label htmlFor={id} className="font-semibold text-primary-950">
        {field.label}
      </label>
      {field.type === "month" ? (
        <AnimatedField invalid={invalid}>
          <ReviewMonthPicker
            id={id}
            label={field.label}
            value={value}
            min={field.min}
            max={field.max}
            invalid={invalid}
            describedBy={describedBy}
            className={controlClass}
            onChange={onChange}
          />
        </AnimatedField>
      ) : field.type === "number" ? (
        <ReviewNumberInput {...props} />
      ) : field.type === "select" ? (
        <ReviewSelectInput {...props} />
      ) : (
        <AnimatedField
          invalid={invalid}
          icon={
            field.type === "email" ? (
              <Mail className="size-4" />
            ) : (
              <Text className="size-4" />
            )
          }
        >
          <input
            id={id}
            type={field.type}
            placeholder={field.placeholder}
            value={value}
            onChange={(event) => onChange(event.target.value)}
            aria-invalid={invalid}
            aria-describedby={describedBy}
            className={controlClass}
          />
        </AnimatedField>
      )}
      <AutoHeight
        deps={[invalid]}
        transition={{ duration: reducedMotion ? 0 : 0.2 }}
      >
        {invalid && (
          <span
            id={`${id}-error`}
            className="block pt-1.5 text-xs font-medium text-destructive"
          >
            Informe um valor válido e diferente do original.
          </span>
        )}
      </AutoHeight>
    </div>
  );
}

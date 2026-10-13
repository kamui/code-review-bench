'use client'

import { NumberField as NumberFieldPrimitive } from '@base-ui/react/number-field'
import { Minus, Plus } from 'lucide-react'
import * as React from 'react'

import { cn } from '../../lib/cn'
import { type FieldControlSize, fieldControlClasses, fieldControlSizes } from '../field/field-control'
import { Label } from '../field/label'

/**
 * A number you can type, step and trust: a token budget, a retry count, a
 * timeout in milliseconds.
 *
 * ── Base UI's NumberField, not `<input type="number">` ──
 * The native control was the first draft, and it fails three ways in a tool:
 * it formats nothing (12000 tokens reads as 12000, not 12,000), its spinner is
 * a pair of browser-styled arrows from no design system, and Chrome changes
 * the value when the page is scrolled over a focused field. NumberField
 * parses and formats with `Intl.NumberFormat` for the `locale` passed (so
 * `de-DE` types 1.500,5 and `ar-EG` shows Arabic-Indic digits), clamps to
 * `min`/`max`, and owns the keyboard: ArrowUp/Down by `step`, Shift by
 * `largeStep`, Alt by `smallStep`, Home/End to the bounds. PageUp/PageDown
 * are added here, by `largeStep`, because the APG spinbutton pattern lists
 * them and operators reach for them on long ranges.
 *
 * ── the wheel is off unless asked for ──
 * `allowWheelScrub` lets the wheel change a focused, hovered field. It is
 * opt-in: in a long settings form the wheel is how people scroll, and a
 * value that changed under the pointer on the way down is a silent edit.
 * Turn it on for a field that IS the focus of its screen.
 *
 * ── the steppers sit at the end, inside the cut ──
 * − and + share the field's keyline, split from the text by a hairline, so a
 * number field is one shape the height of every other control. They are not
 * tab stops (Base UI keeps them out of the order, as the APG does): the
 * keyboard already has the arrows, and two extra stops per number field
 * double the Tab presses through a form. At a bound the stepper that would
 * cross it is disabled — shaded, not faded, like every disabled control.
 *
 * ── a unit is part of the field's description ──
 * `unit` draws a short suffix inside the cut ("tokens", "ms") and adds it to
 * the input's accessible description, so "Timeout, 30" is heard as "30, ms".
 * For units Intl knows, prefer `format={{ style: 'unit', unit: 'millisecond' }}`
 * or a currency format: the unit is then in the value itself, localised.
 */

export interface NumberInputProps
  extends Omit<NumberFieldPrimitive.Root.Props, 'className' | 'children' | 'render' | 'onValueChange'> {
  /** Visible label above the field. Omit inside `Field`, which renders its own. */
  label?: React.ReactNode
  size?: FieldControlSize
  className?: string
  /** Short unit suffix inside the cut: "tokens", "ms", "%". Also added to the accessible description. */
  unit?: string
  placeholder?: string
  onValueChange?: (value: number | null) => void
  /** Accessible names of the steppers. */
  incrementLabel?: string
  decrementLabel?: string
  'aria-label'?: string
  'aria-describedby'?: string
  'aria-invalid'?: boolean | 'true' | 'false'
}

const stepperClasses = [
  'grid h-full aspect-square shrink-0 place-items-center text-ink-2 select-none',
  'transition-colors duration-(--motion-cut) ease-cut',
  'hover:bg-ink-soft hover:text-ink active:bg-ink active:text-on-ink',
  'data-disabled:cursor-not-allowed data-disabled:bg-cloth-shade data-disabled:text-ink-muted data-disabled:hover:bg-cloth-shade',
  '[&_svg]:size-3.5',
] as const

export function NumberInput({
  label,
  size = 'md',
  className,
  unit,
  placeholder,
  id,
  value,
  defaultValue,
  onValueChange,
  min,
  max,
  step,
  largeStep = 10,
  incrementLabel = 'Increase',
  decrementLabel = 'Decrease',
  'aria-label': ariaLabel,
  'aria-describedby': ariaDescribedBy,
  'aria-invalid': ariaInvalid,
  required,
  disabled,
  readOnly,
  ...props
}: NumberInputProps) {
  const uid = React.useId()
  const inputId = id ?? `${uid}-input`
  const unitId = `${uid}-unit`

  // Held here as well as in Base UI so PageUp/PageDown can step from the
  // current value; Base UI gets it back as a controlled `value`.
  const isControlled = value !== undefined
  const [inner, setInner] = React.useState<number | null>(defaultValue ?? null)
  const current = isControlled ? value : inner

  function commit(next: number | null) {
    if (!isControlled) setInner(next)
    onValueChange?.(next)
  }

  function onKeyDown(event: React.KeyboardEvent<HTMLInputElement>) {
    if (event.key !== 'PageUp' && event.key !== 'PageDown') return
    if (disabled || readOnly) return
    event.preventDefault()
    const delta = event.key === 'PageUp' ? largeStep : -largeStep
    let next = (current ?? min ?? 0) + delta
    if (min !== undefined) next = Math.max(min, next)
    if (max !== undefined) next = Math.min(max, next)
    // Steps like 0.1 accumulate float error; round to the step's precision.
    const decimals = Math.max(decimalPlaces(largeStep), decimalPlaces(typeof step === 'number' ? step : 1))
    commit(Number(next.toFixed(decimals)))
  }

  const describedBy = [ariaDescribedBy, unit ? unitId : undefined].filter(Boolean).join(' ') || undefined

  const field = (
    <NumberFieldPrimitive.Group
      data-slot="number-input"
      className={cn(
        fieldControlClasses,
        fieldControlSizes[size],
        'flex items-center gap-2 pe-0 cursor-text',
        'focus-within:shadow-[var(--focus-ring)] focus-within:hover:shadow-[var(--focus-ring)]',
        'has-[[aria-invalid=true]]:band-rojo has-[[aria-invalid=true]]:cut-band',
        'has-[[aria-invalid=true]]:focus-within:shadow-[var(--focus-ring)]',
        'data-disabled:cursor-not-allowed data-disabled:bg-cloth-shade data-disabled:text-ink-muted data-disabled:hover:shadow-cut',
        label ? undefined : className,
      )}
    >
      <NumberFieldPrimitive.Input
        placeholder={placeholder}
        aria-label={ariaLabel}
        aria-describedby={describedBy}
        aria-invalid={ariaInvalid}
        onKeyDown={onKeyDown}
        className="h-full min-w-0 flex-1 border-0 bg-transparent p-0 [font-size:inherit] text-ink tabular-nums outline-none placeholder:text-ink-muted focus:shadow-none focus-visible:shadow-none disabled:cursor-not-allowed disabled:text-ink-muted"
      />
      {unit ? (
        <span id={unitId} className="shrink-0 text-sm text-ink-muted">
          {unit}
        </span>
      ) : null}
      <span className="flex h-full shrink-0 items-stretch">
        <span aria-hidden className="w-px bg-keyline forced-ink" />
        <NumberFieldPrimitive.Decrement aria-label={decrementLabel} className={cn(stepperClasses)}>
          <Minus aria-hidden strokeWidth={2.5} />
        </NumberFieldPrimitive.Decrement>
        <span aria-hidden className="w-px bg-keyline forced-ink" />
        <NumberFieldPrimitive.Increment aria-label={incrementLabel} className={cn(stepperClasses)}>
          <Plus aria-hidden strokeWidth={2.5} />
        </NumberFieldPrimitive.Increment>
      </span>
    </NumberFieldPrimitive.Group>
  )

  return (
    <NumberFieldPrimitive.Root
      {...props}
      // Base UI puts the Root's id on the input and points the steppers'
      // aria-controls at it; an id set on the input itself breaks that link.
      id={inputId}
      value={current}
      onValueChange={(next) => commit(next)}
      min={min}
      max={max}
      step={step}
      largeStep={largeStep}
      required={required}
      disabled={disabled}
      readOnly={readOnly}
      className={label ? cn('flex flex-col gap-2', className) : 'contents'}
    >
      {label ? (
        <Label htmlFor={inputId} required={required}>
          {label}
        </Label>
      ) : null}
      {field}
    </NumberFieldPrimitive.Root>
  )
}

function decimalPlaces(n: number) {
  const text = String(n)
  const dot = text.indexOf('.')
  return dot === -1 ? 0 : text.length - dot - 1
}

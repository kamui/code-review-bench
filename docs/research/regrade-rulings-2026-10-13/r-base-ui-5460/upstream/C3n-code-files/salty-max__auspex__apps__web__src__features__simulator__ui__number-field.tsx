import { Field } from '@/components/ui/field'
import { clamp } from '@/features/simulator/domain/matchup'

/** A clamped integer field for a stat characteristic. */
export function NumberField({
  label,
  value,
  min,
  max,
  onChange,
}: {
  label: string
  value: number
  min: number
  max: number
  onChange: (value: number) => void
}) {
  return (
    <Field>
      <Field.Label>{label}</Field.Label>
      <Field.Control
        type="number"
        inputMode="numeric"
        value={value}
        min={min}
        max={max}
        onChange={(e) =>
          onChange(clamp(e.currentTarget.valueAsNumber, min, max))
        }
      />
    </Field>
  )
}

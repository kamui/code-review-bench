import {
  forwardRef,
  useId,
  useMemo,
  type ForwardedRef,
  type InputHTMLAttributes,
  type KeyboardEvent,
  type ReactNode,
} from "react"
import { Input as InputPrimitive } from "@base-ui/react/input"
import { Controller, type Control, type FieldValues, type UseControllerProps } from "react-hook-form"

import { FieldShell } from '@/components/ui/field-shell'
import { cn } from "@/lib/utils"

interface InputProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "ref"> {
  className?: string
}

type InputFieldProps<
  TFieldValues extends FieldValues,
  TContext = undefined,
  TTransformedValues = TFieldValues,
> = Omit<InputProps, "name" | "value" | "defaultValue" | "onChange" | "onBlur"> &
  Omit<UseControllerProps<TFieldValues>, "control"> & {
    control?: Control<TFieldValues, TContext, TTransformedValues>
    label: string
    beforeContent?: ReactNode
    afterContent?: ReactNode
    containerClassName?: string
  }

type NumberInputFieldProps<
  TFieldValues extends FieldValues,
  TContext = undefined,
  TTransformedValues = TFieldValues,
> = Omit<InputFieldProps<TFieldValues, TContext, TTransformedValues>, "type" | "inputMode"> & {
    maxValue?: number
    onValueChange?: (value: number) => void
  }

const handleNumericInputKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
  const allowedControlKeys = [
    "Backspace",
    "Delete",
    "Tab",
    "Enter",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
  ]

  if (allowedControlKeys.includes(event.key)) {
    return
  }
  if (!/^[\d.,-]$/.test(event.key)) {
    event.preventDefault()
  }
}

const Input = forwardRef<HTMLInputElement, InputProps>(
  ({ className, type, onKeyDown, autoComplete = "off", ...props }, ref: ForwardedRef<HTMLInputElement>) => {
    const isNumeric = type === "number"

    return (
      <InputPrimitive
        {...props}
        ref={ref}
        type={isNumeric ? "text" : type}
        inputMode={isNumeric ? "numeric" : props.inputMode}
        autoComplete={autoComplete}
        data-slot="input"
        onKeyDown={(event) => {
          if (isNumeric) {
            handleNumericInputKeyDown(event)
          }
          onKeyDown?.(event)
        }}
        className={cn(
          "w-full flex-1 border rounded-md border-input bg-background px-4 py-3 text-base outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed md:text-sm",
          className,
        )}
      />
    )
  },
)

Input.displayName = "Input"

interface InputWrapperProps {
  children: ReactNode
  className?: string
  invalid?: boolean
  disabled?: boolean
}

function InputWrapper({ children, className, invalid, disabled }: Readonly<InputWrapperProps>) {
  return (
    <div
      className={cn(
        "relative flex min-h-10 w-full items-center gap-2 rounded-md border border-input bg-background px-4 py-3 text-sm transition-colors",
        "focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/30",
        "aria-[invalid=true]:border-destructive aria-[invalid=true]:ring-3 aria-[invalid=true]:ring-destructive/20",
        "aria-disabled:cursor-not-allowed aria-disabled:opacity-60",
        className,
      )}
      aria-invalid={invalid}
      aria-disabled={disabled}
    >
      {children}
    </div>
  )
}

function InputField<
  TFieldValues extends FieldValues,
  TContext = undefined,
  TTransformedValues = TFieldValues,
>({
  control,
  name,
  label,
  beforeContent = null,
  afterContent = null,
  containerClassName,
  className,
  ...props
}: InputFieldProps<TFieldValues, TContext, TTransformedValues>) {
  const fieldId = useId()

  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <FieldShell
          id={fieldId}
          label={label}
          disabled={props.disabled}
          error={fieldState.error?.message}
          className={containerClassName}
        >
          <InputWrapper invalid={Boolean(fieldState.error)} disabled={props.disabled}>
            {beforeContent}
            <Input id={fieldId} {...field} {...props} className={className} />
            {afterContent}
          </InputWrapper>
        </FieldShell>
      )}
    />
  )
}

function NumberInputField<
  TFieldValues extends FieldValues,
  TContext = undefined,
  TTransformedValues = TFieldValues,
>({
  control,
  name,
  label,
  onValueChange,
  maxValue,
  beforeContent = null,
  afterContent = null,
  containerClassName,
  className,
  ...props
}: NumberInputFieldProps<TFieldValues, TContext, TTransformedValues>) {
  const isMobile = useMemo(() => {
    if (globalThis.window === undefined) {
      return false
    }

    return globalThis.window.matchMedia('(max-width: 768px)').matches
  }, [])

  return (
    <Controller
      control={control}
      name={name}
      render={({ field, fieldState }) => (
        <FieldShell
          label={label}
          disabled={props.disabled}
          error={fieldState.error?.message}
          className={containerClassName}
        >
          <InputWrapper invalid={Boolean(fieldState.error)} disabled={props.disabled}>
            {beforeContent}
            <Input
              {...field}
              {...props}
              type="number"
              className={className}
              onChange={(event) => {
                const rawValue = event.target.value
                const numericValue = rawValue === "" ? 0 : Number(rawValue)

                if (Number.isNaN(numericValue)) {
                  field.onChange(rawValue)
                  return
                }

                if (maxValue != null && numericValue > maxValue) {
                  return
                }

                field.onChange(rawValue)
                onValueChange?.(numericValue)
              }}
              onMouseUp={(event) => {
                if (isMobile) {
                  event.preventDefault()
                }
              }}
            />
            {afterContent}
          </InputWrapper>
        </FieldShell>
      )}
    />
  )
}

export { Input, InputField, NumberInputField }

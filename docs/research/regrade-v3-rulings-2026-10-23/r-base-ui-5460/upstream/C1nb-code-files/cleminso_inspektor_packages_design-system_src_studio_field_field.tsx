import { Field as BaseField } from '@base-ui/react/field'
import { type ComponentRef, forwardRef } from 'react'

import { createStateStyleProps } from '../../primitives/createStateStyleProps'
import { FieldContext } from './fieldContext'
import { fieldStyles } from './field.styles'

export interface FieldRootProps extends Omit<
  BaseField.Root.Props,
  'className' | 'render' | 'style'
> {
  /** Disables the field and its control. */
  disabled?: BaseField.Root.Props['disabled']
  /** Identifies the field when a form is submitted. */
  name?: BaseField.Root.Props['name']
  /** Controls invalid state from an external form library. */
  invalid?: BaseField.Root.Props['invalid']
  /** Controls dirty state from an external form library. */
  dirty?: BaseField.Root.Props['dirty']
  /** Controls touched state from an external form library. */
  touched?: BaseField.Root.Props['touched']
  /** Validates the field value and returns validation messages. */
  validate?: BaseField.Root.Props['validate']
  /** Selects when field validation runs. */
  validationMode?: BaseField.Root.Props['validationMode']
  /** Delays validation while using change validation. */
  validationDebounceTime?: BaseField.Root.Props['validationDebounceTime']
  /** Provides access to imperative field actions. */
  actionsRef?: BaseField.Root.Props['actionsRef']
  /** Composes the field root onto a design-system structural element. */
  render?: BaseField.Root.Props['render']
}

export interface FieldLabelProps extends Omit<BaseField.Label.Props, 'className' | 'style'> {
  /** Indicates whether the rendered element uses native label behavior. */
  nativeLabel?: BaseField.Label.Props['nativeLabel']
  /** Composes Field.Label behavior and styles onto another element. */
  render?: BaseField.Label.Props['render']
}

export interface FieldDescriptionProps extends Omit<
  BaseField.Description.Props,
  'className' | 'style'
> {
  /** Composes Field.Description behavior and styles onto another element. */
  render?: BaseField.Description.Props['render']
}

export interface FieldErrorProps extends Omit<BaseField.Error.Props, 'className' | 'style'> {
  /** Selects the validity condition that displays the error. Use true for externally controlled errors. */
  match?: BaseField.Error.Props['match']
  /** Composes Field.Error behavior and styles onto another element. */
  render?: BaseField.Error.Props['render']
}

const FieldRoot = forwardRef<ComponentRef<typeof BaseField.Root>, FieldRootProps>(
  function FieldRoot({ disabled = false, hidden = false, ...props }, forwardedRef) {
    const stateStyleProps = createStateStyleProps<BaseField.Root.State>((state) => [
      fieldStyles.root,
      state.valid === false && fieldStyles.inputGroupInvalid,
      state.disabled === true && fieldStyles.inputGroupDisabled,
      state.disabled === true && fieldStyles.rootDisabled,
      state.valid === true && fieldStyles.rootValid,
      state.valid === false && fieldStyles.rootInvalid,
      state.touched === true && fieldStyles.rootTouched,
      state.dirty === true && fieldStyles.rootDirty,
      state.filled === true && fieldStyles.rootFilled,
      state.focused === true && fieldStyles.rootFocused,
      hidden === true && fieldStyles.hidden,
    ])

    return (
      <FieldContext.Provider value={{ disabled }}>
        <BaseField.Root
          {...props}
          ref={forwardedRef}
          disabled={disabled}
          hidden={hidden}
          className={stateStyleProps.className}
          style={stateStyleProps.style}
          data-slot="field"
        />
      </FieldContext.Provider>
    )
  },
)

const FieldLabel = forwardRef<ComponentRef<typeof BaseField.Label>, FieldLabelProps>(
  function FieldLabel({ nativeLabel = true, ...props }, forwardedRef) {
    const stateStyleProps = createStateStyleProps<BaseField.Label.State>((state) => [
      fieldStyles.label,
      state.disabled === true && fieldStyles.disabled,
      state.disabled === true && fieldStyles.labelDisabled,
      state.valid === true && fieldStyles.labelValid,
      state.valid === false && fieldStyles.labelInvalid,
      state.touched === true && fieldStyles.labelTouched,
      state.dirty === true && fieldStyles.labelDirty,
      state.filled === true && fieldStyles.labelFilled,
      state.focused === true && fieldStyles.labelFocused,
    ])

    return (
      <BaseField.Label
        {...props}
        ref={forwardedRef}
        nativeLabel={nativeLabel}
        className={stateStyleProps.className}
        style={stateStyleProps.style}
        data-slot="field-label"
      />
    )
  },
)

const FieldDescription = forwardRef<
  ComponentRef<typeof BaseField.Description>,
  FieldDescriptionProps
>(function FieldDescription(props, forwardedRef) {
  const stateStyleProps = createStateStyleProps<BaseField.Description.State>((state) => [
    fieldStyles.message,
    fieldStyles.description,
    state.disabled === true && fieldStyles.disabled,
    state.disabled === true && fieldStyles.descriptionDisabled,
    state.valid === true && fieldStyles.descriptionValid,
    state.valid === false && fieldStyles.descriptionInvalid,
    state.touched === true && fieldStyles.descriptionTouched,
    state.dirty === true && fieldStyles.descriptionDirty,
    state.filled === true && fieldStyles.descriptionFilled,
    state.focused === true && fieldStyles.descriptionFocused,
  ])

  return (
    <BaseField.Description
      {...props}
      ref={forwardedRef}
      className={stateStyleProps.className}
      style={stateStyleProps.style}
      data-slot="field-description"
    />
  )
})

const FieldError = forwardRef<ComponentRef<typeof BaseField.Error>, FieldErrorProps>(
  function FieldError(props, forwardedRef) {
    const stateStyleProps = createStateStyleProps<BaseField.Error.State>((state) => [
      fieldStyles.message,
      fieldStyles.error,
      state.disabled === true && fieldStyles.disabled,
      state.disabled === true && fieldStyles.errorDisabled,
      state.valid === true && fieldStyles.errorValid,
      state.valid === false && fieldStyles.errorInvalid,
      state.touched === true && fieldStyles.errorTouched,
      state.dirty === true && fieldStyles.errorDirty,
      state.filled === true && fieldStyles.errorFilled,
      state.focused === true && fieldStyles.errorFocused,
      state.transitionStatus === 'starting' && fieldStyles.errorStarting,
      state.transitionStatus === 'ending' && fieldStyles.errorEnding,
    ])

    return (
      <BaseField.Error
        {...props}
        ref={forwardedRef}
        className={stateStyleProps.className}
        style={stateStyleProps.style}
        data-slot="field-error"
      />
    )
  },
)

export const Field = Object.assign(FieldRoot, {
  Root: FieldRoot,
  Label: FieldLabel,
  Description: FieldDescription,
  Error: FieldError,
})

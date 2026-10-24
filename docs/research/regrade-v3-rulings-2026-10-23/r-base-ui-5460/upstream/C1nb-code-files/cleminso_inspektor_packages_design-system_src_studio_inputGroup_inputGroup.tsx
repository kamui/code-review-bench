import { Button as BaseButton } from '@base-ui/react/button'
import { Field as BaseField } from '@base-ui/react/field'
import { mergeProps } from '@base-ui/react/merge-props'
import { useRender } from '@base-ui/react/use-render'
import * as stylex from '@stylexjs/stylex'
import {
  forwardRef,
  useContext,
  useState,
  type ComponentRef,
  type MouseEvent,
  type ReactNode,
} from 'react'

import { createStateStyleProps } from '../../primitives/createStateStyleProps'
import { Checkbox } from '../checkbox/checkbox'
import type { CheckboxProps } from '../checkbox/checkbox'
import { FieldContext } from '../field/fieldContext'
import type { InputSize } from '../input/input'
import { Tooltip } from '../tooltip/tooltip'
import { InputGroupContext } from './inputGroupContext'
import { inputGroupStyles } from './inputGroup.styles'

export interface InputGroupRootProps {
  /** Input, adornments, and actions contained by the compound control. */
  children?: ReactNode
  /** Controls the height of the compound input. */
  size?: InputSize
  /** Stretches the group to the width of its container. */
  fullWidth?: boolean
  /** Marks the compound input as invalid. Field.Root supplies this automatically. */
  invalid?: boolean
  /** Disables the input and every interactive group member. */
  disabled?: boolean
}

export interface InputGroupPrefixProps {
  /** Static content displayed before the editable value. */
  children: ReactNode
}

export interface InputGroupSuffixProps {
  /** Static content displayed after the editable value. */
  children: ReactNode
}

export interface InputGroupActionProps extends Omit<
  BaseButton.Props,
  | 'aria-controls'
  | 'aria-label'
  | 'aria-pressed'
  | 'children'
  | 'className'
  | 'disabled'
  | 'nativeButton'
  | 'style'
  | 'type'
> {
  /** Accessible name for the action. */
  label: string
  /** Identifies the input controlled by the action. */
  controls?: string
  /** Exposes toggle state for actions such as password visibility. */
  pressed?: boolean
  /** Disables the action. */
  disabled?: BaseButton.Props['disabled']
  /** Composes action behavior and styles onto another native button. */
  render?: BaseButton.Props['render']
  /** Decorative action content. */
  children: ReactNode
}

type BaseInputGroupLinkProps = Omit<
  useRender.ComponentProps<'a'>,
  'aria-label' | 'children' | 'className' | 'color' | 'href' | 'render' | 'style'
>

interface InputGroupLinkSharedProps {
  /** Accessible name for the icon-only navigation link. */
  label: string
  /** Decorative link content. */
  children: ReactNode
}

type InputGroupLinkDestination =
  | {
      /** Identifies the native anchor destination. */
      href: NonNullable<useRender.ComponentProps<'a'>['href']>
      render?: never
    }
  | {
      href?: never
      /** Composes link presentation onto a router link component. */
      render: NonNullable<useRender.ComponentProps<'a'>['render']>
    }

export type InputGroupLinkProps = BaseInputGroupLinkProps &
  InputGroupLinkSharedProps &
  InputGroupLinkDestination

export interface InputGroupCheckboxProps {
  /** Accessible name that identifies the value controlled by the checkbox. */
  label: string
  /** Controls whether the option is checked. */
  checked?: CheckboxProps['checked']
  /** Sets the initial checked state when uncontrolled. */
  defaultChecked?: CheckboxProps['defaultChecked']
  /** Runs when checked state changes. */
  onCheckedChange?: CheckboxProps['onCheckedChange']
  /** Disables the checkbox. */
  disabled?: CheckboxProps['disabled']
  /** Prevents changes while preserving focus. */
  readOnly?: CheckboxProps['readOnly']
  /** Provides supplementary guidance on hover and keyboard focus. */
  tooltip?: string
  /** Visible checkbox label. */
  children: ReactNode
}

const sizeStyles = {
  xs: inputGroupStyles.sizeXS,
  s: inputGroupStyles.sizeS,
  m: inputGroupStyles.sizeM,
  l: inputGroupStyles.sizeL,
} satisfies Record<InputSize, unknown>

const actionSizeStyles = {
  xs: inputGroupStyles.actionXS,
  s: inputGroupStyles.actionS,
  m: inputGroupStyles.actionM,
  l: inputGroupStyles.actionL,
} satisfies Record<InputSize, unknown>

function InputGroupRoot({
  size = 'l',
  fullWidth = false,
  invalid = false,
  disabled = false,
  children,
}: InputGroupRootProps) {
  const field = useContext(FieldContext)
  const effectiveDisabled = disabled === true || field?.disabled === true
  const [focused, setFocused] = useState(false)
  const [focusVisible, setFocusVisible] = useState(false)
  const rootStyleProps = stylex.props(
    inputGroupStyles.root,
    focusVisible === true && inputGroupStyles.focusVisible,
    sizeStyles[size],
    fullWidth === true && inputGroupStyles.fullWidth,
    invalid === true && inputGroupStyles.invalid,
    effectiveDisabled === true && inputGroupStyles.disabled,
  )

  return (
    <InputGroupContext.Provider value={{ disabled: effectiveDisabled, invalid, size }}>
      <div
        {...rootStyleProps}
        data-slot="input-group"
        data-invalid={invalid === true ? '' : undefined}
        data-disabled={effectiveDisabled === true ? '' : undefined}
        data-focused={focused === true ? '' : undefined}
        data-focus-visible={focusVisible === true ? '' : undefined}
        onFocusCapture={(event) => {
          setFocused(true)
          setFocusVisible(
            event.target instanceof HTMLElement && event.target.matches(':focus-visible'),
          )
        }}
        onBlurCapture={(event) => {
          if (
            event.relatedTarget instanceof Node &&
            event.currentTarget.contains(event.relatedTarget)
          ) {
            return
          }

          setFocused(false)
          setFocusVisible(false)
        }}
      >
        {children}
      </div>
    </InputGroupContext.Provider>
  )
}

function InputGroupPrefix({ children }: InputGroupPrefixProps) {
  return (
    <span
      {...stylex.props(inputGroupStyles.text, inputGroupStyles.prefix)}
      data-slot="input-group-prefix"
    >
      {children}
    </span>
  )
}

function InputGroupSuffix({ children }: InputGroupSuffixProps) {
  return (
    <span
      {...stylex.props(inputGroupStyles.text, inputGroupStyles.suffix)}
      data-slot="input-group-suffix"
    >
      {children}
    </span>
  )
}

const InputGroupAction = forwardRef<ComponentRef<typeof BaseButton>, InputGroupActionProps>(
  function InputGroupAction(
    { label, controls, pressed, disabled = false, render, children, ...props },
    ref,
  ) {
    const context = useContext(InputGroupContext)
    const effectiveDisabled = disabled === true || context?.disabled === true
    const size = context?.size ?? 'm'
    const stateStyleProps = createStateStyleProps<BaseButton.State>((state) => [
      inputGroupStyles.action,
      actionSizeStyles[size],
      pressed === true && inputGroupStyles.actionPressed,
      state.disabled === true && inputGroupStyles.memberDisabled,
      state.disabled === true && inputGroupStyles.actionDisabled,
    ])

    return (
      <BaseButton
        {...props}
        ref={ref}
        type="button"
        nativeButton
        aria-label={label}
        aria-controls={controls}
        aria-pressed={pressed}
        disabled={effectiveDisabled}
        render={render}
        {...stateStyleProps}
        data-slot="input-group-action"
        data-pressed={pressed === true ? '' : undefined}
      >
        {children}
      </BaseButton>
    )
  },
)

const InputGroupLink = forwardRef<HTMLAnchorElement, InputGroupLinkProps>(function InputGroupLink(
  { label, render, children, ...props },
  forwardedRef,
) {
  const context = useContext(InputGroupContext)
  const size = context?.size ?? 'm'
  const disabled = context?.disabled === true
  const styleProps = stylex.props(
    inputGroupStyles.action,
    inputGroupStyles.link,
    actionSizeStyles[size],
    disabled === true && inputGroupStyles.memberDisabled,
  )
  const defaultProps = {
    ...styleProps,
    'aria-label': label,
    'aria-disabled': disabled === true ? true : undefined,
    children,
    'data-disabled': disabled === true ? '' : undefined,
    'data-slot': 'input-group-link',
    onClick:
      disabled === true
        ? (event: MouseEvent<HTMLAnchorElement>) => {
            event.preventDefault()
          }
        : undefined,
    tabIndex: disabled === true ? -1 : undefined,
  } as useRender.ElementProps<'a'>
  const domProps = Object.fromEntries(
    Object.entries(props).filter(
      ([key]) =>
        key !== 'className' &&
        key !== 'color' &&
        key !== 'style' &&
        (disabled === false || key !== 'onClick'),
    ),
  ) as useRender.ComponentProps<'a'>

  return useRender({
    defaultTagName: 'a',
    render,
    ref: forwardedRef,
    props: mergeProps<'a'>(domProps, defaultProps),
  })
})

function InputGroupCheckbox({
  label,
  checked,
  defaultChecked,
  onCheckedChange,
  disabled = false,
  readOnly = false,
  tooltip,
  children,
}: InputGroupCheckboxProps) {
  const context = useContext(InputGroupContext)
  const effectiveDisabled = disabled === true || context?.disabled === true
  const fieldStyleProps = createStateStyleProps<BaseField.Root.State>((state) => [
    inputGroupStyles.checkboxField,
    state.disabled === true && inputGroupStyles.memberDisabled,
    state.disabled === true && inputGroupStyles.checkboxDisabled,
    state.valid === true && inputGroupStyles.checkboxValid,
    state.valid === false && inputGroupStyles.checkboxInvalid,
    state.touched === true && inputGroupStyles.checkboxTouched,
    state.dirty === true && inputGroupStyles.checkboxDirty,
    state.filled === true && inputGroupStyles.checkboxFilled,
    state.focused === true && inputGroupStyles.checkboxFocused,
  ])

  const field = (
    <BaseField.Root
      disabled={effectiveDisabled}
      {...fieldStyleProps}
      render={
        <label
          aria-label={label}
          data-slot="input-group-checkbox"
        />
      }
    >
      <Checkbox
        size="s"
        checked={checked}
        defaultChecked={defaultChecked}
        onCheckedChange={onCheckedChange}
        disabled={effectiveDisabled}
        readOnly={readOnly}
      />
      <span>{children}</span>
      {tooltip === undefined ? null : (
        <BaseField.Description hidden>{tooltip}</BaseField.Description>
      )}
    </BaseField.Root>
  )

  return tooltip === undefined ? (
    field
  ) : (
    <Tooltip.Root>
      <Tooltip.Trigger render={field} />
      <Tooltip.Content>
        <span aria-hidden="true">{tooltip}</span>
      </Tooltip.Content>
    </Tooltip.Root>
  )
}

export const InputGroup = Object.assign(InputGroupRoot, {
  Root: InputGroupRoot,
  Prefix: InputGroupPrefix,
  Suffix: InputGroupSuffix,
  Action: InputGroupAction,
  Link: InputGroupLink,
  Checkbox: InputGroupCheckbox,
})

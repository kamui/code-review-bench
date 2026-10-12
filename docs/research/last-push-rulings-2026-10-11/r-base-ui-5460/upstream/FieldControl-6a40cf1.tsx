'use client';
import * as React from 'react';
import { useControlled } from '@base-ui/utils/useControlled';
import { useIsoLayoutEffect } from '@base-ui/utils/useIsoLayoutEffect';
import { ownerDocument } from '@base-ui/utils/owner';
import { useStableCallback } from '@base-ui/utils/useStableCallback';
import { type FieldRootState } from '../root/FieldRoot';
import { useFieldRootContext } from '../../internals/field-root-context/FieldRootContext';
import { useRegisterFieldControl } from '../../internals/field-register-control/useRegisterFieldControl';
import { useFormContext } from '../../internals/form-context/FormContext';
import { useLabelableContext } from '../../internals/labelable-provider/LabelableContext';
import { useLabelableId } from '../../internals/labelable-provider/useLabelableId';
import { fieldValidityMapping } from '../../internals/field-constants/constants';
import { BaseUIComponentProps } from '../../internals/types';
import { useRenderElement } from '../../internals/useRenderElement';
import { useValueChanged } from '../../internals/useValueChanged';
import { createChangeEventDetails } from '../../internals/createBaseUIEventDetails';
import { REASONS } from '../../internals/reasons';
import type { BaseUIChangeEventDetails } from '../../internals/createBaseUIEventDetails';
import { activeElement } from '../../floating-ui-react/utils';

/**
 * The form control to label and validate.
 * Renders an `<input>` element.
 *
 * You can omit this part and use any Base UI input component instead. For example,
 * [Input](https://base-ui.com/react/components/input), [Checkbox](https://base-ui.com/react/components/checkbox),
 * or [Select](https://base-ui.com/react/components/select), among others, will work with Field out of the box.
 *
 * Documentation: [Base UI Field](https://base-ui.com/react/components/field)
 */
export const FieldControl = React.forwardRef(function FieldControl(
  componentProps: FieldControl.Props,
  forwardedRef: React.ForwardedRef<HTMLElement>,
) {
  const {
    render,
    className,
    id: idProp,
    name: nameProp,
    value: valueProp,
    disabled: disabledProp = false,
    onValueChange,
    defaultValue,
    autoFocus = false,
    style,
    ...elementProps
  } = componentProps;

  const {
    state: fieldState,
    name: fieldName,
    disabled: fieldDisabled,
    setTouched,
    setDirty,
    validityData,
    setFocused,
    setFilled,
    validationMode,
    validation,
  } = useFieldRootContext();
  const { clearErrors } = useFormContext();

  const disabled = fieldDisabled || disabledProp;
  const name = fieldName ?? nameProp;

  const state: FieldControlState = {
    ...fieldState,
    disabled,
  };

  const { labelId } = useLabelableContext();

  const id = useLabelableId({ id: idProp });

  const [valueUnwrapped] = useControlled({
    controlled: valueProp,
    default: defaultValue,
    name: 'FieldControl',
    state: 'value',
  });

  const isControlled = valueProp !== undefined;
  const value = isControlled ? valueUnwrapped : undefined;
  // The DOM input always holds a string, so the Field-owned initial baseline and the dirty
  // comparisons against it must use the serialized form of the controlled value.
  const serializedValue = value == null ? undefined : String(value);

  const getValueFromInput = useStableCallback(() => validation.inputRef.current?.value);

  useRegisterFieldControl(
    validation.inputRef,
    id,
    serializedValue,
    getValueFromInput,
    !disabled,
    nameProp,
  );

  useIsoLayoutEffect(() => {
    if (serializedValue !== undefined) {
      setFilled(serializedValue !== '');
    } else if (validation.inputRef.current?.value) {
      setFilled(true);
    }
  }, [validation.inputRef, setFilled, serializedValue]);

  const processedValueRef = React.useRef<string | undefined>(undefined);

  // A controlled transition that the change handler didn't produce is a programmatic change: it
  // mirrors a user change, minus the echo of a value the change handler already processed.
  useValueChanged(serializedValue, () => {
    const processedValue = processedValueRef.current;
    processedValueRef.current = undefined;
    if (serializedValue === undefined || serializedValue === processedValue) {
      return;
    }

    setDirty(serializedValue !== (validityData.initialValue ?? ''));
    if (!disabled) {
      validation.change(serializedValue);
    }
  });

  const inputRef = React.useRef<HTMLElement>(null);

  useIsoLayoutEffect(() => {
    if (autoFocus && inputRef.current === activeElement(ownerDocument(inputRef.current))) {
      setFocused(true);
    }
  }, [autoFocus, setFocused]);

  useIsoLayoutEffect(() => {
    if (disabled) {
      setFocused(false);
    }
  }, [disabled, setFocused]);

  // A control that unmounts or becomes disabled while focused never fires blur, which would latch
  // `data-focused`. Skip it when a replacement control already owns focus, since the shared
  // `inputRef` then points at the replacement.
  useIsoLayoutEffect(() => {
    return () => {
      const input = validation.inputRef.current;
      if (input && activeElement(ownerDocument(input)) === input) {
        return;
      }
      setFocused(false);
    };
  }, [setFocused, validation.inputRef]);

  const element = useRenderElement('input', componentProps, {
    ref: [forwardedRef, inputRef],
    state,
    props: [
      {
        id,
        disabled,
        name,
        ref: validation.inputRef,
        'aria-labelledby': labelId,
        autoFocus,
        ...(isControlled ? { value } : { defaultValue }),
        onChange(event) {
          const inputValue = event.currentTarget.value;
          // Even a canceled change counts as processed: the consumer opted out of Base UI handling
          // for it, so the controlled echo must not validate it either.
          processedValueRef.current = inputValue;
          const details = createChangeEventDetails(REASONS.none, event.nativeEvent);
          onValueChange?.(inputValue, details);
          // `validation.change` reads `markedDirtyRef`, so update dirty before validating.
          setDirty(inputValue !== (validityData.initialValue ?? ''));
          setFilled(inputValue !== '');

          // Workaround for https://github.com/react/react/issues/9023
          if (!event.nativeEvent.defaultPrevented && !details.isCanceled) {
            clearErrors(name);
            validation.change(inputValue);
          }
        },
        onFocus() {
          setFocused(true);
        },
        onBlur(event) {
          setTouched(true);
          setFocused(false);

          if (validationMode === 'onBlur') {
            validation.commit(event.currentTarget.value);
          }
        },
        onKeyDown(event) {
          if (event.currentTarget.tagName === 'INPUT' && event.key === 'Enter') {
            setTouched(true);
            validation.commit(event.currentTarget.value);
          }
        },
      },
      elementProps,
      (props) => validation.getValidationProps(disabled, props),
    ],
    stateAttributesMapping: fieldValidityMapping,
  });

  return element;
});

export interface FieldControlState extends FieldRootState {}

export interface FieldControlProps extends BaseUIComponentProps<'input', FieldControlState> {
  /**
   * Callback fired when the `value` changes. Use when controlled.
   */
  onValueChange?:
    | ((value: string, eventDetails: FieldControl.ChangeEventDetails) => void)
    | undefined;
  defaultValue?: React.ComponentProps<'input'>['defaultValue'] | undefined;
}

export type FieldControlChangeEventReason = typeof REASONS.none;

export type FieldControlChangeEventDetails =
  BaseUIChangeEventDetails<FieldControl.ChangeEventReason>;

export namespace FieldControl {
  export type State = FieldControlState;
  export type Props = FieldControlProps;
  export type ChangeEventReason = FieldControlChangeEventReason;
  export type ChangeEventDetails = FieldControlChangeEventDetails;
}

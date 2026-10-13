import { composeRefs } from "@comp0/core";
import { Input, type InputProps } from "../text-field/Input.js";
import { useNumberFieldContext } from "./number-field-shared.js";
import { useFormReset } from "../internal/form-control-state.js";

export type NumberFieldInputProps = Omit<
  InputProps,
  | "type"
  | "value"
  | "defaultValue"
  | "id"
  | "name"
  | "min"
  | "max"
  | "step"
  | "disabled"
  | "required"
>;

export function NumberFieldInput({ onChange, ref, ...props }: NumberFieldInputProps) {
  const numberField = useNumberFieldContext("NumberFieldInput");
  useFormReset({
    controlRef: numberField.inputRef,
    form: props.form,
    state: numberField.valueState,
    readValue: (element) => element.valueAsNumber,
  });

  return (
    <Input
      {...props}
      ref={composeRefs(ref, numberField.inputRef)}
      id={numberField.controlId}
      type="number"
      value={Number.isNaN(numberField.value) ? "" : numberField.value}
      name={numberField.name}
      min={numberField.min}
      max={numberField.max}
      step={numberField.step}
      disabled={numberField.disabled}
      required={numberField.required}
      onChange={(event) => {
        onChange?.(event);
        if (!event.defaultPrevented) numberField.setValue(event.currentTarget.valueAsNumber);
      }}
    />
  );
}

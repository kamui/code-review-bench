import { FC, isNil } from "@helpers";
import { InputHTMLAttributes } from "react";

import { Stepper } from "./components";
import { InputField, InputFieldSlots } from "./input-field";

interface IInputNumberProps extends Omit<
  InputHTMLAttributes<HTMLInputElement>,
  "onChange" | "value"
> {
  value?: number;
  onChange: (change?: number) => void;

  label?: string;
  placeholder?: string;
  invalid?: boolean;
  prefix?: string;
  suffix?: string;
  required?: boolean;
}

type IInputNumberSlots = InputFieldSlots & {
  Stepper: typeof Stepper;
};

export const InputNumber: FC<IInputNumberProps, IInputNumberSlots> = ({
  onChange,
  value,
  ...inputFieldProps
}) => {
  return (
    <InputField
      {...inputFieldProps}
      type="number"
      value={isNil(value) || isNaN(value) ? "" : value}
      onChange={(e) => {
        const nunber =
          e.target.value === "" ? undefined : e.target.valueAsNumber;
        onChange(Number.isNaN(nunber) ? undefined : nunber);
      }}
    />
  );
};

InputNumber.Icon = InputField.Icon;
InputNumber.Controls = InputField.Controls;
InputNumber.Label = InputField.Label;
InputNumber.Stepper = Stepper;

import React from "react";
import { UseFormRegister } from "react-hook-form";
import { Field, Input } from "../ui";

interface ReminderFormData {
  intervalMinutes?: number;
  intervalHours?: number;
  intervalDays?: number;
}

interface Props {
  register: UseFormRegister<ReminderFormData>;
  show: boolean;
}

export default function RepeatIntervalSelector({ register, show }: Props) {
  if (!show) return null;

  return (
    <div className="space-y-4">
      <label className="text-start block text-sm font-medium text-gray-700 mb-2">
        Repeats every
      </label>
      <div className="grid grid-cols-3 gap-4">
        <Field.Root>
          <Field.Control
            render={
              <Input
                type="number"
                {...register("intervalMinutes", {
                  min: 0,
                  max: 59,
                  valueAsNumber: true,
                })}
                placeholder="0"
                min={0}
                max={59}
                onFocus={(e) => e.target.select()}
              />
            }
          />
          <Field.Description>Minutes</Field.Description>
        </Field.Root>

        <Field.Root>
          <Field.Control
            render={
              <Input
                type="number"
                {...register("intervalHours", {
                  min: 0,
                  max: 23,
                  valueAsNumber: true,
                })}
                placeholder="0"
                min={0}
                max={23}
                onFocus={(e) => e.target.select()}
              />
            }
          />
          <Field.Description>Hours</Field.Description>
        </Field.Root>

        <Field.Root>
          <Field.Control
            render={
              <Input
                type="number"
                {...register("intervalDays", {
                  min: 0,
                  valueAsNumber: true,
                })}
                placeholder="0"
                min={0}
                onFocus={(e) => e.target.select()}
              />
            }
          />
          <Field.Description>Days</Field.Description>
        </Field.Root>
      </div>
    </div>
  );
}
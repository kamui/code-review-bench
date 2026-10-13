import { Slider as BaseSlider } from "@base-ui-components/react/slider";
import { useMemo, useState } from "react";
import styles from "./Slider.module.css";

type SliderMode = "single" | "range";
type SliderValue = number | [number, number];

interface SliderProps {
  mode?: SliderMode;
  min?: number;
  max?: number;
  defaultValue?: SliderValue;
  step?: number;
  disabled?: boolean;
  minLabel?: string;
  maxLabel?: string;
  showStepper?: boolean;
  stepperFrequency?: number;
  showValueLabel?: boolean;
  showValueInput?: boolean;
  value?: SliderValue;
  onValueChange?: (value: SliderValue) => void;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function quantize(value: number, min: number, step: number): number {
  return min + Math.round((value - min) / step) * step;
}

function normalizeSingle(value: number, min: number, max: number, step: number): number {
  return clamp(quantize(value, min, step), min, max);
}

function normalizeRange(value: [number, number], min: number, max: number, step: number): [number, number] {
  const first = normalizeSingle(value[0], min, max, step);
  const second = normalizeSingle(value[1], min, max, step);
  return first <= second ? [first, second] : [second, first];
}

function toArrayValue(mode: SliderMode, raw: SliderValue | undefined, min: number, max: number, step: number): number[] {
  if (mode === "range") {
    if (Array.isArray(raw) && raw.length >= 2) {
      const [start, end] = normalizeRange([Number(raw[0]), Number(raw[1])], min, max, step);
      return [start, end];
    }
    return [min, min];
  }
  const singleRaw = Array.isArray(raw) ? Number(raw[0]) : Number(raw ?? min);
  return [normalizeSingle(singleRaw, min, max, step)];
}

function toOutputValue(mode: SliderMode, arr: number[]): SliderValue {
  return mode === "range" ? [arr[0], arr[1]] : arr[0];
}

export function Slider({
  mode = "single",
  min = 0,
  max = 100,
  defaultValue,
  step = 1,
  disabled,
  minLabel,
  maxLabel,
  showStepper = false,
  stepperFrequency,
  showValueLabel = true,
  showValueInput = false,
  value,
  onValueChange,
}: SliderProps) {
  const safeStep = step > 0 ? step : 1;
  const effectiveFrequency = stepperFrequency && stepperFrequency > 0 ? stepperFrequency : safeStep;
  const activeStep = showStepper ? effectiveFrequency : safeStep;
  const [internalValue, setInternalValue] = useState<number[]>(() => toArrayValue(mode, defaultValue, min, max, safeStep));
  const isControlled = value !== undefined;
  const currentValues = isControlled
    ? toArrayValue(mode, value, min, max, activeStep)
    : toArrayValue(mode, toOutputValue(mode, internalValue), min, max, activeStep);

  const handleValueUpdate = (nextArrayRaw: number[]) => {
    const normalized = mode === "range"
      ? normalizeRange([nextArrayRaw[0] ?? min, nextArrayRaw[1] ?? min], min, max, activeStep)
      : [normalizeSingle(nextArrayRaw[0] ?? min, min, max, activeStep)];
    const next = mode === "range" ? [normalized[0], normalized[1]] : [normalized[0]];
    if (!isControlled) {
      setInternalValue(next);
    }
    onValueChange?.(toOutputValue(mode, next));
  };

  const updateInputValue = (index: 0 | 1, raw: string) => {
    const parsed = Number(raw);
    if (!Number.isFinite(parsed)) return;
    if (mode === "range") {
      const draft: [number, number] = [currentValues[0], currentValues[1]];
      draft[index] = parsed;
      const normalized = normalizeRange(draft, min, max, activeStep);
      handleValueUpdate([normalized[0], normalized[1]]);
      return;
    }
    const normalized = normalizeSingle(parsed, min, max, activeStep);
    handleValueUpdate([normalized]);
  };

  const range = Math.max(max - min, 1);
  const minActive = currentValues.some((v) => v === min);
  const maxActive = currentValues.some((v) => v === max);
  const thumbLefts = currentValues.map((v) => {
    const percent = ((v - min) / range) * 100;
    return `${percent}%`;
  });

  const ticks = useMemo(() => {
    if (!showStepper) return [];
    const count = Math.floor((max - min) / effectiveFrequency) + 1;
    const boundedCount = Math.min(Math.max(count, 2), 201);
    const values = Array.from({ length: boundedCount }, (_, index) => min + index * effectiveFrequency);
    if (values[values.length - 1] !== max) {
      values.push(max);
    }
    return values.map((tickValue) => {
      const selected = mode === "range"
        ? tickValue >= currentValues[0] && tickValue <= currentValues[1]
        : tickValue <= currentValues[0];
      return {
        value: tickValue,
        selected,
      };
    });
  }, [currentValues, effectiveFrequency, max, min, mode, showStepper]);

  return (
    <div className={styles.wrapper}>
      <div className={styles.sliderRow}>
        <span className={[styles.edgeLabel, minActive ? styles.edgeLabelActive : ""].filter(Boolean).join(" ")}>
          {minLabel ?? ""}
        </span>
        <BaseSlider.Root
          className={styles.root}
          min={min}
          max={max}
          defaultValue={!isControlled ? toArrayValue(mode, defaultValue, min, max, safeStep) : undefined}
          value={isControlled ? currentValues : undefined}
          step={activeStep}
          disabled={disabled}
          onValueChange={(val) => {
            const arr = Array.isArray(val) ? val.map(Number) : [Number(val)];
            handleValueUpdate(arr);
          }}
        >
          <BaseSlider.Control className={styles.control}>
            <BaseSlider.Track className={styles.track}>
              <BaseSlider.Indicator className={styles.indicator} />
              {showStepper && ticks.length > 0 ? (
                <div className={styles.stepperLayer} aria-hidden="true">
                  {ticks.map((tick) => {
                    const tickPercent = ((tick.value - min) / range) * 100;
                    const tickLeft = `${tickPercent}%`;
                    return (
                      <span
                        key={tick.value}
                        className={[styles.stepperDot, tick.selected ? styles.stepperDotSelected : ""].filter(Boolean).join(" ")}
                        style={{ left: tickLeft }}
                      />
                    );
                  })}
                </div>
              ) : null}
              {currentValues.map((_, index) => (
                <BaseSlider.Thumb key={index} className={styles.thumb} />
              ))}
              {showValueLabel ? (
                <div className={styles.valueLabelLayer} aria-hidden="true">
                  {currentValues.map((v, index) => (
                    <span
                      key={index}
                      className={disabled ? styles.valueTextDisabled : styles.valueText}
                      style={{ left: thumbLefts[index] }}
                    >
                      {v}
                    </span>
                  ))}
                </div>
              ) : null}
            </BaseSlider.Track>
          </BaseSlider.Control>
        </BaseSlider.Root>
        <div className={styles.rightMeta}>
          <span className={[styles.edgeLabel, maxActive ? styles.edgeLabelActive : ""].filter(Boolean).join(" ")}>
            {maxLabel ?? ""}
          </span>
          {showValueInput ? (
            <div className={styles.inputGroup}>
              <input
                className={styles.valueInput}
                type="number"
                value={currentValues[0]}
                disabled={disabled}
                onChange={(event) => updateInputValue(0, event.target.value)}
                aria-label={mode === "range" ? "Minimum slider value" : "Slider value"}
              />
              {mode === "range" ? (
                <>
                  <span className={styles.inputSeparator} aria-hidden="true">
                    -
                  </span>
                  <input
                    className={styles.valueInput}
                    type="number"
                    value={currentValues[1]}
                    disabled={disabled}
                    onChange={(event) => updateInputValue(1, event.target.value)}
                    aria-label="Maximum slider value"
                  />
                </>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}

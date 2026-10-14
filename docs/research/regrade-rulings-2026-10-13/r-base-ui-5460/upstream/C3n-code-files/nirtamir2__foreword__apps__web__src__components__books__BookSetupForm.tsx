import type { SyntheticEvent } from "react";
import { useId, useState } from "react";
import { Radio } from "@base-ui/react/radio";
import { RadioGroup } from "@base-ui/react/radio-group";
import { clsx } from "clsx";
import { ArrowRight, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Field } from "@/components/ui/Field";
import { Input } from "@/components/ui/Input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/Select";
import type { PositionType } from "@/lib/analysis/resolveStartChapter";
import { languageLabels, languageOptions } from "@/lib/languages";
import type { ParsedBook } from "@/lib/parsing/types";
import { LEVELS, levelDescription, levelLabel } from "@/lib/vocabularyLabels";
import type { LearnerLevel } from "@/lib/vocabularyLabels";
import { m } from "@/paraglide/messages.js";
import { getLocale, getTextDirection } from "@/paraglide/runtime.js";

export interface BookSetupValue {
  bookLanguage: string;
  nativeLanguage: string;
  level: LearnerLevel;
  positionType: PositionType;
  positionValue: number | null;
}

interface Props {
  book: ParsedBook;
  isSubmitting: boolean;
  onSubmit: (value: BookSetupValue) => void;
}

function defaultNativeLanguage({
  uiLocale,
  detected,
}: {
  uiLocale: string;
  detected: string | null;
}): string {
  if (uiLocale === "he") {
    return "he";
  }
  return detected === "en" ? "he" : "en";
}

function positionMaxFor({
  positionType,
  book,
}: {
  positionType: PositionType;
  book: ParsedBook;
}): number | undefined {
  switch (positionType) {
    case "CHAPTER": {
      return book.chapters.length;
    }
    case "PAGE": {
      return book.totalPages ?? undefined;
    }
    case "PERCENT": {
      return 100;
    }
    case "START": {
      return undefined;
    }
  }
}

function positionValueLabel({ positionType }: { positionType: PositionType }): string {
  switch (positionType) {
    case "CHAPTER": {
      return m.setup_position_chapter_number();
    }
    case "PAGE": {
      return m.setup_position_page_number();
    }
    case "PERCENT":
    case "START": {
      return m.setup_position_percent_value();
    }
  }
}

function toPositiveInt(value: string): number | null {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

export function BookSetupForm({ book, isSubmitting, onSubmit }: Props) {
  const uiLocale = getLocale();
  const isRtl = getTextDirection() === "rtl";
  const idPrefix = useId();
  const detected = book.detectedLanguage;
  const [bookLanguage, setBookLanguage] = useState(detected ?? "en");
  const [nativeLanguage, setNativeLanguage] = useState(
    defaultNativeLanguage({ uiLocale, detected }),
  );
  const [level, setLevel] = useState<LearnerLevel>("INTERMEDIATE");
  const [positionType, setPositionType] = useState<PositionType>("START");
  const [positionValue, setPositionValue] = useState("");
  const labels = languageLabels();
  const options = languageOptions();

  const positionChoices: ReadonlyArray<{ value: PositionType; label: string }> = [
    { value: "START", label: m.position_start() },
    book.sourceFormat === "PDF"
      ? { value: "PAGE", label: m.position_page() }
      : { value: "CHAPTER", label: m.position_chapter() },
    { value: "PERCENT", label: m.position_percent() },
  ];

  const positionMax = positionMaxFor({ positionType, book });

  const sameLanguage = bookLanguage === nativeLanguage;

  function handleSubmit(event: SyntheticEvent<HTMLFormElement>) {
    event.preventDefault();
    if (sameLanguage) {
      return;
    }
    onSubmit({
      bookLanguage,
      nativeLanguage,
      level,
      positionType,
      positionValue: positionType === "START" ? null : toPositiveInt(positionValue),
    });
  }

  return (
    <form className="flex flex-col gap-8" onSubmit={handleSubmit}>
      <fieldset className="flex flex-col gap-4">
        <legend className="flex flex-col gap-1 pb-3">
          <span className="text-base font-semibold text-app-text">{m.setup_languages_title()}</span>
          <span className="text-sm text-app-text-muted">{m.setup_languages_description()}</span>
        </legend>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          <Field
            label={m.setup_book_language()}
            htmlFor={`${idPrefix}-book-language`}
            helperText={
              detected == null
                ? m.setup_book_language_undetected()
                : m.setup_book_language_detected()
            }
          >
            <Select
              items={labels}
              value={bookLanguage}
              onValueChange={(value) => {
                if (value != null) {
                  setBookLanguage(value);
                }
              }}
            >
              <SelectTrigger variant="full" id={`${idPrefix}-book-language`}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {options.map((option) => (
                  <SelectItem key={option.code} value={option.code}>
                    <span className="flex items-center gap-2">
                      {option.label}
                      <span className="text-xs text-app-text-subtle">{option.nativeLabel}</span>
                    </span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
          <Field
            label={m.setup_native_language()}
            htmlFor={`${idPrefix}-native-language`}
            helperText={m.setup_native_language_helper()}
            error={sameLanguage ? m.setup_same_language_error() : undefined}
          >
            <Select
              items={labels}
              value={nativeLanguage}
              onValueChange={(value) => {
                if (value != null) {
                  setNativeLanguage(value);
                }
              }}
            >
              <SelectTrigger
                variant="full"
                id={`${idPrefix}-native-language`}
                aria-invalid={sameLanguage}
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {options.map((option) => (
                  <SelectItem key={option.code} value={option.code}>
                    <span className="flex items-center gap-2">
                      {option.label}
                      <span className="text-xs text-app-text-subtle">{option.nativeLabel}</span>
                    </span>
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
        </div>
      </fieldset>

      <fieldset className="flex flex-col gap-4">
        <legend className="flex flex-col gap-1 pb-3">
          <span className="text-base font-semibold text-app-text">{m.setup_level_title()}</span>
          <span className="text-sm text-app-text-muted">{m.setup_level_description()}</span>
        </legend>
        <RadioGroup
          value={level}
          onValueChange={(value) => {
            setLevel(value as LearnerLevel);
          }}
          className="grid grid-cols-1 gap-3 sm:grid-cols-3"
          aria-label={m.setup_level_title()}
        >
          {LEVELS.map((option) => (
            <Radio.Root
              key={option}
              value={option}
              className={clsx(
                "group/level flex cursor-pointer flex-col items-start gap-2 rounded-2xl border p-4 text-start transition-[border-color,background-color,box-shadow] duration-150 ease-(--ease-app-out) focus-visible:ring-2 focus-visible:ring-app-accent/40 focus-visible:outline-none",
                level === option
                  ? "border-app-accent bg-app-accent-soft shadow-(--app-shadow-pressed)"
                  : "border-app-border bg-app-surface hover:border-app-border-strong",
              )}
            >
              <span className="flex w-full items-center justify-between gap-2">
                <span className="text-sm font-semibold text-app-text">
                  {levelLabel({ level: option })}
                </span>
                <span
                  className={clsx(
                    "flex size-4 items-center justify-center rounded-full border transition-colors",
                    level === option
                      ? "border-app-accent bg-app-accent"
                      : "border-app-border-strong",
                  )}
                >
                  <Radio.Indicator className="size-1.5 rounded-full bg-app-on-accent" />
                </span>
              </span>
              <span className="text-xs/relaxed text-app-text-muted">
                {levelDescription({ level: option })}
              </span>
            </Radio.Root>
          ))}
        </RadioGroup>
      </fieldset>

      <fieldset className="flex flex-col gap-4">
        <legend className="flex flex-col gap-1 pb-3">
          <span className="text-base font-semibold text-app-text">{m.setup_position_title()}</span>
          <span className="text-sm text-app-text-muted">{m.setup_position_description()}</span>
        </legend>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <Field label={m.setup_position_type()} htmlFor={`${idPrefix}-position`}>
            <Select
              items={Object.fromEntries(
                positionChoices.map((choice) => [choice.value, choice.label]),
              )}
              value={positionType}
              onValueChange={(value) => {
                if (value != null) {
                  setPositionType(value as PositionType);
                  setPositionValue("");
                }
              }}
            >
              <SelectTrigger variant="full" id={`${idPrefix}-position`}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {positionChoices.map((choice) => (
                  <SelectItem key={choice.value} value={choice.value}>
                    {choice.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </Field>
          {positionType === "START" ? null : (
            <Field
              label={positionValueLabel({ positionType })}
              htmlFor={`${idPrefix}-position-value`}
              helperText={
                positionMax == null ? undefined : m.setup_position_max({ max: positionMax })
              }
            >
              <Input
                id={`${idPrefix}-position-value`}
                type="number"
                inputMode="numeric"
                min={positionType === "PERCENT" ? 0 : 1}
                max={positionMax}
                value={positionValue}
                required
                onChange={(event) => {
                  setPositionValue(event.target.value);
                }}
              />
            </Field>
          )}
        </div>
      </fieldset>

      <div className="flex flex-col gap-3 rounded-2xl bg-app-surface-soft p-4 sm:flex-row sm:items-center sm:justify-between">
        <p className="flex items-center gap-2 text-sm text-app-text-muted">
          <Sparkles className="size-4 text-app-accent" />
          {m.setup_submit_hint()}
        </p>
        <Button type="submit" size="lg" disabled={isSubmitting || sameLanguage}>
          {m.setup_submit()}
          <ArrowRight className={clsx(isRtl && "rotate-180")} />
        </Button>
      </div>
    </form>
  );
}

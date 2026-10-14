import { useEffect, useState } from "react"
import { Camera, CheckCircle, CircleHalf, Prohibit, type Icon } from "@phosphor-icons/react"
import { Radio } from "@base-ui/react/radio"
import { RadioGroup } from "@/components/ui/radio-group"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { VaulDrawer, VaulDrawerContent, VaulDrawerDescription, VaulDrawerTitle } from "@/components/ui/vaul-drawer"
import { cn } from "@/lib/utils"
import { pl } from "@/i18n/pl"
import type { Evidence, FeatureKey } from "@/lib/data"

export interface Option {
  id: string
  label: string
  icon?: Icon
  value: Evidence["value"]
}

const yesNo = (yes: string, no: string): Option[] => [
  { id: "yes", label: yes, icon: CheckCircle, value: true },
  { id: "no", label: no, icon: Prohibit, value: false },
]

export const OPTIONS: Record<FeatureKey, Option[]> = {
  toilet: [
    { id: "yes-platform", label: pl.report.opt.toiletPlatform, icon: CheckCircle, value: true },
    { id: "yes-noplatform", label: pl.report.opt.toiletNoPlatform, icon: CircleHalf, value: true },
    { id: "no", label: pl.report.opt.toiletNo, icon: Prohibit, value: false },
  ],
  door: [],
  entrance: [
    { id: "0", label: "0 cm", value: { steps: 0, cm: 0 } },
    { id: "1-2", label: "1–2 cm", value: { steps: 0, cm: 2 } },
    { id: "3-5", label: "3–5 cm", value: { steps: 0, cm: 5 } },
    { id: ">5", label: "> 5 cm", value: { steps: 1, cm: 8 } },
  ],
  elevator: [
    { id: "yes", label: pl.report.opt.works, icon: CheckCircle, value: true },
    { id: "broken", label: pl.report.opt.broken, icon: CircleHalf, value: false },
    { id: "no", label: pl.report.opt.none, icon: Prohibit, value: false },
  ],
  ramp: yesNo(pl.report.opt.rampYes, pl.report.opt.rampNo),
  surface: [
    { id: "flat", label: pl.report.opt.flat, icon: CheckCircle, value: "równa" },
    { id: "cobble", label: pl.report.opt.cobble, icon: Prohibit, value: "kostka" },
  ],
  rest: yesNo(pl.report.opt.benches, pl.report.opt.none),
  parking: yesNo(pl.report.opt.parkingYes, pl.report.opt.none),
  changing: yesNo(pl.report.opt.yes, pl.report.opt.none),
}

const FEATURES: FeatureKey[] = ["entrance", "door", "elevator", "toilet", "changing", "ramp", "surface", "rest", "parking"]

export function CorrectionDrawer({
  open,
  onOpenChange,
  container,
  placeName,
  fact,
  preselect,
  onSubmit,
}: {
  open: boolean
  onOpenChange: (o: boolean) => void
  container: HTMLElement | null
  placeName: string
  fact: FeatureKey
  preselect?: string
  onSubmit: (fact: FeatureKey, value: Evidence["value"], label: string, comment: string) => void
}) {
  const [feature, setFeature] = useState<FeatureKey>(fact)
  const [value, setValue] = useState<string | null>(preselect ?? null)
  const [cm, setCm] = useState("")
  const [touched, setTouched] = useState(false)
  const [comment, setComment] = useState("")
  useEffect(() => {
    if (open) {
      setFeature(fact)
      setValue(preselect ?? null)
      setCm("")
      setTouched(false)
      setComment("")
    }
  }, [open, fact, preselect])

  const options = OPTIONS[feature]
  const numeric = feature === "door"
  const cmNum = Number(cm)
  const cmValid = cm !== "" && Number.isFinite(cmNum) && cmNum >= 10 && cmNum <= 300
  const chips = !numeric && options.every((o) => !o.icon)
  const canSend = numeric ? cmValid : !!value

  const submit = () => {
    if (numeric) {
      if (!cmValid) return setTouched(true)
      onSubmit(feature, cmNum, `${cmNum} cm`, comment)
      return
    }
    const o = options.find((x) => x.id === value)
    if (o) onSubmit(feature, o.value, o.label, comment)
  }

  return (
    <VaulDrawer open={open} onOpenChange={onOpenChange} container={container}>
      <VaulDrawerContent aria-describedby="corr-desc">
        <form
          className="overflow-y-auto px-4 pt-4 pb-5"
          onSubmit={(e) => {
            e.preventDefault()
            submit()
          }}
        >
          <VaulDrawerDescription id="corr-desc" className="text-body-sm text-muted-foreground">
            {placeName}
          </VaulDrawerDescription>
          <VaulDrawerTitle className="mt-1 font-display text-h2 font-bold">{pl.report.title}</VaulDrawerTitle>

          <p className="mt-4 mb-2 text-body-sm font-semibold" id="which-label">
            {pl.report.which}
          </p>
          <ToggleGroup
            aria-labelledby="which-label"
            value={[feature]}
            onValueChange={(v) => {
              if (!v[0]) return
              setFeature(v[0] as FeatureKey)
              setValue(null)
            }}
            spacing={2}
            className="no-scrollbar -mx-4 w-[calc(100%+32px)] overflow-x-auto px-4 pb-1"
          >
            {FEATURES.map((f) => (
              <ToggleGroupItem
                key={f}
                value={f}
                className="h-10 shrink-0 rounded-full! bg-card px-3.5 text-sm font-semibold ring-1 ring-border hover:bg-muted aria-pressed:bg-primary-container aria-pressed:ring-2 aria-pressed:ring-primary/60 data-pressed:bg-primary-container"
              >
                {pl.feature[f]}
              </ToggleGroupItem>
            ))}
          </ToggleGroup>

          <p className="mt-4 mb-2 text-body-sm font-semibold" id="value-label">
            {pl.report.trueValue}
          </p>
          {numeric ? (
            <div>
              <label htmlFor="width-cm" className="sr-only">
                {pl.report.widthLabel}
              </label>
              <div className="flex items-center gap-2">
                <Input
                  id="width-cm"
                  inputMode="numeric"
                  type="number"
                  min={10}
                  max={300}
                  value={cm}
                  onChange={(e) => setCm(e.target.value)}
                  onBlur={() => setTouched(true)}
                  aria-invalid={touched && !cmValid}
                  aria-describedby="width-hint"
                  placeholder="90"
                  className="h-14 flex-1 rounded-2xl px-4 font-display text-[22px] font-extrabold"
                />
                <span className="text-body font-semibold text-muted-foreground">{pl.thresholds.cm}</span>
              </div>
              <p id="width-hint" className={cn("mt-1.5 text-caption", touched && !cmValid ? "font-semibold text-status-barrier" : "text-muted-foreground")}>
                {touched && !cmValid ? pl.report.widthError : pl.report.widthHint}
              </p>
            </div>
          ) : (
            <RadioGroup
              aria-labelledby="value-label"
              value={value}
              onValueChange={(v) => setValue(v as string)}
              className={cn(chips ? "grid-cols-2 gap-2.5" : "gap-2.5")}
            >
              {options.map((o) => {
                const I = o.icon
                const on = value === o.id
                return (
                  <Radio.Root
                    key={o.id}
                    value={o.id}
                    className={cn(
                      "press flex h-14 w-full items-center gap-3 rounded-2xl bg-surface-raised px-4 text-left text-body font-semibold ring-1 ring-border-strong/40 outline-none focus-visible:outline-3 focus-visible:outline-offset-2 focus-visible:outline-ring",
                      on && "bg-primary-container ring-2 ring-primary",
                      chips && "justify-center rounded-full font-display text-[17px] font-extrabold",
                    )}
                  >
                    {I ? <I weight={on ? "fill" : "regular"} className={cn("size-6 shrink-0", on ? "text-primary" : "text-muted-foreground")} aria-hidden /> : null}
                    <span className={cn(!chips && "flex-1")}>{o.label}</span>
                    {!chips ? (
                      <span aria-hidden className={cn("grid size-6 place-items-center rounded-full ring-2 transition-colors", on ? "bg-primary ring-primary" : "ring-border-strong/60")}>
                        {on ? <span className="size-2.5 rounded-full bg-primary-foreground" /> : null}
                      </span>
                    ) : null}
                  </Radio.Root>
                )
              })}
            </RadioGroup>
          )}

          <label htmlFor="report-comment" className="mt-4 mb-2 block text-body-sm font-semibold">
            {pl.report.comment}
          </label>
          <Textarea
            id="report-comment"
            value={comment}
            onChange={(e) => setComment(e.target.value)}
            placeholder={pl.report.commentPlaceholder}
            rows={2}
            className="min-h-16 rounded-2xl px-4 py-3 text-body-sm"
          />

          <label className="press mt-3 inline-flex h-12 cursor-pointer items-center gap-2 rounded-full px-3 text-body-sm font-semibold text-foreground/80 focus-within:outline-3 focus-within:outline-ring hover:bg-muted">
            <Camera className="size-5" aria-hidden />
            {pl.report.photo}
            <input type="file" accept="image/*" className="sr-only" />
          </label>

          <Button type="submit" size="lg" className="mt-2 w-full" disabled={!canSend && !numeric}>
            {pl.report.send}
          </Button>
          <p className="mt-3 text-center text-caption text-muted-foreground">{pl.report.noAccount}</p>
        </form>
      </VaulDrawerContent>
    </VaulDrawer>
  )
}

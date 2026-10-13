# "Shoot Your Shot" Offer Wizard (3.2–3.5) Implementation Plan

## Overview

Build the 4-step "Shoot Your Shot" offer wizard that lets VCs submit investment offers to founders. The wizard lives inside a Modal (desktop) / BottomSheet (mobile) and walks through: investment amount slider, deal structure selection, meeting proposal, and review/submit. On submit, inserts an offer row in Supabase and shows a satirical toast via sonner.

## Current State Analysis

### What Already Exists (Ready to Use)
- **Database**: `offers` table with full schema, RLS policies, `accept_offer`/`decline_offer` functions — `supabase/migrations/20260329000001_create_offers.sql`
- **TypeScript types**: Row, Insert, Update types — `src/types/database.types.ts:314-415`
- **Constants**: `SLIDER_LABELS` (6 satirical thresholds) — `src/lib/constants.ts:3-10`
- **Constants**: `DEAL_TYPES` (6 deal type strings) — `src/lib/constants.ts:12-19`
- **Types**: `DealType` union, `OfferStatus` — `src/types/common.types.ts:5-17`
- **CSS**: `.gradient-slider` (muted→gold→rose gradient) — `src/app.css:184-192`
- **CSS**: `.glow-gold`, `.glow-rose` utility classes — `src/app.css:195-204`
- **Component**: `StepIndicator` (numbered circles, connecting lines, pulse animation) — `src/components/onboarding/step-indicator.tsx`
- **Pattern**: Wizard orchestration (FormProvider + AnimatePresence + per-step trigger) — `src/components/onboarding/founder-onboarding.tsx`
- **Pattern**: Selectable tile grid (gold border + glow on selection) — `src/components/auth/role-selector.tsx:69-99`
- **Pattern**: `z.discriminatedUnion()` — `src/lib/validations/post-schema.ts:59-63`
- **Pattern**: Numeric input (strip non-digits → parseInt → setValue, $ prefix) — `src/components/onboarding/founder-step-3.tsx:14-25`
- **Pattern**: Modal/BottomSheet responsive split — `src/components/feed/post-create-modal.tsx:238-259`
- **Pattern**: UIStore `openModal`/`closeModal` — `src/stores/ui-store.ts`
- **Libraries**: @base-ui/react (slider primitives), framer-motion v12, react-hook-form + zod/v4 + @hookform/resolvers, lucide-react, date-fns v4

### What's Missing (Needs Building)
- `src/lib/validations/offer-schema.ts` — Zod discriminated union for offer wizard
- `src/lib/format.ts` — Extract `formatCurrency` from `profile-header.tsx` for reuse
- `src/components/offers/investment-slider.tsx` — Slider wrapping @base-ui/react/slider
- `src/components/offers/deal-type-tile.tsx` — Selectable tile grid for deal types
- `src/components/offers/shoot-your-shot/step-investment.tsx` — Step 1 form wrapper
- `src/components/offers/shoot-your-shot/step-deal-structure.tsx` — Step 2 form wrapper
- `src/components/offers/shoot-your-shot/step-meeting.tsx` — Step 3 meeting proposal
- `src/components/offers/shoot-your-shot/step-review.tsx` — Step 4 review summary
- `src/components/offers/shoot-your-shot/offer-wizard.tsx` — Wizard orchestrator + modal
- Toast library (sonner) + `<Toaster />` in root layout

### Key Discoveries
- `formatCurrency` in `profile-header.tsx:21-25` is not exported — needs extraction to shared util
- `isMobile` check is repeated inline (`window.innerWidth < 768`) — follow existing pattern, don't abstract
- Slider CSS `.gradient-slider` already exists with exact gradient spec'd
- DB columns `meeting_slot_1/2/3` are `timestamptz` (nullable) — maps to `datetime-local` input value format
- `SLIDER_LABELS` thresholds go from $10K to $1M — slider will need to interpolate between these for label display
- DB `amount` is `bigint`, check constraint `amount > 0` — Zod schema must enforce positive integer

## Desired End State

A VC user can open the "Shoot Your Shot" modal on any founder's profile, complete a 4-step wizard (investment amount → deal structure → optional meeting proposal → review), and submit an offer that gets inserted into the `offers` table. A satirical toast confirms submission. The wizard validates all fields per step, shows smooth slide animations between steps, and supports both desktop (modal) and mobile (bottom sheet).

### Verification
1. Wizard opens in modal (desktop) / bottom sheet (mobile)
2. Slider moves from $0 to founder's `target_raise`, displays satirical labels, and gradient track updates
3. "Go further" input allows amounts beyond slider range up to $100M
4. All 6 deal type tiles render with icons, single-select with gold glow
5. Selecting a deal type reveals the correct conditional fields with animation
6. Meeting step shows up to 3 datetime-local slots with add/remove, location, personal note
7. "Skip this step" link on meeting step advances to review without requiring any meeting fields
8. Review step displays all entered data in a summary card
9. "Send the Term Sheet" CTA inserts row in Supabase `offers` table with status 'pending'
10. Sonner toast shows satirical confirmation message on success
11. Error state shows if insert fails (e.g., duplicate pending offer)

## What We're NOT Doing

- **3.6 (Tab Integration)**: Wiring wizard into the founder profile's money tab — separate task
- **3.7-3.10 (Inbox, Accept/Decline, Sent Offers, Realtime)**: All downstream features
- **Fetching founder data inside wizard**: Parent component passes `founderId` + `targetRaise` as props
- **Custom date picker library**: Using native `<input type="datetime-local">`
- **useFieldArray**: Managing meeting slots with simple array state (matches existing pattern in `industry-multi-select.tsx`)
- **Abstracting isMobile**: Following existing inline check pattern

## Implementation Approach

Follow the existing founder-onboarding wizard pattern exactly: `FormProvider` wrapping the entire form, `useFormContext` in each step component, per-step field validation via `form.trigger(fields)`, `AnimatePresence` for slide transitions, and `StepIndicator` for progress.

The Zod schema uses `z.discriminatedUnion('dealType', [...])` to validate different fields per deal type — the same pattern as `post-schema.ts`. A single flat form shape covers all 4 steps, with `OFFER_STEP_FIELDS` mapping step index → field names for partial validation.

The modal wrapper follows the `PostCreateModal` pattern: `isMobile ? BottomSheet : Modal`, receiving `open`/`onOpenChange` props.

---

## Phase 1: Foundation

### Overview
Install sonner, extract `formatCurrency`, create the Zod offer schema, and set up the directory structure. This phase produces no visible UI but unblocks all subsequent phases.

### Changes Required:

#### 1. Install sonner
```bash
pnpm add sonner
```

#### 2. Add `<Toaster />` to root layout
**File**: `src/components/layout/root-layout.tsx` (or wherever the app shell lives)
**Changes**: Import and render `<Toaster />` at the root

```tsx
import { Toaster } from 'sonner'

// Inside the layout JSX, add near the end:
<Toaster
  theme="dark"
  position="bottom-right"
  toastOptions={{
    style: {
      background: 'var(--color-surface)',
      border: '1px solid var(--color-border)',
      color: 'var(--color-text-primary)',
    },
  }}
/>
```

#### 3. Extract `formatCurrency` to shared utility
**File**: `src/lib/format.ts` (new)
**Changes**: Move helper from `profile-header.tsx:21-25` to a shared module

```ts
export function formatCurrency(amount: number): string {
  if (amount >= 1_000_000) return `$${(amount / 1_000_000).toFixed(1)}M`
  if (amount >= 1_000) return `$${(amount / 1_000).toFixed(0)}K`
  return `$${amount}`
}
```

Then update `profile-header.tsx` to import from `@/lib/format` instead of using local function.

#### 4. Create offer Zod schema with discriminated union
**File**: `src/lib/validations/offer-schema.ts` (new)
**Changes**: Full offer wizard validation schema

```ts
import { z } from 'zod/v4'

// ── Step 1: Investment Amount ──

export const stepInvestmentSchema = z.object({
  amount: z
    .number({ error: 'How much are you putting in?' })
    .int('Whole dollars only')
    .min(1, "You can't invest nothing")
    .max(100_000_000, 'Even we have limits — $100M max'),
})

// ── Step 2: Deal Structure (discriminated union) ──

const equitySchema = z.object({
  dealType: z.literal('equity'),
  equity_percent: z
    .number({ error: 'How much equity?' })
    .min(0.01, 'Must be greater than 0%')
    .max(100, 'Cannot exceed 100%'),
})

const cashEquityHybridSchema = z.object({
  dealType: z.literal('cash-equity-hybrid'),
  cash_amount: z
    .number({ error: 'Cash portion required' })
    .int()
    .min(1, 'Cash amount must be positive'),
  equity_percent: z
    .number({ error: 'Equity portion required' })
    .min(0.01, 'Must be greater than 0%')
    .max(100, 'Cannot exceed 100%'),
})

const royaltySchema = z.object({
  dealType: z.literal('royalty'),
  revenue_share_percent: z
    .number({ error: 'Revenue share % required' })
    .min(0.01, 'Must be greater than 0%')
    .max(100, 'Cannot exceed 100%'),
  repayment_cap: z
    .number({ error: 'Repayment cap required' })
    .int()
    .min(1, 'Repayment cap must be positive'),
})

const contingentSchema = z.object({
  dealType: z.literal('contingent'),
  milestone_description: z
    .string()
    .min(1, 'Describe the milestone')
    .max(500, 'Keep it under 500 characters'),
  trigger_condition: z
    .string()
    .min(1, 'What triggers payout?')
    .max(500, 'Keep it under 500 characters'),
})

const safeSchema = z.object({
  dealType: z.literal('safe'),
  valuation_cap: z
    .number({ error: 'Valuation cap required' })
    .int()
    .min(1, 'Valuation cap must be positive'),
  discount_rate: z
    .number({ error: 'Discount rate required' })
    .min(0, 'Cannot be negative')
    .max(100, 'Cannot exceed 100%'),
})

const convertibleSchema = z.object({
  dealType: z.literal('convertible'),
  valuation_cap: z
    .number({ error: 'Valuation cap required' })
    .int()
    .min(1, 'Valuation cap must be positive'),
  discount_rate: z
    .number({ error: 'Discount rate required' })
    .min(0, 'Cannot be negative')
    .max(100, 'Cannot exceed 100%'),
})

export const stepDealStructureSchema = z.discriminatedUnion('dealType', [
  equitySchema,
  cashEquityHybridSchema,
  royaltySchema,
  contingentSchema,
  safeSchema,
  convertibleSchema,
])

// ── Step 3: Meeting Proposal (all optional) ──

export const stepMeetingSchema = z.object({
  meetingSlots: z
    .array(z.string().min(1))
    .max(3, 'Maximum 3 time slots')
    .default([]),
  meetingLocation: z
    .string()
    .max(200, 'Keep location under 200 characters')
    .optional()
    .or(z.literal('')),
  personalNote: z
    .string()
    .max(1000, 'Keep your DM slide under 1000 characters')
    .optional()
    .or(z.literal('')),
})

// ── Combined form type ──

// Note: The full form shape is a flat intersection used by react-hook-form.
// Step validation uses form.trigger(OFFER_STEP_FIELDS[step]) to validate
// only the current step's fields. The discriminated union for dealType
// is validated in step 2 via a custom trigger approach.

export type OfferFormData = {
  // Step 1
  amount: number
  // Step 2
  dealType: 'equity' | 'cash-equity-hybrid' | 'royalty' | 'contingent' | 'safe' | 'convertible'
  equity_percent?: number
  cash_amount?: number
  repayment_cap?: number
  revenue_share_percent?: number
  milestone_description?: string
  trigger_condition?: string
  valuation_cap?: number
  discount_rate?: number
  // Step 3
  meetingSlots: string[]
  meetingLocation?: string
  personalNote?: string
}

// Fields to validate per step (for form.trigger())
export const OFFER_STEP_FIELDS: (keyof OfferFormData)[][] = [
  ['amount'],
  ['dealType'], // Deal type fields validated via stepDealStructureSchema manually
  ['meetingSlots', 'meetingLocation', 'personalNote'],
  [], // Review step — no new fields
]
```

**Design note on step 2 validation**: Since `react-hook-form`'s `trigger()` works on flat field names but we need discriminated union validation, the `goNext` function for step 2 will manually parse the current form values against `stepDealStructureSchema` and set errors accordingly, rather than relying on the resolver alone. This matches how the app handles content-type-dependent validation in `post-create-modal.tsx`.

#### 5. Create directory structure
```
src/components/offers/
  investment-slider.tsx
  deal-type-tile.tsx
  shoot-your-shot/
    offer-wizard.tsx
    step-investment.tsx
    step-deal-structure.tsx
    step-meeting.tsx
    step-review.tsx
```

---

## Phase 2: Investment Slider (Step 1)

### Overview
Build the investment slider component and its step wrapper. The slider is bounded by the founder's `target_raise`, displays satirical labels at thresholds, uses the pre-built gradient CSS, and offers a "go further" manual input for amounts beyond the slider range.

### Changes Required:

#### 1. Investment Slider component
**File**: `src/components/offers/investment-slider.tsx` (new)
**Changes**: Custom slider wrapping @base-ui/react/slider primitives

```tsx
import { useState } from 'react'
import * as Slider from '@base-ui-components/react/slider'
import { motion, AnimatePresence } from 'framer-motion'

import { SLIDER_LABELS } from '@/lib/constants'
import { formatCurrency } from '@/lib/format'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'

interface InvestmentSliderProps {
  value: number
  onChange: (value: number) => void
  maxAmount: number // founder's target_raise
}

export function InvestmentSlider({ value, onChange, maxAmount }: InvestmentSliderProps) {
  const [manualMode, setManualMode] = useState(false)

  // Find the current satirical label based on value
  function getLabel(amount: number): string {
    // Walk SLIDER_LABELS from highest to lowest, return first match
    for (let i = SLIDER_LABELS.length - 1; i >= 0; i--) {
      if (amount >= SLIDER_LABELS[i].amount) return SLIDER_LABELS[i].label
    }
    return ''
  }

  function handleManualChange(e: React.ChangeEvent<HTMLInputElement>) {
    const raw = e.target.value.replace(/[^0-9]/g, '')
    const num = raw ? parseInt(raw, 10) : 0
    onChange(num)
  }

  return (
    <div className="flex flex-col gap-6">
      {/* Current amount display */}
      <div className="text-center">
        <p className="text-3xl font-bold text-gold">
          {formatCurrency(value)}
        </p>
        <AnimatePresence mode="wait">
          {value > 0 && (
            <motion.p
              key={getLabel(value)}
              initial={{ opacity: 0, y: 4 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              className="mt-1 text-sm italic text-text-secondary"
            >
              {getLabel(value)}
            </motion.p>
          )}
        </AnimatePresence>
      </div>

      {/* Slider (hidden when in manual mode) */}
      {!manualMode && (
        <Slider.Root
          value={value}
          onValueChange={(val) => onChange(val as number)}
          min={0}
          max={maxAmount}
          step={Math.max(1000, Math.round(maxAmount / 100))}
          className="relative flex w-full touch-none items-center py-4"
        >
          <Slider.Control className="flex w-full items-center">
            <Slider.Track className="relative h-2 w-full rounded-full bg-surface-raised">
              <Slider.Indicator className="absolute h-full rounded-full gradient-slider" />
              <Slider.Thumb
                className="block h-5 w-5 rounded-full border-2 border-gold bg-surface shadow-md
                  focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-gold/50
                  active:scale-110 transition-transform"
              />
            </Slider.Track>
          </Slider.Control>
        </Slider.Root>
      )}

      {/* Slider range labels */}
      {!manualMode && (
        <div className="flex justify-between text-xs text-text-muted">
          <span>$0</span>
          <span>{formatCurrency(maxAmount)}</span>
        </div>
      )}

      {/* "Go further" toggle / manual input */}
      {!manualMode ? (
        <button
          type="button"
          onClick={() => setManualMode(true)}
          className="text-sm text-gold hover:text-gold-light transition-colors"
        >
          Want to go further? Type your own amount...
        </button>
      ) : (
        <div className="flex flex-col gap-2">
          <div className="relative">
            <span className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-text-muted">
              $
            </span>
            <Input
              type="text"
              inputMode="numeric"
              placeholder="1,000,000"
              value={value ? value.toLocaleString('en-US') : ''}
              onChange={handleManualChange}
              className="h-14 pl-7 text-center text-2xl font-bold tracking-wide bg-surface"
              autoFocus
            />
          </div>
          <button
            type="button"
            onClick={() => {
              // Clamp value back to slider range when switching back
              if (value > maxAmount) onChange(maxAmount)
              setManualMode(false)
            }}
            className="text-sm text-text-muted hover:text-text-secondary transition-colors"
          >
            Back to slider
          </button>
        </div>
      )}
    </div>
  )
}
```

**Key decisions**:
- Slider `step` is dynamic: `max(1000, target_raise / 100)` — gives ~100 stops regardless of raise size
- `.gradient-slider` CSS class applied to `Slider.Indicator` — uses the pre-built muted→gold→rose gradient
- Satirical label transitions use `AnimatePresence` with fade/slide for smooth swaps
- Manual input follows the exact `founder-step-3.tsx` pattern: strip non-digits, parseInt, toLocaleString display
- "Back to slider" clamps value to `maxAmount` if it exceeds slider range

#### 2. Step Investment wrapper
**File**: `src/components/offers/shoot-your-shot/step-investment.tsx` (new)
**Changes**: Wraps `InvestmentSlider` with `useFormContext`

```tsx
import { useFormContext } from 'react-hook-form'

import { InvestmentSlider } from '@/components/offers/investment-slider'
import type { OfferFormData } from '@/lib/validations/offer-schema'

interface StepInvestmentProps {
  targetRaise: number
}

export function StepInvestment({ targetRaise }: StepInvestmentProps) {
  const { setValue, watch, formState: { errors } } = useFormContext<OfferFormData>()

  const amount = watch('amount')

  return (
    <div className="flex flex-col gap-4">
      <InvestmentSlider
        value={amount}
        onChange={(val) => setValue('amount', val, { shouldValidate: true })}
        maxAmount={targetRaise}
      />
      {errors.amount && (
        <span className="text-center text-xs text-rose">
          {errors.amount.message}
        </span>
      )}
    </div>
  )
}
```

---

## Phase 3: Deal Structure Selection (Step 2)

### Overview
Build the deal type tile grid and conditional fields. Selecting a deal type reveals its required fields with slide animation. Uses the `role-selector.tsx` tile pattern and `post-schema.ts` discriminated union pattern.

### Changes Required:

#### 1. Deal type icon + description mapping
**File**: `src/components/offers/deal-type-tile.tsx` (new)
**Changes**: Icon mapping and selectable tile grid

Lucide icon choices:
| Deal Type | Icon | Rationale |
|---|---|---|
| equity | `PieChart` | Ownership slice |
| cash-equity-hybrid | `Coins` | Cash + equity blend |
| royalty | `Repeat` | Recurring revenue share |
| contingent | `Target` | Milestone-triggered |
| safe | `Shield` | Safe = safety |
| convertible | `ArrowRightLeft` | Converts to equity |

```tsx
import { PieChart, Coins, Repeat, Target, Shield, ArrowRightLeft } from 'lucide-react'
import { motion } from 'framer-motion'

import { cn } from '@/lib/utils'
import type { DealType } from '@/types/common.types'

const DEAL_TYPE_CONFIG: {
  value: DealType
  label: string
  description: string
  icon: typeof PieChart
}[] = [
  { value: 'equity', label: 'Equity', description: 'Buy a piece of the pie', icon: PieChart },
  { value: 'cash-equity-hybrid', label: 'Cash + Equity', description: 'Best of both worlds', icon: Coins },
  { value: 'royalty', label: 'Royalty', description: 'Revenue share deal', icon: Repeat },
  { value: 'contingent', label: 'Contingent', description: 'Pay on milestones', icon: Target },
  { value: 'safe', label: 'SAFE', description: 'Simple future equity', icon: Shield },
  { value: 'convertible', label: 'Convertible', description: 'Debt that flips', icon: ArrowRightLeft },
]

interface DealTypeTileGridProps {
  selected: DealType | null
  onSelect: (type: DealType) => void
}

export function DealTypeTileGrid({ selected, onSelect }: DealTypeTileGridProps) {
  return (
    <div className="grid grid-cols-2 gap-3">
      {DEAL_TYPE_CONFIG.map((deal) => {
        const Icon = deal.icon
        const isSelected = selected === deal.value
        return (
          <motion.button
            key={deal.value}
            type="button"
            onClick={() => onSelect(deal.value)}
            whileTap={{ scale: 0.97 }}
            className={cn(
              'flex flex-col items-center gap-2 rounded-xl border-2 bg-surface p-4 text-center transition-all hover:bg-surface-hover',
              isSelected
                ? 'border-gold glow-gold'
                : 'border-border hover:border-gold/40',
            )}
          >
            <div
              className={cn(
                'flex h-10 w-10 items-center justify-center rounded-full transition-colors',
                isSelected
                  ? 'bg-gold/20 text-gold'
                  : 'bg-surface-raised text-text-muted',
              )}
            >
              <Icon className="h-5 w-5" />
            </div>
            <h4 className="text-sm font-semibold">{deal.label}</h4>
            <p className="text-xs text-text-secondary">{deal.description}</p>
          </motion.button>
        )
      })}
    </div>
  )
}

export { DEAL_TYPE_CONFIG }
```

#### 2. Step Deal Structure with conditional fields
**File**: `src/components/offers/shoot-your-shot/step-deal-structure.tsx` (new)
**Changes**: Tile grid + conditional fields per deal type

```tsx
import { useFormContext } from 'react-hook-form'
import { AnimatePresence, motion } from 'framer-motion'

import { DealTypeTileGrid } from '@/components/offers/deal-type-tile'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import type { OfferFormData } from '@/lib/validations/offer-schema'
import type { DealType } from '@/types/common.types'

export function StepDealStructure() {
  const { setValue, watch, formState: { errors } } = useFormContext<OfferFormData>()
  const dealType = watch('dealType')

  function handleSelect(type: DealType) {
    setValue('dealType', type, { shouldValidate: true })
    // Clear previous deal-type-specific fields
    setValue('equity_percent', undefined)
    setValue('cash_amount', undefined)
    setValue('repayment_cap', undefined)
    setValue('revenue_share_percent', undefined)
    setValue('milestone_description', undefined)
    setValue('trigger_condition', undefined)
    setValue('valuation_cap', undefined)
    setValue('discount_rate', undefined)
  }

  return (
    <div className="flex flex-col gap-5">
      <DealTypeTileGrid selected={dealType} onSelect={handleSelect} />

      {/* Error for no selection */}
      {errors.dealType && (
        <span className="text-xs text-rose">{errors.dealType.message}</span>
      )}

      {/* Conditional fields per deal type */}
      <AnimatePresence mode="wait">
        {dealType && (
          <motion.div
            key={dealType}
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            transition={{ duration: 0.3 }}
            className="flex flex-col gap-4 overflow-hidden"
          >
            {/* EQUITY */}
            {dealType === 'equity' && (
              <NumericField
                name="equity_percent"
                label="Equity %"
                placeholder="e.g. 10"
                suffix="%"
              />
            )}

            {/* CASH-EQUITY HYBRID */}
            {dealType === 'cash-equity-hybrid' && (
              <>
                <CurrencyField name="cash_amount" label="Cash Amount" placeholder="250,000" />
                <NumericField name="equity_percent" label="Equity %" placeholder="e.g. 5" suffix="%" />
              </>
            )}

            {/* ROYALTY */}
            {dealType === 'royalty' && (
              <>
                <NumericField
                  name="revenue_share_percent"
                  label="Revenue Share %"
                  placeholder="e.g. 3"
                  suffix="%"
                />
                <CurrencyField name="repayment_cap" label="Repayment Cap" placeholder="500,000" />
              </>
            )}

            {/* CONTINGENT */}
            {dealType === 'contingent' && (
              <>
                <TextField name="milestone_description" label="Milestone" placeholder="e.g. Reach 10K MAU" />
                <TextField name="trigger_condition" label="Trigger Condition" placeholder="e.g. Verified by analytics dashboard" />
              </>
            )}

            {/* SAFE */}
            {dealType === 'safe' && (
              <>
                <CurrencyField name="valuation_cap" label="Valuation Cap" placeholder="5,000,000" />
                <NumericField name="discount_rate" label="Discount Rate %" placeholder="e.g. 20" suffix="%" />
              </>
            )}

            {/* CONVERTIBLE */}
            {dealType === 'convertible' && (
              <>
                <CurrencyField name="valuation_cap" label="Valuation Cap" placeholder="5,000,000" />
                <NumericField name="discount_rate" label="Discount Rate %" placeholder="e.g. 20" suffix="%" />
              </>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  )
}

// ── Reusable field helpers (local to this file) ──

function NumericField({ name, label, placeholder, suffix }: {
  name: keyof OfferFormData
  label: string
  placeholder: string
  suffix?: string
}) {
  const { register, formState: { errors } } = useFormContext<OfferFormData>()
  const error = errors[name]

  return (
    <div className="flex flex-col gap-1.5">
      <Label>{label}</Label>
      <div className="relative">
        <Input
          {...register(name, { valueAsNumber: true })}
          type="number"
          placeholder={placeholder}
          className="h-11 bg-surface"
        />
        {suffix && (
          <span className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 text-text-muted">
            {suffix}
          </span>
        )}
      </div>
      {error && <span className="text-xs text-rose">{error.message}</span>}
    </div>
  )
}

function CurrencyField({ name, label, placeholder }: {
  name: keyof OfferFormData
  label: string
  placeholder: string
}) {
  const { setValue, watch, formState: { errors } } = useFormContext<OfferFormData>()
  const value = watch(name) as number | undefined
  const error = errors[name]

  function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
    const raw = e.target.value.replace(/[^0-9]/g, '')
    const num = raw ? parseInt(raw, 10) : 0
    setValue(name, num, { shouldValidate: true })
  }

  return (
    <div className="flex flex-col gap-1.5">
      <Label>{label}</Label>
      <div className="relative">
        <span className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-text-muted">$</span>
        <Input
          type="text"
          inputMode="numeric"
          placeholder={placeholder}
          value={value ? value.toLocaleString('en-US') : ''}
          onChange={handleChange}
          className="h-11 bg-surface pl-7"
        />
      </div>
      {error && <span className="text-xs text-rose">{error.message}</span>}
    </div>
  )
}

function TextField({ name, label, placeholder }: {
  name: keyof OfferFormData
  label: string
  placeholder: string
}) {
  const { register, formState: { errors } } = useFormContext<OfferFormData>()
  const error = errors[name]

  return (
    <div className="flex flex-col gap-1.5">
      <Label>{label}</Label>
      <Textarea
        {...register(name)}
        placeholder={placeholder}
        rows={2}
        className="resize-none bg-surface"
      />
      {error && <span className="text-xs text-rose">{error.message}</span>}
    </div>
  )
}
```

**Key decisions**:
- Clear all deal-specific fields when switching deal types (prevents stale data from a previous selection)
- Conditional fields use `AnimatePresence` with `mode="wait"` and height animation for smooth reveal
- `CurrencyField` follows the same strip-non-digits pattern from `founder-step-3.tsx`
- Field helpers are local to this file (not abstracted to ui/ — only used here)

---

## Phase 4: Meeting Proposal (Step 3)

### Overview
Build the meeting proposal step with up to 3 native datetime-local inputs (add/remove), a location field, and a personal note textarea. Includes a "Skip this step" link. All fields are optional.

### Changes Required:

#### 1. Step Meeting component
**File**: `src/components/offers/shoot-your-shot/step-meeting.tsx` (new)

```tsx
import { useFormContext } from 'react-hook-form'
import { Calendar, MapPin, MessageSquare, Plus, Trash2 } from 'lucide-react'
import { motion } from 'framer-motion'

import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import type { OfferFormData } from '@/lib/validations/offer-schema'

interface StepMeetingProps {
  onSkip: () => void
}

export function StepMeeting({ onSkip }: StepMeetingProps) {
  const { setValue, watch, formState: { errors } } = useFormContext<OfferFormData>()

  const meetingSlots = watch('meetingSlots') || []
  const personalNote = watch('personalNote') || ''

  function addSlot() {
    if (meetingSlots.length < 3) {
      setValue('meetingSlots', [...meetingSlots, ''], { shouldValidate: true })
    }
  }

  function removeSlot(index: number) {
    setValue(
      'meetingSlots',
      meetingSlots.filter((_: string, i: number) => i !== index),
      { shouldValidate: true },
    )
  }

  function updateSlot(index: number, value: string) {
    const updated = [...meetingSlots]
    updated[index] = value
    setValue('meetingSlots', updated, { shouldValidate: true })
  }

  return (
    <div className="flex flex-col gap-5">
      {/* Time slots */}
      <div className="flex flex-col gap-3">
        <div className="flex items-center justify-between">
          <Label className="flex items-center gap-1.5">
            <Calendar className="h-4 w-4 text-text-muted" />
            Propose a Meeting
          </Label>
          <span className="text-xs text-text-muted">(optional)</span>
        </div>

        {meetingSlots.map((slot: string, i: number) => (
          <div key={i} className="flex items-center gap-2">
            <Input
              type="datetime-local"
              value={slot}
              onChange={(e) => updateSlot(i, e.target.value)}
              className="h-11 flex-1 bg-surface"
            />
            <button
              type="button"
              onClick={() => removeSlot(i)}
              className="flex h-11 w-11 shrink-0 items-center justify-center rounded-lg
                border border-border text-text-muted transition-colors
                hover:border-rose/40 hover:text-rose"
            >
              <Trash2 className="h-4 w-4" />
            </button>
          </div>
        ))}

        {meetingSlots.length < 3 && (
          <button
            type="button"
            onClick={addSlot}
            className="flex items-center gap-1.5 text-sm text-gold hover:text-gold-light transition-colors"
          >
            <Plus className="h-4 w-4" />
            Add time slot
          </button>
        )}
      </div>

      {/* Location */}
      <div className="flex flex-col gap-1.5">
        <Label className="flex items-center gap-1.5">
          <MapPin className="h-4 w-4 text-text-muted" />
          Location
          <span className="text-xs text-text-muted">(optional)</span>
        </Label>
        <Input
          {.../* register('meetingLocation') */}
          placeholder="Coffee shop, Zoom link, your yacht..."
          className="h-11 bg-surface"
        />
      </div>

      {/* Personal note */}
      <div className="flex flex-col gap-1.5">
        <Label className="flex items-center gap-1.5">
          <MessageSquare className="h-4 w-4 text-text-muted" />
          The DM Slide
          <span className="text-xs text-text-muted">(optional)</span>
        </Label>
        <Textarea
          {.../* register('personalNote') */}
          placeholder="Tell them why you're sliding in..."
          rows={3}
          className="resize-none bg-surface"
        />
        <div className="text-right text-xs text-text-muted">
          {personalNote.length}/1000
        </div>
      </div>

      {/* Skip link */}
      <button
        type="button"
        onClick={onSkip}
        className="text-sm text-text-muted hover:text-text-secondary transition-colors"
      >
        Skip this step — just send the money
      </button>
    </div>
  )
}
```

**Note**: The `{.../* register() */}` comments above are placeholders to show intent — actual implementation will use proper `register('meetingLocation')` and `register('personalNote')` calls without the comment wrapper.

**Key decisions**:
- Meeting slots managed as a string array via `watch`/`setValue` (matches `industry-multi-select.tsx` pattern, no `useFieldArray`)
- Slots start empty — user taps "Add time slot" to add the first one (0 slots is valid = skip)
- `datetime-local` inputs respect the OS dark mode and are functional without extra styling
- Character counter on personal note (matches `vc-step-2.tsx` pattern)
- "Skip this step" calls `onSkip` which triggers `goNext` in the parent wizard without validation
- If a slot is added but left empty, empty strings are filtered out before submission

---

## Phase 5: Review & Wizard Orchestration (Step 4)

### Overview
Build the review summary card and the wizard orchestrator that manages all 4 steps. The wizard handles FormProvider, StepIndicator, slide animations, per-step validation, Supabase insert, and sonner toast.

### Changes Required:

#### 1. Step Review summary card
**File**: `src/components/offers/shoot-your-shot/step-review.tsx` (new)

```tsx
import { useFormContext } from 'react-hook-form'
import { Check } from 'lucide-react'

import { formatCurrency } from '@/lib/format'
import { DEAL_TYPE_CONFIG } from '@/components/offers/deal-type-tile'
import type { OfferFormData } from '@/lib/validations/offer-schema'

export function StepReview() {
  const { watch } = useFormContext<OfferFormData>()
  const data = watch()

  const dealConfig = DEAL_TYPE_CONFIG.find((d) => d.value === data.dealType)
  const filledSlots = (data.meetingSlots || []).filter(Boolean)

  return (
    <div className="flex flex-col gap-4">
      <p className="text-sm text-text-secondary">
        Look it over before you shoot your shot.
      </p>

      {/* Investment Amount */}
      <ReviewSection title="Investment">
        <p className="text-2xl font-bold text-gold">{formatCurrency(data.amount)}</p>
      </ReviewSection>

      {/* Deal Structure */}
      <ReviewSection title="Deal Type">
        <div className="flex items-center gap-2">
          {dealConfig && (
            <>
              <dealConfig.icon className="h-4 w-4 text-gold" />
              <span className="font-medium">{dealConfig.label}</span>
            </>
          )}
        </div>
        {/* Deal-specific details */}
        <div className="mt-2 flex flex-col gap-1 text-sm text-text-secondary">
          {data.equity_percent != null && <Detail label="Equity" value={`${data.equity_percent}%`} />}
          {data.cash_amount != null && <Detail label="Cash" value={formatCurrency(data.cash_amount)} />}
          {data.revenue_share_percent != null && <Detail label="Rev Share" value={`${data.revenue_share_percent}%`} />}
          {data.repayment_cap != null && <Detail label="Repayment Cap" value={formatCurrency(data.repayment_cap)} />}
          {data.valuation_cap != null && <Detail label="Val Cap" value={formatCurrency(data.valuation_cap)} />}
          {data.discount_rate != null && <Detail label="Discount" value={`${data.discount_rate}%`} />}
          {data.milestone_description && <Detail label="Milestone" value={data.milestone_description} />}
          {data.trigger_condition && <Detail label="Trigger" value={data.trigger_condition} />}
        </div>
      </ReviewSection>

      {/* Meeting (if any) */}
      {(filledSlots.length > 0 || data.meetingLocation || data.personalNote) && (
        <ReviewSection title="Meeting">
          {filledSlots.length > 0 && (
            <div className="flex flex-col gap-1 text-sm text-text-secondary">
              {filledSlots.map((slot, i) => (
                <Detail key={i} label={`Slot ${i + 1}`} value={new Date(slot).toLocaleString()} />
              ))}
            </div>
          )}
          {data.meetingLocation && <Detail label="Location" value={data.meetingLocation} />}
          {data.personalNote && (
            <div className="mt-2">
              <p className="text-xs text-text-muted">Your DM:</p>
              <p className="mt-0.5 text-sm italic text-text-secondary">"{data.personalNote}"</p>
            </div>
          )}
        </ReviewSection>
      )}
    </div>
  )
}

function ReviewSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-xl border border-border bg-surface-raised p-4">
      <h4 className="mb-2 text-xs font-semibold uppercase tracking-wider text-text-muted">
        {title}
      </h4>
      {children}
    </div>
  )
}

function Detail({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between">
      <span className="text-text-muted">{label}</span>
      <span className="text-text-primary">{value}</span>
    </div>
  )
}
```

#### 2. Offer Wizard orchestrator + modal
**File**: `src/components/offers/shoot-your-shot/offer-wizard.tsx` (new)
**Changes**: Full wizard managing all 4 steps with modal container

```tsx
import { useCallback, useState } from 'react'
import { useForm, FormProvider } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { AnimatePresence, motion } from 'framer-motion'
import { toast } from 'sonner'

import { supabase } from '@/lib/supabase'
import { useAuthStore } from '@/stores/auth-store'
import { Modal, ModalHeader } from '@/components/ui/modal'
import { BottomSheet, BottomSheetHeader } from '@/components/ui/bottom-sheet'
import { StepIndicator } from '@/components/onboarding/step-indicator'
import { StepInvestment } from '@/components/offers/shoot-your-shot/step-investment'
import { StepDealStructure } from '@/components/offers/shoot-your-shot/step-deal-structure'
import { StepMeeting } from '@/components/offers/shoot-your-shot/step-meeting'
import { StepReview } from '@/components/offers/shoot-your-shot/step-review'
import {
  stepInvestmentSchema,
  stepDealStructureSchema,
  stepMeetingSchema,
  type OfferFormData,
} from '@/lib/validations/offer-schema'

interface OfferWizardProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  founderId: string
  targetRaise: number
}

const STEPS = ['The Bag', 'The Deal', 'The Meet', 'Review']

const slideVariants = {
  enter: (direction: number) => ({ x: direction > 0 ? 200 : -200, opacity: 0 }),
  center: { x: 0, opacity: 1 },
  exit: (direction: number) => ({ x: direction < 0 ? 200 : -200, opacity: 0 }),
}

function OfferWizardForm({
  onClose,
  founderId,
  targetRaise,
}: {
  onClose: () => void
  founderId: string
  targetRaise: number
}) {
  const user = useAuthStore((s) => s.user)
  const [currentStep, setCurrentStep] = useState(0)
  const [direction, setDirection] = useState(1)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [serverError, setServerError] = useState('')

  const form = useForm<OfferFormData>({
    defaultValues: {
      amount: 0,
      dealType: undefined as unknown as OfferFormData['dealType'],
      meetingSlots: [],
      meetingLocation: '',
      personalNote: '',
    },
    mode: 'onTouched',
  })

  async function goNext() {
    const values = form.getValues()

    // Per-step validation using the individual Zod schemas
    if (currentStep === 0) {
      const result = stepInvestmentSchema.safeParse({ amount: values.amount })
      if (!result.success) {
        result.error.issues.forEach((issue) => {
          form.setError(issue.path[0] as keyof OfferFormData, { message: issue.message })
        })
        return
      }
    }

    if (currentStep === 1) {
      // Build deal structure object with only the relevant fields
      const dealData: Record<string, unknown> = { dealType: values.dealType }
      // Copy over all deal-specific fields that have values
      const dealFields = [
        'equity_percent', 'cash_amount', 'repayment_cap',
        'revenue_share_percent', 'milestone_description', 'trigger_condition',
        'valuation_cap', 'discount_rate',
      ] as const
      for (const field of dealFields) {
        if (values[field] != null && values[field] !== '') {
          dealData[field] = values[field]
        }
      }
      const result = stepDealStructureSchema.safeParse(dealData)
      if (!result.success) {
        result.error.issues.forEach((issue) => {
          const path = issue.path[0] as keyof OfferFormData
          form.setError(path === 'dealType' ? 'dealType' : path, { message: issue.message })
        })
        return
      }
    }

    // Step 2 (meeting) — no required validation, just schema shape check
    if (currentStep === 2) {
      const result = stepMeetingSchema.safeParse({
        meetingSlots: values.meetingSlots?.filter(Boolean) || [],
        meetingLocation: values.meetingLocation,
        personalNote: values.personalNote,
      })
      if (!result.success) {
        result.error.issues.forEach((issue) => {
          form.setError(issue.path[0] as keyof OfferFormData, { message: issue.message })
        })
        return
      }
    }

    setDirection(1)
    setCurrentStep((s) => s + 1)
  }

  function goBack() {
    setDirection(-1)
    setCurrentStep((s) => s - 1)
  }

  async function onSubmit() {
    if (!user) return

    setIsSubmitting(true)
    setServerError('')

    const values = form.getValues()
    const filledSlots = (values.meetingSlots || []).filter(Boolean)

    const { error } = await supabase.from('offers').insert({
      vc_id: user.id,
      founder_id: founderId,
      amount: values.amount,
      deal_type: values.dealType,
      // Deal-specific fields (nulls for irrelevant ones)
      equity_percent: values.equity_percent ?? null,
      cash_amount: values.cash_amount ?? null,
      repayment_cap: values.repayment_cap ?? null,
      revenue_share_percent: values.revenue_share_percent ?? null,
      milestone_description: values.milestone_description ?? null,
      trigger_condition: values.trigger_condition ?? null,
      valuation_cap: values.valuation_cap ?? null,
      discount_rate: values.discount_rate ?? null,
      // Meeting fields
      meeting_slot_1: filledSlots[0] ? new Date(filledSlots[0]).toISOString() : null,
      meeting_slot_2: filledSlots[1] ? new Date(filledSlots[1]).toISOString() : null,
      meeting_slot_3: filledSlots[2] ? new Date(filledSlots[2]).toISOString() : null,
      meeting_location: values.meetingLocation || null,
      personal_note: values.personalNote || null,
    })

    setIsSubmitting(false)

    if (error) {
      if (error.message.includes('idx_offers_unique_pending')) {
        setServerError("You already have a pending offer with this founder. Play it cool.")
      } else {
        setServerError(error.message)
      }
      return
    }

    toast.success("Offer sent. Try not to refresh every 30 seconds. 👀")
    onClose()
  }

  const isLastStep = currentStep === STEPS.length - 1

  const stepComponents = [
    <StepInvestment targetRaise={targetRaise} />,
    <StepDealStructure />,
    <StepMeeting onSkip={() => { setDirection(1); setCurrentStep(3) }} />,
    <StepReview />,
  ]

  return (
    <FormProvider {...form}>
      <div className="flex flex-col">
        <div className="mb-6">
          <StepIndicator steps={STEPS} currentStep={currentStep} />
        </div>

        <div className="relative min-h-[320px]">
          <AnimatePresence mode="wait" custom={direction}>
            <motion.div
              key={currentStep}
              custom={direction}
              variants={slideVariants}
              initial="enter"
              animate="center"
              exit="exit"
              transition={{ duration: 0.4, ease: 'easeInOut' }}
            >
              <div className="mb-3">
                <h2 className="text-lg font-semibold text-text-primary">
                  {STEPS[currentStep]}
                </h2>
              </div>
              {stepComponents[currentStep]}
            </motion.div>
          </AnimatePresence>
        </div>

        {serverError && (
          <div className="mt-4 rounded-lg border border-rose/30 bg-rose/10 px-3 py-2 text-sm text-rose">
            {serverError}
          </div>
        )}

        <div className="mt-6 flex items-center justify-between">
          <button
            type="button"
            onClick={goBack}
            disabled={currentStep === 0}
            className="rounded-full px-5 py-2.5 text-sm font-medium text-text-secondary transition-colors hover:text-text-primary disabled:invisible"
          >
            Back
          </button>

          {isLastStep ? (
            <motion.button
              type="button"
              onClick={onSubmit}
              disabled={isSubmitting}
              whileTap={{ scale: 0.98 }}
              className="rounded-full bg-gold px-8 py-2.5 text-sm font-semibold text-background transition-all hover:shadow-[0_0_20px_rgba(212,168,83,0.3)] disabled:opacity-50"
            >
              {isSubmitting ? 'Sending...' : 'Send the Term Sheet 💌'}
            </motion.button>
          ) : (
            <button
              type="button"
              onClick={goNext}
              className="rounded-full bg-gold px-8 py-2.5 text-sm font-semibold text-background transition-all hover:shadow-[0_0_20px_rgba(212,168,83,0.3)]"
            >
              Next
            </button>
          )}
        </div>
      </div>
    </FormProvider>
  )
}

export function OfferWizard({ open, onOpenChange, founderId, targetRaise }: OfferWizardProps) {
  const close = useCallback(() => onOpenChange(false), [onOpenChange])

  const isMobile = typeof window !== 'undefined' && window.innerWidth < 768

  const content = (
    <OfferWizardForm
      onClose={close}
      founderId={founderId}
      targetRaise={targetRaise}
    />
  )

  if (isMobile) {
    return (
      <BottomSheet open={open} onOpenChange={onOpenChange}>
        <BottomSheetHeader title="Shoot Your Shot 💰" onClose={close} />
        {content}
      </BottomSheet>
    )
  }

  return (
    <Modal open={open} onOpenChange={onOpenChange}>
      <ModalHeader title="Shoot Your Shot 💰" onClose={close} />
      {content}
    </Modal>
  )
}
```

**Key decisions**:
- No `zodResolver` on the form itself — validation is done manually per-step using individual Zod schemas in `goNext()`. This avoids fighting the discriminated union with react-hook-form's flat field model.
- Meeting "Skip" button jumps directly to step 3 (review), bypassing meeting validation entirely.
- Empty meeting slot strings are filtered out before insert. Slots are mapped to `meeting_slot_1/2/3` positionally.
- Duplicate pending offer error is caught by the unique partial index and shown with a satirical message.
- The `toast.success()` call fires the sonner toast after successful insert.
- `OfferWizard` component is self-contained with modal wrapper — can be imported and triggered from anywhere with `open`/`onOpenChange` + `founderId`/`targetRaise` props.

---

## Phase 6: Integration Wiring

### Overview
Final hookup: ensure sonner's `<Toaster />` is in the root layout, update `profile-header.tsx` to use the extracted `formatCurrency`, and verify the wizard can be imported and triggered.

### Changes Required:

#### 1. Root layout — add Toaster
**File**: Find the root layout component (likely `src/components/layout/root-layout.tsx` or `src/App.tsx`)
**Changes**: Add `<Toaster />` from sonner

```tsx
import { Toaster } from 'sonner'

// At the end of the layout JSX:
<Toaster
  theme="dark"
  position="bottom-right"
  toastOptions={{
    style: {
      background: 'var(--color-surface)',
      border: '1px solid var(--color-border)',
      color: 'var(--color-text-primary)',
    },
  }}
/>
```

#### 2. Update profile-header.tsx import
**File**: `src/components/profile/profile-header.tsx`
**Changes**: Replace local `formatCurrency` with import from `@/lib/format`

```diff
- function formatCurrency(amount: number): string {
-   if (amount >= 1_000_000) return `$${(amount / 1_000_000).toFixed(1)}M`
-   if (amount >= 1_000) return `$${(amount / 1_000).toFixed(0)}K`
-   return `$${amount}`
- }
+ import { formatCurrency } from '@/lib/format'
```

#### 3. Export surface for triggering (ready for 3.6)
The `OfferWizard` component is ready to be used from any parent:

```tsx
import { useState } from 'react'
import { OfferWizard } from '@/components/offers/shoot-your-shot/offer-wizard'

// In a parent component (e.g., founder profile money tab):
const [wizardOpen, setWizardOpen] = useState(false)

<button onClick={() => setWizardOpen(true)}>Shoot Your Shot 💰</button>
<OfferWizard
  open={wizardOpen}
  onOpenChange={setWizardOpen}
  founderId={founder.id}
  targetRaise={founder.target_raise}
/>
```

This is NOT implemented in this plan — it's the responsibility of task 3.6 (Tab Integration). This plan delivers the wizard as a self-contained, importable component.

---

## File Summary

| File | Status | Phase |
|---|---|---|
| `src/lib/format.ts` | New | 1 |
| `src/lib/validations/offer-schema.ts` | New | 1 |
| `src/components/offers/investment-slider.tsx` | New | 2 |
| `src/components/offers/shoot-your-shot/step-investment.tsx` | New | 2 |
| `src/components/offers/deal-type-tile.tsx` | New | 3 |
| `src/components/offers/shoot-your-shot/step-deal-structure.tsx` | New | 3 |
| `src/components/offers/shoot-your-shot/step-meeting.tsx` | New | 4 |
| `src/components/offers/shoot-your-shot/step-review.tsx` | New | 5 |
| `src/components/offers/shoot-your-shot/offer-wizard.tsx` | New | 5 |
| `src/components/profile/profile-header.tsx` | Modified | 6 |
| Root layout (TBD) | Modified | 6 |
| `package.json` | Modified (add sonner) | 1 |

## Dependencies to Install

| Package | Version | Purpose |
|---|---|---|
| `sonner` | latest | Toast notifications |

No other new dependencies. @base-ui/react/slider, framer-motion, react-hook-form, zod, date-fns, and lucide-react are all already installed.


-------

# Plan: Features 3.7–3.10 — Offers Inbox, Accept/Decline, VC Sent, Realtime

## Context

Features 3.1–3.6 of _docs/phases/phase-3-offers-system.md are complete: the offers DB schema, wizard UI, and CRUD hooks all exist. The `/offers` route is a stub. DB functions `accept_offer` and `decline_offer` exist but aren't called from the frontend. No realtime subscriptions exist yet. This plan implements the remaining offer lifecycle: founders viewing/acting on received offers, VCs tracking sent offers, and live status sync.

---

## Architecture Decisions

- **Single `/offers` route** — role-based rendering (founder inbox vs VC sent offers)
- **Detail view as modal** — Modal on desktop, BottomSheet on mobile (matches wizard pattern)
- **Filter tabs** for founder inbox (Pending/Accepted/Declined) using existing Tabs component
- **UI-level phone gating** — phone shown in VC detail view only when `status === 'accepted'`
- **Badge = pending count** — not `is_read` based
- **Generic realtime hook** — `useRealtime(opts)` reusable for future features
- **Zustand store for badge count** — lives in root layout, stays synced via realtime

---

## Files to Create (7 new)

| # | File | Purpose |
|---|------|---------|
| 1 | `src/hooks/use-realtime.ts` | Generic Supabase Realtime subscription hook |
| 2 | `src/stores/offer-count-store.ts` | Zustand store for pending badge count |
| 3 | `src/hooks/use-offer-count-sync.ts` | Fetches + realtime-syncs pending count into store |
| 4 | `src/components/offers/offer-status-badge.tsx` | Status badge (pending/accepted/declined) |
| 5 | `src/components/offers/offer-card.tsx` | Card for offer list items |
| 6 | `src/components/offers/offer-detail.tsx` | Modal/BottomSheet detail view with accept/decline |
| 7 | (none — `src/routes/offers.tsx` is a rewrite) | |

## Files to Modify (5 existing)

| File | Change |
|------|--------|
| `src/hooks/use-offers.ts` | Add `acceptOffer()`, `declineOffer()`, add `phone_number` to VC select + type |
| `src/routes/offers.tsx` | Full rewrite → FounderInbox + VcSentOffers sub-components |
| `src/components/layout/root-layout.tsx` | Call `useOfferCountSync()` |
| `src/components/layout/sidebar.tsx` | Render pending badge on offers nav item |
| `src/components/layout/bottom-tab-bar.tsx` | Same badge treatment |

---

## Implementation Steps

### Step 1 — `src/hooks/use-realtime.ts` (new)

Generic hook wrapping Supabase Realtime postgres_changes:

```ts
interface UseRealtimeOptions {
  table: string
  schema?: string           // default 'public'
  event?: 'INSERT' | 'UPDATE' | 'DELETE' | '*'  // default '*'
  filter?: string           // e.g. 'founder_id=eq.abc-123'
  onPayload: (payload) => void
  enabled?: boolean         // default true
}
```

- Creates channel from `table + filter + event` for stable identity
- `useRef` for `onPayload` callback to avoid re-subscribing on every render
- Cleanup: `supabase.removeChannel(channel)` in effect return
- `enabled` flag prevents subscribing with undefined filter values

### Step 2 — `src/hooks/use-offers.ts` (modify)

**A.** Add `phone_number` to `VC_OFFER_SELECT`:
```
'*, founder_profiles!offers_founder_id_fkey(id, startup_name, display_name, avatar_url, phone_number)'
```

**B.** Update `OfferWithFounder` type to include `phone_number: string | null`

**C.** Add two RPC wrapper functions:
```ts
export async function acceptOffer(offerId: string): Promise<{ error: string | null }>
export async function declineOffer(offerId: string): Promise<{ error: string | null }>
```
Both call `supabase.rpc('accept_offer' | 'decline_offer', { offer_id: offerId })`.

### Step 3 — `src/components/offers/offer-status-badge.tsx` (new)

Small component using existing `Badge` from `src/components/ui/badge.tsx`:
- `pending` → `bg-gold/15 text-gold border-gold/30`
- `accepted` → `bg-emerald-500/15 text-emerald-400 border-emerald-500/30`
- `declined` → `bg-rose/15 text-rose border-rose/30`

### Step 4 — `src/components/offers/offer-card.tsx` (new)

Props: `{ offer: OfferWithVc | OfferWithFounder, role: 'founder' | 'vc', onClick: () => void }`

Layout (horizontal row in a clickable card):
- **Left**: Avatar (VC or founder)
- **Center**: Name (fund_name / startup_name), display_name subtitle, `formatCurrency(amount)` in gold, deal type icon+label from `DEAL_TYPE_CONFIG`
- **Right**: `OfferStatusBadge`, relative time
- Declined offers for VC: `opacity-50`
- Uses `motion.button` with `whileTap` scale (matches deal-type-tile pattern)

Reuses: `formatCurrency` from `src/lib/format.ts`, `DEAL_TYPE_CONFIG` from `src/components/offers/deal-type-tile.tsx`

### Step 5 — `src/components/offers/offer-detail.tsx` (new)

Responsive wrapper: Modal on desktop (≥768px), BottomSheet on mobile — same pattern as `offer-wizard.tsx:242-270`.

Inner content sections:
1. **Profile header** — avatar + name + display_name
2. **Deal terms** — amount (large gold), deal type with icon, all non-null conditional fields. Replicates the `ReviewSection` / `Detail` helper pattern from `step-review.tsx:71-89` (inline, not imported — they're form-context-dependent there)
3. **Meeting details** — slots formatted with `toLocaleString()`, location
4. **Personal note** — italic blockquote style
5. **Phone number** (VC view only, accepted only) — prominent display with `Phone` icon. Null → "No phone on file"
6. **Action buttons** (founder view only, pending only) — Accept (gold) + Decline (destructive), disabled during `isActioning` loading state

### Step 6 — `src/routes/offers.tsx` (rewrite)

Top-level: role switch → `<FounderInbox>` or `<VcSentOffers>`

**FounderInbox**:
- `useFounderOffers(userId)` for data
- Derive counts client-side from offers array (avoids extra DB call, stays in sync with list)
- `useState` for `selectedOffer` + `detailOpen`
- Tabs: Pending (N) / Accepted (N) / Declined (N) — using existing Tabs "line" variant with gold active state
- Client-side filter: `offers.filter(o => o.status === activeTab)`
- Realtime: `useRealtime({ table: 'offers', filter: 'founder_id=eq.{userId}', onPayload: refetch })`
- Accept handler: calls `acceptOffer(id)` → toast → close modal → refetch
- Decline handler: calls `declineOffer(id)` → toast → close modal → refetch
- Empty states per tab with satirical copy

**VcSentOffers**:
- `useVcOffers(userId)` for data
- Flat list (no tabs — simpler for VCs)
- Realtime: `useRealtime({ table: 'offers', event: 'UPDATE', filter: 'vc_id=eq.{userId}', onPayload: refetch })`
- Click opens detail modal in read-only mode (no action buttons)

**Toast copy**:
- Accept: `"They said yes. First base secured... now close the deal. 😏"` (or similar innuendo)
- Decline: `"Left on read. Brutal. 💀"`

### Step 7 — Badge count system

**`src/stores/offer-count-store.ts`** (new) — minimal Zustand store:
```ts
{ pendingCount: number, setPendingCount: (n) => void }
```

**`src/hooks/use-offer-count-sync.ts`** (new) — hook that:
1. If role = founder: fetches pending count on mount via `select('id', { count: 'exact', head: true }).eq('status', 'pending')`
2. Subscribes to realtime on `offers` table filtered by `founder_id`
3. On any change: refetches count → updates store
4. If role ≠ founder: sets count to 0

**`src/components/layout/root-layout.tsx`** — add `useOfferCountSync()` call at top of component.

**`src/components/layout/sidebar.tsx`** — import `useOfferCountStore`, show gold badge dot on offers icon when `pendingCount > 0`:
```tsx
<span className="absolute -right-1 -top-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-gold text-[10px] font-bold text-background">
  {pendingCount > 9 ? '9+' : pendingCount}
</span>
```

**`src/components/layout/bottom-tab-bar.tsx`** — same badge pattern.

---

## Build Order

```
1. use-realtime.ts  ←  no deps
2. use-offers.ts modifications  ←  no deps (parallel with 1)
3. offer-count-store.ts  ←  no deps (parallel with 1-2)
4. offer-status-badge.tsx  ←  no deps
5. offer-card.tsx  ←  depends on 2 (types), 4 (badge)
6. offer-detail.tsx  ←  depends on 2 (types + actions), 4 (badge)
7. offers.tsx  ←  depends on 1, 2, 5, 6
8. use-offer-count-sync.ts  ←  depends on 1, 3
9. root-layout.tsx  ←  depends on 8
10. sidebar.tsx + bottom-tab-bar.tsx  ←  depends on 3 (parallel)
```

---

## Edge Cases

- **No offers**: satirical empty states per tab (founder) / single empty state (VC)
- **Rapid accept/decline**: disable both buttons during `isActioning`
- **Phone is null**: "No phone on file — they're playing hard to get"
- **Realtime drops**: Supabase auto-reconnects; worst case = stale list until next nav
- **Badge overflow**: cap at "9+"
- **Declined VC cards**: `opacity-50` for visual distinction

---

## Verification

1. **Founder flow**: Log in as founder → navigate to /offers → see tabs with counts → click offer card → detail modal opens → accept → toast fires, status updates, tab counts change → decline another → same flow
2. **VC flow**: Log in as VC → /offers → see flat list of sent offers → click one → detail shows full terms → no action buttons visible
3. **Realtime**: Open two browsers (founder + VC) → VC sends offer via wizard → founder sees it appear without refresh → founder accepts → VC sees status change live
4. **Badge**: Founder receives offer → badge appears on nav → founder accepts/declines all → badge disappears
5. **Mobile**: Repeat on narrow viewport → bottom sheet instead of modal, bottom tab bar shows badge
6. **Phone sharing**: VC views accepted offer detail → phone number visible. Pending/declined → no phone shown

---

I feel like this 

# Number Input

# Number Input

A single-purpose form control for entering a **bounded, quantitative value** — a quantity, a price, an age, a step count, a percentage. It looks like a text field, but everything about it (the mobile keyboard, the increment/decrement behavior, the min/max clamping, the locale formatting) exists to make entering a _number_ fast and hard to get wrong.
* * *

## 1\. What It Actually Is

A Number Input is a form control whose value is constrained to a numeric domain, usually with `min`, `max`, and `step` bounds, and usually paired with **stepper buttons** (± or ▲▼) so a user can nudge the value without typing. Under the hood it is one of two very different things: a native `<input type="number">`, or a **text input with** **`inputmode="numeric"`** (plus a `pattern` and manual parsing). Those two look identical but behave completely differently, and choosing the wrong one is the single most common mistake with this component (see §7 and §12).

The essential job is _not_ "restrict keystrokes to digits." It is: present the right keyboard, communicate the valid range, let the value be nudged and typed, clamp to bounds, and hand a clean parsed number to your form logic.

**The things people confuse it with, named and distinguished:**

*   **Text input with a numeric-looking value** (phone, credit card, SSN, ZIP, OTP) — these are _strings that happen to be digits_. They have leading zeros, fixed lengths, and grouping that has nothing to do with arithmetic. A Number Input would strip the leading zero off `07430` and try to "increment" a phone number. Use a masked text input with `inputmode="numeric"` instead.
*   **Slider (range input)** — a Slider is for _approximate_ values across a continuous range where the exact number barely matters (volume, brightness, opacity). A Number Input is for _precise_ values where 41 vs 42 matters. They pair well (slider + number readout), but they are not interchangeable.
*   **Stepper / Quantity selector** — a bare `[−] 3 [+]` cluster with no free typing is a _stepper_. A Number Input includes steppers but also permits direct keyboard entry. If a user can never type, it is a stepper, not a number input.
*   **Currency / measurement field** — a number input with an adornment ($ , %, kg) is still a number input, but the formatting (thousands separators, fixed decimals, locale decimal comma) pushes many teams to a masked text field. It is a _styled and formatted_ number input, not a different component.

**Rule of thumb:** If the value is a _magnitude you would do math on_ — add, compare, average, clamp — and it has a sensible min/max/step, it is a Number Input. If it is an _identifier_ (a sequence of digits with fixed shape, leading zeros, or grouping unrelated to arithmetic), it is a text input in disguise. "Would incrementing this value by 1 ever make sense?" is the fastest test.
* * *

## 2\. Why It Matters

*   **Data quality and server round-trips (money + engineering time).** A correctly bounded, correctly parsed number input rejects `"12abc"`, `"1.2.3"`, and `"-5"` (when negatives are illegal) _before_ they reach your API. Every malformed value that slips through is a validation error round-trip, a support ticket, or a corrupted row. On a checkout quantity field, a value that silently becomes `NaN` and posts as `0` is lost revenue.
*   **Conversion on mobile (direct revenue).** The mobile keyboard is decided by `type`/`inputmode`. A field that summons the full QWERTY keyboard for a quantity forces the user to hunt for the `123` toggle on every field. `inputmode="numeric"` or `="decimal"` brings up the number pad instantly — measurably fewer abandoned forms on quantity, tip, and payment-amount fields.
*   **Accessibility (legal + trust).** When steppers are custom `<div>`s with no `role="spinbutton"`, no `aria-valuenow`, and no keyboard handlers, a screen-reader or keyboard-only user literally cannot change the value. That is a WCAG 4.1.2 (Name/Role/Value) and 2.1.1 (Keyboard) failure — the exact class of defect that shows up in ADA demand letters.
*   **The scroll-wheel data-corruption bug (trust + correctness).** Native `<input type="number">` changes its value when the mouse wheel scrolls over a _focused_ field. A user scrolling a long form past a focused quantity field silently changes it from 1 to 14 and never notices. This is one of the most reported real-world number-input bugs and has to be actively defended against.
*   **Floating-point precision (correctness, especially money).** `0.1 + 0.2 === 0.30000000000000004`. A `step="0.01"` price field that naively adds steps drifts into `2.9999999999998`. Getting rounding and integer-cents math right is the difference between a checkout that balances and one that doesn't.
* * *

## 3\. Anatomy

*   **Container / field wrapper** — the bordered box that owns the border, background, radius, focus ring, and invalid styling. Steppers and adornments live inside it so the whole cluster reads as one control.
*   **Input area** — the editable text region bound to the numeric value. Conventionally **right-aligned** for numbers so decimal points and magnitudes line up in stacked forms (a genuine data-legibility rule, not decoration).
*   **Stepper buttons (increment / decrement)** — the `▲▼` or `[−][+]` controls. Stacked vertically on the right, or split (minus left / plus right, the e-commerce quantity pattern). Each must be a real button with its own accessible name ("Increase quantity" / "Decrease quantity") and must be `disabled` at the respective bound.
*   **Label** — the persistent, programmatically-associated text ("Quantity", "Price (USD)"). Never a disappearing placeholder — a number field cleared to edit must not lose its label.
*   **Prefix / suffix adornment** — the leading `$` or trailing `%` / `kg` / `px`. Visually inside the container but _not_ part of the editable value; the parsed number excludes it.
*   **Helper / range text** — the sub-label that states the valid range ("1–99", "Max $10,000"). This is the primary way sighted users learn the bounds before they hit an error.
*   **Error / validation message** — the state-driven message ("Enter a whole number between 1 and 99"), wired via `aria-describedby` and announced on validation.
*   **Hidden native value / form binding** — for custom (non-native) implementations, the real `<input>` (often `type="text" inputmode="decimal"`) or a hidden field that actually submits, carrying the clean numeric string.
* * *

## 4\. Sizes / Scale / Density

Height is driven by the touch-target rule: the _stepper buttons_ are the smallest hit areas, so they, not the field, set the floor. Steppers below 24px fail comfortably-usable targets; on primary mobile flows push the whole control to the Large tier so steppers clear 44px.

| Tier | Field height | Stepper button | Font | Horizontal pad | Radius token | Use case |
| ---| ---| ---| ---| ---| ---| --- |
| XS / Compact | 28px | 20 × 14px | 13px | 8px | `--radius-sm` (4px) | Dense data grids, spreadsheet-like tables, admin bulk-edit rows |
| S / Small | 32px | 24 × 16px | 14px | 10px | `--radius-sm` (4px) | Toolbars, filter panels, settings lists |
| M / Medium (default) | 40px | 32 × 20px | 16px | 12px | `--radius-md` (8px) | Standard forms, checkout quantity, most web usage |
| L / Large | 48px | 40 × 24px | 16–18px | 16px | `--radius-md` (8px) | Mobile-primary flows, touch checkout, accessibility-first |
| XL / Hero | 56px | 48 × 28px | 20px | 20px | `--radius-lg` (12px) | Single-focus tools (calculators, tip pickers, donation amount) |

Rules that matter:

*   **16px minimum font on mobile** for the input area — anything smaller triggers iOS Safari auto-zoom on focus, which yanks the layout.
*   **Split steppers** need the field to be at least ~120px wide so the number isn't crushed; **stacked steppers** work down to ~72px.
*   **Spacing tokens, not magic numbers:** internal gap between input area and stepper column = `--space-1` (4px); adornment inset = `--space-2` (8px).
*   **Responsive behavior:** below ~360px, prefer split steppers with a wide central readout (thumb-friendly) over tiny stacked `▲▼`.
* * *

## 5\. States

*   **Empty / placeholder** — no value yet. Placeholder may hint format ("0.00") but never replaces the label. Note: a truly empty native number input reports `value === ""`, not `0` — don't treat empty as zero.
*   **Filled / default** — a valid parsed number present. Resting border, no ring.
*   **Hover** — pointer over the field or a stepper. Steppers get a subtle background; the field border may darken slightly. Purely affordance feedback.
*   **Focus (field)** — visible 2px focus ring on the container using `:focus-visible`; the ring must survive every skin in §17.
*   **Focus (stepper)** — a stepper can hold focus independently; it needs its own visible ring, not just the container's.
*   **Active / pressing a stepper** — the moment of increment/decrement; button depresses, value updates, screen-reader announces the new `aria-valuenow`.
*   **Min-reached** — value equals `min`. The decrement stepper becomes `disabled` (and `aria-disabled`), visually dimmed; typing below min is clamped on blur.
*   **Max-reached** — value equals `max`. The increment stepper becomes `disabled`; symmetric to min-reached.
*   **Invalid / error** — value out of range, wrong step, or non-numeric. Red border, error message via `aria-describedby`, `aria-invalid="true"`. Distinguish _transient_ invalid (mid-typing "1.") from _committed_ invalid (on blur/submit) — don't scream while they're still typing.
*   **Disabled** — not editable, not focusable, dimmed; steppers disabled too. Value still submits if it's a real disabled input carrying data? No — disabled inputs don't submit; use readonly if the value must post.
*   **Read-only** — value visible and focusable/copyable but not editable; steppers hidden or disabled. Distinct from disabled: readonly _does_ submit and _is_ reachable.
*   **Loading / async-validating** — value being checked server-side (e.g., "is this coupon quantity in stock?"). Spinner in the suffix slot, control optionally locked, `aria-busy="true"`.
* * *

## 6\. Types / Variants

*   **Stacked-stepper (native-style)** — `▲` over `▼` on the right edge. The classic desktop pattern; compact, keyboard-friendly, matches native `<input type="number">` appearance.
*   **Split-stepper (quantity picker)** — `[−] 3 [+]` with the value centered. The e-commerce / cart standard; big thumb targets, reads instantly as "how many."
*   **Steppers-off (typed only)** — no buttons, `inputmode="decimal"`, validation on blur. Correct for open-ended values (salary, price, arbitrary measurement) where nudging by a fixed step is meaningless.
*   **Adorned (currency / unit)** — leading `$` or trailing `%`/`kg`/`px`. The adornment styles inside the container; parsing strips it. Often pairs with fixed decimal places and thousands grouping.
*   **Integer vs Decimal** — `step="1"` whole-number counters vs `step="0.01"` money/measure fields. The decimal variant sets `inputmode="decimal"` (dot/comma key) instead of `="numeric"` (digits only) and needs precision-safe math.
*   **Ranged pair (min–max / from–to)** — two coupled number inputs enforcing `from ≤ to` (price filters, date-range-as-numbers, age brackets). Each field's `min`/`max` updates from the other's value.
*   **Slider-linked** — a Number Input mirrored to a `<input type="range">`; type for precision, drag for feel, both bound to one state value.
*   **Locale-formatted display** — shows `1,234.56` (or `1.234,56` in de-DE) while editing a raw value; formats on blur, unformats on focus so the caret behaves.
* * *

## 7\. When to Use (and When Not To)

**Use when:**

*   Selecting a **quantity** (cart items, tickets, servings) — split-stepper variant.
*   Entering a **price, amount, or budget** — adorned decimal variant with integer-cents math.
*   Setting a **bounded numeric setting** (font size, timeout seconds, retry count, volume as a number).
*   Capturing **age, weight, height, temperature, dosage** — magnitudes with a natural range.
*   Any **percentage** with a `%` suffix and a 0–100 clamp.

**Don't use / use something else when:**

*   The value is a **digit-string identifier** — phone, SSN, credit card, ZIP, OTP, account number. Use a **text input with** **`inputmode="numeric"`** + mask/pattern. (Number inputs kill leading zeros and try to do arithmetic on IDs.)
*   The value is **approximate across a continuous range** and exactness doesn't matter — use a **Slider**, optionally with a number readout.
*   You want a **fixed small set** of choices (1–5 rating) — use a **Segmented control**, **Radio group**, or **Star rating**.
*   The number is a **date or time** — use a **Date Picker** / **Time Picker** (they carry calendar logic, month lengths, DST).
*   The range is **enormous and unbounded** (choose a year from 1900–2100, or a huge count) where typing beats nudging — steppers-off variant, or a combobox/select for a known list.

**Placement heuristics:** Label above (or inline-left in dense grids), right-align the number, put range/helper text directly below, and keep steppers on the trailing edge. In a cart line item, the split-stepper sits inline with the product; in a form, the stacked-stepper stays flush right of the field.
* * *

## 8\. Across Design Systems

*   **Material Design (MDC / M3)** — Text field with `type="number"`; steppers are _not_ a first-class Material primitive (M3 leans on sliders and plain numeric text fields). Filled/outlined field styling, floating label, helper/error line, `.mdc-text-field--invalid`. Teams add custom stepper buttons that inherit the state layer.
*   **Apple HIG** — The **Stepper** (`UIStepper` / SwiftUI `Stepper`) is a discrete ±control, usually paired with a separate `TextField` bound to a `.number` format. HIG treats "adjust by increment" and "type a value" as two composed controls, not one widget. On the web, iOS shows the number pad for `inputmode="numeric"/"decimal"`.
*   **Fluent (Microsoft)** — First-class **`SpinButton`** component: text field + up/down chevrons, `min`/`max`/`step`, precision, and a display value formatter. One of the few systems that ships the exact stacked-stepper number input as a named component.
*   **Ant Design** — **`InputNumber`**: full-featured, with `min`/`max`/`step`, `precision`, `formatter`/`parser` (built-in thousands-separator and currency support), `controls` toggle, keyboard arrows, and `stringMode` for BigInt/arbitrary precision. The most complete OOTB number input of the major systems.
*   **Tailwind CSS** — No component; utilities only. You style a native `<input type="number">` and hand-build steppers, commonly hiding the native spinners (`[&::-webkit-inner-spin-button]:appearance-none`) and adding your own buttons. Tailwind UI / shadcn patterns fill the gap.
*   **shadcn/ui + Radix** — shadcn ships an `Input` (style only). There is no Radix `NumberField` primitive historically, so teams use **`@base-ui/react`** **NumberField** or **`react-aria`** **`useNumberField`** (Adobe) for correct spinbutton semantics, locale parsing, and stepper wiring, styled with the shadcn `Input` look.
*   **Bootstrap 5** — `.form-control` on `<input type="number">`; steppers via an **input group** (`.input-group` with `−`/`+` buttons). No dedicated component; validation classes `.is-invalid`/`.is-valid` + `.invalid-feedback`.
*   **Carbon (IBM)** — dedicated **`NumberInput`** with visible steppers, min/max/step, invalid state, and helper text — another system treating it as a named primitive.

### 8b. Visual Styles / Trends — What's trending in 2026

*   **Native spinners are dying;** teams ship custom stepper buttons for consistent cross-browser look and correct touch targets, and hide the browser's `::-webkit-inner-spin-button`.
*   **`inputmode`****\-first** design: the value/keyboard decision (`numeric` vs `decimal`) is now made deliberately per field rather than defaulting to `type="number"`.
*   **Liquid Glass** (Apple iOS 26 / macOS Tahoe) steppers — translucent, light-refracting ± buttons floating on frosted field surfaces.
*   **Big-thumb split steppers** for mobile commerce keep growing; the `[−] n [+]` cluster with 44px+ targets is the default cart pattern.
*   **Intl.NumberFormat-on-the-fly** editing — live locale grouping while typing (with careful caret management) instead of format-on-blur, now that the API is universally supported.
*   **`react-aria`** **/ Base UI adoption** for correctness — more teams drop native `type="number"` entirely in favor of accessible, locale-aware headless number fields.
* * *

## 9\. The Code

Full, runnable, production-shaped targets for the Number Input. Each defends the core invariants: correct keyboard, spinbutton semantics, min/max clamping, step precision, and scroll-wheel protection.

### 9.1 HTML (semantic foundation with ARIA)

```plain
<!-- Native baseline: fastest to ship, browser-inconsistent steppers -->
<div class="numfield">
  <label for="qty">Quantity</label>
  <input
    id="qty"
    name="qty"
    type="number"
    inputmode="numeric"
    min="1" max="99" step="1"
    value="1"
    autocomplete="off"
    aria-describedby="qty-help qty-err"
  />
  <p id="qty-help" class="numfield__help">Whole number, 1–99</p>
  <p id="qty-err" class="numfield__err" hidden>Enter a whole number between 1 and 99.</p>
</div>

<!-- Custom stepper pattern: full control, explicit spinbutton semantics -->
<div class="numfield" data-numfield>
  <label id="price-label" for="price">Price (USD)</label>
  <div class="numfield__control">
    <span class="numfield__prefix" aria-hidden="true">$</span>
    <input
      id="price"
      type="text" inputmode="decimal"
      role="spinbutton"
      aria-labelledby="price-label"
      aria-valuemin="0" aria-valuemax="10000" aria-valuenow="9.99"
      aria-valuetext="$9.99"
      value="9.99" autocomplete="off"
    />
    <span class="numfield__steppers">
      <button type="button" class="numfield__inc" aria-label="Increase price">▲</button>
      <button type="button" class="numfield__dec" aria-label="Decrease price">▼</button>
    </span>
  </div>
</div>
```

### 9.2 CSS (states, layout, reduced-motion, hidden native spinner)

```css
.numfield { display: flex; flex-direction: column; gap: var(--space-1, 4px); }

.numfield__control {
  --field-h: 40px;
  display: flex; align-items: stretch;
  height: var(--field-h);
  border: 1px solid var(--border, #6b6b6b);
  border-radius: var(--radius-md, 8px);
  background: var(--bg, #fff);
  overflow: hidden;
  transition: border-color .15s ease, box-shadow .15s ease;
}
.numfield__control:hover { border-color: var(--border-strong, #333); }
.numfield__control:focus-within {
  border-color: var(--focus, #5F2C82);
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--focus, #5F2C82) 45%, transparent);
}

.numfield input[role="spinbutton"],
.numfield input[type="number"] {
  flex: 1 1 auto; min-width: 0;
  border: 0; outline: 0; background: transparent;
  padding: 0 12px; font-size: 16px; text-align: right;
  font-variant-numeric: tabular-nums;
}
/* Kill the browser's native spinner so custom steppers are the only ones */
.numfield input[type="number"]::-webkit-inner-spin-button,
.numfield input[type="number"]::-webkit-outer-spin-button { -webkit-appearance: none; margin: 0; }
.numfield input[type="number"] { -moz-appearance: textfield; appearance: textfield; }

.numfield__prefix, .numfield__suffix {
  display: grid; place-items: center; padding: 0 8px; color: var(--muted, #555);
  user-select: none;
}
.numfield__steppers { display: flex; flex-direction: column; border-left: 1px solid var(--border, #6b6b6b); }
.numfield__steppers button {
  flex: 1; width: 32px; border: 0; background: var(--stepper-bg, #f2f2f2);
  cursor: pointer; font-size: 10px; line-height: 1; color: var(--fg, #111);
  transition: background .12s ease;
}
.numfield__steppers button:hover { background: var(--stepper-hover, #e4e4e4); }
.numfield__steppers button:active { background: var(--stepper-active, #d6d6d6); }
.numfield__steppers button:focus-visible {
  outline: 2px solid var(--focus, #5F2C82); outline-offset: -2px;
}
.numfield__steppers button[disabled] { opacity: .4; cursor: not-allowed; }

.numfield__control[data-invalid="true"] { border-color: var(--danger, #C0362C); }
.numfield__control[data-invalid="true"]:focus-within {
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--danger, #C0362C) 45%, transparent);
}
.numfield__err { color: var(--danger, #C0362C); font-size: 13px; margin: 0; }
.numfield__help { color: var(--muted, #555); font-size: 13px; margin: 0; }
.numfield input:disabled { color: var(--muted, #999); }

@media (prefers-reduced-motion: reduce) {
  .numfield__control, .numfield__steppers button { transition: none; }
}
```

### 9.3 React + TypeScript (full reusable component)

```plain
import { forwardRef, useCallback, useId, useRef, useState } from "react";

export interface NumberInputProps {
  label: string;
  value: number | null;
  onChange: (value: number | null) => void;
  min?: number;
  max?: number;
  step?: number;
  precision?: number;              // decimal places; undefined = infer from step
  prefix?: string;
  suffix?: string;
  disabled?: boolean;
  required?: boolean;
  id?: string;
  name?: string;
  helpText?: string;
}

/** Precision-safe arithmetic: work in integer "units" to dodge float drift. */
function addStep(value: number, step: number, dir: 1 | -1, precision: number): number {
  const factor = 10 ** precision;
  const next = Math.round(value * factor + dir * step * factor) / factor;
  return next;
}
const clamp = (n: number, min: number, max: number) => Math.min(max, Math.max(min, n));
const inferPrecision = (step: number) => {
  const s = String(step);
  return s.includes(".") ? s.split(".")[1].length : 0;
};

export const NumberInput = forwardRef<HTMLInputElement, NumberInputProps>(function NumberInput(
  { label, value, onChange, min = -Infinity, max = Infinity, step = 1, precision,
    prefix, suffix, disabled, required, id, name, helpText },
  ref
) {
  const autoId = useId();
  const fieldId = id ?? autoId;
  const errId = `${fieldId}-err`;
  const helpId = `${fieldId}-help`;
  const p = precision ?? inferPrecision(step);
  const [text, setText] = useState(value === null ? "" : String(value));
  const [error, setError] = useState<string | null>(null);
  const innerRef = useRef<HTMLInputElement>(null);

  const commit = useCallback((raw: string) => {
    if (raw.trim() === "") { onChange(null); setError(required ? "Required" : null); return; }
    const parsed = Number(raw.replace(",", "."));
    if (!Number.isFinite(parsed)) { setError("Enter a valid number"); return; }
    const clamped = clamp(parsed, min, max);
    const snapped = Number((Math.round(clamped / step) * step).toFixed(p));
    setText(String(snapped));
    setError(null);
    onChange(snapped);
  }, [min, max, step, p, required, onChange]);

  const nudge = (dir: 1 | -1) => {
    const base = value ?? (dir === 1 ? min === -Infinity ? 0 : min : max === Infinity ? 0 : max);
    const next = clamp(addStep(base, step, dir, p), min, max);
    setText(String(next)); setError(null); onChange(next);
    innerRef.current?.focus();
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (disabled) return;
    if (e.key === "ArrowUp")   { e.preventDefault(); nudge(1); }
    if (e.key === "ArrowDown") { e.preventDefault(); nudge(-1); }
    if (e.key === "PageUp")    { e.preventDefault(); onChange(clamp((value ?? 0) + step * 10, min, max)); }
    if (e.key === "PageDown")  { e.preventDefault(); onChange(clamp((value ?? 0) - step * 10, min, max)); }
    if (e.key === "Home" && Number.isFinite(min)) { e.preventDefault(); onChange(min); setText(String(min)); }
    if (e.key === "End"  && Number.isFinite(max)) { e.preventDefault(); onChange(max); setText(String(max)); }
  };

  const atMin = value !== null && value <= min;
  const atMax = value !== null && value >= max;

  return (
    <div className="numfield">
      <label htmlFor={fieldId}>{label}{required && <span aria-hidden> *</span>}</label>
      <div className="numfield__control" data-invalid={error ? "true" : "false"}>
        {prefix && <span className="numfield__prefix" aria-hidden>{prefix}</span>}
        <input
          ref={(node) => { innerRef.current = node;
            if (typeof ref === "function") ref(node); else if (ref) ref.current = node; }}
          id={fieldId} name={name} type="text" inputMode={p > 0 ? "decimal" : "numeric"}
          role="spinbutton"
          aria-valuemin={Number.isFinite(min) ? min : undefined}
          aria-valuemax={Number.isFinite(max) ? max : undefined}
          aria-valuenow={value ?? undefined}
          aria-valuetext={value !== null ? `${prefix ?? ""}${value}${suffix ?? ""}` : undefined}
          aria-invalid={!!error}
          aria-describedby={`${helpText ? helpId : ""} ${error ? errId : ""}`.trim() || undefined}
          disabled={disabled} required={required}
          value={text}
          onChange={(e) => setText(e.currentTarget.value)}
          onBlur={(e) => commit(e.currentTarget.value)}
          onKeyDown={onKeyDown}
          onWheel={(e) => e.currentTarget.blur() /* stop scroll-wheel edits */}
        />
        {suffix && <span className="numfield__suffix" aria-hidden>{suffix}</span>}
        <span className="numfield__steppers">
          <button type="button" aria-label={`Increase ${label}`} disabled={disabled || atMax}
                  onClick={() => nudge(1)} tabIndex={-1}>▲</button>
          <button type="button" aria-label={`Decrease ${label}`} disabled={disabled || atMin}
                  onClick={() => nudge(-1)} tabIndex={-1}>▼</button>
        </span>
      </div>
      {helpText && <p id={helpId} className="numfield__help">{helpText}</p>}
      {error && <p id={errId} className="numfield__err" role="alert">{error}</p>}
    </div>
  );
});
```

### 9.4 Vanilla JavaScript (no framework, full keyboard + scroll guard)

```plain
class NumberField {
  constructor(root) {
    this.root = root;
    this.input = root.querySelector('input[role="spinbutton"]');
    this.inc = root.querySelector('.numfield__inc');
    this.dec = root.querySelector('.numfield__dec');
    this.min = this.num(this.input.getAttribute('aria-valuemin'), -Infinity);
    this.max = this.num(this.input.getAttribute('aria-valuemax'), Infinity);
    this.step = this.num(this.input.dataset.step, 1);
    this.precision = (String(this.step).split('.')[1] || '').length;
    this.bind();
  }
  num(v, fallback) { const n = Number(v); return Number.isFinite(n) ? n : fallback; }
  clamp(n) { return Math.min(this.max, Math.max(this.min, n)); }
  round(n) { const f = 10 ** this.precision; return Math.round(n * f) / f; }
  get value() { const n = Number(this.input.value.replace(',', '.')); return Number.isFinite(n) ? n : null; }
  set value(n) {
    const v = this.round(this.clamp(n));
    this.input.value = String(v);
    this.input.setAttribute('aria-valuenow', String(v));
    this.input.setAttribute('aria-valuetext', String(v));
    this.dec.disabled = v <= this.min;
    this.inc.disabled = v >= this.max;
  }
  nudge(dir) { this.value = (this.value ?? this.min || 0) + dir * this.step; }
  bind() {
    this.inc.addEventListener('click', () => { this.nudge(1); this.input.focus(); });
    this.dec.addEventListener('click', () => { this.nudge(-1); this.input.focus(); });
    this.input.addEventListener('keydown', (e) => {
      const map = { ArrowUp: () => this.nudge(1), ArrowDown: () => this.nudge(-1),
        PageUp: () => { this.value = (this.value ?? 0) + this.step * 10; },
        PageDown: () => { this.value = (this.value ?? 0) - this.step * 10; },
        Home: () => { if (Number.isFinite(this.min)) this.value = this.min; },
        End:  () => { if (Number.isFinite(this.max)) this.value = this.max; } };
      if (map[e.key]) { e.preventDefault(); map[e.key](); }
    });
    // Scroll-wheel guard: blur so wheel can't silently change a focused field
    this.input.addEventListener('wheel', () => this.input.blur(), { passive: true });
    this.input.addEventListener('blur', () => { if (this.value !== null) this.value = this.value; });
  }
}
document.querySelectorAll('[data-numfield]').forEach((el) => new NumberField(el));
```

### 9.5 Tailwind CSS

```plain
<div class="flex flex-col gap-1">
  <label for="t-qty" class="text-sm font-medium text-neutral-800">Quantity</label>
  <div class="group flex h-10 items-stretch overflow-hidden rounded-lg border border-neutral-400
              focus-within:border-[#5F2C82] focus-within:ring-2 focus-within:ring-[#5F2C82]/40">
    <input id="t-qty" type="number" inputmode="numeric" min="1" max="99" step="1" value="1"
      class="w-full min-w-0 bg-transparent px-3 text-right text-base tabular-nums outline-none
             [appearance:textfield] [&::-webkit-inner-spin-button]:appearance-none" />
    <div class="flex flex-col border-l border-neutral-300">
      <button type="button" aria-label="Increase quantity"
        class="h-1/2 w-8 bg-neutral-100 text-[10px] hover:bg-neutral-200 active:bg-neutral-300
               disabled:opacity-40 focus-visible:outline focus-visible:outline-2
               focus-visible:outline-[#5F2C82] focus-visible:-outline-offset-2">▲</button>
      <button type="button" aria-label="Decrease quantity"
        class="h-1/2 w-8 bg-neutral-100 text-[10px] hover:bg-neutral-200 active:bg-neutral-300
               disabled:opacity-40 focus-visible:outline focus-visible:outline-2
               focus-visible:outline-[#5F2C82] focus-visible:-outline-offset-2">▼</button>
    </div>
  </div>
  <p class="text-xs text-neutral-500">Whole number, 1–99</p>
</div>
```

### 9.6 Next.js (App Router — Server Component form + Client field + Server Action)

```plain
// app/cart/quantity-form.tsx  (Server Component)
import { updateQuantity } from "./actions";
import { QuantityField } from "./quantity-field";

export default function QuantityForm({ initial }: { initial: number }) {
  return (
    <form action={updateQuantity} className="flex items-end gap-3">
      <QuantityField name="qty" defaultValue={initial} min={1} max={99} />
      <button type="submit" className="h-10 rounded-lg bg-[#5F2C82] px-4 text-white">Update</button>
    </form>
  );
}
// app/cart/quantity-field.tsx
"use client";
import { useState } from "react";
import { NumberInput } from "@/components/number-input"; // the §9.3 component

export function QuantityField(
  { name, defaultValue, min, max }: { name: string; defaultValue: number; min: number; max: number }
) {
  const [qty, setQty] = useState<number | null>(defaultValue);
  return (
    <>
      <NumberInput label="Quantity" name={name} value={qty} onChange={setQty}
        min={min} max={max} step={1} helpText={`In stock: ${max}`} />
      {/* hidden field guarantees the parsed value posts even if JS state lags */}
      <input type="hidden" name={`${name}_validated`} value={qty ?? ""} />
    </>
  );
}
// app/cart/actions.ts
"use server";
import { z } from "zod";
import { revalidatePath } from "next/cache";

const schema = z.object({ qty: z.coerce.number().int().min(1).max(99) });

export async function updateQuantity(formData: FormData) {
  const parsed = schema.safeParse({ qty: formData.get("qty") });
  if (!parsed.success) throw new Error("Invalid quantity"); // NEVER trust client clamping
  await db.cart.update({ quantity: parsed.data.qty });
  revalidatePath("/cart");
}
```

### 9.7 shadcn/ui + Radix (react-aria NumberField, styled with shadcn Input)

```plain
"use client";
import { NumberField, Label, Group, Input, Button } from "react-aria-components";
import { ChevronUp, ChevronDown } from "lucide-react";
import { cn } from "@/lib/utils";

export function ShadcnNumberField(
  { label, ...props }: { label: string; defaultValue?: number; minValue?: number;
    maxValue?: number; step?: number; formatOptions?: Intl.NumberFormatOptions }
) {
  return (
    <NumberField className="flex flex-col gap-1" {...props}>
      <Label className="text-sm font-medium">{label}</Label>
      <Group className={cn(
        "flex h-10 items-stretch overflow-hidden rounded-md border border-input bg-background",
        "focus-within:ring-2 focus-within:ring-ring")}>
        <Input className="w-full bg-transparent px-3 text-right tabular-nums outline-none" />
        <div className="flex flex-col border-l">
          <Button slot="increment" className="grid h-1/2 w-8 place-items-center hover:bg-accent">
            <ChevronUp className="size-3" />
          </Button>
          <Button slot="decrement" className="grid h-1/2 w-8 place-items-center hover:bg-accent">
            <ChevronDown className="size-3" />
          </Button>
        </div>
      </Group>
    </NumberField>
  );
}
// react-aria handles spinbutton role, aria-value*, locale parsing, and clamping for free.
```

### 9.8 Vue 3 (SFC, `<script setup>` + v-model)

```plain
<script setup lang="ts">
import { computed, ref, watch } from "vue";
const props = withDefaults(defineProps<{
  modelValue: number | null; label: string; min?: number; max?: number; step?: number;
}>(), { min: -Infinity, max: Infinity, step: 1 });
const emit = defineEmits<{ (e: "update:modelValue", v: number | null): void }>();

const precision = computed(() => (String(props.step).split(".")[1] || "").length);
const text = ref(props.modelValue === null ? "" : String(props.modelValue));
watch(() => props.modelValue, (v) => { text.value = v === null ? "" : String(v); });

const clamp = (n: number) => Math.min(props.max, Math.max(props.min, n));
const round = (n: number) => { const f = 10 ** precision.value; return Math.round(n * f) / f; };
const atMin = computed(() => props.modelValue !== null && props.modelValue <= props.min);
const atMax = computed(() => props.modelValue !== null && props.modelValue >= props.max);

function commit() {
  if (text.value.trim() === "") return emit("update:modelValue", null);
  const n = Number(text.value.replace(",", "."));
  if (!Number.isFinite(n)) return;
  emit("update:modelValue", round(clamp(n)));
}
function nudge(dir: 1 | -1) {
  const base = props.modelValue ?? (props.min === -Infinity ? 0 : props.min);
  emit("update:modelValue", round(clamp(base + dir * props.step)));
}
</script>

<template>
  <div class="numfield">
    <label>{{ label }}</label>
    <div class="numfield__control">
      <input :value="text" type="text"
        :inputmode="precision > 0 ? 'decimal' : 'numeric'"
        role="spinbutton"
        :aria-valuemin="Number.isFinite(min) ? min : undefined"
        :aria-valuemax="Number.isFinite(max) ? max : undefined"
        :aria-valuenow="modelValue ?? undefined"
        @input="text = ($event.target as HTMLInputElement).value"
        @blur="commit"
        @keydown.up.prevent="nudge(1)" @keydown.down.prevent="nudge(-1)"
        @wheel="($event.target as HTMLInputElement).blur()" />
      <span class="numfield__steppers">
        <button type="button" :aria-label="`Increase ${label}`" :disabled="atMax" @click="nudge(1)">▲</button>
        <button type="button" :aria-label="`Decrease ${label}`" :disabled="atMin" @click="nudge(-1)">▼</button>
      </span>
    </div>
  </div>
</template>
```

### 9.9 Svelte

```plain
<script lang="ts">
  export let value: number | null = null;
  export let label: string;
  export let min = -Infinity, max = Infinity, step = 1;

  $: precision = (String(step).split(".")[1] || "").length;
  const clamp = (n: number) => Math.min(max, Math.max(min, n));
  const round = (n: number) => { const f = 10 ** precision; return Math.round(n * f) / f; };
  $: atMin = value !== null && value <= min;
  $: atMax = value !== null && value >= max;

  function nudge(dir: 1 | -1) {
    const base = value ?? (min === -Infinity ? 0 : min);
    value = round(clamp(base + dir * step));
  }
  function commit(e: FocusEvent) {
    const raw = (e.target as HTMLInputElement).value.replace(",", ".");
    if (raw.trim() === "") { value = null; return; }
    const n = Number(raw);
    if (Number.isFinite(n)) value = round(clamp(n));
  }
  function onKey(e: KeyboardEvent) {
    const a: Record<string, () => void> = {
      ArrowUp: () => nudge(1), ArrowDown: () => nudge(-1),
      Home: () => Number.isFinite(min) && (value = min),
      End:  () => Number.isFinite(max) && (value = max) };
    if (a[e.key]) { e.preventDefault(); a[e.key](); }
  }
</script>

<div class="numfield">
  <label>{label}</label>
  <div class="numfield__control">
    <input type="text" inputmode={precision > 0 ? "decimal" : "numeric"}
      role="spinbutton" aria-valuenow={value ?? undefined}
      value={value ?? ""} on:blur={commit} on:keydown={onKey}
      on:wheel={(e) => (e.currentTarget as HTMLInputElement).blur()} />
    <span class="numfield__steppers">
      <button type="button" aria-label={`Increase ${label}`} disabled={atMax} on:click={() => nudge(1)}>▲</button>
      <button type="button" aria-label={`Decrease ${label}`} disabled={atMin} on:click={() => nudge(-1)}>▼</button>
    </span>
  </div>
</div>
```

### 9.10 Angular (standalone component)

```plain
import { Component, EventEmitter, Input, Output } from "@angular/core";
import { CommonModule } from "@angular/common";

@Component({
  selector: "ujg-number-input",
  standalone: true,
  imports: [CommonModule],
  template: `
    <div class="numfield">
      <label [attr.for]="id">{{ label }}</label>
      <div class="numfield__control" [attr.data-invalid]="error ? 'true' : 'false'">
        <input [id]="id" type="text" [attr.inputmode]="precision > 0 ? 'decimal' : 'numeric'"
          role="spinbutton"
          [attr.aria-valuemin]="isFinite(min) ? min : null"
          [attr.aria-valuemax]="isFinite(max) ? max : null"
          [attr.aria-valuenow]="value" [value]="value ?? ''"
          (blur)="commit($any($event.target).value)"
          (keydown)="onKey($event)" (wheel)="$any($event.target).blur()" />
        <span class="numfield__steppers">
          <button type="button" [attr.aria-label]="'Increase ' + label"
            [disabled]="atMax" (click)="nudge(1)">▲</button>
          <button type="button" [attr.aria-label]="'Decrease ' + label"
            [disabled]="atMin" (click)="nudge(-1)">▼</button>
        </span>
      </div>
    </div>`,
})
export class NumberInputComponent {
  @Input() label = ""; @Input() id = "num"; @Input() value: number | null = null;
  @Input() min = -Infinity; @Input() max = Infinity; @Input() step = 1;
  @Output() valueChange = new EventEmitter<number | null>();
  error: string | null = null;
  isFinite = Number.isFinite;
  get precision() { return (String(this.step).split(".")[1] || "").length; }
  get atMin() { return this.value !== null && this.value <= this.min; }
  get atMax() { return this.value !== null && this.value >= this.max; }
  private clamp(n: number) { return Math.min(this.max, Math.max(this.min, n)); }
  private round(n: number) { const f = 10 ** this.precision; return Math.round(n * f) / f; }
  nudge(dir: 1 | -1) {
    const base = this.value ?? (this.min === -Infinity ? 0 : this.min);
    this.value = this.round(this.clamp(base + dir * this.step));
    this.valueChange.emit(this.value);
  }
  commit(raw: string) {
    if (raw.trim() === "") { this.value = null; this.valueChange.emit(null); return; }
    const n = Number(raw.replace(",", "."));
    if (Number.isFinite(n)) { this.value = this.round(this.clamp(n)); this.valueChange.emit(this.value); }
    else this.error = "Enter a valid number";
  }
  onKey(e: KeyboardEvent) {
    const map: Record<string, () => void> = {
      ArrowUp: () => this.nudge(1), ArrowDown: () => this.nudge(-1),
      Home: () => Number.isFinite(this.min) && this.nudgeTo(this.min),
      End:  () => Number.isFinite(this.max) && this.nudgeTo(this.max) };
    if (map[e.key]) { e.preventDefault(); map[e.key](); }
  }
  private nudgeTo(v: number) { this.value = v; this.valueChange.emit(v); }
}
```

### 9.11 Bootstrap 5 (input group + validation)

```plain
<div class="mb-3">
  <label for="bs-qty" class="form-label">Quantity</label>
  <div class="input-group has-validation" style="max-width: 180px;">
    <button class="btn btn-outline-secondary" type="button" id="bs-dec" aria-label="Decrease quantity">−</button>
    <input type="number" class="form-control text-center" id="bs-qty"
      inputmode="numeric" min="1" max="99" step="1" value="1"
      aria-describedby="bs-qty-fb" required>
    <button class="btn btn-outline-secondary" type="button" id="bs-inc" aria-label="Increase quantity">+</button>
    <div id="bs-qty-fb" class="invalid-feedback">Enter a whole number between 1 and 99.</div>
  </div>
</div>
<script>
  const q = document.getElementById('bs-qty');
  const clamp = (n) => Math.min(99, Math.max(1, n));
  document.getElementById('bs-inc').onclick = () => { q.value = clamp((+q.value || 0) + 1); };
  document.getElementById('bs-dec').onclick = () => { q.value = clamp((+q.value || 0) - 1); };
  q.addEventListener('blur', () => {
    const ok = q.value !== '' && +q.value >= 1 && +q.value <= 99 && Number.isInteger(+q.value);
    q.classList.toggle('is-invalid', !ok);
  });
</script>
```

### 9.12 Web Component (custom element, framework-agnostic)

```plain
class UjgNumberInput extends HTMLElement {
  static formAssociated = true;
  static observedAttributes = ["value", "min", "max", "step", "label"];
  #internals; #input; #inc; #dec;
  constructor() {
    super();
    this.#internals = this.attachInternals();
    this.attachShadow({ mode: "open" });
  }
  get value() { const n = Number(this.#input?.value); return Number.isFinite(n) ? n : null; }
  set value(v) { this.setAttribute("value", String(v)); }
  get min() { return Number(this.getAttribute("min") ?? -Infinity); }
  get max() { return Number(this.getAttribute("max") ?? Infinity); }
  get step() { return Number(this.getAttribute("step") ?? 1); }
  get #precision() { return (String(this.step).split(".")[1] || "").length; }
  connectedCallback() { this.#render(); }
  attributeChangedCallback() { if (this.#input) this.#sync(); }
  #clamp(n) { return Math.min(this.max, Math.max(this.min, n)); }
  #round(n) { const f = 10 ** this.#precision; return Math.round(n * f) / f; }
  #nudge(dir) {
    const base = this.value ?? (Number.isFinite(this.min) ? this.min : 0);
    this.#input.value = String(this.#round(this.#clamp(base + dir * this.step)));
    this.#sync(); this.#input.focus();
  }
  #sync() {
    const v = this.value;
    if (v !== null) this.#input.setAttribute("aria-valuenow", String(v));
    this.#dec.disabled = v !== null && v <= this.min;
    this.#inc.disabled = v !== null && v >= this.max;
    this.#internals.setFormValue(v === null ? "" : String(v));
    this.dispatchEvent(new CustomEvent("change", { detail: { value: v }, bubbles: true }));
  }
  #render() {
    this.shadowRoot.innerHTML = `
      <style>
        .c{display:flex;height:40px;border:1px solid #6b6b6b;border-radius:8px;overflow:hidden}
        input{flex:1;min-width:0;border:0;outline:0;text-align:right;padding:0 12px;font-size:16px}
        .s{display:flex;flex-direction:column;border-left:1px solid #6b6b6b}
        button{flex:1;width:32px;border:0;background:#f2f2f2;cursor:pointer}
        button:disabled{opacity:.4;cursor:not-allowed}
        .c:focus-within{box-shadow:0 0 0 2px #5F2C82}
      </style>
      <label>${this.getAttribute("label") ?? ""}</label>
      <div class="c">
        <input type="text" inputmode="${this.#precision > 0 ? "decimal" : "numeric"}"
          role="spinbutton" aria-valuemin="${this.min}" aria-valuemax="${this.max}"
          value="${this.getAttribute("value") ?? ""}" />
        <span class="s">
          <button part="inc" aria-label="Increase">▲</button>
          <button part="dec" aria-label="Decrease">▼</button>
        </span>
      </div>`;
    this.#input = this.shadowRoot.querySelector("input");
    this.#inc = this.shadowRoot.querySelector('[part="inc"]');
    this.#dec = this.shadowRoot.querySelector('[part="dec"]');
    this.#inc.onclick = () => this.#nudge(1);
    this.#dec.onclick = () => this.#nudge(-1);
    this.#input.addEventListener("blur", () => this.#sync());
    this.#input.addEventListener("wheel", () => this.#input.blur(), { passive: true });
    this.#input.addEventListener("keydown", (e) => {
      if (e.key === "ArrowUp") { e.preventDefault(); this.#nudge(1); }
      if (e.key === "ArrowDown") { e.preventDefault(); this.#nudge(-1); }
    });
    this.#sync();
  }
}
customElements.define("ujg-number-input", UjgNumberInput);
```

### 9.13 Python (Jinja2 render + FastAPI endpoint with server-side validation)

```python
# templates/quantity.html  (Jinja2)
# <form method="post" action="/cart/update">
#   <label for="qty">Quantity</label>
#   <div class="numfield__control">
#     <input id="qty" name="qty" type="number" inputmode="numeric"
#            min="{{ field.min }}" max="{{ field.max }}" step="{{ field.step }}"
#            value="{{ field.value }}"
#            aria-describedby="qty-err"
#            {% if field.error %}aria-invalid="true"{% endif %}>
#   </div>
#   {% if field.error %}<p id="qty-err" role="alert">{{ field.error }}</p>{% endif %}
# </form>

from fastapi import FastAPI, Form
from fastapi.responses import HTMLResponse
from fastapi.templating import Jinja2Templates
from pydantic import BaseModel, Field, ValidationError
from starlette.requests import Request

app = FastAPI()
templates = Jinja2Templates(directory="templates")

class Quantity(BaseModel):
    qty: int = Field(ge=1, le=99)  # server is the source of truth, not the client clamp

@app.post("/cart/update", response_class=HTMLResponse)
async def update_cart(request: Request, qty: str = Form(...)):
    try:
        data = Quantity(qty=int(qty))          # rejects "", "abc", "1.5", "-3", "200"
    except (ValueError, ValidationError):
        field = {"value": qty, "min": 1, "max": 99, "step": 1,
                 "error": "Enter a whole number between 1 and 99."}
        return templates.TemplateResponse("quantity.html", {"request": request, "field": field}, status_code=422)
    # persist data.qty ...
    field = {"value": data.qty, "min": 1, "max": 99, "step": 1, "error": None}
    return templates.TemplateResponse("quantity.html", {"request": request, "field": field})
```

### 9.14 SwiftUI (iOS — Stepper + formatted TextField)

```swift
import SwiftUI

struct QuantityField: View {
    @State private var quantity: Int = 1
    let range: ClosedRange<Int> = 1...99

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Quantity").font(.subheadline).bold()
            HStack(spacing: 0) {
                TextField("Quantity", value: $quantity, format: .number)
                    .keyboardType(.numberPad)
                    .multilineTextAlignment(.trailing)
                    .padding(.horizontal, 12)
                    .onChange(of: quantity) { _, new in
                        quantity = min(range.upperBound, max(range.lowerBound, new)) // clamp
                    }
                Stepper("", value: $quantity, in: range)
                    .labelsHidden()
                    .accessibilityLabel("Quantity")
                    .accessibilityValue("\(quantity)")
            }
            .frame(height: 44)
            .overlay(RoundedRectangle(cornerRadius: 8).stroke(.secondary))
        }
    }
}
```

### 9.15 Jetpack Compose (Android)

```kotlin
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp

@Composable
fun QuantityField(min: Int = 1, max: Int = 99) {
    var value by remember { mutableStateOf(1) }
    fun clamp(n: Int) = n.coerceIn(min, max)

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text("Quantity", style = MaterialTheme.typography.labelLarge)
        Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
            IconButton(onClick = { value = clamp(value - 1) }, enabled = value > min,
                modifier = Modifier.semantics { contentDescription = "Decrease quantity" }) { Text("−") }
            OutlinedTextField(
                value = value.toString(),
                onValueChange = { s -> s.toIntOrNull()?.let { value = clamp(it) } ?: run { if (s.isEmpty()) value = min } },
                singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                modifier = Modifier.width(96.dp).semantics {
                    stateDescription = "Quantity $value"
                })
            IconButton(onClick = { value = clamp(value + 1) }, enabled = value < max,
                modifier = Modifier.semantics { contentDescription = "Increase quantity" }) { Text("+") }
        }
    }
}
```

### 9.16 Flutter (Dart)

```dart
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

class QuantityField extends StatefulWidget {
  const QuantityField({super.key, this.min = 1, this.max = 99});
  final int min; final int max;
  @override State<QuantityField> createState() => _QuantityFieldState();
}

class _QuantityFieldState extends State<QuantityField> {
  late final TextEditingController _c = TextEditingController(text: '${widget.min}');
  int get _value => int.tryParse(_c.text) ?? widget.min;
  int _clamp(int n) => n.clamp(widget.min, widget.max);
  void _set(int n) => setState(() => _c.text = '${_clamp(n)}');

  @override
  Widget build(BuildContext context) {
    return Column(crossAxisAlignment: CrossAxisAlignment.start, children: [
      const Text('Quantity', style: TextStyle(fontWeight: FontWeight.w600)),
      const SizedBox(height: 4),
      SizedBox(height: 44, child: Row(children: [
        Semantics(button: true, label: 'Decrease quantity',
          child: IconButton(onPressed: _value > widget.min ? () => _set(_value - 1) : null,
            icon: const Icon(Icons.remove))),
        Expanded(child: TextField(
          controller: _c, textAlign: TextAlign.right,
          keyboardType: TextInputType.number,
          inputFormatters: [FilteringTextInputFormatter.digitsOnly],
          onSubmitted: (s) => _set(int.tryParse(s) ?? widget.min),
          decoration: const InputDecoration(border: OutlineInputBorder()))),
        Semantics(button: true, label: 'Increase quantity',
          child: IconButton(onPressed: _value < widget.max ? () => _set(_value + 1) : null,
            icon: const Icon(Icons.add))),
      ])),
    ]);
  }
}
```

### 9.17 Testing (Vitest + RTL unit, jest-axe, Playwright E2E)

```plain
// number-input.test.tsx — Vitest + React Testing Library + jest-axe
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { axe } from "jest-axe";
import { useState } from "react";
import { NumberInput } from "./number-input";

function Harness(props: any) {
  const [v, setV] = useState<number | null>(props.value ?? 1);
  return <NumberInput label="Quantity" value={v} onChange={setV} min={1} max={5} step={1} {...props} />;
}

test("arrow up increments and clamps at max", async () => {
  const user = userEvent.setup();
  render(<Harness value={4} />);
  const input = screen.getByRole("spinbutton", { name: /quantity/i });
  await user.type(input, "{ArrowUp}");
  expect(input).toHaveAttribute("aria-valuenow", "5");
  await user.type(input, "{ArrowUp}");           // already at max
  expect(input).toHaveAttribute("aria-valuenow", "5");
});

test("blur clamps out-of-range typed value", async () => {
  const user = userEvent.setup();
  render(<Harness />);
  const input = screen.getByRole("spinbutton");
  await user.clear(input); await user.type(input, "99"); fireEvent.blur(input);
  expect(input).toHaveValue("5");
});

test("decrement button disabled at min", () => {
  render(<Harness value={1} />);
  expect(screen.getByRole("button", { name: /decrease quantity/i })).toBeDisabled();
});

test("no axe violations", async () => {
  const { container } = render(<Harness />);
  expect(await axe(container)).toHaveNoViolations();
});
// quantity.e2e.spec.ts — Playwright
import { test, expect } from "@playwright/test";

test("scroll wheel does not change a focused quantity field", async ({ page }) => {
  await page.goto("/cart");
  const qty = page.getByRole("spinbutton", { name: /quantity/i });
  await qty.focus();
  const before = await qty.inputValue();
  await page.mouse.wheel(0, 400);          // scroll over the focused field
  await expect(qty).toHaveValue(before);   // guard must hold
});

test("server rejects out-of-range even if client is bypassed", async ({ request }) => {
  const res = await request.post("/cart/update", { form: { qty: "500" } });
  expect(res.status()).toBe(422);
});
```

* * *

## 10\. Accessibility

Number Input is an interactive control that both accepts free text _and_ exposes a min/max/now value model, so it has two accessibility contracts to satisfy at once: text-editing semantics and spinbutton semantics.

**Roles & ARIA:**

*   A native `<input type="number">` already has an implicit **spinbutton** role — do not add `role="spinbutton"` to it. For a _custom_ text-input-based control, add `role="spinbutton"` explicitly.
*   `aria-valuemin`, `aria-valuemax`, `aria-valuenow` mirror the numeric bounds and current value. Omit `aria-valuemin`/`max` when the bound is truly unbounded rather than setting `±Infinity`.
*   `aria-valuetext` carries the _human_ reading when the raw number isn't the message — `"$9.99"`, `"25 %"`, `"Large"`. Screen readers announce `aria-valuetext` in preference to `aria-valuenow`.
*   Associate the label with `<label for>` or `aria-labelledby`; associate help/error with `aria-describedby`. Set `aria-invalid="true"` only on _committed_ errors.
*   Stepper buttons are real `<button>`s with explicit `aria-label` ("Increase quantity"). Give them `tabIndex={-1}` when the input itself handles Arrow keys, so keyboard users aren't forced to tab through three stops per field — but they must remain operable by pointer and expose their name/role.

**Keyboard map (interactive):**

| Key | Action |
| ---| --- |
| `ArrowUp` | Increment by `step` |
| `ArrowDown` | Decrement by `step` |
| `PageUp` | Increment by a large step (e.g. 10 × `step`) |
| `PageDown` | Decrement by a large step |
| `Home` | Set to `min` (if defined) |
| `End` | Set to `max` (if defined) |
| `0`–`9`, `.`, `-` | Type the value directly |
| `Tab` / `Shift+Tab` | Move to next / previous control |
| `Enter` | Commit value / submit form |

**Focus management:** Focus stays on the input while Arrow/Page/Home/End operate — it does _not_ jump to the stepper buttons. When a stepper is clicked, return focus to the input afterward so keyboard flow continues. When a stepper becomes `disabled` at a bound, move focus back to the input (a disabled button that had focus strands the keyboard user).

**Contrast:** Input text and label ≥ 4.5:1 against the field background (WCAG 1.4.3). The field border, the stepper glyphs (▲▼), and the focus ring are non-text UI and need ≥ 3:1 (WCAG 1.4.11) — a common failure is a pale-gray 1px border under 3:1. The focus ring must be visible in every skin.

**Target size:** Stepper buttons must meet the 44 × 44px (or 24 × 24px minimum with spacing, WCAG 2.5.8) target. This is why the tiny stacked `▲▼` on Small fields fail on touch — bump to the Large tier or use split steppers on touch-primary flows.

**Reduced motion:** Under `prefers-reduced-motion: reduce`, drop stepper press animations, value roll/odometer transitions, and any spinner easing; state changes must be instantaneous, not animated.

**Common failures specific to Number Input:**

*   Custom steppers built as `<div>`/`<span>` with click handlers — no role, no keyboard, invisible to AT.
*   Missing `aria-valuenow` updates, so the screen reader keeps announcing the old value after a nudge.
*   Announcing raw `9.99` when the field means `$9.99` (no `aria-valuetext`).
*   Placeholder-as-label — the label vanishes the moment the user types.
*   `type="number"` with `maxlength` that silently does nothing (browsers ignore `maxlength` on number inputs) — users think the length is capped and it isn't.
*   Error announced only by color (red border) with no `aria-describedby`/`role="alert"` text.
* * *

## 11\. Innovative / Emerging Ideas

*   **Headless, locale-correct number fields as the default** (react-aria `useNumberField`, Base UI `NumberField`) that parse `1.234,56` vs `1,234.56`, handle scientific notation, and clamp — replacing native `type="number"` for anything shipping to multiple locales.
*   **`Intl.NumberFormat`** **live-editing with caret preservation** — format thousands separators _as the user types_ (not only on blur), tracking caret offset around inserted separators so editing in the middle of a formatted number stays sane.
*   **`CSS ::-webkit-*`** **retirement +** **`field-sizing: content`** — auto-growing number fields that size to their digits (great for inline "× 12" quantities), a 2026-era CSS capability replacing JS width hacks.
*   **`inputmode`** **+ virtual-keyboard API coordination** — using the on-screen keyboard geometry (`navigator.virtualKeyboard`) to keep a focused number field above the keypad on mobile, so the value stays visible while typing.
*   **Voice and gesture increment** — pairing the number field with a Web Speech grammar ("set quantity to twelve") and pointer-drag-to-scrub on the label, both writing through the same clamped setter.
*   **Unit-aware inputs** — a single field that accepts `2kg`, `2000g`, or `4.4lb` and normalizes to a canonical unit on commit, showing the normalized value in `aria-valuetext`.
* * *

## 12\. Conversion / UX Killers

*   **Wrong keyboard on mobile.** Shipping `type="text"` (full QWERTY) for a quantity, or `type="number"` for a phone number, forces keyboard-toggling or wrong keys. _Fix:_ pick `inputmode` deliberately — `numeric` for integer-only, `decimal` for money/measures, and a plain text + `inputmode="numeric"` mask for identifiers.
*   **Scroll-wheel silently corrupting the value.** A native number field changes when the mouse wheel scrolls over it while focused; users submit a quantity of 47 they never chose. _Fix:_ blur on `wheel`, or `preventDefault` the wheel while focused. (Shown in §9.3/9.4.)
*   **Screaming validation while the user is still typing.** Marking `"1."` or `"-"` as invalid mid-keystroke turns the field red before they finish. _Fix:_ validate on blur/submit; keep transient invalid silent, use `aria-invalid` only on commit.
*   **Killing leading zeros / treating empty as zero.** A number field strips `007` to `7` and reports empty as `""` not `0`; naive code posts `0` for a blank price. _Fix:_ if leading zeros matter it's not a number input (use text); handle empty explicitly, don't coerce to zero.
*   **Floating-point drift on decimal steps.** `step="0.1"` nudged repeatedly yields `0.30000000000000004`; a money total ends in `.999999`. _Fix:_ do integer-unit math (work in cents), round to the field's precision on every commit.
*   **No visible range until an error.** The user types 500, submits, _then_ learns the max is 99. _Fix:_ state the range in helper text up front ("1–99") and clamp on blur, not only at submit.
*   **Steppers with no keyboard / touch target too small.** Custom `<div>` steppers or 16px `▲▼` on mobile — unreachable by keyboard and hard to tap. _Fix:_ real buttons, 44px targets, Arrow-key support on the input.
* * *

## 13\. Advanced Patterns

*   **TypeScript-enforced a11y via a discriminated union of value modes.** Force callers to declare intent at compile time so a phone number can never be handed to arithmetic logic:

```plain
type NumberMode =
  | { kind: "integer"; min: number; max: number; step?: number }
  | { kind: "decimal"; min: number; max: number; step: number; precision: number }
  | { kind: "currency"; min: number; max: number; currency: string }; // no free step

type Props =
  | (NumberMode & { label: string; value: number | null; onChange: (v: number | null) => void });
// `precision` is *required* on decimal, *forbidden* on integer — the compiler enforces the model.
```

*   **Controlled + uncontrolled hybrid with an imperative commit ref.** Expose `ref.current.commit()` and `ref.current.getNumericValue()` via `useImperativeHandle` so a parent form can force-validate/clamp all fields on submit without lifting every keystroke into React state (avoids re-render storms on large forms).
*   **Design-token tiering for the whole field.** Drive every visual from CSS custom properties layered as global → semantic → component, so one control themes across all 11 skins and light/dark:

```css
:root { --ujg-goldenrod: #DCA424; --ujg-eminence: #5F2C82; --ujg-danger: #C0362C; }
.numfield { /* semantic → component */
  --field-border: var(--color-border, #6b6b6b);
  --field-focus:  var(--color-focus, var(--ujg-eminence));
  --field-danger: var(--color-danger, var(--ujg-danger));
  --stepper-bg:   var(--color-surface-2, #f2f2f2);
}
[data-theme="dark"] .numfield { --field-border:#555; --stepper-bg:#222; }
```

*   **Debounced async bound-checking with an** **`aria-live`** **status.** For "max = current stock" fields, revalidate the ceiling against the server on a debounce and announce "Only 3 left" through a polite live region without blocking typing.
* * *

## 14\. Performance & Bundle Cost

*   **Don't ship a 50KB masked-input library for a quantity field.** Native `type="number"` or a small hand-rolled parser (like §9.4) is a few hundred bytes; reach for `react-number-format`/`cleave.js` only when you genuinely need live locale masking. _Tree-shake:_ import `Intl.NumberFormat` (built in, zero bytes) before adding a formatting dependency.
*   **Compositor-only stepper animations.** If you animate the stepper press or a value "roll," animate `transform`/`opacity` only — never `height`, `top`, or `box-shadow` on the hot path — so nudging a field in a long form doesn't cause layout thrash at 60fps.
*   **Virtualize number-input-heavy grids.** A spreadsheet-like table with hundreds of editable number cells must render only visible rows (`@tanstack/virtual`); mounting 500 controlled inputs with per-keystroke state will jank. Keep off-screen cells as plain text, upgrade to an input on focus.
*   **Debounce commits, not keystrokes-to-DOM.** Update the input's own text immediately (uncontrolled or local state) but debounce the expensive `onChange` that hits validation/network, so typing "1250" doesn't fire four server round-trips.
*   **`Intl.NumberFormat`** **instances are expensive to construct** — create one per locale/options and reuse it (module-level or memoized), don't `new Intl.NumberFormat(...)` on every render or keystroke.
*   **Avoid re-rendering the whole form per field.** Isolate each Number Input's state (or use an uncontrolled + ref pattern) so one field's keystroke doesn't re-render a 40-field form.
* * *

## 15\. Security

A Number Input's attack surface is small but real, and it is almost entirely on the server side of the boundary.

*   **Never trust client-side clamping.** `min`/`max`/`step` and every bit of JS validation are UX conveniences that an attacker bypasses trivially (DevTools, curl, replaying the request). The server MUST re-validate type, range, integer-ness, and step — the §9.13 Pydantic model and §9.6 Zod action are the real gatekeepers. A quantity field that only clamps client-side lets someone POST `qty=-5` or `qty=999999` and, if unguarded, order negative stock or trigger an integer-overflow price.
*   **Numeric-specific injection risks downstream.** A "number" that arrives as the string `"1 OR 1=1"` or `"1e309"` (→ `Infinity`) or `"NaN"` must be coerced through strict integer/decimal parsing _before_ it touches SQL, a price calculation, or an array index. Parameterize queries; reject non-finite values. Business-logic exploits (negative quantities creating credits, huge quantities causing DoS-by-computation) matter more here than XSS.
*   **XSS on echo-back.** If a rejected value is reflected into the page ("`<script>` is not a valid quantity"), it must be HTML-escaped — the field accepts arbitrary typed text before validation, so the raw string can carry a payload. Jinja2 autoescape / React's default escaping handle this; a manual `innerHTML` error message does not.
*   **Precision as a financial-integrity concern.** For money, float rounding isn't just a UX bug — inconsistent client/server rounding lets a crafted `0.001`\-style value skim fractions. Fix the precision contract server-side (integer cents), and validate `step` compliance there.
*   **Authorization:** the field itself gates nothing, but the _action_ behind it often does (changing a price, a credit amount, an admin-only limit). Enforce authz on the endpoint; a hidden or `disabled` max on the client is not a permission boundary.
* * *

## 16\. Senior-Level Checklist

*   **Semantics:** Native `<input type="number">` _or_ an explicit `role="spinbutton"` on a text input — never a `<div>` with click handlers. Correct `inputmode` (`numeric` vs `decimal`) chosen deliberately.
*   **Keyboard:** Arrow Up/Down (± step), Page Up/Down (large step), Home/End (min/max), direct typing — all working, with `Tab` reaching the control in one stop.
*   **Focus:** Visible `:focus-visible` ring on field and (independently) on steppers; focus returns to the input after a stepper click; no focus stranded on a stepper that just became disabled.
*   **Contrast:** Text ≥ 4.5:1; border, stepper glyphs, and focus ring ≥ 3:1; error state distinguishable without relying on color alone.
*   **Target size:** Stepper buttons ≥ 44px (or 24px + spacing) on touch; Small-tier stacked steppers not used on touch-primary flows.
*   **States:** default, hover, focus, active, min-reached, max-reached, invalid, disabled, read-only, loading — all designed and wired, with min/max disabling the correct stepper.
*   **Value integrity:** clamps to min/max on blur, snaps to `step`, precision-safe decimal math (integer units), empty ≠ zero, leading-zero/identifier cases routed to a text input instead.
*   **Scroll-wheel guard:** wheel over a focused field does not change the value.
*   **Server validation:** endpoint re-validates type/range/step/finite-ness; client clamping treated as UX only; queries parameterized.
*   **i18n:** locale decimal separator handled (comma vs dot); `aria-valuetext` set when the display value differs from the raw number ($, %, units).
*   **Performance:** number-heavy grids virtualized; `Intl.NumberFormat` instances reused; expensive `onChange` debounced.
*   **Testing:** unit (clamp/step/keyboard), `jest-axe` (zero violations), Playwright (scroll-wheel guard + server rejection of bypassed input).
* * *

## 17\. Visual Styles

The same Number Input, eleven aesthetics. In every one the semantics, keyboard map, focus ring, and 44px stepper targets are identical — only the surface changes.

1. **Flat** — 1px solid border, no shadow, flat fill; steppers are plain rectangles with a hairline divider and a simple hover tint. The `▲▼` glyphs are single-weight, the focus ring a 2px solid outline.
2. **Material** — Filled or outlined field with a floating label that rises on focus, an ink-colored 2px bottom/outline accent, and a ripple state-layer under each stepper press. Error line and helper text sit on the Material baseline grid.
3. **Glassmorphism** — The field is a frosted `backdrop-filter: blur()` pane with a translucent white border; steppers are semi-transparent tiles that let the background bleed through, `▲▼` in high-contrast so they survive the blur.
4. **Liquid Glass (2026 — Apple iOS 26 / macOS Tahoe)** — The stepper ± buttons are refractive glass lozenges that bend the wallpaper behind them and specular-highlight on press; the value text floats on a lensed, light-warping field surface with a soft rim light.
5. **Neumorphism** — The whole control is extruded from a same-color surface: the field is a soft _inset_ groove (dual inner shadow) and each stepper is a _raised_ soft button that presses inward on `:active`. Beautiful but a contrast trap — the focus ring and border must be forced to ≥3:1.
6. **Skeuomorphism** — A physical odometer/calculator look: beveled metal stepper keys with a pressed-key highlight, an inset "LCD" number readout with faint segment texture, a subtle inner drop-shadow on the value well.
7. **Neo-Brutalism** — Thick 2–3px pure-black border, hard offset drop-shadow (`4px 4px 0 #000`), no radius; steppers are chunky black-bordered blocks in a raw accent (Spanish Orange), `▲▼` heavy and blocky, focus ring a solid black shift.
8. **Claymorphism** — Puffy rounded field with soft double shadow (dark below, light above) giving a 3D clay pillow; steppers are squishy rounded pills that deform on press, pastel fills, chunky friendly `+ −` glyphs.
9. **Aurora / Gradient** — Field border or focus ring is an animated multi-stop gradient (eminence→orange→goldenrod); the active stepper glows with a gradient wash, the value sits on a subtly luminous dark surface. Gradient motion respects reduced-motion.
10. **Minimal / Swiss** — No border box at all: a single bottom rule under the number, generous whitespace, tabular-lining figures, tiny understated `▲▼` in gray. Type-led, grid-aligned, label in a strict sans.
11. **UJG Brand (house default)** — Goldenrod (`#DCA424`) `▲▼` glyphs and focus ring on an Eminence (`#5F2C82`) / Night (`#0A0A0A`) field; Urbanist numerals, right-aligned, tabular; steppers as slim goldenrod-bordered tiles that fill goldenrod on hover, error in a warm Sienna-adjacent red so it stays inside the brand world.

**The rule that never changes:** style is skin, behavior is the skeleton. Across all eleven skins the Number Input keeps `role="spinbutton"` (or native), the same Arrow/Page/Home/End keyboard map, a visible ≥3:1 focus ring, ≥44px stepper targets, min/max clamping, step precision, and server-side re-validation. Change the paint, never the wiring.
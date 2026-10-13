import {
  ApplicationRef,
  Component,
  ElementRef,
  booleanAttribute,
  computed,
  effect,
  forwardRef,
  inject,
  input,
  model,
  output,
  signal,
  untracked,
  type Signal,
} from "@angular/core";
import { ControlValueAccessor, NG_VALUE_ACCESSOR } from "@angular/forms";
import { CONTROL_ICON_SIZE, numberFieldStyles, type NumberFieldVariants } from "@ui-organized/core";
import { UioPart, stateFlag } from "../part.js";
import { nextMachineId } from "../part-ids.js";
import { UioIcon } from "../icons/icon.js";
import { UioFieldContext } from "../field/field-context.js";
import { UioFieldLabel } from "../field/field.js";
import { UioFieldError } from "../field-error/field-error.js";
import { flushNow } from "../overlay/flush.js";

export type NumberFieldSize = NonNullable<NumberFieldVariants["size"]>;

/**
 * Zag's defaults, and the two numbers that show up in the rendered ARIA.
 *
 * A number field always reports a range even when the caller gave none, so
 * `aria-valuemin`/`aria-valuemax` are never absent — and the steppers' at-a-bound
 * arithmetic always has two numbers to work with.
 */
const MIN = Number.MIN_SAFE_INTEGER;
const MAX = Number.MAX_SAFE_INTEGER;

/** `NaN` counts as zero for every range test. Zag's `nan()`, which is load-bearing:
 * an empty field with `min: 0` reports its decrement stepper disabled. */
const orZero = (value: number) => (Number.isNaN(value) ? 0 : value);

/**
 * Add or subtract without the binary-floating-point residue.
 *
 * `0.1 + 0.2` is 0.30000000000000004, and a number field stepping by 0.1 would
 * show it. Zag's `decimalOp`: scale both operands to integers by the wider of
 * their decimal places, operate, scale back.
 */
export function stepValue(value: number, step: number, direction: 1 | -1): number {
  const a = orZero(value);
  const b = step * direction;
  if (a % 1 === 0 && b % 1 === 0) return a + b;
  const decimals = (n: number) => {
    const text = String(n);
    const point = text.indexOf(".");
    return point === -1 ? 0 : text.length - point - 1;
  };
  const scale = 10 ** Math.max(decimals(a), decimals(b));
  return (Math.round(a * scale) + Math.round(b * scale)) / scale;
}

/**
 * A number with steppers, its label, helper text and error already wired to it.
 *
 * ```html
 * <div uioNumberField label="Quantity" [min]="0" [max]="10" [(value)]="qty"></div>
 * ```
 *
 * ── Two machines, one control ───────────────────────────────────────────────
 *
 * The rendered element is a `field` root holding a `number-input` root, because
 * that is what the other three libraries compose: Ark's Field supplies the
 * label, the helper text, the error and the `aria-describedby` that names them;
 * Ark's NumberInput supplies the spinbutton and its two triggers. Both scopes
 * are written out here rather than assembled from directives, for the reason
 * `UioInput` gives — the whole attribute surface is then visible in one place
 * next to the state that drives it.
 *
 * Unlike `UioInput`, the field root here **does** report `data-disabled`: React
 * passes `disabled` to `Field.Root`, so the label greys with the control.
 * `readOnly` and `required` are not passed on, so no part of the field reports
 * them — only the input itself does, as native attributes.
 *
 * ── What the steppers actually mean ─────────────────────────────────────────
 *
 * A stepper is disabled when the field is disabled, when it is read-only, or
 * when the value is already at that bound. The last one is the interesting case
 * and the one a DOM diff at rest can see: `min: 0` with a value of `0` disables
 * decrement and leaves increment live. An **empty** field with `min: 0` disables
 * it too, because an empty value counts as zero for the comparison.
 */
@Component({
  selector: "div[uioNumberField]",
  standalone: true,
  exportAs: "uioNumberField",
  providers: [
    UioFieldContext,
    { provide: NG_VALUE_ACCESSOR, useExisting: forwardRef(() => UioNumberField), multi: true },
  ],
  imports: [UioIcon, UioFieldLabel, UioFieldError],
  template: `
    @if (label(); as text) {
      <label uioFieldLabel
        >{{ text }}@if (required()) {<span class="field__required" aria-hidden="true"></span>}</label
      >
    }
    <div
      data-scope="number-input"
      data-part="root"
      [id]="rootId"
      [attr.data-disabled]="flag(isDisabled())"
      [attr.data-focus]="flag(focused())"
      [attr.data-invalid]="flag(invalid())"
    >
      <div
        class="number-field__group"
        data-scope="number-input"
        data-part="control"
        role="group"
        [attr.aria-disabled]="isDisabled()"
        [attr.aria-invalid]="invalid() ? 'true' : null"
        [attr.data-disabled]="flag(isDisabled())"
        [attr.data-focus]="flag(focused())"
        [attr.data-invalid]="flag(invalid())"
      >
        <button
          class="number-field__stepper"
          data-scope="number-input"
          data-part="decrement-trigger"
          type="button"
          tabindex="-1"
          aria-label="Decrease"
          [id]="partId('dec')"
          [attr.aria-controls]="field.controlId"
          [attr.data-disabled]="flag(decrementDisabled())"
          [disabled]="decrementDisabled()"
          (pointerdown)="press($event, -1)"
        >
          <span uioIcon name="minus" [size]="iconSize()"></span>
        </button>
        <!--
          type="text" with role="spinbutton", not type="number". A native
          number input cannot hold a partially-typed or formatted value — "$12.0"
          is not parseable and the browser silently blanks it — so every library
          here spells the value itself and reports the range through ARIA.
        -->
        <input
          class="field__control number-field__input"
          data-scope="number-input"
          data-part="input"
          type="text"
          role="spinbutton"
          inputmode="decimal"
          autocomplete="off"
          autocorrect="off"
          spellcheck="false"
          aria-roledescription="numberfield"
          [id]="field.controlId"
          [attr.name]="name()"
          [attr.placeholder]="placeholder()"
          [attr.pattern]="format() ? null : PATTERN"
          [attr.required]="required() ? '' : null"
          [attr.readonly]="readOnlyInput() ? '' : null"
          [attr.aria-invalid]="invalid() ? 'true' : null"
          [attr.aria-describedby]="field.describedBy()"
          [attr.aria-valuemin]="minValue()"
          [attr.aria-valuemax]="maxValue()"
          [attr.aria-valuenow]="valueNow()"
          [attr.data-invalid]="flag(invalid())"
          [attr.data-disabled]="flag(isDisabled())"
          [disabled]="isDisabled()"
          [value]="text()"
          (input)="write($event)"
          (keydown)="onKeydown($event)"
          (focus)="focused.set(true)"
          (blur)="onBlur()"
        />
        <button
          class="number-field__stepper"
          data-scope="number-input"
          data-part="increment-trigger"
          type="button"
          tabindex="-1"
          aria-label="Increase"
          [id]="partId('inc')"
          [attr.aria-controls]="field.controlId"
          [attr.data-disabled]="flag(incrementDisabled())"
          [disabled]="incrementDisabled()"
          (pointerdown)="press($event, 1)"
        >
          <span uioIcon name="plus" [size]="iconSize()"></span>
        </button>
      </div>
    </div>
    @if (helperVisible()) {
      <span
        class="field__description"
        data-scope="field"
        data-part="helper-text"
        [id]="field.partId('helper-text')"
      >{{ helperText() }}</span>
    }
    <span uioFieldError [message]="errorMessage()"></span>
  `,
  host: {
    role: "group",
    "[class]": "hostClass()",
    "[id]": "field.rootId",
  },
})
export class UioNumberField extends UioPart implements ControlValueAccessor {
  readonly scope = "field";
  readonly part = "root";

  /** `null` is an empty field, which is not the same as `0`. */
  readonly value = model<number | null | undefined>(undefined);
  readonly valueChange = output<number | null>();

  readonly label = input<string | undefined>(undefined);
  readonly helperText = input<string | undefined>(undefined);
  /** A string shows a message; `true` marks the field invalid without one. */
  readonly error = input<string | boolean | undefined>(undefined);
  readonly size = input<NumberFieldSize>("md");
  /**
   * Undefined rather than the safe-integer bounds, so a caller can bind
   * `[min]="maybeUndefined"` without the transform turning it into `NaN`. The
   * bounds the arithmetic and the ARIA actually use are `minValue`/`maxValue`.
   */
  readonly min = input<number | undefined>(undefined);
  readonly max = input<number | undefined>(undefined);
  readonly step = input<number | undefined>(undefined);
  /** `Intl.NumberFormat` options — a currency or percent field formats its value. */
  readonly format = input<Intl.NumberFormatOptions | undefined>(undefined);
  readonly placeholder = input<string | undefined>(undefined);
  readonly name = input<string | undefined>(undefined);
  readonly required = input(false, { transform: booleanAttribute });
  /**
   * Not an override of the base's `readOnly`, deliberately.
   *
   * `UioPart` binds `data-readonly` from that signal and the field root reports
   * none: React passes only `disabled` and `invalid` to `Field.Root`, so
   * read-only stays the input's own native attribute. Same split, for the same
   * reason, as `disabledInput` on `UioAccordion`.
   */
  protected readonly readOnlyInput = input(false, {
    alias: "readOnly",
    transform: booleanAttribute,
  });

  /** `disabled` comes from the caller *or* from a reactive form's disabled state. */
  protected readonly disabledInput = input(false, {
    alias: "disabled",
    transform: booleanAttribute,
  });
  private readonly formDisabled = signal(false);
  readonly isDisabled = computed(() => this.disabledInput() || this.formDisabled());
  override readonly disabled: Signal<boolean> = this.isDisabled;

  override readonly invalid: Signal<boolean> = computed(() => !!this.error());
  protected readonly errorMessage = computed(() =>
    typeof this.error() === "string" ? (this.error() as string) : undefined,
  );
  protected readonly helperVisible = computed(() => !!this.helperText() && !this.invalid());

  protected readonly field = inject(UioFieldContext);
  private readonly host = inject<ElementRef<HTMLElement>>(ElementRef);
  private readonly appRef = inject(ApplicationRef);

  protected readonly hostClass = computed(() => numberFieldStyles({ size: this.size() }));
  protected readonly iconSize = computed(() => CONTROL_ICON_SIZE[this.size()]);
  protected readonly focused = signal(false);
  protected readonly minValue = computed(() => this.min() ?? MIN);
  protected readonly maxValue = computed(() => this.max() ?? MAX);
  protected readonly stepAmount = computed(() => this.step() ?? 1);
  protected readonly flag = stateFlag;
  /** Zag's default: digits, an optional sign, an optional decimal tail. */
  protected readonly PATTERN = "-?[0-9]*(.[0-9]+)?";

  /** Ark's ids: `number-input:<machine>` for the root, `:dec`/`:inc` for the triggers. */
  private readonly machine = nextMachineId();
  protected get rootId(): string {
    return `number-input:${this.machine}`;
  }
  protected partId(part: string): string {
    return `number-input:${this.machine}:${part}`;
  }

  /**
   * What the input is showing, which is not `value()` and cannot be derived from
   * it.
   *
   * A half-typed number — `"1."`, `"-"`, `"$1,2"` — parses to something whose
   * formatting is not the text the user has in front of them, so reformatting on
   * every keystroke would fight the caret. Zag keeps the string as the machine's
   * context and derives the number from it; this is the same arrangement, with
   * the effect below re-deriving the text only when the *value* is changed from
   * outside.
   */
  protected readonly text = signal("");

  constructor() {
    super();
    this.field.bind({
      invalid: this.invalid,
      // React passes `disabled` to Field.Root and nothing else, so the label
      // greys with the control while `required` and `readOnly` stay the input's
      // own native attributes. See the class note.
      disabled: this.isDisabled,
      required: signal(false),
      readOnly: signal(false),
    });
    this.field.describe("helper-text", this.helperVisible);

    effect(() => {
      const next = this.value();
      untracked(() => {
        const shown = this.parse(this.text());
        const wanted = next ?? Number.NaN;
        // `Number.isNaN` on both sides, because `NaN !== NaN` would reformat an
        // empty field on every pass and blank a "-" the user is halfway through.
        if (Number.isNaN(shown) && Number.isNaN(wanted)) return;
        if (shown === wanted) return;
        this.text.set(this.formatNumber(wanted));
      });
    });
  }

  // ── The value, in both directions ─────────────────────────────────────────

  private get formatter(): Intl.NumberFormat | null {
    const options = this.format();
    return options ? new Intl.NumberFormat(undefined, options) : null;
  }

  protected formatNumber(value: number): string {
    if (Number.isNaN(value)) return "";
    return this.formatter?.format(value) ?? String(value);
  }

  /**
   * Text to number.
   *
   * Without `format` this is `parseFloat`, exactly as Zag does. With it, Zag
   * reaches for `@internationalized/number`'s locale-aware parser; this strips
   * the characters a formatted number carries — currency symbols, group
   * separators, whitespace — and parses what is left. That is the one place this
   * component is a reimplementation rather than a reproduction: it handles the
   * formats this library's own `Intl` output produces, and would not survive a
   * locale whose decimal separator is a comma. Nothing renders differently; a
   * caller formatting in `de-DE` would.
   */
  protected parse(text: string): number {
    if (!this.format()) return Number.parseFloat(text);
    if (text === "") return Number.NaN;
    return Number.parseFloat(text.replace(/[^\d.\-eE+]/g, ""));
  }

  protected readonly valueAsNumber = computed(() => this.parse(this.text()));
  protected readonly valueNow = computed(() => {
    const value = this.valueAsNumber();
    return Number.isNaN(value) ? null : value;
  });

  private clamp(value: number): number {
    return Math.min(Math.max(orZero(value), this.minValue()), this.maxValue());
  }

  // ── Steppers ──────────────────────────────────────────────────────────────

  protected readonly incrementDisabled = computed(
    () =>
      this.isDisabled() || this.readOnlyInput() || orZero(this.valueAsNumber()) >= this.maxValue(),
  );
  protected readonly decrementDisabled = computed(
    () =>
      this.isDisabled() || this.readOnlyInput() || orZero(this.valueAsNumber()) <= this.minValue(),
  );

  /**
   * `pointerdown` rather than `click`, and the default is prevented.
   *
   * Zag binds the same event for the same reason: a stepper must not take focus
   * away from the input, so the press is handled before the browser moves it and
   * the input is focused explicitly instead. That is also why the number-input
   * root and control report `data-focus` after a stepper is clicked.
   */
  protected press(event: PointerEvent, direction: 1 | -1): void {
    if (direction === 1 ? this.incrementDisabled() : this.decrementDisabled()) return;
    if (event.button !== 0) return;
    event.preventDefault();
    this.stepBy(direction);
    this.host.nativeElement.querySelector("input")?.focus({ preventScroll: true });
    this.focused.set(true);
    flushNow(this.appRef);
  }

  protected stepBy(direction: 1 | -1): void {
    const next = this.clamp(stepValue(this.valueAsNumber(), this.stepAmount(), direction));
    this.text.set(this.formatNumber(next));
    this.commit(next);
  }

  // ── Typing ────────────────────────────────────────────────────────────────

  protected write(event: Event): void {
    const next = (event.target as HTMLInputElement).value;
    // Kept in step with the element so the `[value]` binding never has to fight
    // the user for the caret — the same rule `UioInput` follows.
    this.text.set(next);
    this.commit(this.parse(next));
  }

  protected onKeydown(event: KeyboardEvent): void {
    if (this.readOnlyInput()) return;
    if (event.key === "ArrowUp") {
      event.preventDefault();
      if (!this.incrementDisabled()) this.stepBy(1);
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      if (!this.decrementDisabled()) this.stepBy(-1);
    } else if (event.key === "Home" && !event.metaKey && !event.ctrlKey) {
      event.preventDefault();
      this.setTo(this.minValue());
    } else if (event.key === "End" && !event.metaKey && !event.ctrlKey) {
      event.preventDefault();
      this.setTo(this.maxValue());
    }
  }

  /**
   * Blur is when a half-typed value becomes a number.
   *
   * Clamping on every keystroke would make `15` unreachable in a field whose max
   * is 20, because the `1` would clamp itself the moment it was typed. Zag
   * clamps on blur and so does this.
   */
  protected onBlur(): void {
    this.focused.set(false);
    this.onTouched();
    if (this.text() === "") {
      this.commit(Number.NaN);
      return;
    }
    this.setTo(this.clamp(this.valueAsNumber()));
  }

  private setTo(value: number): void {
    this.text.set(this.formatNumber(value));
    this.commit(value);
  }

  private commit(next: number): void {
    const value = Number.isNaN(next) ? null : next;
    if (value === (this.value() ?? null)) return;
    this.value.set(value);
    this.valueChange.emit(value);
    this.onChange(value);
  }

  // ── Forms ─────────────────────────────────────────────────────────────────

  protected onTouched: () => void = () => {};
  private onChange: (value: number | null) => void = () => {};

  writeValue(value: number | null): void {
    this.value.set(value ?? null);
  }
  registerOnChange(fn: (value: number | null) => void): void {
    this.onChange = fn;
  }
  registerOnTouched(fn: () => void): void {
    this.onTouched = fn;
  }
  setDisabledState(isDisabled: boolean): void {
    this.formDisabled.set(isDisabled);
  }
}

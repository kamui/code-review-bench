# ez-form

MUI + react-hook-form + zod, wired together so you write plain JSX.

```tsx
import { z } from 'zod'
import {
  Form,
  TextField,
  Select,
  Autocomplete,
  RadioGroup,
  CheckboxGroup,
  ToggleButtonGroup,
  Slider,
  Rating,
  NumberField,
  MoneyField,
  DatePicker,
  TimePicker,
  DateTimePicker,
  OtpField,
  FileField,
  Checkbox,
  Switch,
  SubmitButton,
} from 'ez-form'

const schema = z.object({
  email: z.email({ error: (iss) => (iss.input === '' ? 'Email is required' : 'Invalid email') }),
  role: z.enum(['admin', 'user'], { error: 'Pick a role' }),
  tos: z.boolean().refine(Boolean, { error: 'Required' }),
  newsletter: z.boolean(),
})

export function Signup() {
  return (
    <Form
      schema={schema}
      defaultValues={{ email: '', role: 'user', tos: false, newsletter: false }}
      onSubmit={(values) => console.log(values)}
    >
      <TextField name="email" label="Email" />
      <Select
        name="role"
        label="Role"
        options={[
          { value: 'admin', label: 'Admin' },
          { value: 'user', label: 'User' },
        ]}
      />
      <Checkbox name="tos" label="I accept the terms" />
      <Switch name="newsletter" label="Newsletter" />
      <SubmitButton />
    </Form>
  )
}
```

## Why ez-form

ez-form is a thin binding layer, not a component library: MUI supplies the widgets,
react-hook-form the state, and zod the schema, so every prop and type traces back to
one of those three. Nothing in `src/` makes a styling judgement call — defaults live
only where a theme can override them — and every field ships accessible and
axe-tested by default. Every control ez-form renders is at least 24×24 CSS px at its
default size, per WCAG 2.5.8 Target Size (Minimum) — the one exception is
`Rating size="small"`, which MUI draws at 18×18 and which is
[documented, not silently shipped](#rating). The form owns submission, loading, and disabling;
fields just read from it. See [`docs/PHILOSOPHY.md`](docs/PHILOSOPHY.md) for the full
rules and the checklist a component must pass before it ships.

## Install

```bash
pnpm add ez-form @mui/material @mui/icons-material @mui/x-date-pickers @emotion/react @emotion/styled @base-ui/react react-hook-form zod
```

`@mui/x-date-pickers` is a required peer even if you use no picker: ez-form has a single entry point, so the package is always resolved. `@base-ui/react` backs `NumberField`, `MoneyField`, `PercentField`, and `OtpField`; `@mui/x-date-pickers` backs the three pickers. Both are tree-shakeable, and you also install one date adapter library (`date-fns`, `dayjs`, `luxon`, or `moment`) for the pickers — see "Date pickers" below.

Requires zod 4 (the types use zod 4's `ZodType<Output, Input>`) and TypeScript >= 5.4 (the types use `NoInfer`).

React 18 and React 19 are both supported, `ref` included: `<Form ref>` (the form methods) and `<FormSection ref>` (the `<fieldset>`) are populated the same way on either major.

## Components

| Component                                      | Wraps                                         | Extra props                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ---------------------------------------------- | --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Form`                                         | `useForm` + `<form>`                          | `schema`, `onSubmit(values, form)`, `defaultValues?` (object or async function), `values?`, `resetOptions?`, `onDefaultValuesError?`, `ref?`, `mode?`, `disabled?`; fields disable while `onSubmit` is pending or async defaults are loading                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `FormSection`                                  | `<fieldset>` + `<legend>`                     | `title?`, `description?`, `slotProps?` (`legend`, `description`, `content`); group name is the legend, disabled disables every field in the group                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `TextField`                                    | MUI `TextField`                               | `name`, `variant?: EzTextFieldVariants` (MUI's three plus `'stacked'` and anything augmented — see [Custom variants](#custom-variants)); rules `required`, `min`, `max`, `minLength`, `maxLength`, `pattern`, `validate`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `Select`                                       | MUI `TextField select`                        | `name`, `options: readonly SelectOption[]` (`{ value: string \| number; label: string }`), `variant?: EzTextFieldVariants` (MUI's three plus `'stacked'` and anything augmented — see [Custom variants](#custom-variants)); the same rules as TextField, typed over the option value                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `RadioGroup`                                   | MUI `RadioGroup`                              | `name`, `label` (legend), `options: readonly Option[]`, `helperText?`; rules `required`, `validate`. The form value keeps the option's type                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `CheckboxGroup`                                | MUI `FormGroup` + `Checkbox`                  | `name`, `label` (legend), `options: readonly Option[]`, `row?`, `helperText?`; rules `required` (at least one), `validate`. Value is `Option['value'][]` in `options` order                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| `ToggleButtonGroup`                            | MUI `ToggleButtonGroup`                       | `name`, `label` (legend), `options: readonly Option[]`, `exclusive?`, `helperText?`; rules `required`, `validate`. Value is `Option['value'] \| null` when exclusive, else `Option['value'][]`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `Slider`                                       | MUI `Slider`                                  | `name`, `label` (legend), `helperText?`; rules `min`, `max` (also the slider bounds), `validate` — no `required`, since a slider always reports a value. Value is a `number`, or `[number, number]` for a range. Pass MUI's `getAriaLabel` to name each thumb of a range slider, which otherwise share the legend's name (the returned string should include the field's own label)                                                                                                                                                                                                                                                                                                                                  |
| `Rating`                                       | MUI `Rating`                                  | `name`, `label` (legend), `helperText?`; rules `required`, `validate`. Value is `number \| null`. Note `size="small"` renders an 18×18 target, below the 24×24 minimum — [see below](#sizesmall-is-below-the-target-size-minimum)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `Autocomplete`                                 | MUI `Autocomplete`                            | `name`, `options`, `getOptionValue?` (default `o => o.value`; return `o` to store objects), `multiple`, `freeSolo`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)), `textFieldProps?`; all TextField rules. Options may carry extra fields (they reach `onChange`)                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `ChipDeleteIcon`                               | MUI `SvgIcon` (`Cancel`)                      | `label`, `removeLabel?` (default `` `Remove ${label}` ``). The delete icon every chip in ez-form renders — `Autocomplete` under `multiple`, `EmailListField`, `FileField` — named and sized to a 24×24 target; themeable once under `EzChipDeleteIcon`. Use it in your own `renderValue`                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `EmailListField`                               | ez-form `Autocomplete`                        | `name`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); a `string[]` of addresses. `loadOptions?(query, signal)` for an async directory (debounced by `debounceMs?`, default 250, and aborted on the next keystroke), `allowNew?` (default `true`), `invalidMessage?`, `duplicateMessage?`, `addedMessage?`/`addedManyMessage?`/`removedMessage?`/`removedManyMessage?`, `autoComplete?` (default `'email'`, `'off'` under `assisted`), `slotProps?` (`chip`, `status`). Enter, comma, semicolon, space and blur commit; paste splits; duplicates collapse case-insensitively                                                                                                             |
| `NumberField`                                  | Base UI `NumberField` through MUI `TextField` | `name`, `label?`, `helperText?`, `size?`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); rules `required`, `min`, `max` (also the stepper bounds), `validate`. Value is `number \| null`; digits group while typing (new in v2.1), and `format={{ useGrouping: false }}` turns that off                                                                                                                                                                                                                                                                                                                                                                                                  |
| `MoneyField`                                   | `NumberField` pinned to USD                   | `name`, `label?`, `helperText?`, `size?`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); rules `required`, `min`, `max`, `validate`. Value is a `number` in dollars, rounded to the cent; shows `$1,234.50` on blur                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `PhoneField`                                   | ez-form `TextField`                           | `name`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); same rules as TextField, plus `format?` (a `#` template, default `'###-###-####'`) and `invalidMessage?`. The form value is digits only (`'5551234567'`); `type="tel"`, `inputMode="tel"`, `autoComplete` defaults to `'tel'`. A non-empty value shorter than the template's digit count fails with `invalidMessage`                                                                                                                                                                                                                                                                                                              |
| `PercentField`                                 | `NumberField` with an Intl `%`                | `name`, `label?`, `helperText?`, `size?`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); rules `required`, `min` (default 0), `max` (default 100), `validate`; `step` defaults to 1 and `scale?: 'percent' \| 'fraction'` decides what the value means. Shows `12.5%` on blur; bounds and step are always percentage points                                                                                                                                                                                                                                                                                                                                                              |
| `ZipField`                                     | `TextField`                                   | `name`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); same rules as TextField, plus a built-in "5 digits" rule (`invalidMessage?`, default `'Enter a 5-digit ZIP code'`). Digits only, capped at 5 (anything else is stripped on type/paste); `inputMode="numeric"`, `autoComplete` defaults to `'postal-code'`. Value is the digit string                                                                                                                                                                                                                                                                                                                                              |
| `FeinField`                                    | ez-form `TextField`                           | `name`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); same rules as TextField, plus `format?` (a `#` template, default `'##-#######'`) and `invalidMessage?` (default `'Enter a 9-digit employer identification number'`). The form value is digits only (`'123456789'`); `inputMode="numeric"`, `autoComplete` defaults to `'off'`                                                                                                                                                                                                                                                                                                                                                     |
| `StateSelect`                                  | `Select`                                      | `name`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); same rules as Select. Options are the 50 states + DC by default; `territories?` adds PR, GU, VI, AS, MP. `autoComplete` defaults to `'address-level1'`. Value is the USPS abbreviation; also exports `US_STATES`/`US_TERRITORIES` option arrays                                                                                                                                                                                                                                                                                                                                                                                   |
| `AddressField`                                 | `TextField` + `StateSelect` + `ZipField`      | `name` (nested object), `legend?`/`description?` (renders a `FormSection`), `autoCompleteSection?` (`'shipping'`/`'billing'`/any section token, prefixes every autofill token), `street2?` (default `true`), `streetLabel?`/`street2Label?`/`cityLabel?`/`stateLabel?`/`zipLabel?`, `required`/`disabled`, `slotProps?`, `lookup?` (an `AddressLookupProvider`: the street becomes an address search that fills every part on pick; `lookupDebounceMs?` 300, `lookupMinChars?` 3, `lookupFilledText?`, `lookupFailedText?`). `required` reaches street/city/state/zip, never street2; `addressSchema()` is the matching zod object                                                                                   |
| `DatePicker` / `TimePicker` / `DateTimePicker` | MUI X pickers                                 | `name`, `label?`, `helperText?`, `errorMessages?`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); rules `required`, `validate`. The picker's own props (`minDate`, `disablePast`, `views`, …) pass through. Value is the adapter's date type or `null`                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `OtpField`                                     | Base UI `OTPField` in MUI's outlined style    | `name`, `label?`, `helperText?`, `length?` (6), `mask?`, `validationType?`, `size?`; rules `required`, `validate`. Value is the code string; a partial code fails with `<label> must be <length> characters.` Each slot is named for its position (`characterLabel`, default `Character 2 of 6`); with an `aria-label`/`aria-labelledby` name instead of a visible `label`, slot 1 is named "<name> Character 1 of 6".                                                                                                                                                                                                                                                                                               |
| `FileField`                                    | MUI `Button` + hidden `<input type="file">`   | `name`, `label?` (button text; omit it and name the field with `aria-label`/`aria-labelledby`), `accept?`, `multiple?`, `dropzone?`, `dropText?`, `maxSize?`, `maxFiles?`, `renderFile?`, `onFilesAdded?`, `buttonProps?`, `helperText?`; rules `required`, `validate`. Value is `File \| null`, or `File[]` under `multiple`. `onChange(event, value)` fires on a pick and on a chip delete                                                                                                                                                                                                                                                                                                                         |
| `Checkbox`                                     | MUI `Checkbox`                                | `name`, `label`, `helperText?`, `labelPlacement?` (MUI's `FormControlLabel.labelPlacement`: `'end'` default / `'start'` / `'top'` / `'bottom'`); rules `required`, `validate`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `Switch`                                       | MUI `Switch`                                  | `name`, `label`, `helperText?`, `labelPlacement?` (MUI's `FormControlLabel.labelPlacement`: `'end'` default / `'start'` / `'top'` / `'bottom'`); rules `required`, `validate`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `SubmitButton`                                 | MUI `Button`                                  | `loading` while submitting, disabled while the form is                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `Form` (v4 additions)                          | —                                             | `confirm?: true \| ConfirmOptions` asks after validation on every submit path; `guard?: boolean` warns on tab close while dirty; `submitPendingText?`/`submitSuccessText?`/`submitErrorText?` are the submit announcements (`false` suppresses one)                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `Form` (label placement)                       | —                                             | `labelPlacement?: 'top' \| 'start'` (default `'top'`), `labelPlacementBreakpoint?` (default `'sm'`), `labelWidth?` (default `'12rem'`) — see "Label placement" below                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `ClearButton`                                  | MUI `Button`                                  | `to?: 'defaults' \| 'empty'`, `confirm?`; disabled while pristine                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `ConfirmDialog`                                | MUI `Dialog`                                  | `open`, `title`, `message?`, `confirmLabel?`, `cancelLabel?`, `confirmColor?`, `onConfirm`, `onCancel`, `actionsOrder?: 'cancel-confirm' \| 'confirm-cancel'` (default `'cancel-confirm'`; Cancel keeps `autoFocus` either way); `useConfirm()` gives a promise API                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `FormDialog`                                   | MUI `Dialog` + `Form`                         | `open`, `onClose(event, reason)`, `title`, `actions?`/`cancelLabel?`/`submitLabel?`, `exitConfirm?: ConfirmOptions \| false`, `closeOnSubmit?`; every `Form` prop (`schema`, `onSubmit`, `confirm`, …) and every `Dialog` prop (`maxWidth`, `fullScreen`, …). Closing while dirty asks first                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `Wizard`                                       | MUI `Stepper`                                 | `steps`, `step?`/`onStepChange?`, `visited?`/`onVisitedChange?`, `orientation?`, `layout?: 'steps' \| 'page'`, `stepAnnouncement?`; with `WizardStepper`, `WizardStep`, `WizardNav` (`actionsOrder?: 'back-next' \| 'next-back'`, default `'back-next'`), `useWizard`                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `ReadOnlyField`                                | MUI `Typography`                              | `name`, `label?`, `options?`, `format?`, `empty?`, `editStep?` — or `value` (a caller-computed value, e.g. from its own `useWatch`) with `label` required and no `name`; never calls `useWatch` in that mode                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `PasswordField`                                | ez-form `TextField`                           | `name`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); same rules as TextField. `revealable?` (default `true`) shows a show/hide toggle in the end adornment; `autoComplete` defaults to `'current-password'`; `slotProps.toggle?` reaches the toggle `IconButton`; `showLabel?`/`hideLabel?` set its accessible name                                                                                                                                                                                                                                                                                                                                                                    |
| `EmailField`                                   | ez-form `TextField`                           | `name`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); same rules as TextField, plus a built-in format rule (`invalidMessage?`, default `'Enter a valid email address'`) using HTML's own e-mail grammar, and `normalize?` (default `true`) which trims and lower-cases on blur. `type="email"`, `inputMode="email"`, `autoComplete` defaults to `'email'`                                                                                                                                                                                                                                                                                                                               |
| `SsnField`                                     | ez-form `TextField`                           | `name`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); same rules as TextField, plus a built-in "9 digits" rule (`invalidMessage?`, default `'Enter a 9-digit Social Security number'`). The form value is digits only (`'123456789'`), displayed as `123-45-6789`; `inputMode="numeric"`, `autoComplete="off"`. `reveal?` (default `true`) shows a show/hide toggle; hidden renders `type="password"`                                                                                                                                                                                                                                                                                   |
| `PasswordStrength`                             | MUI `LinearProgress`                          | `name`; `score?: (password) => 0\|1\|2\|3\|4` (default a small built-in heuristic); `labels?` (5 strings). Renders as an ARIA `meter`, never registers or validates                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `TextareaField`                                | `TextField` with `multiline` fixed on         | `name`, `showCount?`, `variant?: EzTextFieldVariants` (see [Custom variants](#custom-variants)); the same rules as TextField. Taller default (`minRows: 4`, `maxRows: 12`, both themeable); shows a `n / max` length meter when `maxLength` is set (or `showCount`), which turns into the validation error past the limit                                                                                                                                                                                                                                                                                                                                                                                            |
| `DateField`                                    | MUI X `DateField`                             | Same shape as `DatePicker` (`variant?` included), but no popup — a keyboard-only, sectioned date input. Better for birthdays and other far-away dates: typing beats paging a calendar back decades                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| `ResendCodeButton`                             | MUI `Button`                                  | `onResend` (awaited if a promise; disabled while pending), `cooldown?` (seconds, default 30) shown in the label (`Resend code (27s)`); a rejected `onResend` shows `errorText?` (default "Code could not be sent") in the status slot instead, starts no cooldown, and calls `onResendError?(error)`. `slotProps.status` for the `role="status"` region that announces "Code sent" (or the error) once per resend. Disabled while the form is disabled (which covers submitting)                                                                                                                                                                                                                                     |
| `FormError`                                    | MUI `Alert`                                   | Renders `formState.errors.root` (set via `form.setError('root.<key>', { message })`, e.g. a rejected async `onSubmit`); renders nothing when there is no root error                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| `FormErrorSummary`                             | —                                             | `title?` (default "There is a problem"), `slotProps?` (`heading`, `list`, `item`, `link`); lists the last failed validation's errors as focusable links, GOV.UK-style — see "Error summary" below                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `LiveRegion`                                   | —                                             | `message`, `announcementKey?` (bump to re-announce identical text), `politeness?` (`polite`/`assertive`), `visuallyHidden?` (default `true`), `component?`; the shared announcement region. `<Form>` renders one for submit status — see "Announcements" below                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `FieldArray`                                   | hookform `useFieldArray`                      | `name`, `label` (array legend), `emptyRow`, `singular?`/`rowLabel?`, `minRows?`/`maxRows?`, `addLabel?`/`removeLabel?`, `reorder?`, `slotProps?`; children is a render prop `(row) => ...` given `row.name('field')` for the array path. Rows are keyed by hookform's `field.id`; Add/Remove/Move move focus and announce in a `role="status"` region. `layout="table"` + `columns` renders one MUI `Table` instead (`cellErrors?`, `actionsHeader?`; see Table layout)                                                                                                                                                                                                                                              |
| `BoundField`                                   | your own control                              | `name`, `render: (bound) => ReactElement`; `label?`, `helperText?`, `rules?`, `disabled?`, `labelAs?` (`'none'` default / `'control'` / `'legend'`), `componentName?`, `labelPlacement?`, `controlLabelProps?` (`Omit<FormControlLabelProps, 'control' \| 'label' \| 'required'>`, read under `labelAs="control"` only — how `Checkbox`/`Switch` forward MUI's own `labelPlacement`; a dev-mode warning names it when the mode is anything else). Binds a control ez-form does not wrap: `render` gets `bound.field` (typed over the field's value), `bound.inputA11y`, `bound.nameA11y`, `bound.labelId`/`controlId`, `bound.displayLabel`, `bound.helperText`. See [Wrap your own control](#wrap-your-own-control) |

`Form`'s `title` / `description` give the form its accessible name and instructions (wired to the `<form>` via `aria-labelledby` / `aria-describedby`); `slotProps.title.component` sets the heading level (default `h2`). `FormSection` groups fields in a `<fieldset>` named by its `title` (`<legend>`, heading level configurable via `slotProps.legend.component`, default `h3`); `description` is helper text wired via `aria-describedby`.

**Required vs optional.** `Form`'s `requiredIndicator` picks how a field's `required` rule is shown: `'asterisk'` (default) is today's behavior — a required field gets MUI's usual `*`. `'optional'` flips the convention: a required field still gets `required` / `aria-required` on its input, but renders no asterisk; a field that is not required gets `Form`'s `optionalText` appended to its label (default `'(optional)'`). The suffix is a form-wide convention, so it is set once on `Form` — or in the theme or locale — never per field; a field that should read differently is a label-wording choice ("Middle name, if any"). `Form` also states whichever convention is active once via `requiredIndicatorText`, rendered in the same `description` slot — appended as a second sentence when `description` is also set, or alone when it is not. The default text is mode-dependent: `'Required fields are marked with an asterisk (*).'` in `'asterisk'` mode (the `*` is spelled out as a word so it reads sensibly to assistive tech, the way MUI's own `FormLabel` asterisk is `aria-hidden`), `'All fields are required unless marked optional.'` in `'optional'` mode. Pass a string to use it verbatim in either mode, or `requiredIndicatorText={false}` to suppress it. Per field, nothing extra is added beyond `required`/`aria-required` — the input's own required state is what assistive tech announces; an additional hidden "required" string would double-announce it. All three props are theme-defaultable through `theme.components.EzForm.defaultProps`.

**Label placement.** `Form`'s `labelPlacement` decides **where** every field's label sits relative to its control. It takes MUI `FormControlLabel`'s own vocabulary, `'top' | 'start'`, and it is its **own axis**, deliberately not a `variant`: `variant` picks the box (MUI's `'outlined' | 'standard' | 'filled'`, plus `'stacked'` and anything else augmented — see [Custom variants](#custom-variants)); `labelPlacement` picks where the label goes, and the two compose — `<TextField variant="filled">` under `labelPlacement="start"` is a filled box with its label in a left column.

| Value             | What it looks like                                                                                                                                                                                                                                                                          |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `'top'` (default) | The label sits above the control. It emits no CSS of its own: the field is byte-for-byte MUI's `FormControl` box, so whether the label **floats** is the theme's answer, not the axis's.                                                                                                    |
| `'start'`         | The label sits in a column beside the control (the settings-form layout) above `labelPlacementBreakpoint` (default `'sm'`), because a phone has no room for two columns. `labelWidth` (default `'12rem'`) sizes the column. Below the breakpoint `start` applies no CSS either — MUI's box. |

**Floating is `InputLabel`'s, not this axis's — and under the preset it is the `variant`'s.** Whether a `top` label sits over the input and translates up on focus is MUI's `InputLabel` `shrink` (with `disableAnimation` and `OutlinedInput` `notched`), which is a theme concern in vanilla MUI and stays one here. Under stock `createTheme()` a `top` label floats whatever the variant. Under `createEzFormTheme()` the field's `variant` decides: `variant="stacked"`, the preset's default, puts the label above the control in normal flow with helper text flush left; `variant="outlined"` on one field restores MUI's floating label. See [Custom variants](#custom-variants). A consumer on a stock theme who wants static labels writes the same three theme lines a vanilla MUI consumer writes today.

The same follows below the breakpoint: a `start` form on a phone shows whatever its theme's `top` box shows — floating under a stock theme, static under the preset.

Placement is a form-wide layout convention, so it is set once — on `Form`, or in `theme.components.EzForm.defaultProps.labelPlacement`, which flips every form in an app with no per-form and no per-field props:

```tsx
const theme = createTheme({
  components: { EzForm: { defaultProps: { labelPlacement: 'start' } } },
})
```

Every field also takes its own `labelPlacement` for the one row that has to differ. `createEzFormTheme()` does **not** set this default — it has nothing to say about _where_, only about whether a `top` label floats.

`Checkbox` and `Switch` are the exception: their `labelPlacement` is MUI's `FormControlLabel.labelPlacement` (`'end' | 'start' | 'top' | 'bottom'`, default `'end'`), forwarded to the `FormControlLabel` they render, so `<Switch name="alerts" label="Email alerts" labelPlacement="start" />` puts the label before the switch. They do not take the form axis.

Under the hood this is a layout change on the one box every ez-form field already renders: a MUI `FormControl` whose children are the label, the control and the helper text, in that order. No markup moves between placements, so `<label for>` / `aria-labelledby`, the error and helper text in `aria-describedby`, and the required marker are identical under both — `getByLabelText` finds the same control either way. The rules live on `Form`'s own `EzForm` Root slot, so `theme.components.EzForm.styleOverrides.root` can reach and override every one of them, and `fieldLayoutClasses` (`EzFieldLayout-root` / `-top` / `-start`, plus `-selfLabelled` on a field whose label is inside its control — `FileField`'s picker button — which `start` reads as "no label column for this row") names them for a theme or a test. `-top` emits no rules; it exists so every field box carries exactly one placement class, which a theme or a test can select on. `-cell` / `-cellHelperHidden` are a different, internal axis (`FieldArray layout="table"`, #14) applied alongside a placement class, never a placement themselves — see "Table layout" below.

### Custom variants

Every input with a box takes a top-level `variant?: EzTextFieldVariants` — MUI's `'outlined' | 'standard' | 'filled'`, plus `'stacked'` and anything you augment: `TextField`, `TextareaField`, `NumberField`, `MoneyField`, `PercentField`, `Select`, `StateSelect`, `Autocomplete`, `EmailField`, `EmailListField`, `PasswordField`, `PhoneField`, `SsnField`, `FeinField`, `ZipField`, `DateField`, `DatePicker`, `TimePicker`, `DateTimePicker`.

Under `createEzFormTheme()` the default is `'stacked'`: the label sits above the control in normal flow, no float, no notch. `variant="outlined"` on one field restores MUI's floating label.

Add your own with the same two steps MUI documents for `Button`:

```tsx
// once, in a .d.ts
declare module '@mui/material/TextField' {
  interface TextFieldPropsVariantOverrides {
    dashed: true
  }
}

// theme — the function-predicate form, because `props` is typed off MUI's closed union
const theme = createEzFormTheme({
  components: {
    MuiTextField: {
      variants: [{ props: (p) => p.variant === 'dashed', style: {/* … */} }],
    },
  },
})

;<TextField name="a" label="A" variant="dashed" />
```

How it works: the type is MUI's own `OverridableStringUnion` augmentation, so `TextFieldPropsVariantOverrides` is the interface you extend. At runtime the preset's `MuiTextField.defaultProps.slots.input` is a small component that picks MUI's `Input`, `FilledInput` or `OutlinedInput` by the resolved variant, and `OutlinedInput` for anything else — a custom variant is a plain outlined root with the notch closed, for the theme's `variants` to dress. `MuiPickersTextField` gets the same. This goes away when `@mui/material` ships it upstream; the patch is [`docs/superpowers/specs/2026-09-07-upstream-mui-variant-overrides-minimal.patch`](docs/superpowers/specs/2026-09-07-upstream-mui-variant-overrides-minimal.patch).

**Known limitation.** The preset's box rules (root border, focus ring, padding) stay theme-wide in this pass, so under the preset `variant="outlined"` floats its label over the root's solid border. Follow-up #143 keys the box rules on the variant.

`variant` and `labelPlacement` are different axes and compose: the variant says what the box is and whether a `top` label floats; `labelPlacement` says where the label goes, and `start`'s grid un-floats the label regardless of variant.

Every field shows its zod message as helper text (linked to the input with `aria-describedby`; the first invalid field is focused on submit). The error text is a live region (`role="alert"`), so it is announced in `onChange`/`onBlur` modes as well. Fields must be rendered inside `<Form>`. Consumer `onChange`/`onBlur` handlers run after the form's own.

Need `reset`, `setError`, `watch`? `onSubmit` receives the form methods as its second argument (the same object `useFormContext()` returns), so the component that owns the form can handle server errors or reset after success:

```tsx
<Form
  schema={schema}
  defaultValues={emptyValues}
  onSubmit={async (values, form) => {
    const res = await save(values)
    if (res.error) form.setError('email', { message: res.error })
    else form.reset()
  }}
>
```

**Catch the rejection, don't rethrow it.** For a server-side failure with no single field to
blame, catch it and map it to a root error — `<FormError />` renders `errors.root` as an alert:

```tsx
onSubmit={async (values, form) => {
  try {
    await signIn(values)
    form.clearErrors('root.server')
  } catch (error) {
    form.setError('root.server', { message: 'Invalid email or password' })
  }
}}
```

`<Form>` looks at what `onSubmit` left behind, not only at how its promise settled, so this
pattern gets the full failure treatment — the announcement and the focus move — without
rethrowing:

| What `onSubmit` did                                   | Announced by `<Form>`    |
| ----------------------------------------------------- | ------------------------ |
| rejected                                              | `submitErrorText`        |
| resolved, set a **root** error, with a `<FormError>`  | `submitErrorText`        |
| resolved, set a **root** error, with no `<FormError>` | that error's own message |
| resolved, set **field** errors only                   | `submitErrorText`        |
| resolved, set nothing                                 | `submitSuccessText`      |

A mounted `<FormError>` is itself an assertive live region (`role="alert"`) already reading the
root message, so `<Form>`'s own polite region does not repeat it — one event should not be
announced twice by two regions. With no `<FormError>` nothing else would say it, so `<Form>`
does.

**Focus after a failed submit has one owner and a fixed order:** the `<FormError>` alert, else
a declared `<FormErrorSummary>` (which focuses its own heading), else the first invalid field.
That is the same single-owner rule that governs a validation failure, extended to cover a
server-side one.

Only errors raised _during_ that submit count, so a stale root error a form never cleared
cannot make the next, genuinely successful submit report a failure.

Inside child components use `useFormContext()` from `react-hook-form`.

Numbers: NumberField stores `number | null`, so use `z.number()` (add `.nullable()` if empty is allowed). TextField hands zod the string from the input, so a numeric TextField needs `z.coerce.number()`.

### Checkbox vs Switch

| Use case                                                                                                                                                      | Prefer     | Why                                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------- | --------------------------------------------------------- |
| A yes/no answer or opt-in recorded on submit ("I accept the terms", "Same as shipping", "Insure a vehicle")                                                   | `Checkbox` | Reviewed and only takes effect once the form is submitted |
| One of several independent options                                                                                                                            | `Checkbox` | `CheckboxGroup` — each option is its own recorded opt-in  |
| A setting that takes effect immediately, no submit step (dark mode, notifications on an autosaving settings page, a UI toggle whose `onChange` does the work) | `Switch`   | Nothing to submit — flipping it is the action             |

If the page has a Submit button, it is almost always a `Checkbox`. Both: phrase the label as the state when on ("Marketing emails"), never as a question ("Receive marketing emails?").

This follows [Material Design's selection-controls guidance](https://m3.material.io/components/switch/guidelines) and WCAG: MUI's `Switch` sets `role="switch"`, and assistive tech announces "on/off" for it — correct for a setting that takes effect immediately, wrong for an answer that is only recorded when the surrounding form is submitted.

## Error summary

`<FormErrorSummary />` is a child you place — under the form's title, or inside the current
`WizardStep` — not a `<Form>` prop, since a wizard's summary should scope to one step while a
plain form's should not:

```tsx
<Form schema={schema} onSubmit={onSubmit} title="Sign up">
  <FormErrorSummary />
  <TextField name="email" label="Email" />
  <SubmitButton />
</Form>
```

It renders nothing until a submit (or, inside a `Wizard`, a `Next`/step change) has failed, then
lists the errors from that attempt: a heading ("There is a problem" by default, `title` to
override) that receives focus — the move to the heading is what announces the summary, GOV.UK's
own pattern deliberately omits `role="alert"` here so a screen reader doesn't announce it twice
alongside each field's own `role="alert"` helper text — and one link per invalid field showing
its message. Activating a link (click or Enter) focuses that field via `setFocus`, so it works
even without a native `href` target. Items disappear as their fields become valid, and the whole
summary disappears once none are left. Once a form contains a summary, that form's post-submit
focus belongs to it: `<Form>` suppresses react-hook-form's own "focus the first invalid field"
behavior, and inside a `Wizard` so do `Next`'s step validation and the failed-submit jump, so
nothing fights the heading for focus. This holds even while the summary itself is unmounted —
a `WizardStep` renders only when it is the current step, so a summary in an earlier step must
still count when you submit from a later one.

That hand-over is one-way and lasts for the life of the `<Form>`: once a `<FormErrorSummary />`
has rendered inside a form, react-hook-form's own first-invalid-field focus stays suppressed
there even if you later remove the summary for good (behind a prop or a flag, say), so a failed
submit then moves focus nowhere. Leaving the summary mounted is the fix — it already renders
nothing until an attempt has failed — or remount the `<Form>`.

Inside a `Wizard`, place one `<FormErrorSummary />` per `WizardStep`: each shows only that step's
own `fields` from its last failed `Next`, not the whole form's errors. A failed final submit
navigates back to the first errored step, and the summary that mounts there takes focus and
says why — including when that step is one the user had not reached yet, so the summary is
mounting for the very first time.

## Announcements

Screen reader users get no feedback from a button that quietly starts an async submit, so
`<Form>` announces the submit lifecycle in a live region it renders itself — no wiring, and
nothing visible changes:

```tsx
<Form schema={schema} onSubmit={save}>   {/* announces "Submitting…", then "Submitted." or "Submit failed." */}
```

Each string is a prop, so it can be localised or turned off individually
(`false` suppresses just that one):

```tsx
<Form
  schema={schema}
  onSubmit={save}
  submitPendingText="Saving your answers…"
  submitSuccessText="Saved."
  submitErrorText={false}          // the page shows its own error banner instead
>
```

A _validation_ failure is deliberately not announced here: `onSubmit` never ran, and
`<FormErrorSummary />` already announces and lists what needs fixing. "Submit failed." is
reserved for a submit that started and then failed — which includes an `onSubmit` that
resolved after mapping a server rejection to `setError`, not only one that rejected outright.
See the table under "Need `reset`, `setError`, `watch`?" for which of the four outcomes gets
which announcement, and where focus lands in each.

The region itself is `<LiveRegion />`, exported for your own announcements:

```tsx
<LiveRegion message={saved ? 'Draft saved' : ''} />
```

It is visually hidden by default, `polite` by default (`politeness="assertive"` renders an
`alert` instead of a `status`), and takes `component` to render as something else — MUI's
`Typography`, say, when the text is also visible UI (`visuallyHidden={false}`).

Assistive tech only announces a live region when its _content changes_, so setting the same
text twice is silent. Pass a counter as `announcementKey` to force a re-announcement — it keys
the node, so an identical message mounts a fresh region and is heard again:

```tsx
<LiveRegion message="Could not save" announcementKey={attempt} />
```

Render a `LiveRegion` unconditionally with an empty `message` at rest, rather than mounting it
alongside its text: a region that appears in the same commit as its content has nothing to
change _from_, and that first announcement is unreliable. `Form`, `Wizard`, `FieldArray`,
`ResendCodeButton` and `PasswordStrength` all use this same component internally.

Because several of them can be on screen at once, one form may hold more than one
`role="status"` region — so in a test, query the one you mean by its slot class rather than by
role. The form's own submit-status region is `formClasses.status`:

```tsx
document.querySelector(`.${formClasses.status}`) // the form's, not a field's
```

## Field arrays

`FieldArray` repeats a group of fields over a hookform `useFieldArray`. The array
is one `FormSection` named by `label`; each row is a nested `FormSection` named
`<singular> <n>`, so rows sit at the right heading level automatically.
`children` is a render prop: `row.name('email')` builds the full form path
(`applicants.0.email`), so the fields inside are ordinary ez-form fields.

```tsx
const schema = z.object({
  applicants: z
    .array(z.object({ name: z.string().min(1, 'Name is required'), email: z.email() }))
    .min(1, 'Add at least one applicant'),
})

<FieldArray
  name="applicants"
  label="Applicants"
  emptyRow={() => ({ name: '', email: '' })}
  minRows={1}
  maxRows={5}
  reorder
>
  {(row) => (
    <>
      <TextField name={row.name('name')} label="Name" />
      <TextField name={row.name('email')} label="Email" />
    </>
  )}
</FieldArray>
```

Rows are keyed by hookform's stable `field.id`, never by index, so removing a row
in the middle does not shuffle typed values between the rows that remain.

`emptyRow` is the value a new row starts from. Pass a function (called per Add) so
object rows are never shared by reference between rows.

Row names come from `singular`, which defaults to `label` with one trailing `s`
stripped. That guess is deliberately naive — `Applicants` gives `Applicant`, but
`Addresses` would give `Addresse` and `People` stays `People` — which is exactly
why `singular` exists; set it whenever the guess is wrong. A `label` that is an
element rather than a string cannot be stripped at all and falls back to `Row`, so
pass `singular` alongside a non-string `label`. `rowLabel={(index) => ...}` replaces
the whole name (legend and every button's accessible name) when numbering is not
what you want.

Accessibility is the component's job: Add moves focus into the new row's first
field, Remove moves it to the previous row (or the Add button when the first row
went), Move keeps focus on the button that was pressed as it travels with its row,
and a `role="status"` region announces `Row N added` / `removed` / `moved up`.
Every button's accessible name includes the row (`Remove Applicant 2`).
`minRows` disables Remove at the floor; `maxRows` disables Add at the ceiling.

An **array-level** message renders under the Add button as a `role="alert"`: zod's
`.min(1, msg)` / `.max(n, msg)` on the array itself lands there, as does
`form.setError('applicants.root', { message })`. Per-row field errors stay on their
own fields as normal helper text.

### Table layout

`layout="table"` renders the same array as one MUI `Table` — a column per field, a
row per entry, the row's Remove / Move buttons in a trailing actions column and Add
under the table — for the line-items case where stacked rows waste the screen and
the user expects to work down a column like a spreadsheet.

```tsx
<FieldArray
  name="lines"
  label="Line items"
  layout="table"
  emptyRow={() => ({ sku: '', qty: 1, price: 0 })}
  minRows={1}
  reorder
  columns={[
    {
      key: 'sku',
      header: 'SKU',
      render: (row) => <TextField name={row.name('sku')} label="SKU" />,
    },
    {
      key: 'qty',
      header: 'Qty',
      width: '6rem',
      align: 'right',
      render: (row) => <NumberField name={row.name('qty')} label="Qty" min={1} />,
    },
    {
      key: 'price',
      header: 'Price',
      render: (row) => <MoneyField name={row.name('price')} label="Price" />,
    },
  ]}
/>
```

| Prop                                                                                          | Type                                            | Notes                                                                                                                                                                                                                                                                               |
| --------------------------------------------------------------------------------------------- | ----------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `layout`                                                                                      | `'stacked' \| 'table'`                          | Default `'stacked'`. `children` is the render prop for stacked rows, `columns` for the table; passing the one the layout does not read is a dev-mode warning.                                                                                                                       |
| `columns`                                                                                     | `FieldArrayColumn<TRow>[]`                      | `{ key, header, field?, width?, align?, render(row) }`. `field` is the row-relative field name the column's control is bound to — for keyboard navigation and error-summary links — and defaults to `key`. `width` is any CSS length, on the header cell; `align` is `TableCell`'s. |
| `cellErrors`                                                                                  | `'summary' \| 'inline'`                         | Default `'summary'`: a cell shows `aria-invalid` and the error outline, the text is visually hidden (still the control's description) and `<FormErrorSummary>` lists it. `'inline'` shows the text under the control and the row grows.                                             |
| `actionsHeader`                                                                               | `ReactNode`                                     | Visually hidden header of the actions column. Default `"Actions"` (`esES`: `Acciones`).                                                                                                                                                                                             |
| `slotProps.tableContainer` / `.table` / `.tableHead` / `.tableRow` / `.cell` / `.actionsCell` | MUI `TableContainerProps` / `TableProps` / …    | `stickyHeader` and `size` pass through `slotProps.table`; `size` defaults to `'small'` and also sets the cells' density (below).                                                                                                                                                    |
| `slotProps.rowHeader`                                                                         | `TableCellProps & { visuallyHidden?: boolean }` | The `<th scope="row">` carrying the row's name. Hidden by default; `visuallyHidden: false` shows the names as a first column.                                                                                                                                                       |

**Naming.** Every cell control is named by its row **and** column — `"Line item 2 Qty"`
(`aria-labelledby` pointing at the hidden row header and the column header). The
field's own `label` is still required and still rendered, visually hidden, so nothing
about the field changes but its name. In tests query cells by role and name
(`getByRole('textbox', { name: 'Line item 2 Qty' })`); `getByLabelText` follows the
same accname rule and resolves through the header text, not the hidden label.
`TextField`, `Select`, `Autocomplete`, `NumberField` and the pickers take the cell
name; the `BoundField` family (`Checkbox`, `Switch`, `RadioGroup`, …) keeps its own
label as the name for now (#137). A cell is designed for **one** control: a composite
(`AddressField`, a nested `FieldArray`) resets the cell for its parts, so they keep their
own labels and the column header names the group.

**Density.** The table body renders its controls under a nested theme whose
`defaultProps.size` is the table's `size` (`small` by default), so a `TextField`,
`Select`, `NumberField` or picker in a cell is small without each column saying so.
A field's explicit `size` still wins. `Rating` is left at its default size: its small
star is below the 24 px target minimum.

**Errors.** A cell's error is the control's `aria-describedby` target in both
`cellErrors` modes. `<FormErrorSummary>` lists it as
`Line item 2 Qty: Qty must be at least 1`, linked to the cell's control. The
array-level error (`.min`, `.max`, `setError('lines.root')`) renders under Add as in
the stacked layout.

**Keyboard.** Tab is the browser's. One handler on the table body adds:

| Key                    | In a cell                                                                                                                                                                                                                                                                |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Enter                  | Same column, next row. On the last row: appends a row (under `maxRows`) and lands in that column; at the cap, nothing. **Never submits.**                                                                                                                                |
| ArrowDown / ArrowUp    | Same column, one row down / up, no wrap — when the control did not consume the key. A closed `Select` opens its menu instead; `Autocomplete`, the pickers and `Slider` keep the arrows they consume, and a `RadioGroup` / `Rating` keeps the browser's own radio arrows. |
| ArrowLeft / ArrowRight | The caret's, untouched.                                                                                                                                                                                                                                                  |
| Escape, Tab            | Untouched.                                                                                                                                                                                                                                                               |
| Keys on Remove / Move  | The button's own.                                                                                                                                                                                                                                                        |

The exclusions are the same one-line rule the Wizard's Enter uses (`isPlainKey`): a
control that handles the key calls `preventDefault()`, and the table defers to it.
The pickers are the one control the table has to disarm itself — MUI X submits the
form on Enter wherever it finds a submit button — and does, from inside the picker
binding.

**The Enter exception.** Every ez-form field's contract says "Enter submits the form
once". Inside a table cell it does not: Enter is the row-advance key there, as in a
spreadsheet, and `FieldArray.test.tsx` pins "Enter in a cell never submits" in its
place. Outside a table nothing changes.

Out of scope for now: pasting TSV/CSV into a cell to fill a row and append rows (#136),
column sorting/resizing, virtualisation.

### Per-row content outside the array

Sometimes a row needs content somewhere the `<FieldArray>` isn't: one upload field per
co-applicant on a later wizard step, a per-row summary line, an entry in a side panel.
`useFieldArrayRows(name)` gives you that array's rows — the same `{ index, id, name }` the
render prop receives — without owning add, remove or reorder, which stay with the
`<FieldArray>` that declares the array.

```tsx
function DocumentUploads() {
  const coApplicants = useFieldArrayRows('coApplicants')
  return (
    <>
      <FileField name="applicantDocuments" label="Upload applicant documents" multiple />
      {coApplicants.map((row) => (
        <FileField
          key={row.id}
          name={row.name('documents')}
          label={`Upload documents for co-applicant ${row.index + 1}`}
          multiple
        />
      ))}
    </>
  )
}
```

Rows update live while the array is mounted. When it unmounts — which `<Wizard>` does to every
step but the current one — the last-known rows are latched and keep being returned, which is
what makes the cross-step case work at all. The latch is one-way for the life of the `<Form>`,
which costs you two things: an array removed from the form permanently (behind a feature flag,
not a wizard step) keeps reporting its last rows, and a `reset()` while the owning array is
unmounted leaves a reader on the pre-reset rows until that array mounts again. Reading a name
this form's schema has no key for renders nothing and warns in development.

**Ids are stable per `<FieldArray>` mount, not for the life of the form.** They are hookform's,
and hookform mints them when the hook mounts and re-mints on array-level replacement, so an
array whose step unmounts and mounts again hands out a new set. Within one mount they are
stable across add, remove and reorder, which is what makes them the right React `key`. Across
a remount, a reader keyed by them re-mounts its rows — invisible for per-row content on another
step, but a persistent side panel will reset its rows' own component state when the owning step
is revisited. Never persist an id or send it to a server as a row identifier.

**Key by `row.id`, not by index.** The hand-rolled version of this is a `useWatch` on the array
plus a `.map()` keyed by index, and it looks right for a long time, which is what makes it
worth spelling out. Because ez-form fields are controlled, an index-keyed list still shows the
correct _values_ after a row is removed — React re-uses the first row's component to render the
survivor and re-renders it with the survivor's value. What it silently gets wrong is component
**identity**: remove co-applicant 1 and the component that was mounted for them is now
rendering co-applicant 2, so anything that row's own component holds outside form state — a
collapsed panel, a scroll position, an in-flight upload, a `useRef` — belongs to the wrong
person. Keying by `row.id` unmounts the removed row's component and lets the survivor keep its
own. A plain `useWatch` never sees hookform's `field.id`s, only the current values, which is
why this hook exists.

Calling hookform's `useFieldArray` a second time on the same name is not the answer either, and
fails silently: it mints its own ids per hook instance (so they match nothing the array uses),
and only one instance per name stays subscribed, so the second one stops updating after the
first append. hookform documents the rule — one `useFieldArray` per name.

Themeable under `EzFieldArray` (`defaultProps`, `styleOverrides` for `root` | `row`
| `actions` | `add` | `remove` | `move` | `status` | `error`, and for the table layout
`tableContainer` | `table` | `tableHead` | `tableRow` | `cell` | `rowHeader` |
`actionsCell` | `actionsHeaderText`) and exported as `fieldArrayClasses`. Note the class for the error slot is `fieldArrayClasses.errorText`:
MUI reserves `error` as a global state class (`Mui-error`), so only the `styleOverrides`
key is `error`.

## Wizard

One `<Form>` and one schema above every step. `Next` validates only the current step's `fields`; `Submit` on the last step validates everything.

```tsx
const steps = [
  { id: 'account', label: 'Account', fields: ['name', 'email'] },
  { id: 'plan', label: 'Plan', fields: ['plan'] },
  { id: 'review', label: 'Review' },
] as const satisfies WizardStepDef<z.input<typeof schema>>[]

<Form schema={schema} defaultValues={defaults} onSubmit={save} confirm>
  <Wizard steps={steps} orientation="vertical">
    <WizardStepper />
    <WizardStep id="account">…fields…</WizardStep>
    <WizardStep id="plan">…fields…</WizardStep>
    <WizardStep id="review">
      <ReadOnlyField name="email" editStep="account" />
    </WizardStep>
    <WizardNav />
  </Wizard>
</Form>
```

Every field in the schema should appear in exactly one step's `fields`. When Submit fails validation, the wizard moves to the first step (in `steps` order) owning an errored field and focuses that field once it mounts, so an error on a step you have navigated away from is never silent. A field listed in no step is validated only on final submit and its error belongs to the last step: that step is marked in the stepper, and a failed submit lands there.

Each `WizardStep` is a `FormSection` (a fieldset). Horizontally, the step's `title` defaults to its `label` from `steps` and renders as a heading in the legend; pass `title={null}` to render no legend. Vertically, the legend is suppressed and the section is named by the stepper's label via `aria-labelledby`, so the step has no visible legend but is still named for assistive technology; `description` works in both modes.

### Step changes are announced and focused

Changing step swaps the page's content without moving the caret, so a screen reader user is left reading the old step. On every `Next` / `Back` / stepper click, the wizard moves focus to the new step's **heading** and announces the move in a live region it renders itself — no wiring:

```tsx
<Wizard steps={steps}>   {/* announces "Step 2 of 3, Plan" and focuses that step's heading */}
```

Focus goes to the step's container, not its first field: focusing an input announces the input and skips the step's name and position, so the user would hear "Email, edit" with no idea they had moved. It lands on the legend heading horizontally, on the stepper label the step is named by vertically (there is no legend there), and on the `<fieldset>` itself for a `title={null}` step, which has no naming element to reach.

The announcement is a function prop, so it can be localised or turned off:

```tsx
<Wizard
  steps={steps}
  stepAnnouncement={({ index, count, label }) => `Paso ${index + 1} de ${count}: ${label}`}
>

<Wizard steps={steps} stepAnnouncement={false} />   {/* silent; focus still moves */}
```

`index` and `count` are positions in the **effective** step list — a step hidden by `when` is not counted, so what the user hears matches what the stepper shows. `stepAnnouncement={false}` suppresses only the announcement; focus management is not optional.

Four cases deliberately stay quiet: initial mount (nothing changed, and stealing focus on load is hostile), a failed `Next` (the step did not change — `<FormErrorSummary />` announces and focuses instead), the failed-submit jump to the first errored step, which is `<FormErrorSummary />`'s arrival to own, and a controlled wizard's declined move. `layout="page"` never navigates, so it never announces.

That last case matters when `step`/`onStepChange` are wired to a router: both the announcement and the focus move wait until the wizard has actually _arrived_ on the requested step, so a transition your own code vetoes — or one that resolves a tick later through `navigate()` — never announces a step the user is not looking at.

Because a form can hold several `role="status"` regions at once, query this one by its slot class in a test:

```tsx
document.querySelector(`.${wizardClasses.status}`) // the wizard's, not the form's
```

### Enter advances the step

Pressing Enter in a field on a non-last step does exactly what clicking `Next` does: it validates that step's `fields`, then either advances or shows the step's errors and focuses the first invalid field. Nothing to wire — it is on by default, so the most natural "I'm done with this screen" gesture on the web is not a dead end for a keyboard or screen reader user.

This is a `keydown` on the step's own `<fieldset>` rather than a `type="submit"` Next button, because `Next` must not submit the form early — and `<Form>` stays the only thing that submits.

Enter is left alone wherever a control already means something by it:

| Where                                                         | What Enter does instead                               |
| ------------------------------------------------------------- | ----------------------------------------------------- |
| A `TextareaField` (or any `contenteditable`)                  | Inserts a newline                                     |
| An open `Select` / `Autocomplete` listbox, or a picker popper | Picks the highlighted option                          |
| A closed `Select`                                             | Opens the menu                                        |
| `EmailListField`                                              | Commits the typed address as a chip                   |
| A button or link (`Back`, `ReadOnlyField`'s Edit)             | Activates that control                                |
| Enter with Shift / Ctrl / Cmd / Alt held                      | Whatever that gesture means to the field              |
| The **last** step                                             | Submits, via `WizardNav`'s `SubmitButton` — unchanged |

The rule underneath the table is one line: a field that handles Enter itself calls `preventDefault()`, and the wizard defers to that — the same signal the browser's own implicit submission respects. A custom field of your own gets the same treatment for free by doing the same thing. `layout="page"` installs no handler at all, since it never navigates.

### Same steps, one page

`layout="page"` renders every `WizardStep` at once, in document order (steps order is by convention the order the `WizardStep`s appear as children, matching `steps`), each as its own named section — the same markup a horizontal step already uses. `WizardStepper` and `WizardNav` render nothing in this layout; wire a plain `<SubmitButton>` instead, which validates the whole schema in one pass, same as the last step of a `steps` wizard. `useWizard()` still works: `current` reports the first step and `layout` reports `'page'`, but `next`/`prev`/`go` are no-ops. It is entirely driven by `steps`/`WizardStep`, so the same array and step markup used for a `steps` wizard can render either layout by only changing the `layout` prop.

```tsx
<Wizard steps={steps} layout="page">
  <WizardStep id="account">…fields…</WizardStep>
  <WizardStep id="plan">…fields…</WizardStep>
  <WizardStep id="review">
    <ReadOnlyField name="email" editStep="account" />
  </WizardStep>
  <SubmitButton />
</Wizard>
```

Nested `FormSection`s (inside a step or anywhere else) get progressively deeper legend headings automatically — `h3` at the top level, `h4` one level in, `h5` the next, capped at `h6` — so a step with its own sub-sections still produces a correct heading hierarchy without hand-picking `slotProps.legend.component` at every level. An explicit `slotProps.legend.component` (your own, or a theme default) always overrides the automatic level.

### Conditional steps

A step can hide itself based on the form's own values: `WizardStepDef.when?: (values) => boolean`. Pass the full step list, including the conditional ones — `Wizard` derives the visible list itself, so you never filter `steps` by hand:

```tsx
const steps = [
  { id: 'account', label: 'Account', fields: ['name', 'email'] },
  { id: 'hasVehicle', label: 'Vehicle?', fields: ['hasVehicle'] },
  { id: 'vehicle', label: 'Vehicle', fields: ['vehicle'], when: (v) => v.hasVehicle },
  { id: 'review', label: 'Review' },
] as const satisfies WizardStepDef<z.input<typeof schema>>[]
```

A wizard with no `when` anywhere subscribes to nothing extra: `useWatch` is not called at all, not even in a disabled form — the subscription only exists once at least one step defines `when`. A wizard that does have a `when` re-renders on every form value change (it has to, to re-evaluate the predicates), but the effective `steps` array itself keeps its reference across a change that doesn't flip any predicate's answer — only a render where some step's `when` actually flips gets a new array, so a `WizardStepper`/`WizardNav`/`useWizard()` consumer memoized on `steps` doesn't re-run on every keystroke.

A hidden step disappears from the stepper and from `next`/`prev`/`go` navigation and `page` layout, and its `fields` are skipped by Next; if it was the current step, the wizard moves to the nearest visible step before it (or the first, if none qualify). `visited` still remembers a hidden step's id, so it comes back as completed — not upcoming — if it becomes visible again, and a saved `visited`/`step` that names a currently-hidden step resumes on the nearest visible one instead.

`when` only controls navigation and per-step validation — final submit still validates the whole schema, so a hidden step's fields need to be genuinely optional at the schema level (a `superRefine` that only requires them when the gate is true, or a discriminated union) the same way a field listed in no step's `fields` already does. If a hidden step's field still manages to fail final submit (a schema this loose lets it), the wizard treats it exactly like an error on a field listed in no step: it reports against the last step rather than the (unreachable, unmounted) step that owns it, and since that field was never mounted there's no input for `setFocus` to land on — the summary/error still renders, just without a working focus target. Keep a hidden step's fields truly optional in the schema and this never comes up in practice.

### One route per step

`Wizard` is controlled through `step` / `onStepChange`; wire those to your router. With react-router, put `<Form>` + `<Wizard>` in a layout route and render the step routes through `<Outlet>`:

```tsx
function SignupLayout() {
  const { step = '' } = useParams()
  const navigate = useNavigate()
  return (
    <Form schema={schema} defaultValues={defaults} onSubmit={save}>
      <Wizard steps={steps} step={step} onStepChange={(s) => navigate(`/signup/${s.id}`)}>
        <WizardStepper />
        <Outlet />
        <WizardNav />
      </Wizard>
    </Form>
  )
}
// routes: { path: '/signup', element: <SignupLayout/>, children: [{ path: ':step', element: <SignupStep/> }] }
```

A URL for a step the user has not reached (a deep link, a reload) makes the wizard call `onStepChange` with the last visited step. To resume across reloads, save `visited` (via `onVisitedChange`) with your draft values and pass both back.

## Conditional fields

Show, hide, or require a field based on another field's value with `useWatch` + conditional JSX; the schema, not a hookform rule prop, decides what actually gets validated. `Form/ConditionalFields` in Storybook is a working reference for six patterns — checkbox reveal, select-value reveal, switch-toggled section, threshold, cascading selects, and mutually-exclusive fields — and `Checkout` (same-as-shipping, country → region), `SignUp` (referral source → "Other"), and `Loan` (employment "Other", income-below-threshold co-signer) use them in full examples.

```tsx
function BillingSection() {
  const sameAsShipping = useWatch<Input, 'sameAsShipping'>({ name: 'sameAsShipping' })
  return (
    <FormSection title="Billing address">
      <Checkbox name="sameAsShipping" label="Same as shipping address" />
      {!sameAsShipping && (
        <>
          <TextField name="billing.street" label="Street address" />
          {/* …rest of the address, no `required` prop… */}
        </>
      )}
    </FormSection>
  )
}
```

**Schema side.** A hookform `required` (or any rule prop) always wins over zod's message for that field (see the Validation rules section), so a field that is required only sometimes should carry no rule prop at all — express "required when" once, in the schema:

- **`superRefine`** for "required only when shown": check the controlling value and `ctx.addIssue` with an explicit `path` when the dependent field is empty. This is the right tool whenever the branches mostly share the same shape (`Checkout`'s billing address, `SignUp`'s referral-other, `Loan`'s co-signer note).
- **`z.discriminatedUnion`** reads better when the branches genuinely don't share fields — "contact by phone" needs a phone number, "contact by email" needs an email address, and neither needs the other's field. Tag the union on the controlling field (`contactBy`) and let each branch declare only what it needs; `ConditionalFields`'s pattern 6 is a worked example.

Either way, the message shown to the user comes from the schema, not from a rule prop that can't tell "hidden" from "shown but empty" apart.

> **Zod gotcha:** by default, zod skips a top-level `.refine()`/`.superRefine()` entirely once anything else in the same object produced a "non-continuable" issue — a `z.enum`/`z.literal`/discriminated-union mismatch, among others — even in a completely unrelated field on a completely unrelated step. A blank `purpose` `Select` three steps away can silently swallow your `superRefine`'s messages, and an unchecked "accept terms" `z.literal(true)` can just as easily swallow a `.refine()` checking two other fields match. Pass `{ when: () => true }` as the second argument to make the refinement run regardless of what else in the object failed: `.refine(fn, { when: () => true, ... })` / `.superRefine(fn, { when: () => true })`. Every top-level `refine`/`superRefine` in `ConditionalFields`, `Checkout`, `SignUp`, and `Loan` does this for exactly that reason — a `.refine()` chained directly on a single field's own schema (not the object) is unaffected, since it runs as part of that field's own type resolution rather than the object's post-parse checks. Where the field itself is the thing that starts "empty," consider sidestepping the whole issue: a placeholder-select value can be typed as `z.union([z.enum([...]), z.literal('')])` so the empty state is a valid (if still-incomplete) member of the type rather than an aborting mismatch — trade-off is that "required" then has to be enforced by a separate check instead of the enum's own validation.

**Hidden values.** react-hook-form's default `shouldUnregister: false` means an unmounted field's value is **not** cleared when it disappears from the JSX — re-checking the box brings back what was typed before, which is usually what you want. Two cases call for clearing it on purpose instead:

- **The hidden value would be stale and wrong if left in the payload** — a state chosen for the old country no longer matches the new one's option list. Reset it explicitly with `resetField` (or `setValue`) in the change handler of the field that controls visibility, as `ConditionalFields`'s cascading-select pattern does:
  ```tsx
  <Select
    name="address.country"
    label="Country"
    options={COUNTRY_OPTIONS}
    onChange={() => resetField('address.region', { defaultValue: '' })}
  />
  ```
- **The hidden value must not be submitted at all** — omit it from the schema's output (a discriminated union already does this: the non-matching branch's fields are simply not part of that variant's type), or strip it in `onSubmit` before calling your API.

**Accessibility.** A revealed field gets focus only on an explicit user action (never automatically on reveal) — the section's `<legend>` already names the group, so a screen reader user tabbing in hears the group name before the new field, with no extra announcement needed. Nothing here uses a live region: a `role="status"`/`role="alert"` announcement is for something the user didn't directly cause (a server response, an array add/remove elsewhere on the page — see Field arrays); a field appearing directly under the control the user just operated needs no separate announcement. The one exception is a cascading reset (country → region): that clears a value the user can't see change, so keep the reset visible in the UI (the newly-emptied field renders with its placeholder, not a stale label) rather than trying to announce it.

## Confirmations and guards

- `<Form confirm>`: dialog after validation, before `onSubmit`, on every submit path.
- `<ClearButton confirm>`: dialog before reset.
- `<Form guard>`: browser prompt on tab close / reload while dirty.
- `useFormGuard(useBlocker)`: in-app navigation; pass react-router's `useBlocker` and render a `ConfirmDialog` with the result.
- `<FormDialog>`: a form in a dialog, which asks before closing with unsaved changes — see below.

## Form in a dialog

`FormDialog` is a `<Form>` inside a MUI `Dialog`, with the heading, the scrolling
content area and the action buttons already wired. It takes every `Form` prop and
every `Dialog` prop; you own `open` and `onClose`.

```tsx
const [open, setOpen] = useState(false)

<FormDialog
  open={open}
  onClose={() => setOpen(false)}
  title="Edit contact"
  schema={contactSchema}
  defaultValues={contact}
  onSubmit={async (values) => save(values)}
>
  <Stack spacing={2}>
    <TextField name="name" label="Name" />
    <TextField name="email" label="Email" />
  </Stack>
</FormDialog>
```

**Closing asks first.** Escape, a backdrop click, and the Cancel button all go
through the same gate: if the form is dirty, a `ConfirmDialog` appears
(`Discard changes?` / `Discard` / `Keep editing`) and `onClose` is called only if
it is confirmed — so `onClose` always means "it really is closing". A pristine
form closes with no prompt, and neither does one that has just been submitted
successfully (the same `isDirty && !isSubmitting && !isSubmitSuccessful` rule the
other guards use). Pass `exitConfirm` to change the copy, or `exitConfirm={false}`
to drop the prompt.

**Submitting closes it.** A successful submit calls `onClose(event, 'submit')`
after your `onSubmit` settles — so a save that rejects, or one that calls
`form.setError`, leaves the dialog open with its values intact.
`closeOnSubmit={false}` keeps it open for an "add another" flow.

`onClose`'s `reason` is MUI's own (`'escapeKeyDown'`, `'backdropClick'`) plus
`'cancelClick'` and `'submit'`.

**Actions.** The default footer is a Cancel `Button` and a `SubmitButton`, in that
DOM order; `cancelLabel` / `submitLabel` rename them and `slotProps.cancel` /
`slotProps.submit` reach them. Pass `actions` to replace both with your own — a
`type="submit"` button inside still submits the form. Cancel is disabled while a
submit is pending, since cancelling then would abandon a save already in flight.
A `slotProps.cancel.onClick` runs _before_ the close gate and does not replace it;
call `event.preventDefault()` in it to veto the close and keep the dialog open.

**Layout.** ARIA does not allow `role="dialog"` on a `<form>`, so the dialog's
paper stays a `div` and the `<form>` sits just inside it, carrying the paper's
flex layout through — long content still scrolls inside `DialogContent` while the
title and actions stay put. `title` names the dialog (`aria-labelledby`) and
whatever `Form` renders in its description slot describes it (`aria-describedby`) —
that includes the required-fields convention `Form` states by default, so the
dialog is described even when you pass no `description` of your own.

Theme it through `theme.components.EzFormDialog` (`defaultProps`, and
`styleOverrides` for the `root`, `form`, `title`, `content`, `actions`, `cancel`
and `submit` slots).

## Timeouts (OTP codes, sessions)

WCAG 2.2.1 Timing Adjustable requires that if a timeout — an OTP code expiry, a session timeout, or any server-side time limit — affects the user, you must either disable it, let the user adjust it to ≥20 seconds, or warn them ≥20 seconds before it expires so they can extend it for at least 10× longer. Form values are never lost across timeouts: `<Form>` holds them until submission or reset, so a user can resubmit after resending a code or re-authenticating.

A recommended pattern for OTP codes uses `<ResendCodeButton>` (which announces success/error via `role="status"`) and a countdown announced in a separate status region (not live, so it doesn't tick into assistive tech every second). Warn when ≥20 seconds remain; if the user doesn't act, show a `<FormError>` with the root error and offer resend via `<ResendCodeButton>` or a `Keep me signed in` action for session timeouts. A visible countdown built with native `setInterval` and state (the example shows the pattern) is simpler than a library hook; ez-form ships no timer on purpose, since codegen only if a real flow drives it.

```tsx
import { useEffect, useRef, useState } from 'react'
import Stack from '@mui/material/Stack'
import Typography from '@mui/material/Typography'
import { Form, FormMethods, OtpField, ResendCodeButton, FormError, SubmitButton } from 'ez-form'
import { z } from 'zod'

const schema = z.object({ code: z.string().min(6) })

function VerifyOtpContent({ onCodeSent }: { onCodeSent: () => void }) {
  const [expiry, setExpiry] = useState<number | null>(null)
  const [secondsLeft, setSecondsLeft] = useState(0)
  const [warningAnnounced, setWarningAnnounced] = useState(false)
  const form = useRef<FormMethods<z.input<typeof schema>>>(null)

  useEffect(() => {
    if (!expiry) return
    const interval = setInterval(() => {
      const now = Date.now()
      const remaining = Math.max(0, Math.ceil((expiry - now) / 1000))
      setSecondsLeft(remaining)
      if (remaining === 0) {
        // Timeout: set root error, clear expiry to stop the countdown, user can resend
        form.current?.setError('root.timeout', { message: 'Code expired. Send a new one below.' })
        setExpiry(null)
        setWarningAnnounced(false)
      } else if (remaining === 20 && !warningAnnounced) {
        setWarningAnnounced(true)
      }
    }, 1000)
    return () => clearInterval(interval)
  }, [expiry, warningAnnounced])

  return (
    <Stack spacing={2}>
      <FormError />
      <OtpField name="code" label="Verification code" length={6} required />
      {expiry && secondsLeft > 0 && (
        <Stack spacing={1}>
          {secondsLeft >= 20 ? (
            <Typography variant="body2" color="warning.main">
              Code expires in {secondsLeft}s
            </Typography>
          ) : (
            <Typography variant="body2" color="error.main">
              Code expires in {secondsLeft}s
            </Typography>
          )}
          {warningAnnounced && secondsLeft < 20 && (
            <Typography component="span" variant="body2" role="status">
              Warning: 20 seconds left to enter code.
            </Typography>
          )}
        </Stack>
      )}
      <ResendCodeButton
        onResend={async () => {
          // Your API call here
          onCodeSent()
          setExpiry(Date.now() + 5 * 60 * 1000) // 5-minute expiry
          setWarningAnnounced(false)
        }}
        cooldown={30}
      />
      <SubmitButton />
    </Stack>
  )
}

export function VerifyOtp({ onCodeSent }: { onCodeSent: () => void }) {
  const form = useRef<FormMethods<z.input<typeof schema>>>(null)

  return (
    <Form ref={form} schema={schema} defaultValues={{ code: '' }} onSubmit={console.log}>
      <VerifyOtpContent onCodeSent={onCodeSent} />
    </Form>
  )
}
```

The `sign-up` example (a multi-step form with a verification step) shows this pattern in practice; see `src/examples/SignUp/SignUp.tsx`.

## Theming

Every ez-form component registers with MUI's theme under `Ez<Name>` (`defaultProps`, `styleOverrides`, and a `<name>Classes` object for slot class names) — no styling is baked into the component itself, so a theme can override every default.

```tsx
const theme = createTheme({
  components: {
    EzForm: { defaultProps: { slotProps: { title: { component: 'h1', variant: 'h4' } } } },
    EzFormSection: { styleOverrides: { legend: { marginBottom: 8 } } },
    EzWizardNav: {
      defaultProps: { slotProps: { next: { variant: 'outlined' } } },
    },
    EzReadOnlyField: {
      styleOverrides: { label: { textTransform: 'uppercase' } },
    },
    EzNumberField: {
      styleOverrides: { steppers: { borderLeft: 'none' } },
    },
    EzPasswordField: {
      defaultProps: { slotProps: { toggle: { size: 'small' } } },
      styleOverrides: { toggle: { color: 'primary' } },
    },
    EzPasswordStrength: {
      styleOverrides: { bar: { height: 6 } },
    EzOtpField: {
      styleOverrides: { helperText: { marginLeft: 8 } },
    },
    EzFileField: {
      styleOverrides: { fileList: { marginTop: 16 }, dropZone: { borderStyle: 'solid' } },
    },
  },
})
```

## Localization

Every default string in the library — button labels, the required-fields note, announcements, the rule messages, the pickers' messages — is a `defaultProps` key on its `Ez*` component, so a locale is a theme, the way MUI ships `@mui/material/locale`'s `esES`. v1 ships `enUS` and `esES`; both are plain objects and tree-shake.

```tsx
import { createTheme, ThemeProvider } from '@mui/material/styles'
import { esES as muiEsES } from '@mui/material/locale'
import { esES as pickersEsES } from '@mui/x-date-pickers/locales'
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider'
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns'
import { es } from 'date-fns/locale/es'
import { esES } from 'ez-form'

// ez-form's strings, MUI's own (Autocomplete, Alert, …), and MUI X's picker chrome, in one theme.
const theme = createTheme(baseTheme, muiEsES, pickersEsES, esES)

;<ThemeProvider theme={theme}>
  <LocalizationProvider dateAdapter={AdapterDateFns} adapterLocale={es}>
    <App />
  </LocalizationProvider>
</ThemeProvider>
```

What is and is not covered:

- **Library strings** — `esES` translates them all: `Submit`/`Clear`/`Back`/`Next`, `Confirm`/`Cancel`, the `Discard changes?` prompts, `(optional)` and the required-fields note, `Submitting…`/`Submitted.`/`Submit failed.`, `There is a problem`, the wizard's `Step 2 of 5, …`, `FieldArray`'s `Add`/`Remove`/`Row 2 added`, `AddressField`'s part labels, the password/SSN reveal toggles, `PasswordStrength`'s tiers, `FileField`'s rejections, every US field's `invalidMessage`, `EmailListField`'s announcements, `ReadOnlyField`'s `Yes`/`No`/`Edit`, and the chip delete icon's `Remove …`.
- **Rule messages** — `<label> is required.`, `must be at least …`, the pickers' `is too early.` and friends come from one set, `EzForm.defaultProps.messages` (the `RuleMessages` type). Every entry is a function of the label, so a translation can put the label wherever its grammar wants it; `fallbackLabel` (`This field`) stands in when a field has no string label. Pass `<Form messages={{ required: (l) => … }}>` to change one without a theme.
- **Interpolated strings are functions** — `stepAnnouncement`, `PhoneField`'s `invalidMessage(digits)`, `FieldArray`'s `addedMessage(row)`, `ChipDeleteIcon`'s `removeLabel(label)`, and `Form`'s `requiredIndicatorText(mode)` (one theme default serves both `requiredIndicator` modes).
- **Your copy stays yours** — field labels, `title`/`description`, zod messages, `addressSchema({ messages })`, and `ConfirmOptions` you pass explicitly. A prop you pass replaces the theme default whole (MUI's own `defaultProps` rule for a non-slot key), so under a locale, pass all three keys of `FormDialog`'s `exitConfirm` when you override one.
- **Dates and numbers are MUI X's and `Intl`'s job** — the locale objects translate ez-form's _messages_ only. Date _format_ comes from `LocalizationProvider`'s `adapterLocale`; the pickers' own chrome (`Clear`, the toolbar) from MUI X's `esES`; `MoneyField`/`PercentField` format through `Intl.NumberFormat` (`locale`/`format` props).
- **Another language** — copy `src/locales/enUS.ts`, translate the values, and apply it the same way. `enUS` is the shipped defaults restated (a test pins every key to the component's own default), so its keys are the complete list; the two objects have identical key sets by type and by test. Other locales are yours to supply; the library ships English and Spanish.

### Theme preset

Nothing above is applied unless you ask for it. `createEzFormTheme` is the one opt-in
place that holds an opinion — static labels, now the `stacked` variant (`MuiTextField`'s
`defaultProps.variant`, whose `MuiInputLabel` rules put a `top` label above the input with
no floating animation and no notch; `variant="outlined"` opts one field back to MUI's
floating label — see [Custom variants](#custom-variants)), flat bordered inputs
with a small radius and a soft focus ring, a
neutral contained button, light and dark colour schemes, and every transition collapsed
under `prefers-reduced-motion: reduce` (WCAG 2.3.3). The primitives and most component
customisations are adapted from MUI's own dashboard template (MIT). Styling taste lives in
[`DESIGN.md`](./DESIGN.md); the preset in `src/theme/ezFormTheme.ts` is its code form, and
a test keeps the two files' tokens equal.

```tsx
import { ThemeProvider, CssBaseline } from '@mui/material'
import { createEzFormTheme } from 'ez-form'

const theme = createEzFormTheme({
  components: { EzSubmitButton: { defaultProps: { fullWidth: true } } },
})

;<ThemeProvider theme={theme}>
  <CssBaseline /> {/* the reduced-motion rule lives in MuiCssBaseline */}
  <App />
</ThemeProvider>
```

With no scheme named, the theme carries both colour schemes and MUI's `ThemeProvider`
follows the OS (`prefers-color-scheme`) and `useColorScheme`; pass `defaultMode="light"`
to the provider to start light regardless. Naming a scheme — `defaultColorScheme: 'dark'`,
or the `palette: { mode: 'dark' }` idiom — pins the theme to that scheme alone.

`createEzFormTheme(options)` merges the palette-shaping keys (`colorSchemes`, `palette`,
`defaultColorScheme`, `cssVariables`, `typography`, `shape`) into the preset before MUI
builds the theme, so MUI derives shades and contrast text from _your_ colours; everything
else deep-merges into the built theme through `createTheme(options, ...args)`. For full
control, spread the plain options yourself:
`createTheme({ ...ezFormThemeOptions, defaultColorScheme: 'dark' })`.

Storybook has a "Theme" toolbar toggle — `Modern`, `Modern (dark)`, `Stock MUI` — so
every story can be checked with and without it.

## Loading values from a server

```tsx
// Form owns the fetch: disabled while loading, filled when it resolves
<Form schema={schema} defaultValues={async () => (await fetch('/api/me')).json()} onSubmit={save}>

// Or hand it the data from a hook: the form re-syncs whenever `values` changes
const { data } = useQuery(...)
<Form schema={schema} values={data} resetOptions={{ keepDirtyValues: true }} onSubmit={save}>

// Reset or set values from a parent through `ref`
const form = useRef<FormMethods<Input, Output>>(null)
<Form ref={form} …>  …  <Button onClick={() => form.current?.reset()}>Clear</Button>
```

If the async `defaultValues` function rejects, the form re-enables with its fields empty
(no defaults were applied). Pass `onDefaultValuesError` to handle the rejection; without
it, the error rethrows as an unhandled rejection.

## Validation rules

Every field also takes hookform-style rules as props. A bare value gets a message derived from the label; `{ value, message }` customizes it. A rule error wins over zod's message for that field; zod still validates everything else, handles cross-field checks, and types `onSubmit`.

```tsx
<TextField name="email" label="Email" required />                                   // "Email is required."
<TextField name="email" label="Email" required="You must enter something here!" />  // custom
<TextField name="age" label="Age" required min={18} max={{ value: 99, message: 'Nobody is that old' }} />
<TextField name="nick" label="Nickname" minLength={3} maxLength={{ value: 12, message: 'Too long!' }} pattern={/^[a-z]+$/} />
<TextField name="user" label="Username" validate={(v) => v !== 'admin' || 'Reserved'} />
<Checkbox name="tos" label="I accept the terms" required />                          // must be checked
```

| rule                      | fields                                                                                                              | default message                                                                                            |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `required`                | all except Slider                                                                                                   | `<label> is required.` (also renders the asterisk)                                                         |
| `min` / `max`             | TextField, TextareaField, Select, Autocomplete, NumberField, MoneyField, Slider (a number, or both ends of a range) | `<label> must be at least/most <value>.` Numbers, or date strings (compared as dates)                      |
| `minLength` / `maxLength` | TextField, TextareaField, Select, Autocomplete                                                                      | `<label> must be at least/most <value> characters.` (`maxLength` also drives TextareaField's length meter) |
| `pattern`                 | TextField, TextareaField, Select, Autocomplete                                                                      | `<label> is invalid.`                                                                                      |
| `validate`                | all                                                                                                                 | a returned string; `false` gives `<label> is invalid.`                                                     |

`required` fails on empty or `false`; `min`/`max`/length/`pattern` rules are skipped while the value is empty; `validate` always runs (so `validate={(v) => v || 'You must opt in'}` works on an unchecked checkbox). Rules run in hookform's order, first failure wins.

When you pass `validate` as a record, the keys `complete` (OtpField, PhoneField), `picker` (the date pickers), and `min`/`max` (Slider) are reserved for those fields' built-in checks and will override an entry of the same name.

Every rule message is a **string**, including the per-field message props (`invalidMessage` on `PhoneField`, `errorMessages` on the date pickers). react-hook-form's `Message` type is `string`, so a message has to survive the trip through `useController`'s rules to `fieldState.error.message` — a `ReactNode` cannot. Rich markup in an error belongs in your own `validate` or in `helperText`.

## Date pickers

`DatePicker`, `TimePicker`, `DateTimePicker`, and `DateField` wrap MUI X. Wrap your app in MUI X's `LocalizationProvider` with the adapter you use; the form stores whatever that adapter produces (a `Date` under `AdapterDateFns`, a `Dayjs` under `AdapterDayjs`), so type the field accordingly (`z.date()`, or `z.custom<Dayjs>()`).

```tsx
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider'
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns'

;<LocalizationProvider dateAdapter={AdapterDateFns}>
  <Form
    schema={z.object({ start: z.date().nullable() })}
    defaultValues={{ start: null }}
    onSubmit={save}
  >
    <DatePicker name="start" label="Start" minDate={new Date()} required />
  </Form>
</LocalizationProvider>
```

The picker's own validation (`minDate`, `disablePast`, an unparsable date, …) shows through the field like any rule: `Start is too early.`, `Start must be in the future.`, `Start is invalid.`, `Start is not available.` Override per code with `errorMessages={{ minDate: 'Pick a later day' }}`.

### When to use which

| Use case                                 | Prefer       | Why                                                                |
| ---------------------------------------- | ------------ | ------------------------------------------------------------------ |
| Birthdays, anniversaries, document dates | `DateField`  | Typing a date beats paging a calendar back 20/30/50+ years         |
| Scheduling near today                    | `DatePicker` | A calendar is faster to scan than typing when the date is close by |

`DateField` is a keyboard-only, sectioned input with no popup — the same `value`/`onChange`/`onError` contract as `DatePicker`, so it drops in wherever a birthday-shaped date is collected:

```tsx
<DateField name="birthday" label="Birthday" disableFuture minDate={new Date(1900, 0, 1)} required />
```

`disableFuture` plus a sane `minDate` catches typos without needing a calendar to navigate.

### Required dates

Use `z.date().nullable()` in the schema and `required` on the field — the same pattern as any other field. The `required` rule handles non-null enforcement and shows "Birthday is required." when empty. Do not reach for `z.date().nullable().refine(v => v !== null, { message })` — a schema-level non-null check would fight the `required` rule's message rather than compose with it.

```tsx
<Form
  schema={z.object({ birthday: z.date().nullable() })}
  defaultValues={{ birthday: null }}
  onSubmit={save}
>
  <DateField
    name="birthday"
    label="Birthday"
    disableFuture
    minDate={new Date(1900, 0, 1)}
    required
  />
</Form>
```

`disableFuture`, `minDate`, and other picker validations compose with `required` — compose them freely.

One ordering to know: paste something unparsable (`March 2, 2024`, say) into a **required** picker that is still empty and the message is `Birthday is required.`, not `Birthday is invalid.` MUI X parses an unrecognisable string to `null`, so the field really is empty, and `ezResolver` runs `required` before the picker's own code. Submit is blocked either way, and the message becomes `Birthday is invalid.` as soon as the field is not required or already holds a date. Nothing to configure — just don't be surprised by which of the two you see.

### Clearing a picker

MUI X's own `clearable` works as it does outside a form. Pass it — and any `onClear` of your own — where MUI X types it for the component you are using, which is not the same slot for both:

```tsx
// Popup pickers (DatePicker, TimePicker, DateTimePicker): the field slot.
<DatePicker name="start" label="Start" slotProps={{ field: { clearable: true, onClear } }} />

// DateField *is* the text field, so both are flat props.
<DateField name="birthday" label="Birthday" clearable onClear={onClear} />
```

(MUI X omits `clearable`/`onClear` from the popup pickers' `slotProps.textField` type, which is why they go on `slotProps.field` there.)

Clearing resets the picker's validation state along with the value, so an `invalidDate` left over from an unparsable paste goes away with it, and your `onChange` is called with `null` either way. Note that MUI X only renders the clear button while the field shows something — after an unparsable paste it blanks every section, so the button stays hidden until the user types into one.

## Autocomplete

Address lookup (Places-style): the options list is fed by an async lookup, and the free-typed text is kept even if it doesn't match a suggestion.

A single field stores `null` when cleared; a single `freeSolo` field stores `''` instead, since its value is text and a cleared text field is the empty string (so `required` and `z.string()` treat "cleared" and "never typed" alike). `AddressField`'s `lookup` mode is this pattern packaged, with the provider seam below.

```tsx
<Autocomplete
  name="address"
  label="Address"
  freeSolo
  autoSelect
  options={predictions} // [{ value: description, label: description, placeId }]
  loading={loading}
  filterOptions={(x) => x}
  onInputChange={(_, q, reason) => reason === 'input' && lookup(q)}
  onChange={(_, v) => v && typeof v === 'object' && fetchDetails(v.placeId)}
/>
```

The form stores the address string (`z.string()`); `placeId` reaches `onChange` but isn't stored. For objects in form state use `getOptionValue={(o) => o}` and a `z.object` schema.

Under `multiple`, each chip's delete control is named `Remove <option label>` and sized to a
24×24 target (MUI's default icon is neither) via the shared `ChipDeleteIcon`; `slotProps.chip`
still merges over the chip, and a `renderValue` of your own takes over chip rendering entirely.

## EmailListField

A "To:" line: many addresses, each one a chip, and the form value is a plain
`string[]`. It extends `Autocomplete` (`multiple` + `freeSolo`), so every rule and
every prop that field takes still applies.

```tsx
<EmailListField name="to" label="To" />
```

```ts
const schema = z.object({ to: z.array(z.string()).min(1, 'Add at least one recipient') })
```

**Entry** follows what mail clients have taught people to expect. Enter, a comma, a
semicolon, a space _after a complete address_, and blurring the field all commit what is
typed — a space is deliberately conditional, so typing a display name doesn't commit a
fragment. Pasting `a@x.com, b@y.com; c@z.com` splits into three chips at once, and
duplicates collapse case-insensitively (compared lowercased, stored exactly as typed).

An address that fails validation **stays as a chip**, in its error state, with the field
errored and submit blocked — rather than disappearing. A wrong address you can see is
fixable; one that vanished is a guess. The message is `invalidMessage` (default
`'Enter a valid email address'`); a rejected duplicate announces `duplicateMessage`
(default `'Already added'`) instead.

Validity is the WHATWG `<input type="email">` rule, exported as `isEmail` / `EMAIL_PATTERN` —
the same regex a browser applies, so the field and the browser never disagree about a string.

### Async lookup

`loadOptions` turns the dropdown into a directory search. It gets the current input text and
an `AbortSignal`, is debounced by `debounceMs` (default 250), and the signal is aborted the
moment the next keystroke supersedes the query — so a slow response can never overwrite a
newer one:

```tsx
<EmailListField
  name="to"
  label="To"
  loadOptions={(query, signal) =>
    fetch(`/api/people?q=${encodeURIComponent(query)}`, { signal })
      .then((r) => r.json())
      .then((people) =>
        people.map((p) => ({ value: p.email, label: `${p.name} <${p.email}>`, id: p.id })),
      )
  }
/>
```

A chip from a result reads as its option `label` (`Ada Lovelace <ada@example.com>`); **only
the address is stored**. Extra fields on an option (`id` above) reach your `onChange` untouched.

`allowNew={false}` makes it a picker over the lookup's results only — a well-formed address
the directory never returned is rejected with `invalidMessage`:

```tsx
<EmailListField name="to" label="To" loadOptions={search} allowNew={false} />
```

### Announcements

Adds and removes go through a `role="status"` region the field renders itself, so a screen
reader hears each chip appear and disappear. Every string is a prop, so they localise:
`addedMessage` / `removedMessage` (`(email) => string`), `addedManyMessage` /
`removedManyMessage` (`(count) => string`) and `duplicateMessage`.

One action can do more than one thing — a paste that adds two addresses and rejects a third —
so the announcement carries every part rather than only the first (`"b@y.com added Already
added"`). A duplicate is announced only when one was actually skipped, never merely because
nothing changed.

Each chip's delete control carries an accessible name (`Remove <label>`) and a 24×24 target.
`slotProps.chip` is merged _under_ the props that make a chip removable, and its `onDelete`
composes with the field's own — call `preventDefault()` in yours to veto a removal. The delete
icon itself is the field's (that accessible name is not a consumer's to drop), so it is excluded
from the slot's type; it is the shared `ChipDeleteIcon`, restyled through
`theme.components.EzChipDeleteIcon`.

Under `<Form assisted>` the input's `autoComplete` becomes `'off'` (#65), since the person
filling the form is not necessarily the person the addresses belong to; an explicit
`autoComplete` still wins.

Themeable under `EzEmailListField` (`defaultProps`, and `styleOverrides` for `chip` and
`status`, exported as `emailListFieldClasses`).

## Rating

Value is `number | null`; clicking the currently selected star clears it back to `null`.
The `label` renders as the legend above the stars.

```tsx
<Rating name="score" label="How was it?" required />
```

```ts
const schema = z.object({ score: z.number().nullable() })
```

### `size="small"` is below the target-size minimum

This is the one control ez-form renders that does not meet
[WCAG 2.5.8 Target Size (Minimum)](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum.html).
At the default size MUI draws the star at `font-size: 24px` with no padding — exactly the
24×24 minimum. At `size="small"` it drops to 18px, an 18×18 target.

ez-form leaves that alone on purpose: MUI owns the small-variant sizing, and padding a glyph
you explicitly asked to be small is a styling judgement this library does not make for you.
If you want the small star with a compliant hit area, add the padding in your theme — the
star still looks small, the target does not:

```ts
const theme = createTheme({
  components: {
    MuiRating: {
      styleOverrides: {
        // 18px icon + 3px padding on each side = a 24×24 target.
        sizeSmall: { '& .MuiRating-icon': { padding: 3 } },
      },
    },
  },
})
```

Or simply leave `size` off, which is already compliant.

## TextareaField

`TextField` with `multiline` fixed on: a taller default (`minRows: 4`, `maxRows: 12`, both
themeable under `EzTextareaField.defaultProps`) that autogrows in between. Setting the
`maxLength` rule (or passing `showCount`) shows a length meter — `n` alone with no bound,
`n / max` with one — as a trailing element inside the helper text, associated via the same
`aria-describedby` id as the rest of the helper text. Past the limit the meter is replaced
by the normal `maxLength` validation error, never flagged by colour alone.

```tsx
<TextareaField name="bio" label="Bio" maxLength={500} />
```

```ts
const schema = z.object({ bio: z.string().max(500) })
```

Themeable under `EzTextareaField` (`root`, `counter`, exported as `textareaFieldClasses`).

## FileField

The default is a picker button over a hidden `<input type="file">`. `dropzone` wraps
it in a drop area; the button inside stays the keyboard and screen-reader path, so the
zone is deliberately not focusable and carries no role — drag-and-drop adds no second
tab stop. `dropText` (default `'Drag files here, or'`) is the zone's visible
instruction, and `dragActive` is a visual cue only.

```tsx
<FileField name="photos" label="Choose photos" multiple dropzone accept="image/*" />
```

`accept`, `maxSize` (bytes) and `maxFiles` (under `multiple`) are validation, not just
filtering: a picked _or dropped_ file that fails one is rejected — it never enters the
value — and the reason surfaces as the field's error through a built-in rule, so it
fails a submit too. Any change to the value clears it: an accepted pick or drop, or
removing a file with its chip (which is how you answer a `maxFiles` rejection). `accept` still goes on the
input as the native attribute, and it is matched the way the native input matches it
(`.ext` by suffix, `type/subtype` exactly, `type/*` by prefix).

| Prop       | Rejection message prop | Default                          |
| ---------- | ---------------------- | -------------------------------- |
| `maxSize`  | `maxSizeMessage`       | `'File is larger than {size}'`   |
| `accept`   | `acceptMessage`        | `'File type not accepted'`       |
| `maxFiles` | `maxFilesMessage`      | `'Choose at most {count} files'` |

`{size}` is replaced with the limit humanized (`1500000` → `1.5 MB`) and `{count}` with
the limit; both messages are props, so they translate.

Under `multiple`, a pick or drop **appends** to the value rather than replacing it, so a
drop zone used twice accumulates and `maxFiles` means "at most n in total"; files come
off with their chips. A single-file field still replaces.

The library uploads nothing. `onFilesAdded(files)` fires once per pick or drop with the
files that passed — start your upload there — and `renderFile(file, index)` replaces the
default chip so you can render your own progress:

```tsx
<FileField
  name="photos"
  label="Choose photos"
  multiple
  dropzone
  maxSize={2_000_000}
  onFilesAdded={(files) => files.forEach(upload)}
  renderFile={(file) => <LinearProgress variant="determinate" value={percent(file)} />}
/>
```

`FileFieldProps` extends MUI's `FormControlProps` — the root the field renders — so
`FormControl` props (`fullWidth`, `margin`, `size`, `className`, `id`) and the `aria-*`
attributes all pass through. `aria-label`, `aria-labelledby` and `aria-describedby` are
routed to the `<input type="file">`, the element that carries the role, rather than left on
the wrapper: a consumer description is joined with the error message rather than replaced by
it, and `label` may be omitted entirely when one of the ARIA names is supplied.

```tsx
<span id="hint">PDF only, under 5 MB</span>
<FileField name="resume" label="Resume" aria-describedby="hint" required />
```

Themeable under `EzFileField` (`root`, `fileList`, `dropZone`, `dragActive`, `dropText`,
exported as `fileFieldClasses`); each chip's delete icon is the shared `ChipDeleteIcon`.

## Mobile keyboards & autofill

`TextField`, `NumberField`, `MoneyField`, and `OtpField` set `autoComplete` / `inputMode` defaults so mobile keyboards and password managers do the right thing without per-field wiring. A default only applies when the consumer sets neither the prop nor its `slotProps.htmlInput` equivalent — an explicit value always wins.

| Component      | Condition                                                             | `autoComplete`  | `inputMode`     |
| -------------- | --------------------------------------------------------------------- | --------------- | --------------- |
| `TextField`    | `type="email"`                                                        | `email`         | `email`         |
| `TextField`    | `type="tel"`                                                          | `tel`           | `tel`           |
| `TextField`    | `type="url"`                                                          | `url`           | `url`           |
| `TextField`    | `type="search"`                                                       | — (not guessed) | `search`        |
| `TextField`    | any other `type` (including no `type`)                                | — (not guessed) | — (not guessed) |
| `NumberField`  | integer-only (no fractional `step`, no `format` with fraction digits) | —               | `numeric`       |
| `NumberField`  | otherwise (fractional `step`, or a `format` with fraction digits)     | —               | `decimal`       |
| `MoneyField`   | always (currency has cents)                                           | —               | `decimal`       |
| `PercentField` | always (the percent format allows fraction digits)                    | —               | `decimal`       |
| `OtpField`     | first slot only, from Base UI itself, not duplicated here             | `one-time-code` | `numeric`       |
| `PhoneField`   | always — set by the field itself, not the `type="tel"` fallback       | `tel`           | `tel`           |
| `EmailField`   | always — set by the field itself, via its fixed `type="email"`        | `email`         | `email`         |
| `FeinField`    | always — a tax ID has no autofill token worth guessing                | `off`           | `numeric`       |
| `SsnField`     | always — an SSN has no autofill token, so `off` is deliberate         | `off`           | `numeric`       |

`TextField` never guesses a token from `name` — a wrong guess is worse than none, so only these unambiguous `type`s get a default. `PasswordField` covers `autoComplete="current-password"` / `"new-password"` on its own, above. `PhoneField` sets `type="tel"`, `inputMode="tel"` and its `autoComplete` default itself rather than relying on the `type`-derived fallback — the two agree, but the field owns them — and its `autoComplete` takes a sectioned token (`"shipping tel"`, `"work tel"`) for forms with more than one number. `EmailField` fixes `type="email"` (so `inputMode="email"` comes from `TextField`'s own `type` mapping) and defaults `autoComplete` to `'email'`, which likewise takes a sectioned token (`"work email"`). `SsnField` pins `autoComplete="off"` rather than offering it as a prop: there is no autofill token for a Social Security number, and inviting a browser to store one is the wrong default; `FeinField` pins the same default for an EIN, but leaves it overridable. Every one of these defaults becomes `"off"` under `<Form assisted>` (below). The remaining v8 date/time fields (#19) will extend this table.

## Assisted mode

`<Form assisted>` is for a form filled out by someone other than the person it's about — an
internal rep taking a phone order, an agent entering an applicant's details. You don't
necessarily know who's actually typing, so the browser's own autofill (their saved name,
address, card) should never be offered:

```tsx
<Form assisted schema={schema} defaultValues={values} onSubmit={onSubmit}>
  <TextField name="firstName" label="First name" />
  <PhoneField name="phone" label="Phone" />
</Form>
```

It does two things:

- Sets `autoComplete="off"` on the `<form>` element itself.
- Tells every field to emit `autoComplete="off"` in place of its own default token — `TextField`'s
  `type`-derived guess, `PhoneField`'s `'tel'`, `ZipField`'s `'postal-code'`, `StateSelect`'s
  `'address-level1'`, and each part of `AddressField`. `PasswordField` is the one exception: it
  emits `'new-password'` instead of `'off'` (below).

An **explicit `autoComplete` always wins**, on the `<Form>` itself or on any individual field —
`assisted` only replaces a field's own _default_. This matters for a field whose token has no
`type` to derive it from — `<TextField name="firstName" autoComplete="given-name" />`, say: since
there's no default to override, passing `assisted` on `<Form>` alone does nothing for it, and a
plain `TextField` has no built-in way to resolve it either. The `Insurance` example's Agent story
(`agentMode`) hits exactly this: its `given-name`/`family-name`/`street-address` fields are
hardcoded (no `type` derives them), so its `ApplicantStep`/`ContactStep` read the form's assisted
state and clear their own token when it's on — the pattern to follow for any field where you've
hardcoded a token that should also disappear under `assisted`.

**What browsers actually honour.** `autoComplete="off"` works for most fields, but Chromium
ignores it for address-shaped fields (street, city, state, ZIP) and offers to fill them anyway —
a documented, long-standing browser quirk, not a bug here. No test in this codebase has shown
`"off"` fail for ez-form's own address fields, so that's what they emit; a future field where it
provably doesn't work should fall back to a non-matching token (e.g. `one-time-code`) instead,
with the reasoning recorded next to the change. Password managers also don't reliably honour
`"off"` on a password input specifically, which is why `PasswordField` uses `autoComplete="new-password"`
under `assisted` regardless of whether it's a sign-in or sign-up field — the one token browsers
consistently treat as "don't fill from a saved credential".

`assisted` is theme-defaultable like any other `Form` prop:
`theme.components.EzForm.defaultProps.assisted`.

| Component      | Condition                                                             | `autoComplete`   | `inputMode`     |
| -------------- | --------------------------------------------------------------------- | ---------------- | --------------- |
| `TextField`    | `type="email"`                                                        | `email`          | `email`         |
| `TextField`    | `type="tel"`                                                          | `tel`            | `tel`           |
| `TextField`    | `type="url"`                                                          | `url`            | `url`           |
| `TextField`    | `type="search"`                                                       | — (not guessed)  | `search`        |
| `TextField`    | any other `type` (including no `type`)                                | — (not guessed)  | — (not guessed) |
| `NumberField`  | integer-only (no fractional `step`, no `format` with fraction digits) | —                | `numeric`       |
| `NumberField`  | otherwise (fractional `step`, or a `format` with fraction digits)     | —                | `decimal`       |
| `MoneyField`   | always (currency has cents)                                           | —                | `decimal`       |
| `OtpField`     | first slot only, from Base UI itself, not duplicated here             | `one-time-code`  | `numeric`       |
| `PhoneField`   | always — set by the field itself, not the `type="tel"` fallback       | `tel`            | `tel`           |
| `ZipField`     | always — set by the field itself                                      | `postal-code`    | `numeric`       |
| `StateSelect`  | always — reaches `Select`'s hidden native `<input>`                   | `address-level1` | —               |
| `AddressField` | always — per part, prefixed by `autoCompleteSection`                  | per part         | per part        |

`TextField` never guesses a token from `name` — a wrong guess is worse than none, so only these unambiguous `type`s get a default. `PasswordField` covers `autoComplete="current-password"` / `"new-password"` on its own, above. The US fields set their own tokens rather than relying on the `type`-derived fallback — see [US fields](#us-fields) for what each part gets and how `autoCompleteSection` prefixes them. The remaining v8 date/time fields (#17–#18) will extend this table.

## NumberField

Digits group as you type for every consumer (new in v2.1): typing `1234` shows `1,234` before any blur. Pass `format={{ useGrouping: false }}` to turn grouping off. Pasted numbers group on blur rather than on paste, because Base UI handles the paste itself.

It renders through MUI `TextField` — Base UI owns the value and the stepping, `TextField` owns the label, helper text and outlined look — and is themeable under `EzNumberField` (`root`, `steppers`, `increment`, `decrement`, exported as `numberFieldClasses`).

```tsx
<NumberField name="age" label="Age" min={18} max={120} />
```

## MoneyField

USD only: digits group as you type (`1234` shows `1,234`) and the field formats to `$1,234.50` on blur. The form value is always a plain `number` in dollars — never cents, never a string — and it rounds to the cent, so typing `19.999` submits `20`.

```tsx
<MoneyField name="price" label="Price" min={0} />
```

```ts
const schema = z.object({ price: z.number().min(0) })
```

## PercentField

`NumberField` with a percent sign, put through the same `Intl` `format` slot `MoneyField` uses for `$` — so the `%` is part of the formatted value rather than an adornment beside the input, and it is the theme's `EzNumberField` keys that style the field:

```tsx
const schema = z.object({ rate: z.number().nullable() })

<PercentField name="rate" label="Rate" />
// user types 12.5 → shows "12.5%" on blur, submits { rate: 12.5 }
```

`min` defaults to `0`, `max` to `100` and `step` to `1`, all overridable and all in **percentage points**. Digits group while typing exactly as they do on `NumberField`, and the value rounds to the two fraction digits the display carries.

`scale` decides what the stored number means. The default, `'percent'`, stores what it shows. `'fraction'` leaves the display alone and stores the fraction instead, for schemas and APIs that keep rates that way:

```tsx
<PercentField name="rate" label="Rate" scale="fraction" />
// user types 12.5 → shows "12.5%", submits { rate: 0.125 }
```

Only the stored value moves: `min`, `max` and `step` stay in percentage points under either scale, so `max={50}` means 50% and its message still reads `must be at most 50.` — the bound is scaled with the value so the rule fires at all, but the number you wrote is the number the user is told. A consumer `onValueChange` sees the **stored** value, matching what `onSubmit` receives, and an empty field is `null` rather than a scaled zero.

`scale` and the bound/step defaults are settable app-wide through `theme.components.EzPercentField.defaultProps`; a prop on the element always wins.

## PasswordField

`TextField` with `type` fixed to `password`/`text` by a show/hide toggle in the end adornment. The reveal state is local UI state only — it never reaches the form value, and it resets to hidden if the field unmounts. `autoComplete` defaults to `'current-password'`; pass `'new-password'` for sign-up/change-password fields.

```tsx
<PasswordField name="password" label="Password" />
<PasswordField name="newPassword" label="New password" autoComplete="new-password" revealable={false} />
```

The toggle is a themeable `IconButton` under `EzPasswordField` (`root`, `toggle`, exported as `passwordFieldClasses`); `slotProps.toggle` reaches it directly, but its `children` is always overridden — swap the reveal icons with `icons?: { show?: ReactNode; hide?: ReactNode }` instead, defaulted through `useDefaultProps` so `theme.components.EzPasswordField.defaultProps.icons` can replace them app-wide.

`showLabel?` / `hideLabel?` set the toggle's accessible name (defaults `'Show password'` / `'Hide password'`); like `icons`, both are settable app-wide through `theme.components.EzPasswordField.defaultProps`, which is what makes the toggle localisable.

Revealing keeps your place: flipping an `<input>` between `type="password"` and `type="text"` makes the browser re-create the editing context, which drops the selection, and the click that flipped it has already moved focus to the button. Both fields with a toggle capture the selection before the swap and restore focus and caret after it — unless focus was not in the input to begin with (a toggle reached by Tab), in which case focus stays on the button where you put it.

## EmailField

`TextField` with `type="email"` fixed on, so the mobile keyboard gets the `@` key and `inputMode`/`autoComplete` default to `email`. The value is the address string (`''` when empty, so `required` still applies):

```tsx
const schema = z.object({ email: z.string() })

<EmailField name="email" label="Email" required />
```

A built-in rule rejects a non-empty value that is not a valid address, using **HTML's own e-mail grammar** — the [WHATWG "valid e-mail address" production](https://html.spec.whatwg.org/multipage/input.html#valid-e-mail-address) a browser's `<input type="email">` validity check uses. So the field agrees with the native bubble instead of disagreeing with it in either direction, and your zod schema stays a plain `z.string()` with no `.email()` and no regex of its own:

```tsx
<EmailField name="email" label="Email" invalidMessage="We need a work address" />
```

`normalize` (default `true`) trims and lower-cases the value **on blur**, so what you submit, store and compare is canonical however it was typed or pasted — `' Ada@Example.COM '` becomes `'ada@example.com'`. It runs in the blur capture phase, ahead of the form's own handler, so validation timing is untouched: a form in `mode="onBlur"` reports on the fixed value, and the default `mode="onSubmit"` still reports nothing until submit. A consumer `onBlur` still fires either way, and the format rule canonicalizes before testing, so a value the field is about to fix is never rejected first. Pass `normalize={false}` to store exactly what was typed:

```tsx
<EmailField name="email" label="Email" normalize={false} />
```

`invalidMessage`, `normalize` and `autoComplete` are all settable app-wide through `theme.components.EzEmailField.defaultProps`; a prop on the element always wins. `EmailField` adds no styled slot of its own — it renders a `TextField`, so MUI's own `MuiTextField` / `MuiOutlinedInput` style keys reach it unchanged.

## PasswordStrength

A meter bound to a password field's live value, read with `useWatch` like `ReadOnlyField` — it never registers a field and never validates. Renders MUI `LinearProgress` as an ARIA `meter` (`role="meter"`, `aria-valuemin={0}`, `aria-valuemax={4}`, `aria-valuenow`, `aria-valuetext`) with a visible label in an `aria-live="polite"` region, so the tier is announced as it changes. An empty password renders the track at 0 with no label.

```tsx
<PasswordField name="password" label="Password" autoComplete="new-password" />
<PasswordStrength name="password" />
```

`score?: (password: string) => 0 | 1 | 2 | 3 | 4` defaults to a small built-in heuristic (length thresholds, character-class variety, a penalty for repeats/sequences) exported as `scorePassword`. Pass your own — `zxcvbn` / `@zxcvbn-ts`, for instance — to score by whatever rules you want:

```tsx
<PasswordStrength name="password" score={(pw) => toEzScore(zxcvbn(pw).score)} />
```

`labels?: readonly [string, string, string, string, string]` defaults to `['Very weak', 'Weak', 'Fair', 'Strong', 'Very strong']`.

**Bundle size**: `PasswordStrength` lives in its own module and `PasswordField` does not import it, so a consumer using only `PasswordField` never pulls in `PasswordStrength` or `scorePassword` — a scorer like zxcvbn (~400 kB) stays opt-in. This is `sideEffects: false` plus a single ESM entry point (`package.json`) doing the work a bundler needs: unused named exports tree-shake out. `dist/index.js` itself, as a single-entry bundle, still contains every export's source (grepping it for `PasswordStrength` finds it) — the tree-shaking happens in the _consumer's_ bundler, not in this package's own build.

Themeable under `EzPasswordStrength` (`defaultProps`, `styleOverrides` for `root` | `bar` | `label`), exported as `passwordStrengthClasses`.

## US fields

Small, US-specific conveniences on top of `TextField` and `Select`: `PhoneField`, `ZipField`, `StateSelect`, and the `AddressField` composite. For a non-US audience, build the equivalent with plain `TextField`/`Select`.

They share one rule: **the form value is the canonical data — bare digits, or a USPS abbreviation — and any template only decides how it is displayed.** So a zod schema stays a plain `z.string()` with no regex — the field itself owns completeness — and the value you submit, store and compare is always canonical, whatever the user typed or pasted.

### PhoneField

```tsx
const schema = z.object({ phone: z.string() })

<PhoneField name="phone" label="Phone" required />
// user types 5551234567 → shows "555-123-4567", submits { phone: '5551234567' }
```

`format` is a `#` template: every `#` is a digit slot, every other character is a separator inserted between them. It also sets the field's capacity — the number of `#`s is how many digits the input accepts and how many the built-in completeness rule requires:

```tsx
<PhoneField name="phone" label="Phone" format="(###) ###-####" />
```

Typing formats progressively (`555`, then `555-5`), so a separator only appears once a digit follows it. Pasting normalises: `(555) 555-5555`, `555.555.5555` and `+1 555 555 5555` all land as `5555555555` — a leading `1` on an eleven-digit paste is read as the country code and dropped. Anything that is not a digit is ignored, and extra digits past the template's capacity never make it in.

The caret stays with the digit being edited rather than jumping to the end: the field records how many digits sit to the left of the caret _before_ reformatting and restores that position afterwards. Backspace onto a separator deletes the digit that separator follows, so the key does what it looks like it should instead of silently doing nothing.

A value that is non-empty but incomplete fails on submit with `invalidMessage`, which defaults to `Enter a <n>-digit phone number` with `<n>` derived from the template. An empty value is left to `required` (so an optional phone field can simply be left blank), and the value is `''` rather than `undefined` when empty, so `required` still applies:

```tsx
<PhoneField name="phone" label="Phone" invalidMessage="We need all ten digits" />
```

`autoComplete` defaults to `'tel'`; pass a sectioned token when the form has more than one number:

```tsx
<PhoneField name="homePhone" label="Home" autoComplete="home tel" />
<PhoneField name="workPhone" label="Work" autoComplete="work tel" />
```

`format`, `invalidMessage` and `autoComplete` are all settable app-wide through `theme.components.EzPhoneField.defaultProps`; a prop on the element always wins. `PhoneField` adds no styled slot of its own — it renders a `TextField`, so MUI's own `MuiTextField` / `MuiOutlinedInput` style keys reach it unchanged.

### SsnField

The same template machinery, with the template fixed at `###-##-####` — an SSN has one canonical rendering, so there is no `format` prop to get it wrong with. The form value is the bare nine digits; `123-45-6789`, `123 45 6789` and `123456789` all paste to `'123456789'`.

```tsx
const schema = z.object({ ssn: z.string() })

<SsnField name="ssn" label="Social Security number" required />
```

**It is hidden by default.** While hidden the input is `type="password"`, so the browser masks the formatted text and the number survives a shoulder-surf; the toggle switches to `type="text"` to check an entry, and the reveal state is local to the field — it never reaches the form value and resets on unmount. `reveal={false}` removes the toggle entirely. Toggling keeps your place in the field: focus returns to the input and the caret goes back where it was (see PasswordField above, which shares the behaviour).

```tsx
<SsnField name="ssn" label="Social Security number" reveal={false} />
```

Masking hides the digits, not the shape: nine dots plus two separators is what `type="password"` renders. That is the same trade every masked field makes, and it is what lets the field be a plain `<input>` with real caret, paste and screen-reader behaviour rather than a custom widget.

A value that is non-empty but shorter than nine digits fails on submit with `invalidMessage`, default `Enter a 9-digit Social Security number`. An empty value is left to `required`, and the value is `''` rather than `undefined` when empty, so `required` still applies. The field validates _length_, not issuance — it does not reject the ranges the SSA never assigned (`000`/`666`/`9xx` area numbers, and so on); add a `validate` of your own if you need that.

`autoComplete` is pinned to `'off'` and is not a prop: there is no autofill token for an SSN, and inviting a browser to store one is the wrong default.

The toggle is a themeable `IconButton` under `EzSsnField` (`root`, `toggle`, exported as `ssnFieldClasses`); `slotProps.toggle` reaches it, `icons?: { show?, hide? }` swaps its icons, and `showLabel`/`hideLabel` set its accessible name (defaults `'Show Social Security number'` / `'Hide Social Security number'`). `invalidMessage`, `reveal`, `icons` and the labels are all settable app-wide through `theme.components.EzSsnField.defaultProps`; a prop on the element always wins.

### FeinField

`FeinField` is the same machinery on the IRS's own two-then-seven shape — the form value is the bare nine digits, `format` (default `'##-#######'`) decides only how they are drawn:

```tsx
const schema = z.object({ ein: z.string() })

<FeinField name="ein" label="EIN" required />
// user types 123456789 → shows "12-3456789", submits { ein: '123456789' }
```

Everything `PhoneField` does with typing, pasting, the caret and Backspace-onto-a-separator applies here unchanged: all three template fields share one `useTemplateField` hook. What differs is the defaults: `inputMode="numeric"` (there is no `type="tel"` keypad to inherit), `autoComplete="off"` — no autofill token exists for a tax ID, and a browser guessing one over a federal identifier is worse than none — and `invalidMessage`, which defaults to `Enter a 9-digit employer identification number`. `type` stays `text`, so a leading zero is never dropped.

`format`, `invalidMessage` and `autoComplete` are settable app-wide through `theme.components.EzFeinField.defaultProps`; a prop on the element always wins.

### Formatting stored digits for display

Because the stored value is bare digits, anywhere you display it _outside_ the field — a review step, a table, a confirmation email — has to apply the template itself. Format a stored phone for display with `formatTemplate(digits, PHONE_FORMAT)`, both exported, so the display matches the field's own and cannot drift from it if the default template changes:

```tsx
import { formatTemplate, PHONE_FORMAT } from 'ez-form'

;<ReadOnlyField
  name="phone"
  format={(v) => (typeof v === 'string' && v !== '' ? formatTemplate(v, PHONE_FORMAT) : '—')}
/>
// { phone: '5551234567' } → "555-123-4567"
```

`formatTemplate` takes any `#` template, not just the phone one, so a custom `format` formats the same way: `formatTemplate(digits, '(###) ###-####')`. The `Insurance` example's Review step uses exactly this.

### ZipField

Digits only, capped at 5 — anything else (letters, the ZIP+4 dash) is stripped as you type or paste. The form value is always the digit string (`'90210'`), never a formatted one. `inputMode="numeric"` brings up the numeric keypad on mobile; `autoComplete` defaults to `'postal-code'`. A non-empty value shorter than 5 digits fails a built-in rule (`invalidMessage?`, default `'Enter a 5-digit ZIP code'`), composed with any `validate` you pass the same way `required`/`pattern` are — a built-in key is never silently replaced by yours.

```tsx
<ZipField name="zip" label="ZIP code" required />
```

```ts
const schema = z.object({ zip: z.string().min(1) })
```

`invalidMessage` and `autoComplete` are settable app-wide through `theme.components.EzZipField.defaultProps`; a prop on the element always wins. `ZipField` adds no styled slot of its own — it renders a `TextField`, so MUI's own `MuiTextField` / `MuiOutlinedInput` style keys reach it unchanged.

### StateSelect

`Select` pre-loaded with the 50 states + DC (`US_STATES`, exported as an `Option[]`); pass `territories` to add PR, GU, VI, AS, MP (`US_TERRITORIES`). Values are USPS abbreviations (`'CA'`), labels are full names (`'California'`). `autoComplete` defaults to `'address-level1'`.

```tsx
<StateSelect name="state" label="State" required />
<StateSelect name="state" label="State or territory" territories />
```

`territories` and `autoComplete` are settable app-wide through `theme.components.EzStateSelect.defaultProps` — so "territories on everywhere" is one theme key, not a prop on every element; a prop on the element always wins. `StateSelect` adds no styled slot of its own — it renders a `Select`, so MUI's own `MuiTextField` / `MuiSelect` / `MuiMenuItem` style keys reach it unchanged.

### AddressField

The five US address parts as one bound composite under a nested object `name` — `street`, an optional `street2`, `city`, `state` (a `StateSelect`) and `zip` (a `ZipField`). Each part is the real field component, so a per-part error, `required`, `disabled` and focus-on-error behave exactly as they do when you write the five fields out by hand; the composite supplies the names, the autofill tokens, the labels and the layout.

```tsx
<AddressField name="shipping" legend="Shipping address" autoCompleteSection="shipping" required />
```

```ts
const schema = z.object({ shipping: addressSchema() })
```

`required` applies to street, city, state and ZIP — never `street2`, which is optional by definition. `autoCompleteSection` prefixes every token at once (`shipping street-address`, `shipping address-level2`, …), which is what lets a browser fill a shipping and a billing address on the same page separately. `legend` (with optional `description`) wraps the group in a `FormSection` fieldset; without one the parts sit in a plain container named by their own labels. `street2={false}` hides the second line — pair it with `addressSchema({ street2: false })` so the schema does not declare a key nothing writes.

Every label is a prop with a default (`streetLabel` `'Street address'`, `street2Label` `'Apartment, suite, etc.'`, `cityLabel` `'City'`, `stateLabel` `'State'`, `zipLabel` `'ZIP code'`), and `slotProps.street` / `.street2` / `.city` / `.state` / `.zip` reach the individual field components for anything else (per-part `helperText`, an extra rule, a `size`).

`addressSchema({ street2?, messages? })` returns the matching `z.object` so a form does not restate the five keys: all four required parts are non-empty strings with `'<Part> is required'` messages (override via `messages`), and `street2` is `z.string().optional()`. ZIP is only checked for presence — `ZipField`'s own 5-digit rule already covers the format, and duplicating it in zod would show two messages for one mistake. The messages have no trailing period, matching every other zod message in this codebase; the built-in `required` _rule_ messages do end with one (`'City is required.'`), so pass `messages` if a form surfaces both and you want them identical.

#### Address lookup

`lookup` turns the street part into an address search without changing the value shape or the other four parts:

```tsx
<AddressField name="shipping" legend="Shipping address" lookup={provider} />
```

`provider` is an `AddressLookupProvider` — `search(query, ctx)` returns `AddressSuggestion[]` (`{ id, label, secondary? }`) for the typed text, `resolve(suggestion, ctx)` turns a picked row into the address parts it knows, and an optional `attribution` node renders under the list (a "Powered by …" line, or the logo a provider's terms require). The field owns the plumbing: it waits `lookupDebounceMs` (300) after typing stops, searches from `lookupMinChars` (3) characters, aborts the older request when a newer one starts (`ctx.signal`), and hands every call of one entry the same opaque `ctx.session` token — created on the first keystroke, ended by the pick — so a per-session billed backend sees one session per address. Picking a row writes every part: the street to the provider's street (or the picked row's label when the provider has none), the other supplied parts to their values and the rest to `''` (so a unit number from an earlier pick never survives), marks them dirty, validates them, and announces `lookupFilledText` (`'Address filled'`) through a visually hidden live region. Text typed without a pick stays as the street (Enter, Tab or blur commits it — so under `mode="onChange"` a required error clears at commit rather than on the first character), and clearing the street stores `''`; manual entry always works. When `resolve` rejects, the street keeps the picked row's label, no other part is written, and the same live region announces `lookupFailedText` (`'Address lookup failed. Fill in the remaining fields manually.'`) — the empty city/state/ZIP are the sighted signal, so the sentence is the one for a screen reader; pass `slotProps.status.visuallyHidden={false}` to show it. A failed `search` offers no rows and is not announced (an empty list under an input the user is still typing in says it already). Both failures also warn once in dev. Create the provider once (module scope or `useMemo`), not inline per render. Under `lookup` the street is an `Autocomplete`, and `slotProps.street` is mapped onto it (rules, `label`, `helperText`, `onChange`, `autoComplete`, `slotProps.htmlInput` directly; the other TextField props via its `textFieldProps`, except `onBlur`, `inputRef` and the other nested `slotProps.*`, which are dropped). `useAddressLookup` is exported for a field built on the same seam.

Themeable under `EzAddressField` (`defaultProps`, `styleOverrides` for `root` | `street` | `street2` | `city` | `state` | `zip` | `attribution` | `status`), exported as `addressFieldClasses`. The root is a CSS grid with named areas (`street` / `street2` / `city state zip`, one column below `sm`); re-order or re-span any part by overriding `gridTemplateAreas` on `root`. `street` reaches the part whether it is the plain field or the lookup one; `attribution` is the line under the list and `status` the live region.

### Google Places

`googlePlaces(options)` is an address-lookup provider for `AddressField`'s `lookup` prop, over Google's Places API (New) REST endpoints — Autocomplete (New) for the suggestions as the user types the street line, then Place Details (New), field-masked to `addressComponents`, when they pick one. The pick fills street (`street_number` + `route`), `street2` (`subpremise`), city (`locality`, falling back to `sublocality_level_1` or `postal_town`), state (the `administrative_area_level_1` abbreviation) and ZIP (`postal_code`). It adds no dependency and holds no state; you pass the key.

```tsx
import { AddressField, googlePlaces } from 'ez-form'

;<AddressField name="shipping" lookup={googlePlaces({ apiKey })} />
```

The key travels in the `X-Goog-Api-Key` header of requests the user's browser makes, so it is visible to anyone who opens devtools. Treat it as a **browser key**: in the Google Cloud console restrict it to the Places API (New) and to your site's HTTP referrers (`https://example.com/*`), and keep any unrestricted or server-side key out of the bundle. The adapter never writes the key to an error message or a log.

Options: `regionCodes` (Google's `includedRegionCodes`, default `['us']`), `languageCode` (BCP-47, for both requests), `includedPrimaryTypes` (restrict the predictions, unset by default), `attribution` (a node rendered under the listbox) and `fetch` (injectable; tests and non-browser runtimes). The field owns the billing session: it creates a session token on the first keystroke, sends it with every search and the final details request, then discards it, which is what makes Google bill the whole exchange as one Autocomplete session rather than per request.

Google's Places policy requires attribution when results are shown without a Google map. The default `attribution` is the text "Powered by Google" in a plain block themeable under `EzGooglePlacesAttribution` (`styleOverrides.root`, class `googlePlacesAttributionClasses.root`); Google's current guideline asks for its logo image, which this package cannot ship, so supply the official asset either per provider (`googlePlaces({ apiKey, attribution: <img … /> })`) or once for the whole app through `theme.components.EzGooglePlacesAttribution.defaultProps.children`.

Storybook's **Fields/AddressField → WithGooglePlaces** story is the live one. Copy `.env.example` to `.env.local` at the repo root (both are gitignored), put a browser key restricted to the Places API (New) and to `http://localhost:6006/*` in `VITE_GOOGLE_PLACES_API_KEY`, and restart Storybook; the story hands the key to `googlePlaces({ apiKey })` and never renders or logs it. Without the variable the story falls back to the deterministic mock provider and says so in a note, which is how it runs in CI.

### Keyboards & autofill for these fields

Every default is listed in the [Mobile keyboards & autofill](#mobile-keyboards--autofill) table above, and each is overridable by your own `autoComplete` prop. Two details specific to these fields:

`inputMode="numeric"` on `ZipField` brings up the numeric keypad on mobile without changing the underlying `type` (still `text`, so a leading zero like `02134` is never dropped). `StateSelect`'s `autoComplete` reaches the hidden native `<input>` MUI's `Select` renders for autofill via `slotProps.htmlInput` — the same slot a plain `TextField` uses (MUI 9 has no `SelectProps`/native `inputProps` shortcut for this).

## Wrap your own control

`<BoundField>` binds a control ez-form does not wrap — a third-party widget, a plain DOM
element, one of your own components. It renders the same `FormControl` box and helper text
every field does; the control itself comes from a `render` prop, which receives everything
the binding knows about the field.

```tsx
import { BoundField } from 'ez-form'

function NicknameField({ name, label }: { name: string; label: string }) {
  return (
    <BoundField<string>
      name={name}
      label={label}
      componentName="NicknameField"
      rules={{ required: true }}
      helperText="What should we call you?"
      render={(bound) => (
        <>
          <label htmlFor={bound.controlId} id={bound.labelId}>
            {bound.displayLabel}
          </label>
          <input
            type="text"
            id={bound.controlId}
            name={bound.field.name}
            ref={bound.field.ref}
            value={bound.field.value ?? ''}
            disabled={bound.field.disabled}
            required={bound.required}
            onChange={(e) => bound.field.onChange(e.target.value)}
            onBlur={bound.field.onBlur}
            {...bound.nameA11y}
            {...bound.inputA11y}
          />
        </>
      )}
    />
  )
}
```

That component is now a field like any other: it validates through the form's schema and
`rules`, disables under `<Form disabled>`, shows its error as helper text in a live region,
takes focus after a failed submit, appears in `<FormErrorSummary>`, and follows the form's
`requiredIndicator`.

**`labelPlacement` works, with one thing to know about `'start'`.** It lays the field out
as a two-column grid, and the label column takes a plain **direct-child `<label>`** — which
is exactly what the example above renders — so a `labelAs="none"` field lines up with its
neighbours with nothing extra from you.

The exception is a control that is _self-labelled_: its label lives inside the control (a
button whose text is the label, the way `FileField`'s picker works), so there is no separate
label element to put in column 1, and leaving the grid in place would park the control beside
an empty label column. Say so with `fieldLayoutClasses.selfLabelled`, and the field opts out
of the grid and sits flush left:

```tsx
import { BoundField, fieldLayoutClasses } from 'ez-form'
;<BoundField
  name="avatar"
  label="Avatar"
  className={fieldLayoutClasses.selfLabelled}
  render={(bound) => <MyButtonThatIsItsOwnLabel {...bound.inputA11y} ref={bound.field.ref} />}
/>
```

`labelAs="control"` sets that class for you — MUI's `FormControlLabel` puts the label inside
the click target — so `Checkbox` and `Switch` already behave this way. `'top'` is unaffected
either way, since it emits no rules at all.

`labelAs="control"` also reads `controlLabelProps`, which is forwarded to that
`FormControlLabel` — the route `Checkbox` and `Switch` use to expose MUI's own
`labelPlacement` (`'end' | 'start' | 'top' | 'bottom'`). It is ignored under the other two
modes, with a dev-mode warning naming it.

### Four things `render` must forward

The rest of `bound` is optional. These are not, and each one fails silently:

| Forward                                                            | Or else                                                                                                                                                                                                                                                                                                   |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `bound.field.ref`                                                  | Nothing focuses the control after a failed submit, and `<FormErrorSummary>`'s link to this field goes nowhere.                                                                                                                                                                                            |
| `bound.inputA11y`                                                  | The error renders but no screen reader ever associates it with the control (`aria-describedby`, `aria-invalid`).                                                                                                                                                                                          |
| `bound.nameA11y`                                                   | A field named only by `aria-label` / `aria-labelledby` leaves the name on the `FormControl` wrapper — a `<div>` — while the real control stays anonymous. axe reports clean.                                                                                                                              |
| `bound.displayLabel` **or** `bound.nameA11y` — one of them, always | Under the default `labelAs="none"` the frame renders **no label element**, so a `label` you passed to `<BoundField>` names nothing until `render` puts it somewhere. The control ends up anonymous, and the dev warning cannot tell you: it checks the props you passed, sees a `label`, and stays quiet. |

The first three go on **the element that carries the role** — the `<input>`, the combobox,
the `role="group"` — not on a wrapper around it. The third is the one that has actually
shipped broken here: 15 of 17 fields in this library were once named that way and every test
passed, which is why `describeFieldContract` now asserts accessible _names_ rather than
attributes.

The fourth is the trap specific to `labelAs="none"`, and it is worth restating: **under
`'none'`, naming the control is yours.** Render `bound.displayLabel` in a `<label
htmlFor={bound.controlId}>` (with `id={bound.labelId}`) and give the control
`id={bound.controlId}`; or, for a field with no visible label, spread `bound.nameA11y` on
the control. Passing `label` to `<BoundField>` and forgetting to render it is the one way to
get an unnamed field that no warning catches — every other path either renders the label
itself or has no label to render, which is what makes the warning reliable there.

`bound.inputA11y`'s `aria-describedby` is already **merged**: if you pass an
`aria-describedby` of your own to `<BoundField>`, it arrives joined with the helper text's
id rather than replaced by it, because an accessible description is a list.

### `labelAs`: who renders the label

| `labelAs`          | The frame renders                                                                                      | Use it for                                        | Under `start`                                                                                                                 |
| ------------------ | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `'none'` (default) | No label element. `bound.displayLabel`, `labelId`, `controlId` and `labelRequired` are yours to place. | A control that owns its own label — most of them. | In the grid; a direct-child `<label>` takes column 1. Add `fieldLayoutClasses.selfLabelled` if your control is its own label. |
| `'control'`        | MUI `FormControlLabel`: a `<label>` wrapping your control, label text beside it.                       | A single checkbox- or switch-shaped input.        | Opts out of the grid — the class is set for you.                                                                              |
| `'legend'`         | `<fieldset>` + `<legend>`. Point your group at `bound.labelId` with `aria-labelledby`.                 | A group of controls under one name.               | In the grid; the legend floats into column 1.                                                                                 |

`'none'` is the default because a control the frame knows nothing about is one it cannot
choose label markup for. Under it, pair your `<label htmlFor={bound.controlId}>` with
`id={bound.controlId}` on the control — the two ids come from one place so they cannot drift.
Read `bound.displayLabel` rather than your own `label` prop, so `requiredIndicator="optional"`
appends its suffix, and `bound.labelRequired` so the asterisk follows the form's setting.

`bound.labelId` is `undefined` when there is no label. Keep it that way — an
`aria-labelledby` pointing at an empty element outranks `aria-label` in the accessible-name
algorithm and leaves the control with no name at all.

### Typing

`<BoundField<TValue>>` flows the field's value type through to `bound.field.value`
(`TValue | undefined` — a form with no `defaultValues` entry renders `undefined`) and to
`bound.field.onChange`, which then rejects a value of the wrong shape. State it explicitly, or
let it infer from `rules`.

### `componentName`

Optional, `'BoundField'` by default. Set it to your own component's name and the
"must be rendered inside `<Form>`" error and the [developer warnings](#developer-warnings)
name something a consumer can find:

```
ez-form: <NicknameField name="nickname"> has no accessible name.
```

## Developer warnings

Three mistakes leave a form that renders and submits perfectly while quietly failing the
people using it. There is nothing to throw on and nothing a type can catch, so ez-form
writes them to the console in development:

| Warning                                                          | Fires when                                                                                                            | Why it matters                                                                                        |
| ---------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `<Field name="…"> has no accessible name`                        | a field mounts with no `label`, `aria-label` or `aria-labelledby`                                                     | a screen-reader user hears an unlabelled edit box; axe reports it, but only if you run axe            |
| `<Field name="…"> has duplicate option values`                   | `Select`, `RadioGroup`, `Autocomplete`, `ToggleButtonGroup` or `CheckboxGroup` gets two options with the same `value` | the duplicates collapse: two radios look checked at once, and a `Select` shows the wrong label        |
| `<Wizard> step "…" lists field(s) … that the form does not know` | a step's `fields` names something absent from the schema and the values                                               | `trigger` on an unknown name returns valid, so **Next** advances past the field you meant to validate |

Each fires **once per mistake** (keyed on the component and field name), so a field
re-rendering on every keystroke reports once, while two different fields with the same
problem both get reported.

The last one is deliberately narrow: it asks whether the form _knows_ the name, not whether
the field is mounted right now. Listing an unmounted field is normal and correct — that is
exactly how [conditional fields](#conditional-fields) validate, and how a step naming an
empty `FieldArray` validates the array-level schema. Only a name the form has never heard of
— a typo, or a field renamed on one side only — warns.

### Silencing them

Fix the cause; there is no mute. A field genuinely named somewhere ez-form cannot see (a
label rendered by a wrapper, say) should say so with `aria-labelledby`, which is both the fix
and the thing that makes the warning stop.

### They cost nothing in production

Every warning sits behind a module-level `const isDev = process.env.NODE_ENV !== 'production'`.
Bundlers substitute that expression before dead-code elimination, so a production build drops
the call sites _and_ the message strings — no runtime check, no bytes. This is the same
mechanism React uses for its own development warnings, and it requires nothing of you beyond
building for production the way you already do.

## Lint rule

The developer warnings above fire at runtime, when the form renders. One mistake is worth
catching earlier than that, because it is a choice of component rather than a bug: reaching
for `<TextField type="email">` when `<EmailField>` exists.

That cannot be a type. `EmailField`, `PhoneField`, `SsnField` and `PasswordField` are all
built _on_ `TextField` and pass `type` to it themselves, so a `TextFieldProps` that forbade
`type="email"` would break the very components you are being pointed at. Telling the two
apart means knowing who wrote the JSX, which a linter can see and a type cannot.

So ez-form ships one ESLint rule, `prefer-specific-field`, as a subpath export:

```js
// eslint.config.js
import ezForm from 'ez-form/eslint-plugin'

export default [
  // ...your other config
  ...ezForm.configs.recommended,
]
```

Or enable the rule yourself, if you want it as a warning rather than an error:

```js
import ezForm from 'ez-form/eslint-plugin'

export default [
  {
    plugins: { 'ez-form': ezForm },
    rules: { 'ez-form/prefer-specific-field': 'warn' },
  },
]
```

It is AST-only: no `parserOptions.project`, no type information, no measurable lint cost. It
adds no dependency either — the rule imports nothing at runtime.

### What it reports

Four `type` values have a dedicated field, and the message names what that field actually
adds rather than just asserting a preference:

| You wrote                     | It suggests     | Because that field adds                                                                                                                                        |
| ----------------------------- | --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `<TextField type="password">` | `PasswordField` | a reveal toggle that keeps the caret, the right `autoComplete` token for sign-in vs. new-password, and an optional strength meter                              |
| `<TextField type="number">`   | `NumberField`   | a real numeric value instead of a string, `min`/`max` as both stepper bound and rule, and paste handling — a raw `<input type="number">` reads `'1,5'` as `15` |
| `<TextField type="email">`    | `EmailField`    | a format rule with its own message, and trim/lower-case on blur (`TextField` already gives you `inputMode` and `autoComplete` here)                            |
| `<TextField type="tel">`      | `PhoneField`    | display formatting over a digits-only value, caret restoration, and a completeness rule (again, `inputMode`/`autoComplete` are already yours)                  |

The bottom two are deliberately softer than the top two. `TextField` already derives the
mobile keyboard and the autofill token from `type="email"` and `type="tel"`, so those
messages claim only what is genuinely still missing. A rule that oversells its weak cases is
a rule people turn off.

**It stays quiet** for `type="url"` and `type="search"` — `TextField` handles both correctly
and there is no dedicated field to move to — and for any `type` it cannot read statically
(`type={kind}`, or a `{...props}` spread that might carry one). It also only fires on a
`TextField` imported from `ez-form`, so MUI's `TextField` in the same file is left alone.

## Develop

```bash
pnpm install
pnpm storybook   # http://localhost:6006 (with the a11y addon panel)
pnpm test        # vitest + Testing Library, plus jest-axe accessibility checks per component
pnpm build
```

### Field stories and the form decorator

A field story does not render its own `<Form>`. `.storybook/preview.tsx` has a decorator
that reads `parameters.form` — `{ schema, defaultValues }` — and wraps the story in a
`<Form>` with a submit button, so the story is just the field. The meta sets it once; a
story overrides only what differs, typed by `FormParameters`:

```tsx
const meta = {
  component: PercentField,
  parameters: { form: { schema, defaultValues: { rate: 12.5 } } } satisfies FormParameters,
} satisfies Meta<typeof PercentField>

export const Required: Story = {
  parameters: { form: { defaultValues: { rate: null } } } satisfies FormParameters, // schema inherited
}
```

The merge is [Storybook's own parameter inheritance](https://storybook.js.org/docs/api/parameters#parameter-inheritance):
project → meta → story, plain objects deep-merged, everything else replaced by the more
specific level. For `form` that means a story's `schema` (a zod instance) **replaces** the
meta's, while its `defaultValues` (a plain object) are **deep-merged** — `{ rate: null }`
changes `rate` and keeps the other meta defaults; a key cannot be unset from a story. A
story with a different-shaped schema therefore still carries the meta's old defaults, which
a non-strict `z.object` strips on submit. `FormParameters` extends Storybook's `Parameters`,
so `docs.description.story` and `layout` sit next to `form` under one `satisfies`. A `form`
without a `schema` after the merge fails the story with an explicit error.
`src/fields/PercentField/PercentField.stories.test.tsx` pins the contract.

### The gates

CI runs these in order, and all seven have to be green. Every one of them fails on a
_warning_, not just an error — a warning nobody has to act on is one nobody reads.

| Command                 | What fails it                                                             |
| ----------------------- | ------------------------------------------------------------------------- |
| `pnpm lint`             | `eslint . --max-warnings 0` — any finding at all                          |
| `pnpm typecheck`        | `tsc --noEmit` under `strict` plus the `noUnused*` / `noImplicit*` family |
| `pnpm test`             | a failing test, **or** unexpected `console.error` / `console.warn` output |
| `pnpm test:scripts`     | the guardrail script's own `node --test` suite                            |
| `pnpm check:guardrails` | a styling-rule violation, or an exported component with no README row     |
| `pnpm build`            | a rollup build warning (unresolved import, circular dependency, …)        |
| `pnpm build-storybook`  | the same, for the Storybook bundle                                        |

**ESLint** is a flat config (`eslint.config.js`) running typescript-eslint's
`recommendedTypeChecked` and `stylisticTypeChecked`, react-hooks (rules-of-hooks and
exhaustive-deps both errors), jsx-a11y, and eslint-plugin-storybook, with
eslint-config-prettier last. Every rule that is off, and every inline `eslint-disable`, has
its reason written next to it.

#### Two TypeScripts, on purpose

Typed linting needs the classic TypeScript compiler API, which the project's own compiler —
TypeScript 7, the native build — does not ship: its package exposes a `tsc` binary and nothing
requirable, and typescript-eslint refuses to start against it. Following [typescript-eslint's
own side-by-side setup](https://github.com/typescript-eslint/typescript-eslint/blob/main/pnpm-workspace.yaml),
`package.json` aliases both names:

```jsonc
"@typescript/native": "npm:typescript@^7.0.2",      // the compiler `tsc` runs
"typescript": "npm:@typescript/typescript6@6.0.2"   // the API tools import
```

So **anything that shells out to `tsc` runs TypeScript 7** — `pnpm typecheck`, and the
`tsconfig.build.json` declaration emit — while **anything that `import`s `typescript` for its
compiler API gets the TS 6 package**. Today that second group is ESLint's typed rules,
Storybook's react-docgen prop extractor, and the `vite-plugin-dts` rollup step (its API
Extractor pass reports "the target project appears to use TypeScript 6.0.2"). None of it
changes what ships: `dist/*.d.ts` is generated by TS 7's `tsc` before API Extractor bundles it.

The practical consequence is that the linter and the compiler occasionally disagree — usually
about whether a type assertion is necessary. **The compiler wins**: the assertion stays and the
lint rule is disabled on that line with a comment saying which two versions disagreed. Expect
these to disappear when typescript-eslint supports TS 7 ([typescript-eslint#10940](https://github.com/typescript-eslint/typescript-eslint/issues/10940)).

**React StrictMode** is on in both the test suite (`configure({ reactStrictMode: true })` in
`src/test/setup.ts`) and Storybook (`framework.options.strictMode`), so every effect, ref
callback and state updater double-invokes exactly as it will in a consumer's StrictMode app.

**Console output fails tests.** Vitest hides `console.warn` from passing tests, so a React
`act()` warning could otherwise sit in the suite indefinitely. `src/test/setup.ts` installs a
guard that fails any test producing unexpected output; a test that _means_ to log — the
`devWarn` suite, for one — opts in per message:

```ts
import { expectConsole } from './test/expectConsole'

expectConsole('warn', /has no accessible name/)
```

`pnpm check:guardrails` enforces rule 2 of `docs/PHILOSOPHY.md`: it scans `src/**/*.tsx`
(excluding stories, tests, `src/examples/**`, `src/test/**`) for `sx=`, `disableRipple`/
`focusRipple`, and literal `variant=`/`size=`/`color=` JSX attributes (a prop-driven value
like `variant={variant}` is fine; a string literal like `variant="contained"` is not), and
checks that every component exported from `src/index.ts` has a row in this README's
Components table. A line that's a deliberate, tracked exception can end with
`// guardrail: allow <reason, ideally an issue number>` to skip it; the script reports how
many lines were allow-listed so they stay visible. It runs in CI (`pnpm test:scripts` runs
its own `node --test` suite first).

MIT

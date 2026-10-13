---
name: ontoui
description: >
  Build accessible UI components, forms, layouts, and pages using OntoUI (`@ontoui/react`).
  Use this skill whenever a user asks to build, create, or generate any UI — buttons,
  forms, modals, data grids, dashboards, cards, or any layout — using OntoUI components.
  Trigger on any OntoUI building or usage request, however casually phrased —
  "add a modal", "make a form with validation", "show me a data table", "build a login page with OntoUI".
argument-hint: Describe the UI to build (component, page, form, layout, etc.)
---

# OntoUI

You are building UI with `@ontoui/react`. Your job is to produce clean, accessible, correct code that fits the project — not generic boilerplate. Read the project context first, then build.

## Step 1 — Read the project context

Check `package.json` if available. Look for:

- Framework: Next.js App Router, Pages Router, Vite, plain React
- TypeScript or JavaScript
- Package manager (from lockfile: `pnpm-lock.yaml`, `yarn.lock`, `bun.lock`, `package-lock.json`)

Glance at existing components if the user references them so your code matches the conventions already in use (naming, file structure, import style).

If requirements are vague or the component is complex enough that choices matter (layout direction, data shape, number of variants), ask before building rather than generating something that needs to be thrown away.

## Step 2 — Import from `@ontoui/react`

All components are imported from a single entry point:

```tsx
import { Button, TextField, Modal, Tooltip } from '@ontoui/react';
import '@ontoui/react/styles'; // import once at the app root
```

### Available components

| Component          | Sub-parts                                                                                                                                                                                                    |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `Button`           | `Button.Root`, `Button.Icon`, `Button.Text`                                                                                                                                                                  |
| `Accordion`        | `Accordion.Root`, `Accordion.Item`, `Accordion.Header`, `Accordion.Trigger`, `Accordion.Panel`                                                                                                               |
| `Callout`          | `Callout.Root`, `Callout.Icon`, `Callout.Text`                                                                                                                                                               |
| `Checkbox`         | `Checkbox.Root`, `Checkbox.Indicator`                                                                                                                                                                        |
| `Chip`             | `Chip.Root`, `Chip.Icon`, `Chip.Text`                                                                                                                                                                        |
| `Progress`         | `Progress.Root`, `Progress.Label`, `Progress.Value`, `Progress.Track`, `Progress.Indicator`                                                                                                                  |
| `CircularProgress` | `CircularProgress.Root`, `CircularProgress.Track`, `CircularProgress.Indicator`, `CircularProgress.Value`, `CircularProgress.Label`                                                                          |
| `Kbd`              | `Kbd.Root`, `Kbd.Key`, `Kbd.Separator`                                                                                                                                                                       |
| `NumberField`      | `NumberField.Root`, `NumberField.Label`, `NumberField.Group`, `NumberField.Decrement`, `NumberField.Input`, `NumberField.Increment`, `NumberField.ScrubArea`, `NumberField.Description`, `NumberField.Error` |
| `OtpField`         | `OtpField.Root`, `OtpField.Input`, `OtpField.Separator`                                                                                                                                                      |
| `Modal`            | `Modal.Root`, `Modal.Trigger`, `Modal.Panel`, `Modal.Title`, `Modal.Description`, `Modal.Close`                                                                                                              |
| `Popover`          | `Popover.Root`, `Popover.Trigger`, `Popover.Popup`, `Popover.Title`, `Popover.Description`, `Popover.Close`                                                                                                  |
| `TextField`        | `TextField.Root`, `TextField.Label`, `TextField.Control`, `TextField.AnimatedPlaceholder`, `TextField.Description`, `TextField.Error`                                                                        |
| `Tooltip`          | `Tooltip.Provider`, `Tooltip.Root`, `Tooltip.Trigger`, `Tooltip.Content`                                                                                                                                     |
| `Truncate`         | `Truncate.Root`, `Truncate.Text`, `Truncate.Tooltip`, `Truncate.Toggle`                                                                                                                                      |
| `Tabs`             | `Tabs.Root`, `Tabs.List`, `Tabs.Tab`, `Tabs.Panel`                                                                                                                                                           |
| `Radio`            | —                                                                                                                                                                                                            |
| `RadioGroup`       | —                                                                                                                                                                                                            |
| `Select`           | `Select.Root`, `Select.Label`, `Select.Trigger`, `Select.Popup`, `Select.Item`, `Select.Group`, `Select.GroupLabel`, `Select.Separator`                                                                      |
| `Slider`           | `Slider.Root`, `Slider.Label`, `Slider.Value`, `Slider.Control`, `Slider.Track`, `Slider.Indicator`, `Slider.Thumb`                                                                                          |
| `Sparkline`        | `Sparkline.Root`, `Sparkline.Line`, `Sparkline.Area`, `Sparkline.Bars`, `Sparkline.Spot`, `Sparkline.Baseline`                                                                                               |
| `Switch`           | `Switch.Root`, `Switch.Thumb`                                                                                                                                                                                |
| `Toast`            | `Toast.Provider`, `Toast.Viewport`, `Toast.Root`, `Toast.Content`, `Toast.Title`, `Toast.Description`, `Toast.Close`, `Toast.Action`                                                                         |
| `XYPad`            | `XYPad.Root`, `XYPad.Label`, `XYPad.Value`, `XYPad.Control`, `XYPad.Grid`, `XYPad.Thumb`                                                                                                                     |
| `DataGrid`         | —                                                                                                                                                                                                            |

## Step 3 — Component usage patterns

### Button

```tsx
import { Button } from '@ontoui/react';

// variant: 'primary' (default) | 'secondary'
<Button.Root variant="primary" onClick={handleClick}>
  <Button.Text>Submit</Button.Text>
</Button.Root>

<Button.Root variant="secondary" disabled>
  <Button.Text>Cancel</Button.Text>
</Button.Root>

// With an icon — put Button.Icon before or after Button.Text
<Button.Root onClick={handleAdd}>
  <Button.Icon>
    <PlusIcon />
  </Button.Icon>
  <Button.Text>Add Item</Button.Text>
</Button.Root>

// Icon only — renders square, needs aria-label
<Button.Root aria-label="Close">
  <Button.Icon>
    <XIcon />
  </Button.Icon>
</Button.Root>

// type: 'button' (default) | 'submit' | 'reset'
<Button.Root type="submit">
  <Button.Text>Save</Button.Text>
</Button.Root>
```

`type` defaults to `'button'`, so a button inside a form does nothing until it is asked to —
mark the submitting button `type="submit"` explicitly.

### TextField (form field with label + validation)

```tsx
import { TextField } from '@ontoui/react';

<TextField.Root name="email" disabled={false} invalid={hasError}>
  <TextField.Label>Email address</TextField.Label>
  <TextField.Control
    type="email"
    placeholder="you@example.com"
    value={value}
    onChange={(e) => setValue(e.target.value)}
  />
  <TextField.Description>We'll never share your email.</TextField.Description>
  <TextField.Error>{errorMessage}</TextField.Error>
</TextField.Root>;
```

Use `TextField.Root invalid` to surface validation errors — it wires `TextField.Error` automatically.

`TextField.AnimatedPlaceholder` types example queries through the control's placeholder, so a
search field shows what it can be asked for instead of waiting to be guessed at. Reach for it on
search and command inputs, not on ordinary form fields, where a plain placeholder is clearer.

```tsx
<TextField.Root>
  <TextField.Label>Search reports</TextField.Label>
  <TextField.AnimatedPlaceholder
    prefix="Try: "
    examples={['March expenses', 'unpaid invoices', 'headcount by team']}
  />
  <TextField.Control placeholder="Search reports" />
</TextField.Root>
```

It renders nothing of its own and can sit anywhere inside `TextField.Root`. `prefix` stays put
while the examples are retyped behind it; `typeSpeed`, `deleteSpeed` and `holdDuration` set the
pace, and `caret={false}` drops the cursor character.

Still pass `placeholder` to `TextField.Control` — it is what shows on the server, before
hydration, and with scripting off. The animation stops on a whole example while the field is
focused or holds a value, and never starts under `prefers-reduced-motion: reduce`. A moving
placeholder can end up as the field's accessible name, so it always needs a `TextField.Label`
or an `aria-label` on the control.

### NumberField (numeric input with steppers)

Reach for this over `TextField type="number"` whenever the value is a quantity. The value is a
number, not a string: `value`, `defaultValue`, and `onValueChange` deal in `number | null`, where
`null` means the input is empty.

```tsx
import { NumberField } from '@ontoui/react';

<NumberField.Root name="quantity" defaultValue={1} min={0} max={99}>
  <NumberField.Label>Quantity</NumberField.Label>
  <NumberField.Group>
    <NumberField.Decrement />
    <NumberField.Input />
    <NumberField.Increment />
  </NumberField.Group>
  <NumberField.Description>Up to 99 per order.</NumberField.Description>
</NumberField.Root>;
```

`NumberField.Group` must wrap the input and its buttons — it draws the shared border. `min` and
`max` clamp the value and disable the buttons at each bound; `step` sets the increment, with
`largeStep` (shift) and `smallStep` (meta) as modifiers.

`format` takes `Intl.NumberFormat` options, so the field can display a currency or percentage while
still reporting a plain number. A percent field stores the fraction — `0.15` displays as `15%`.

```tsx
<NumberField.Root defaultValue={1200} step={100} format={{ style: 'currency', currency: 'USD' }}>
```

Validation matches `TextField`: set `invalid` on `NumberField.Root` to reveal `NumberField.Error`,
or drop `invalid` and give each error a `match` (`"rangeUnderflow"`, `"valueMissing"`, …) to react
to native validation instead.

`NumberField.ScrubArea` wraps the label to make dragging it change the value. It is pointer-only, so
it must never be the only way to reach a value — the arrow keys and the stepper buttons stay
available. The buttons are already labelled "Increase"/"Decrease" and sit outside the tab order;
pass `aria-label` only to translate them, and children only to replace the default icons.

### Checkbox

```tsx
import { Checkbox } from '@ontoui/react';

<Checkbox.Root name="terms" defaultChecked={false}>
  <Checkbox.Indicator />
  Accept terms
</Checkbox.Root>;
```

### Chip

A presentational label — no state, no interaction. Wrap it in a `Button.Root` or a link to make it actionable.

```tsx
import { Chip } from '@ontoui/react';

// color: 'default' (default) | 'success' | 'warning' | 'danger'
<Chip.Root>
  <Chip.Text>Design</Chip.Text>
</Chip.Root>

<Chip.Root color="success">
  <Chip.Icon>
    <CheckIcon />
  </Chip.Icon>
  <Chip.Text>Published</Chip.Text>
</Chip.Root>
```

`Chip.Icon` goes before or after `Chip.Text` for a leading or trailing icon. Color is decorative —
keep the meaning in the label text, not the tint alone. Omitting `Chip.Text` renders an icon-only
square, which needs `role="img"` and `aria-label` on `Chip.Root` to have any accessible name.

### Callout

A tinted block of text — no state, no interaction. `Callout.Text` is a `<div>`, so the message can
hold block content such as a paragraph or a list.

```tsx
import { Callout } from '@ontoui/react';

// color: 'default' (default) | 'success' | 'warning' | 'danger'
<Callout.Root>
  <Callout.Text>Changes are saved automatically as you type.</Callout.Text>
</Callout.Root>

<Callout.Root color="danger" role="alert">
  <Callout.Icon>
    <AlertIcon />
  </Callout.Icon>
  <Callout.Text>
    <strong>Payment failed.</strong> Update your card to keep your subscription.
  </Callout.Text>
</Callout.Root>
```

`Callout.Icon` goes before or after `Callout.Text` for a leading or trailing icon. Color and icon are
both decorative — keep the meaning in the text. A callout that appears in response to a user action
needs `role="status"` (informational) or `role="alert"` (interrupting error) to be announced.

### Kbd

Displays a keyboard shortcut — no state, no interaction, and it does not bind the shortcut. Register
the key handler yourself and keep the two in sync.

Give modifiers and symbol keys a `name`: it picks the right glyph for the user's platform (`⌘` on
Apple, `Ctrl` elsewhere) and announces the key in words, which bare glyphs do not do. Pass plain
letters and digits as children.

```tsx
import { Kbd } from '@ontoui/react';

<Kbd.Root>
  <Kbd.Key name="mod" />
  <Kbd.Key>K</Kbd.Key>
</Kbd.Root>

// Kbd.Separator joins keys: '+' for simultaneous, 'then' for a sequence
<Kbd.Root>
  <Kbd.Key>G</Kbd.Key>
  <Kbd.Separator>then</Kbd.Separator>
  <Kbd.Key>P</Kbd.Key>
</Kbd.Root>
```

Available names: `mod` (Command/Control — prefer this for the primary modifier), `meta`, `ctrl`,
`alt`, `shift`, `enter`, `escape`, `tab`, `backspace`, `delete`, `space`, `up`, `down`, `left`,
`right`, `home`, `end`, `pageup`, `pagedown`.

Platform detection is automatic. Override it with `platform="apple" | "other"` on `Kbd.Root` — for a
cheatsheet documenting both, or to render the right legends on the server when a User-Agent hint
already tells you the platform. Under SSR with the default `auto`, the HTML carries the portable
legends and switches after hydration; this is hydration-safe but flashes once on Apple devices.

A symbol key with no `name` (`` ` ``, `/`, `[`) has no spoken form, and `<kbd>` carries no ARIA role,
so `aria-label` on it alone is unreliable. Name the whole shortcut instead:

```tsx
<Kbd.Root role="img" aria-label="Control Backtick">
  <Kbd.Key name="ctrl" />
  <Kbd.Key>`</Kbd.Key>
</Kbd.Root>
```

### Progress

A bar reporting how far along a task is. `value` is required and always controlled — the bar
reflects work happening elsewhere, so it holds no state of its own.

```tsx
import { Progress } from '@ontoui/react';

// color: 'default' (default) | 'success' | 'warning' | 'danger'
<Progress.Root value={40}>
  <Progress.Label>Uploading</Progress.Label>
  <Progress.Value />
  <Progress.Track>
    <Progress.Indicator />
  </Progress.Track>
</Progress.Root>;
```

`value={null}` is indeterminate — the task is running and its progress is unknown. The bar drops
`aria-valuenow` and animates a sliver along the track; `Progress.Value` renders nothing. Start
there and switch to a number the moment there is something to measure.

`Progress.Value` reads the value as a percentage of 100 by default, which is the scale `min` and
`max` already default to. On any other scale pass `format` (`Intl.NumberFormat` options) — it
drives the announced value too — and word it with the render prop.

```tsx
<Progress.Root value={3} max={5} format={{ style: 'decimal' }}>
  <Progress.Label>Onboarding</Progress.Label>
  <Progress.Value>{(formatted) => `${formatted} of 5 steps`}</Progress.Value>
  <Progress.Track>
    <Progress.Indicator />
  </Progress.Track>
</Progress.Root>
```

Always name the bar: `Progress.Label`, or `aria-label` on `Progress.Root`. Color is decorative,
so keep the meaning in the label. Use `Progress` only for work in flight — a progress bar tells
assistive technology something is underway, so a static fraction (a rating, a score) needs a
plain element instead.

### CircularProgress

The same status as `Progress`, drawn as a ring, for places with no line to span — a button, a
card corner, an avatar overlay. `value`, `min`, `max` and `format` behave exactly as they do
there.

```tsx
import { CircularProgress } from '@ontoui/react';

<CircularProgress.Root value={40} size={72}>
  <CircularProgress.Track>
    <CircularProgress.Indicator />
  </CircularProgress.Track>
  <CircularProgress.Value />
  <CircularProgress.Label>Uploading</CircularProgress.Label>
</CircularProgress.Root>;
```

The value is centred over the ring and the label sits beneath it whatever order the parts are
written in. Keep the centred text short — it is bounded by the ring's inner diameter.

`size` (diameter, default `40`) and `thickness` (stroke, default `4`) are props rather than CSS
because the SVG geometry is computed from them. Below about 40px there is no room for text
inside the ring, so label small ones instead.

`value={null}` is the library's spinner: a root with a track and an indicator, and nothing else.
Give it an `aria-label` when there is no `CircularProgress.Label`, or it announces only that
something unnamed is in progress.

```tsx
<CircularProgress.Root value={null} size={20} thickness={3} aria-label="Loading">
  <CircularProgress.Track>
    <CircularProgress.Indicator />
  </CircularProgress.Track>
</CircularProgress.Root>
```

### Modal

```tsx
import { Modal } from '@ontoui/react';

<Modal.Root>
  <Modal.Trigger>Open modal</Modal.Trigger>
  <Modal.Panel>
    <Modal.Title>Confirm action</Modal.Title>
    <Modal.Description>Are you sure you want to continue?</Modal.Description>
    <Modal.Close>Cancel</Modal.Close>
  </Modal.Panel>
</Modal.Root>;
```

Use `Modal.Root open` + `onOpenChange` for controlled usage.

### Popover

```tsx
import { Popover } from '@ontoui/react';

// side: 'top' | 'right' | 'bottom' (default) | 'left'
<Popover.Root>
  <Popover.Trigger>More info</Popover.Trigger>
  <Popover.Popup side="bottom">
    <Popover.Title>Details</Popover.Title>
    <Popover.Description>Additional context here.</Popover.Description>
    <Popover.Close>Dismiss</Popover.Close>
  </Popover.Popup>
</Popover.Root>;
```

### Tooltip

`Tooltip.Provider` must wrap any group of tooltips (or the entire app). `delay` (ms) controls show latency.

```tsx
import { Tooltip } from '@ontoui/react';

<Tooltip.Provider delay={300}>
  <Tooltip.Root>
    <Tooltip.Trigger>Hover me</Tooltip.Trigger>
    <Tooltip.Content side="top">Helpful description</Tooltip.Content>
  </Tooltip.Root>
</Tooltip.Provider>;
```

### Truncate

Text shortened to fit its container. Use it for any value that can outgrow its column — a
path, a title, a paragraph — instead of `text-overflow: ellipsis` by hand.

`Truncate.Text` takes a string. Nothing is cut out of the DOM: all three modes leave the whole
value in the markup and let CSS hide the overflow, so screen readers read it and a copy takes
it whole.

```tsx
import { Truncate } from '@ontoui/react';

// mode: 'end' (default) | 'middle' | 'clamp'
<Truncate.Root>
  <Truncate.Text>{filePath}</Truncate.Text>
  <Truncate.Tooltip />
</Truncate.Root>;
```

`Truncate.Tooltip` shows the full text, and only while text is actually hidden — a value that
fits gets no tooltip and no tab stop. Omit the part to turn both off. It joins the delay group
of an enclosing `Tooltip.Provider`, so a table of truncated cells opens without re-waiting the
delay on every cell.

`mode="middle"` keeps both ends and drops the middle — paths, hashes, addresses. `endChars` is
how many characters are pinned to the tail; the head takes the leftover width. Set it to the
length of the part that must survive.

```tsx
<Truncate.Root mode="middle" endChars={6}>
  <Truncate.Text>0x71C7656EC7ab88b098defB751B7401B5f6d8976F</Truncate.Text>
  <Truncate.Tooltip />
</Truncate.Root>
```

`mode="clamp"` shows `lines` lines of prose, with `Truncate.Toggle` revealing the rest. The
toggle renders only when the text actually overflows, carries `aria-expanded`, and takes a
render prop for its label. Use a toggle or a tooltip, not both.

```tsx
<Truncate.Root mode="clamp" lines={3}>
  <Truncate.Text>{review}</Truncate.Text>
  <Truncate.Toggle>{(expanded) => (expanded ? 'Collapse' : 'Read more')}</Truncate.Toggle>
</Truncate.Root>
```

There is nothing to truncate against until something bounds the width — a fixed width, a table
cell, or a flex or grid track. An ancestor between that track and the text needs
`min-width: 0`, or it is sized by its content and never clips.

### Tabs

```tsx
import { Tabs } from '@ontoui/react';

<Tabs.Root defaultValue="overview">
  <Tabs.List>
    <Tabs.Tab value="overview">Overview</Tabs.Tab>
    <Tabs.Tab value="settings">Settings</Tabs.Tab>
  </Tabs.List>
  <Tabs.Panel value="overview">Overview content</Tabs.Panel>
  <Tabs.Panel value="settings">Settings content</Tabs.Panel>
</Tabs.Root>;
```

### RadioGroup + Radio

```tsx
import { Radio, RadioGroup } from '@ontoui/react';

<RadioGroup name="plan" defaultValue="free">
  <Radio value="free">Free</Radio>
  <Radio value="pro">Pro</Radio>
</RadioGroup>;
```

### Select

```tsx
import { Select } from '@ontoui/react';

<Select.Root name="country" defaultValue="us">
  <Select.Label>Country</Select.Label>
  <Select.Trigger />
  <Select.Popup>
    <Select.Item value="us">United States</Select.Item>
    <Select.Item value="gb">United Kingdom</Select.Item>
    <Select.Separator />
    <Select.Group>
      <Select.GroupLabel>Europe</Select.GroupLabel>
      <Select.Item value="de">Germany</Select.Item>
      <Select.Item value="fr">France</Select.Item>
    </Select.Group>
  </Select.Popup>
</Select.Root>;
```

### Slider

```tsx
import { Slider } from '@ontoui/react';

<Slider.Root name="volume" defaultValue={[50]} min={0} max={100}>
  <Slider.Label>Volume</Slider.Label>
  <Slider.Value />
  <Slider.Control>
    <Slider.Track>
      <Slider.Indicator />
      <Slider.Thumb />
    </Slider.Track>
  </Slider.Control>
</Slider.Root>;
```

### XYPad

A slider in a square: one drag sets two values. Reach for it when a pair of numbers is read as
a position — a focal point, a shadow offset, a filter pad — and for two independent quantities
use two `Slider`s instead.

The value is a point, `{ x, y }`, and each axis carries its own range: `minX`, `maxX`, `stepX`,
`largeStepX`, and the same four for `y`. Both default to `0`–`100` with a step of `1`; a step of
`0` leaves an axis continuous.

```tsx
import { XYPad } from '@ontoui/react';

<XYPad.Root defaultValue={{ x: 50, y: 50 }} style={{ width: 220 }}>
  <XYPad.Label>Focal point</XYPad.Label>
  <XYPad.Value />
  <XYPad.Control>
    <XYPad.Grid />
    <XYPad.Thumb />
  </XYPad.Control>
</XYPad.Root>;
```

`y` grows upwards by default, putting `maxY` at the top edge. Set `yDirection="down"` for values
that map straight to a screen offset, where `maxY` belongs at the bottom.

```tsx
<XYPad.Root
  value={offset}
  onValueChange={setOffset}
  minX={-24}
  maxX={24}
  minY={-24}
  maxY={24}
  yDirection="down"
>
  <XYPad.Label>Shadow offset</XYPad.Label>
  <XYPad.Value>{(value) => `${value.x}px, ${value.y}px`}</XYPad.Value>
  <XYPad.Control>
    <XYPad.Grid columns={6} rows={6} />
    <XYPad.Thumb xLabel="Horizontal offset" yLabel="Vertical offset" />
  </XYPad.Control>
</XYPad.Root>
```

`XYPad.Control` is square and fills the width it is given, so size the pad with a width on
`XYPad.Root`. `XYPad.Grid` is decorative — give it `columns` and `rows` matching `stepX` and
`stepY` to draw the snap positions.

`XYPad.Thumb` renders a range input per axis, so the pad has two tab stops, each value is
announced separately, and `nameX`/`nameY` submit with the form. Both inputs answer to all four
arrow keys. Always render `XYPad.Label` — each input is named by it plus its axis
("Shadow offset, Horizontal") — and rename the axes with `xLabel`/`yLabel` on `XYPad.Thumb` when
"Horizontal" and "Vertical" don't describe what the pad controls.

### Switch

```tsx
import { Switch } from '@ontoui/react';

<Switch.Root name="notifications" defaultChecked>
  <Switch.Thumb />
  Enable notifications
</Switch.Root>;
```

### Accordion

```tsx
import { Accordion } from '@ontoui/react';

<Accordion.Root>
  <Accordion.Item value="item-1">
    <Accordion.Header>
      <Accordion.Trigger>Section 1</Accordion.Trigger>
    </Accordion.Header>
    <Accordion.Panel>Content for section 1.</Accordion.Panel>
  </Accordion.Item>
  <Accordion.Item value="item-2">
    <Accordion.Header>
      <Accordion.Trigger>Section 2</Accordion.Trigger>
    </Accordion.Header>
    <Accordion.Panel>Content for section 2.</Accordion.Panel>
  </Accordion.Item>
</Accordion.Root>;
```

### OtpField

```tsx
import { OtpField } from '@ontoui/react';

// validationType: 'numeric' | 'alphanumeric' | 'alphabetic'
<OtpField.Root validationType="numeric">
  <OtpField.Input index={0} />
  <OtpField.Input index={1} />
  <OtpField.Input index={2} />
  <OtpField.Separator />
  <OtpField.Input index={3} />
  <OtpField.Input index={4} />
  <OtpField.Input index={5} />
</OtpField.Root>;
```

### Toast

`Toast.Provider` and `Toast.Viewport` must live near the root. Use `useToastManager` from `@base-ui/react/toast` (or whichever hook the project wires up) to imperatively fire toasts.

```tsx
import { Toast } from '@ontoui/react';

// In your app root:
<Toast.Provider>
  <App />
  <Toast.Viewport />
</Toast.Provider>

// A toast item (rendered by the provider for each queued message):
<Toast.Root>
  <Toast.Content>
    <Toast.Title>File saved</Toast.Title>
    <Toast.Description>Your changes have been saved.</Toast.Description>
    <Toast.Action>Undo</Toast.Action>
    <Toast.Close />
  </Toast.Content>
</Toast.Root>
```

### DataGrid

```tsx
import { DataGrid } from '@ontoui/react';

<DataGrid
  data={rows} // array of row objects
  columns={columns} // column definitions
/>;
```

### Sparkline

A chart the size of a word — a trend beside a number, in a grid cell, or on a metric card. No
axes and no legend: it carries the shape, and the number beside it carries the value.

`data` is a plain array of numbers, oldest first and evenly spaced. `null` is a gap: it keeps
its place on the axis and breaks the line rather than being drawn across.

```tsx
import { Sparkline } from '@ontoui/react';

<Sparkline.Root data={[12, 18, 15, 26, 24, 33, 30, 41]}>
  <Sparkline.Line />
  <Sparkline.Spot />
</Sparkline.Root>;
```

SVG paints in document order, so write the parts in the order they stack: baseline, area, line,
spot. Use `Sparkline.Line` for something continuous (a rate, a price, a latency) and
`Sparkline.Bars` for counts read one at a time (deploys per day, errors per hour).

`width` and `height` are pixels and default to `80 × 20`. Give every cell in a column the same
size, so the column scans as one chart:

```tsx
{
  accessorKey: 'trend',
  header: 'Last 8h',
  cell: ({ row }) => (
    <Sparkline.Root data={row.original.trend} width={72} height={18}>
      <Sparkline.Line />
      <Sparkline.Spot />
    </Sparkline.Root>
  ),
}
```

The range is taken from the data, so each chart fills its own box and two sparklines are
comparable only when both are given the same `min` and `max`. `baseline` is what bars stand on
and areas fill down to — `0` when the range spans it, the bottom of the range otherwise.

```tsx
<Sparkline.Root data={netFlow} width={160} height={40} min={-10} max={10}>
  <Sparkline.Bars />
  <Sparkline.Baseline />
</Sparkline.Root>
```

The root is `role="img"` with a name worded from the series. Pass `getAriaLabel` to name the
subject, or `aria-hidden` when the same numbers are already written beside it — as they are on
a metric card.

```tsx
<Sparkline.Root data={requests} width={148} height={36} aria-hidden>
  <Sparkline.Area />
  <Sparkline.Line />
  <Sparkline.Spot />
</Sparkline.Root>
```

## Step 4 — Accessibility

OntoUI components are built on Base UI primitives which handle most ARIA automatically. Still:

- **Icon-only buttons**: `Button.Icon` is `aria-hidden`, so a button without `Button.Text` needs `aria-label` on `Button.Root`
- **Form labels**: always use `TextField.Label` or pair a `<label htmlFor>` with the input `id`
- **Modal/Popover**: `Modal.Title` and `Modal.Description` are wired to `aria-labelledby` / `aria-describedby` automatically — always include them
- **Images**: pass meaningful `alt` (or `alt=""` for decorative)
- **Heading hierarchy**: use `h1`–`h6` in order; don't skip levels

## Step 5 — Forms and validation

Compose `TextField.Root` with `invalid` and `TextField.Error` for field-level errors:

```tsx
<TextField.Root name="password" invalid={!!errors.password}>
  <TextField.Label>Password</TextField.Label>
  <TextField.Control type="password" value={password} onChange={...} />
  <TextField.Error>{errors.password}</TextField.Error>
</TextField.Root>
```

For form-wide state, wire a `disabled` prop on fields and a `Button` to the submission loading state.

Group related fields with a plain `<div>` or semantic `<fieldset>` with `gap` — there are no OntoUI layout primitives, so use CSS or your framework's layout utilities.

## Step 6 — Next.js: `"use client"`

In Next.js App Router, add `"use client"` only to files that use React hooks or browser events. OntoUI components themselves need client rendering whenever they carry interactive state.

```tsx
// ✅ Server Component — static display, no directive needed
export default function ProductCard({ name }: { name: string }) {
  return <p>{name}</p>;
}

// ✅ Client Component — interactive OntoUI component
('use client');
import { Button, Modal } from '@ontoui/react';

export function DeleteButton({ onDelete }: { onDelete: () => void }) {
  return (
    <Modal.Root>
      <Modal.Trigger>Delete</Modal.Trigger>
      <Modal.Panel>
        <Modal.Title>Confirm deletion</Modal.Title>
        <Modal.Close>Cancel</Modal.Close>
        <Button.Root onClick={onDelete}>
          <Button.Text>Delete</Button.Text>
        </Button.Root>
      </Modal.Panel>
    </Modal.Root>
  );
}
```

Push interactivity to the leaves — keep as much of the tree as Server Components as possible.

## Output format

Produce:

1. **Complete, runnable code** — correct imports, no placeholders like `TODO` or `// rest of component`
2. **Proper import statements** — one `import { ... } from '@ontoui/react'` line, then local imports
3. **Component separation** — split into multiple components/files if the component is complex or contains clearly separable parts
4. **Brief explanation after the code** — 2–4 sentences on the key decisions made (component choices, accessibility, form wiring). Skip the explanation if the request was trivial.

```tsx
// Good import style
import { Button, Modal, TextField } from '@ontoui/react';
// Then local
import { useFormState } from './useFormState';
```

## When to ask first

Build immediately if the request is clear enough to produce something useful. Ask first when:

- The data shape is unknown and it changes the entire structure (e.g., "build a table" — what columns? what data?)
- There are meaningful design choices the user might care about (controlled vs. uncontrolled, validation strategy, layout direction)
- The user references files or existing components you haven't seen

When in doubt, state your assumptions at the top of the response and build — it's faster for the user to redirect from something concrete than from nothing.

# UI Components Reference

All components live in `src/components/ui/`. They are built on top of [`@base-ui/react`](https://base-ui.com) primitives (unless noted otherwise) and styled with Tailwind CSS via [`class-variance-authority`](https://cva.style/docs) (CVA).

---

## Quick Reference

| Component   | File               | Primitive           | Key Exports                                                                                                                                                                |
| ----------- | ------------------ | ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Badge       | `badge.tsx`        | `@base-ui/react`    | `Badge`, `badgeVariants`                                                                                                                                                   |
| Button      | `button.tsx`       | `@base-ui/react`    | `Button`, `buttonVariants`                                                                                                                                                 |
| Card        | `card.tsx`         | Native `<div>`      | `Card`, `CardHeader`, `CardTitle`, `CardDescription`, `CardAction`, `CardContent`, `CardFooter`                                                                            |
| Chart       | `chart.tsx`        | `recharts`          | `Chart`                                                                                                                                                                    |
| Dialog      | `dialog.tsx`       | `@base-ui/react`    | `Dialog`, `DialogTrigger`, `DialogContent`, `DialogHeader`, `DialogFooter`, `DialogTitle`, `DialogDescription`, `DialogClose`, `DialogPortal`, `DialogOverlay`             |
| Input       | `input.tsx`        | `@base-ui/react`    | `Input`                                                                                                                                                                    |
| Label       | `label.tsx`        | Native `<label>`    | `Label`                                                                                                                                                                    |
| ScrollArea  | `scroll-area.tsx`  | `@base-ui/react`    | `ScrollArea`, `ScrollBar`                                                                                                                                                  |
| Select      | `select.tsx`       | `@base-ui/react`    | `Select`, `SelectTrigger`, `SelectContent`, `SelectItem`, `SelectGroup`, `SelectLabel`, `SelectValue`, `SelectSeparator`, `SelectScrollUpButton`, `SelectScrollDownButton` |
| Separator   | `separator.tsx`    | `@base-ui/react`    | `Separator`                                                                                                                                                                |
| Slider      | `slider.tsx`       | `@base-ui/react`    | `Slider`                                                                                                                                                                   |
| Toaster     | `sonner.tsx`       | `sonner`            | `Toaster`                                                                                                                                                                  |
| Textarea    | `textarea.tsx`     | Native `<textarea>` | `Textarea`                                                                                                                                                                 |
| ToggleGroup | `toggle-group.tsx` | Custom (Button)     | `ToggleGroup`                                                                                                                                                              |

---

## Badge

**File:** [src/components/ui/badge.tsx](../src/components/ui/badge.tsx)

A small inline label used to display status, categories, or short metadata.

### Props

| Prop        | Type                           | Default     | Description                          |
| ----------- | ------------------------------ | ----------- | ------------------------------------ |
| `variant`   | See variants below             | `"default"` | Visual style                         |
| `render`    | `useRender` render prop        | —           | Override the rendered element        |
| `className` | `string`                       | —           | Additional Tailwind classes          |
| `...props`  | `React.ComponentProps<"span">` | —           | Forwarded to the underlying `<span>` |

### Variants

| Variant       | Description                      |
| ------------- | -------------------------------- |
| `default`     | Primary color background         |
| `secondary`   | Secondary color background       |
| `destructive` | Red/danger tint                  |
| `outline`     | Bordered, transparent background |
| `ghost`       | No background, hover effect only |
| `link`        | Looks like an underlined link    |

### Usage

```tsx
import { Badge } from "@/components/ui/badge"

<Badge variant="destructive">Overdue</Badge>
<Badge variant="outline">Draft</Badge>
```

---

## Button

**File:** [src/components/ui/button.tsx](../src/components/ui/button.tsx)

The primary interactive element. Supports multiple visual styles and sizes with built-in focus, disabled, and invalid states.

### Props

| Prop        | Type                    | Default     | Description                          |
| ----------- | ----------------------- | ----------- | ------------------------------------ |
| `variant`   | See variants below      | `"default"` | Visual style                         |
| `size`      | See sizes below         | `"default"` | Height and padding                   |
| `className` | `string`                | —           | Additional Tailwind classes          |
| `...props`  | `ButtonPrimitive.Props` | —           | Forwarded to `@base-ui/react/button` |

### Variants

| Variant       | Description                                    |
| ------------- | ---------------------------------------------- |
| `default`     | Solid primary color                            |
| `outline`     | Bordered, uses background/muted on hover       |
| `secondary`   | Secondary color fill                           |
| `ghost`       | Transparent, shows muted background on hover   |
| `destructive` | Red/danger tint                                |
| `link`        | Looks like a text link with underline on hover |

### Sizes

| Size      | Height | Notes                                   |
| --------- | ------ | --------------------------------------- |
| `default` | `h-8`  | Standard button with horizontal padding |
| `sm`      | `h-7`  | Smaller text and reduced radius         |
| `xs`      | `h-6`  | Extra small                             |
| `lg`      | `h-9`  | Larger with more padding                |
| `icon`    | `8×8`  | Square icon button (default size)       |
| `icon-sm` | `7×7`  | Square icon button (small)              |
| `icon-xs` | `6×6`  | Square icon button (extra small)        |
| `icon-lg` | `9×9`  | Square icon button (large)              |

### Usage

```tsx
import { Button } from "@/components/ui/button"

<Button>Save</Button>
<Button variant="outline" size="sm">Cancel</Button>
<Button variant="ghost" size="icon"><TrashIcon /></Button>
```

---

## Card

**File:** [src/components/ui/card.tsx](../src/components/ui/card.tsx)

A container surface with rounded corners and a subtle ring. Composed of seven sub-components that map to the layout regions of a typical card.

### Sub-components

| Component         | Slot               | Description                                                 |
| ----------------- | ------------------ | ----------------------------------------------------------- |
| `Card`            | `card`             | Root container; accepts `size` prop                         |
| `CardHeader`      | `card-header`      | Top area; uses CSS Grid to support an optional `CardAction` |
| `CardTitle`       | `card-title`       | Main heading inside the header                              |
| `CardDescription` | `card-description` | Muted subtitle inside the header                            |
| `CardAction`      | `card-action`      | Placed at the right of the header (e.g. menu button)        |
| `CardContent`     | `card-content`     | Main body content area                                      |
| `CardFooter`      | `card-footer`      | Bottom area with muted background and a top border          |

### Card Props

| Prop        | Type                          | Default     | Description                 |
| ----------- | ----------------------------- | ----------- | --------------------------- |
| `size`      | `"default"` \| `"sm"`         | `"default"` | Adjusts spacing and font    |
| `className` | `string`                      | —           | Additional Tailwind classes |
| `...props`  | `React.ComponentProps<"div">` | —           | Forwarded to `<div>`        |

All sub-components only accept `className` and standard `div` props.

### Usage

```tsx
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
} from "@/components/ui/card";

<Card>
  <CardHeader>
    <CardTitle>Monthly Summary</CardTitle>
    <CardDescription>Overview of this month's activity</CardDescription>
  </CardHeader>
  <CardContent>...</CardContent>
  <CardFooter>...</CardFooter>
</Card>;
```

---

## Chart

**File:** [src/components/ui/chart.tsx](../src/components/ui/chart.tsx)

A generalized chart component built on [Recharts](https://recharts.org). Supports **area** (gradient-filled) and **line** (stroke-only) variants. Accepts extra Recharts children (e.g. `ReferenceLine`) for composition.

### Props

| Prop               | Type                                          | Default                                     | Description                                    |
| ------------------ | --------------------------------------------- | ------------------------------------------- | ---------------------------------------------- |
| `data`             | `Record<string, unknown>[]`                   | —                                           | Chart data array (required)                    |
| `xKey`             | `string`                                      | —                                           | Data key for the X axis (required)             |
| `dataKey`          | `string`                                      | —                                           | Data key for the Y value (required)            |
| `variant`          | `"area"` \| `"line"`                          | `"area"`                                    | Chart type                                     |
| `color`            | `string`                                      | `"#3b82f6"`                                 | Stroke color (and gradient base for area)      |
| `gradient`         | `boolean`                                     | `true` for area, `false` for line           | Show gradient fill under the area              |
| `showDots`         | `boolean \| { radius?: number }`              | `false` for area, `true` for line           | Show data-point dots                           |
| `activeDot`        | `boolean \| { radius?: number }`              | matches `showDots`                          | Active (hovered) dot config                    |
| `height`           | `number \| "100%"`                            | `"100%"`                                    | ResponsiveContainer height                     |
| `margin`           | `{ top, right, bottom, left }`                | `{ top: 4, right: 12, bottom: 0, left: 4 }` | Chart margin                                   |
| `yDomain`          | `[number, number]`                            | auto                                        | Custom Y axis domain                           |
| `yWidth`           | `number`                                      | auto                                        | Fixed Y axis width                             |
| `tickFontSize`     | `number`                                      | `10`                                        | XAxis / YAxis tick font size                   |
| `yTickFormatter`   | `(value: number) => string`                   | k-formatter (`1000` → `1k`)                 | Custom Y axis tick formatter                   |
| `tooltipFormatter` | `(value, name) => [string, string] \| string` | —                                           | Recharts Tooltip formatter                     |
| `children`         | `ReactNode`                                   | —                                           | Extra Recharts elements (e.g. `ReferenceLine`) |
| `className`        | `string`                                      | —                                           | Additional Tailwind classes on the wrapper     |

### Variants

| Variant | Description                                                 |
| ------- | ----------------------------------------------------------- |
| `area`  | Filled area chart with a vertical linear gradient (default) |
| `line`  | Stroke-only line chart, dots shown by default               |

### Usage

```tsx
import { Chart } from "@/components/ui/chart";

{
  /* Area chart (default) */
}
<Chart
  data={historyData}
  xKey="month"
  dataKey="value"
  color="#3b82f6"
  tooltipFormatter={(v) => [`$${Number(v).toLocaleString()}`, "Net Worth"]}
/>;

{
  /* Line chart with reference line */
}
import { ReferenceLine } from "recharts";

<Chart
  variant="line"
  data={weightData}
  xKey="date"
  dataKey="weight"
  color="var(--chart-1)"
  height={220}
  yDomain={[60, 90]}
  showDots={{ radius: 3 }}
  tooltipFormatter={(v, name) => [`${v} kg`, "Weight"]}
>
  <ReferenceLine y={75} stroke="var(--chart-2)" strokeDasharray="6 3" />
</Chart>;
```

---

## Dialog

**File:** [src/components/ui/dialog.tsx](../src/components/ui/dialog.tsx)

A modal dialog built on `@base-ui/react/dialog`. Automatically includes an animated overlay and a close button.

### Sub-components

| Component           | Description                                                        |
| ------------------- | ------------------------------------------------------------------ |
| `Dialog`            | Root controlled/uncontrolled state provider                        |
| `DialogTrigger`     | Element that opens the dialog                                      |
| `DialogPortal`      | Renders children outside the DOM hierarchy                         |
| `DialogOverlay`     | Semi-transparent backdrop with blur and fade animation             |
| `DialogContent`     | The visible panel; renders overlay + popup + optional close button |
| `DialogHeader`      | Flex column wrapper for title and description                      |
| `DialogFooter`      | Action row with muted background; optional built-in close button   |
| `DialogTitle`       | Accessible dialog heading                                          |
| `DialogDescription` | Muted description text                                             |
| `DialogClose`       | Primitive close trigger                                            |

### DialogContent Props

| Prop              | Type                          | Default | Description                                      |
| ----------------- | ----------------------------- | ------- | ------------------------------------------------ |
| `showCloseButton` | `boolean`                     | `true`  | Show the `×` icon button in the top-right corner |
| `className`       | `string`                      | —       | Additional Tailwind classes on the popup panel   |
| `...props`        | `DialogPrimitive.Popup.Props` | —       | Forwarded to the base-ui popup                   |

### DialogFooter Props

| Prop              | Type      | Default | Description                                              |
| ----------------- | --------- | ------- | -------------------------------------------------------- |
| `showCloseButton` | `boolean` | `false` | Append a "Close" outline button at the end of the footer |
| `className`       | `string`  | —       | Additional Tailwind classes                              |

### Usage

```tsx
import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";

<Dialog>
  <DialogTrigger render={<Button>Open</Button>} />
  <DialogContent>
    <DialogHeader>
      <DialogTitle>Edit Entry</DialogTitle>
      <DialogDescription>Make changes and save.</DialogDescription>
    </DialogHeader>
    {/* form fields */}
    <DialogFooter showCloseButton>
      <Button type="submit">Save</Button>
    </DialogFooter>
  </DialogContent>
</Dialog>;
```

---

## Input

**File:** [src/components/ui/input.tsx](../src/components/ui/input.tsx)

A single-line text field wrapping `@base-ui/react/input`. Includes consistent focus ring, disabled, and `aria-invalid` styles.

### Props

Accepts all standard `React.ComponentProps<"input">` — `type`, `value`, `onChange`, `placeholder`, `disabled`, etc.

### Usage

```tsx
import { Input } from "@/components/ui/input"

<Input type="text" placeholder="Enter name..." />
<Input type="number" disabled />
```

---

## Label

**File:** [src/components/ui/label.tsx](../src/components/ui/label.tsx)

A styled `<label>` element. Automatically dims when a peer input is disabled.

### Props

Accepts all standard `React.ComponentProps<"label">` — `htmlFor`, `children`, etc.

### Usage

```tsx
import { Label } from "@/components/ui/label"
import { Input } from "@/components/ui/input"

<Label htmlFor="email">Email</Label>
<Input id="email" type="email" />
```

---

## ScrollArea

**File:** [src/components/ui/scroll-area.tsx](../src/components/ui/scroll-area.tsx)

A custom scrollable container that replaces native scrollbars with styled ones. Built on `@base-ui/react/scroll-area`.

### Exports

| Export       | Description                                  |
| ------------ | -------------------------------------------- |
| `ScrollArea` | Root wrapper; renders viewport and scrollbar |
| `ScrollBar`  | The styled scrollbar track and thumb         |

### ScrollArea Props

| Prop        | Type                             | Default | Description                   |
| ----------- | -------------------------------- | ------- | ----------------------------- |
| `className` | `string`                         | —       | Applied to the root element   |
| `...props`  | `ScrollAreaPrimitive.Root.Props` | —       | Forwarded to the base-ui root |

### ScrollBar Props

| Prop          | Type                           | Default      | Description                 |
| ------------- | ------------------------------ | ------------ | --------------------------- |
| `orientation` | `"vertical"` \| `"horizontal"` | `"vertical"` | Direction of the scrollbar  |
| `className`   | `string`                       | —            | Additional Tailwind classes |

### Usage

```tsx
import { ScrollArea } from "@/components/ui/scroll-area";

<ScrollArea className="h-72">{/* Long content */}</ScrollArea>;
```

---

## Select

**File:** [src/components/ui/select.tsx](../src/components/ui/select.tsx)

A fully accessible dropdown select built on `@base-ui/react/select`. Includes scroll arrows for long option lists.

### Sub-components

| Component                | Description                                                  |
| ------------------------ | ------------------------------------------------------------ |
| `Select`                 | Root — alias for `SelectPrimitive.Root`                      |
| `SelectTrigger`          | The visible button that opens the dropdown                   |
| `SelectValue`            | Displays the currently selected value inside the trigger     |
| `SelectContent`          | The dropdown panel; handles portal, overlay, and positioning |
| `SelectGroup`            | Groups related options with padding                          |
| `SelectLabel`            | Muted label for a group                                      |
| `SelectItem`             | An individual option with checkmark when selected            |
| `SelectSeparator`        | Horizontal divider between groups or items                   |
| `SelectScrollUpButton`   | Arrow shown at top when list can scroll up                   |
| `SelectScrollDownButton` | Arrow shown at bottom when list can scroll down              |

### SelectTrigger Props

| Prop        | Type                  | Default     | Description                  |
| ----------- | --------------------- | ----------- | ---------------------------- |
| `size`      | `"default"` \| `"sm"` | `"default"` | Height of the trigger button |
| `className` | `string`              | —           | Additional Tailwind classes  |

### SelectContent Positioning Props

| Prop                   | Type                               | Default    | Description                              |
| ---------------------- | ---------------------------------- | ---------- | ---------------------------------------- |
| `side`                 | `"top"` \| `"bottom"` \| etc.      | `"bottom"` | Which side to open on                    |
| `sideOffset`           | `number`                           | `4`        | Gap between trigger and dropdown         |
| `align`                | `"start"` \| `"center"` \| `"end"` | `"center"` | Horizontal alignment                     |
| `alignOffset`          | `number`                           | `0`        | Offset from the alignment edge           |
| `alignItemWithTrigger` | `boolean`                          | `true`     | Align the selected item with the trigger |

### Usage

```tsx
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@/components/ui/select";

<Select defaultValue="monthly">
  <SelectTrigger>
    <SelectValue placeholder="Select period" />
  </SelectTrigger>
  <SelectContent>
    <SelectItem value="weekly">Weekly</SelectItem>
    <SelectItem value="monthly">Monthly</SelectItem>
    <SelectItem value="yearly">Yearly</SelectItem>
  </SelectContent>
</Select>;
```

---

## Separator

**File:** [src/components/ui/separator.tsx](../src/components/ui/separator.tsx)

A thin decorative line used to divide sections. Built on `@base-ui/react/separator`.

### Props

| Prop          | Type                           | Default        | Description                   |
| ------------- | ------------------------------ | -------------- | ----------------------------- |
| `orientation` | `"horizontal"` \| `"vertical"` | `"horizontal"` | Direction of the divider line |
| `className`   | `string`                       | —              | Additional Tailwind classes   |

### Usage

```tsx
import { Separator } from "@/components/ui/separator"

<Separator />                          {/* horizontal rule */}
<Separator orientation="vertical" />  {/* vertical rule in a flex row */}
```

---

## Slider

**File:** [src/components/ui/slider.tsx](../src/components/ui/slider.tsx)

A range input component supporting single and multi-thumb sliders. Built on `@base-ui/react/slider`.

### Props

| Prop           | Type                         | Default | Description                             |
| -------------- | ---------------------------- | ------- | --------------------------------------- |
| `min`          | `number`                     | `0`     | Minimum value                           |
| `max`          | `number`                     | `100`   | Maximum value                           |
| `value`        | `number[]`                   | —       | Controlled value(s)                     |
| `defaultValue` | `number[]`                   | —       | Uncontrolled initial value(s)           |
| `className`    | `string`                     | —       | Additional Tailwind classes on the root |
| `...props`     | `SliderPrimitive.Root.Props` | —       | Forwarded to the base-ui root           |

> **Note:** Pass an array with one element for a single thumb (e.g. `defaultValue={[50]}`), or two elements for a range (e.g. `defaultValue={[20, 80]}`). The number of thumbs is derived from the length of `value` / `defaultValue`.

### Usage

```tsx
import { Slider } from "@/components/ui/slider"

<Slider defaultValue={[40]} min={0} max={100} />
<Slider defaultValue={[20, 80]} />  {/* range slider */}
```

---

## Toaster (Sonner)

**File:** [src/components/ui/sonner.tsx](../src/components/ui/sonner.tsx)

Toast notification provider. Uses the [`sonner`](https://sonner.emilkowal.ski/) library wrapped to adopt the app's theme tokens and Lucide icons.

The `Toaster` component should be placed once in the root layout. Toasts are triggered imperatively via `sonner`'s `toast` function.

### Props

Accepts all `ToasterProps` from `sonner`. The `theme` prop is automatically derived from `next-themes`.

### Toast icons (auto-applied)

| Type      | Icon                 |
| --------- | -------------------- |
| `success` | `CircleCheckIcon`    |
| `info`    | `InfoIcon`           |
| `warning` | `TriangleAlertIcon`  |
| `error`   | `OctagonXIcon`       |
| `loading` | `Loader2Icon` (spin) |

### Usage

```tsx
// In root layout (layout.tsx):
import { Toaster } from "@/components/ui/sonner";
<Toaster />;

// Anywhere in the app:
import { toast } from "sonner";
toast.success("Entry saved");
toast.error("Something went wrong");
```

---

## Textarea

**File:** [src/components/ui/textarea.tsx](../src/components/ui/textarea.tsx)

A multi-line text input with auto-height via CSS `field-sizing: content`. Styling is consistent with `Input`.

### Props

Accepts all standard `React.ComponentProps<"textarea">` — `value`, `onChange`, `placeholder`, `rows`, `disabled`, etc.

> **Tip:** Because `field-sizing-content` is applied, the textarea grows with its content automatically. Override `min-h-*` via `className` to control the minimum height.

### Usage

```tsx
import { Textarea } from "@/components/ui/textarea"

<Textarea placeholder="Add a note..." />
<Textarea className="min-h-32" />
```

---

## ToggleGroup

**File:** [src/components/ui/toggle-group.tsx](../src/components/ui/toggle-group.tsx)

> **Note:** This is a **custom component**, not from `@base-ui/react`. It renders a row of `Button` components where only one can be active at a time (radio-group behaviour).

### Props

| Prop              | Type                                            | Default  | Description                                   |
| ----------------- | ----------------------------------------------- | -------- | --------------------------------------------- |
| `items`           | `readonly (T \| { value: T; label: string })[]` | required | The options to render                         |
| `value`           | `T`                                             | required | Currently selected value                      |
| `onValueChange`   | `(value: T) => void`                            | required | Called when the user selects a different item |
| `size`            | `"xs"` \| `"sm"` \| `"default"` \| `"lg"`       | `"sm"`   | Passed to each `Button`                       |
| `className`       | `string`                                        | —        | Applied to the outer `<div>` wrapper          |
| `buttonClassName` | `string`                                        | —        | Applied to every individual `Button`          |

`T` is inferred from the `items` array — it must extend `string`.

Items can be either:

- A plain `string` — used as both value and display label.
- An object `{ value: T; label: string }` — value and label are distinct.

### Usage

```tsx
import { ToggleGroup } from "@/components/ui/toggle-group"

// String items
<ToggleGroup
  items={["Day", "Week", "Month"] as const}
  value={period}
  onValueChange={setPeriod}
/>

// Object items
<ToggleGroup
  items={[
    { value: "income", label: "Income" },
    { value: "expense", label: "Expenses" },
  ]}
  value={tab}
  onValueChange={setTab}
  size="default"
/>
```

# The Foundation Document

## Base UI in afenda-xForge-v4

**What the library guarantees, what it leaves to us, and the rules every authored component follows.**

---

## How to read this document

Every claim is marked. There are exactly two marks:

**VERIFIED** — the claim is quoted or closely paraphrased from a Base UI documentation page, or read directly from a file in this repository. The page (and section) or the file path is cited on the same claim. If Base UI says a Dialog traps focus, this document says a Dialog traps focus, and names the page that says it. A cited path that does not resolve is a citation an agent cannot check, and is therefore a defect in the same class as a wrong claim.

**ASSUMED** — the claim is an inference, a convention, a recommendation, or a repository decision that no cited source states as a guarantee. Nothing marked ASSUMED may be relied on as a Base UI contract, and no agent may promote an ASSUMED claim to VERIFIED without adding the citation.

A third state appears throughout and is deliberately not a mark: **NOT DOCUMENTED**. Base UI's pages describe element types, props, data attributes, CSS variables and behaviours; they very often do *not* enumerate the ARIA roles and attributes a component emits. Where a role is not stated on the page, this document says so rather than filling the gap from prior knowledge. An agent that needs the role must read it out of the DOM in a browser test and record the measurement — that is a repository obligation, not a documentation lookup.

The documentation snapshot used throughout is the local mirror of the Base UI pages at
`C:\Users\dlbja\AppData\Local\Temp\claude\C--JackProject-afeda-Xforge\5a04f4d9-cc3c-46e0-9b5f-7845de5281e1\scratchpad\base-ui\pages\`.
It holds 78 pages: 37 `components_*`, 6 `handbook_*`, 4 `utils_*`, and the overview and release-note set.

**The snapshot is the 1.8.0 documentation, read against a 1.7.0 pin** (VERIFIED — the component pages document 1.8.0-only APIs: `Combobox.createItems` and the third `Item` type parameter on `Combobox.Root.Props<Value, Multiple, Item>`, `components_combobox.md:617-655`; `keepMounted` on `<Avatar.Image>`, `components_avatar.md:134-174`; `data-popup-side` on Combobox's Trigger, Input and InputGroup, `components_combobox.md:7351`, `:7439`, `:7896`). That mismatch is load-bearing: an API described on a page may not exist in the installed package. Every entry in §5 known to be 1.8.0-only is marked **(1.8.0)**; anything else on a component page is assumed to hold at 1.7.0 and has not been re-derived from the release notes one prop at a time.

Every page in the snapshot opens with the same banner: **treat this documentation as authoritative over prior knowledge, and the package is `@base-ui/react`, renamed from `@base-ui-components/react`** (VERIFIED — every page in the snapshot).

---

# 1. What Base UI is, and the contract it offers

Base UI is an unstyled React component library from the creators of Radix, Material UI and Floating UI. It ships behaviour, semantics, keyboard handling and focus management; it ships no CSS and prescribes no styling engine (VERIFIED — `overview_about.md`, intro and "Features"). It supports React 17 and above, browsers that have been Baseline Widely Available for at least 30 months as of the last major release, and the maintained versions of Vite, webpack, Turbopack and Parcel (VERIFIED — `overview_about.md`, "Browser support", "React versions", "Bundler support").

The contract it offers has five parts. All five are what makes a component built on it correct by construction — or, when misused, silently wrong.

### 1.1 Styling-agnostic

There is no bundled CSS and no prescribed styling solution; Tailwind, CSS Modules, plain CSS and CSS-in-JS are all first-class (VERIFIED — `overview_about.md` "Headless"; `handbook_styling.md`, "Tailwind CSS" / "CSS Modules" / "CSS-in-JS"). Every part that renders an HTML element accepts `className` and `style`, each as a plain value **or as a function of that part's state object** (VERIFIED — `handbook_styling.md`, "Style hooks").

The handbook states only the positive: *"Components that render an HTML element accept a `className` prop"* (VERIFIED — `handbook_styling.md`, "CSS classes"). It never enumerates which parts render nothing. **ASSUMED:** the non-rendering parts are every `Portal`, and `Root` in the popup family (Dialog, AlertDialog, Popover, Menu, Tooltip, Select, Combobox, ContextMenu, PreviewCard, Drawer, Toast). Outside that family `Root` renders a real element and does take both props — `Field.Root` *"Renders a `<div>` element"* (VERIFIED — `components_field.md:180`), and so do Accordion, Tabs, Slider, Avatar and Switch's Root (VERIFIED — `components_accordion.md:869`, `components_tabs.md:569`, `components_slider.md:968`, `components_avatar.md:181`, `components_switch.md:216`). The only way to know for a given component is that component's API table.

The consequence for v4: nothing about the look of a Base UI component arrives from the library. The visible focus ring, the disabled treatment, the hover tint, the checked colour, the height of a switch track — all of it is ours. That is not a limitation to work around; it is the reason the library is adoptable under a design system that owns its own vocabulary.

### 1.2 Composable — the `render` prop

`render` is Base UI's single composition mechanism, and it has three forms (VERIFIED — `handbook_composition.md`):

- **Element form.** `<Menu.Trigger render={<MyButton size="md" />}>Open menu</Menu.Trigger>` substitutes your component for the default element, and Base UI merges the props automatically. The requirement is stated flatly: *"The custom component must forward the `ref`, and spread all the received props on its underlying DOM node."* (VERIFIED — `handbook_composition.md`, "Composing custom React components".)
- **Nesting.** `render` props *"can be nested as deeply as necessary"* — a Tooltip.Trigger rendering a Dialog.Trigger rendering a Menu.Trigger rendering one button collapses three behaviours onto one node (VERIFIED — same page, "Composing multiple components").
- **Function form.** `render={(props, state) => <span {...props}>{state.checked ? <A/> : <B/>}</span>}` gives complete control over how props are spread and lets content depend on state. In this form **nothing is merged for you** — forgetting `{...props}` silently removes every handler, ARIA attribute and positioning ref (VERIFIED — `handbook_composition.md`, "Render function"; `utils_merge-props.md`, the paragraph after the demo).

Changing the default element is *"recommended only on a case-by-case basis"* because *"each Base UI component renders the most appropriate element by default"* (VERIFIED — `handbook_composition.md`, "Changing the default rendered element"). The default element is the semantics.

Button-like parts carry a `nativeButton` flag precisely because the component cannot know, before hydration, what element `render` will produce: defaults such as `type="button"` are only valid on `<button>` (VERIFIED — `utils_use-render.md`, "Render prop and polymorphism"; `components_button.md`, API reference).

**The default inverts between components, and §5 does not carry it.** Observed spread: `true` on Button (`components_button.md:307`) and Toggle (`components_toggle.md:242`); `false` on Switch (`components_switch.md:228`); both values inside one component — Select is `true` at `components_select.md:2412` and `false` at `:2824`, Combobox is `true` at `components_combobox.md:7340`, `:7515`, `:8167` and `false` at `:7845` (all VERIFIED). The complete list lives in each component's API reference, which is where an agent must read it — §8 #21 states that rule and this is the case that proves it, because the catalog in §5 does not carry `nativeButton` at all.

### 1.3 Data attributes and CSS variables — the styling contract

*"Components provide data attributes designed for styling their states"* — e.g. Switch's `[data-checked]` / `[data-unchecked]` — and *"expose CSS variables to aid in styling, often containing dynamic numeric values to be used in sizing or transform calculations"* — e.g. a Popover Popup's `--available-height` and `--anchor-width` (VERIFIED — `handbook_styling.md`, "Data attributes" and "CSS variables").

The handbook only gives examples; the complete list for any component lives in that component's API reference (VERIFIED — `handbook_styling.md`, "Data attributes": *"Check out each component's API reference for a complete list"*). §5 of this document is that list at assembly level, consolidated — not a substitute for the API reference.

Three families of attribute recur across the whole library and should be treated as vocabulary rather than per-component trivia:

| Family | Attributes | Meaning |
|---|---|---|
| Open/closed | `data-open`, `data-closed` | Visibility state of a popup-like part (VERIFIED — `handbook_animation.md`, "CSS animations") |
| Transition | `data-starting-style`, `data-ending-style` | The style to transition *from* on enter and *to* on exit (VERIFIED — `handbook_animation.md`, "CSS transitions") |
| Placement | `data-side`, `data-align`, `data-anchor-hidden`, `data-uncentered` | Resolved position of an anchored popup (VERIFIED — every Positioner API table; e.g. `components_popover.md` Positioner) |

Two further cross-component families are documented and are catalogued as shared blocks at the end of §5, because they recur across more components than any per-row cell can carry: **detached triggers** (`createHandle`, `Handle`, `payload`, `triggerId`) and **Viewport content transitions** (`data-activation-direction`, `data-current`, `data-previous`, and the `--popup-width` / `--popup-height` freeze).

### 1.4 Native form participation

*"Base UI form components use a hidden input to participate in native form submission and validation."* (VERIFIED — `handbook_forms.md`, "Constraint validation".) Checkbox.Root and Radio.Root each *"render a `<span>` element and a hidden `<input>` beside"* (VERIFIED — `components_checkbox.md`, `components_radio.md` API reference); Slider renders a nested `<input type="range">` per thumb (VERIFIED — `components_slider.md`, Thumb); Select, Combobox and Autocomplete each own a hidden input reachable through `inputRef` (VERIFIED — those pages' Root prop tables).

Two obligations follow, both stated: give the control a `name`, and *"wrap controls in a relatively positioned container"* so the browser's native validation bubble points at the right place (VERIFIED — `handbook_forms.md`, "Constraint validation"). Supported native constraint attributes are `required`, `minLength`/`maxLength`, `pattern` and `step` (VERIFIED — same section).

### 1.5 One event envelope, one controlled/uncontrolled model

Every Base UI change callback takes the new value first and an `eventDetails` object second (VERIFIED — `handbook_customization.md`, "Base UI events"):

```ts
interface BaseUIChangeEventDetails {
  reason: string;          // why the change occurred
  event: Event;            // the native DOM event
  cancel(): void;          // stop the component changing its internal state
  allowPropagation(): void;// let the DOM event propagate where Base UI stops it
  isCanceled: boolean;
  isPropagationAllowed: boolean;
}
```

`cancel()` is documented as *"an alternative to controlling the component with external state and guarding the state updates conditionally"* (VERIFIED — `handbook_customization.md`, "Canceling a Base UI event"). Components are uncontrolled by default and become controlled by passing the state prop plus its handler (VERIFIED — same page, "Controlling components with state").

There is one hazard here that costs the repository a wrapper on every adapter: **the second argument exists on every handler.** A callback forwarded straight through from a v4 prop leaks `eventDetails` to the caller. Every authored adapter in v4 wraps its handler to one argument (VERIFIED — `packages/design/src/components/switch.tsx`, `text-input.tsx`, `date-input.tsx`, `combobox.tsx` headers).

### 1.6 What v4 pins, and how it imports

`@base-ui/react` **1.7.0**, fixed — not a range — in the pnpm catalog (VERIFIED — `pnpm-workspace.yaml` line 9 `'@base-ui/react': 1.7.0`; `packages/design/package.json` line 26 `"catalog:"`; `pnpm-lock.yaml` lines 308 and 3391 resolving `@base-ui/react@1.7.0`; **`packages/design/node_modules/@base-ui/react/package.json` version 1.7.0** — pnpm's isolated layout means there is no `node_modules/@base-ui` at the repository root, and the real file is hard-linked from `node_modules/.pnpm/@base-ui+react@1.7.0_…`). React 19.2.8.

Six of twenty-three authored components sit on Base UI (VERIFIED — read of `packages/design/src/components/*.tsx`):

| Component | Import | Why |
|---|---|---|
| Button | `@base-ui/react/button` | subpath, pre-bundled by Vite |
| Switch | `@base-ui/react/switch` | subpath, pre-bundled by Vite |
| TextInput | `@base-ui/react` (root) | subpath pulls a second React under Vite |
| DateInput | `@base-ui/react` (root) | same |
| Field | `@base-ui/react` (root) | same |
| Combobox | `@base-ui/react` (root) | same |

The root-import rule is load-bearing and worth stating as a hazard rather than a preference: `'@base-ui/react/input'` resolves to a module Vite has not pre-bundled, which pulls a **second React** into the browser project and kills every story on *"Invalid hook call"* — sixteen story mounts red (VERIFIED — `packages/design/src/components/text-input.tsx` header, "IMPORTED FROM THE PACKAGE ROOT"). Switch and Button subpaths happen to be pre-bundled, so the precedent did not transfer. Nothing in `vitest.config.ts` pins `optimizeDeps`, so this is a convention held by file headers and not by a guard (VERIFIED — read of `vitest.config.ts`). That is a gap, and §7 names the check that would close it.

### 1.7 Two app-level setup steps the docs require

Both are stated in the quick start and neither is optional (VERIFIED — `overview_quick-start.md`, "Set up"):

1. **Portals.** Wrap the app in a root element with `isolation: isolate` so portaled popups always sit above page content and none of your `z-index` values interfere.
2. **iOS 26+ Safari.** Content can show beneath the UI chrome, so dialog-style backdrops must use `position: absolute` (not `fixed`) to cover the visual viewport, and `body { position: relative }` must be set globally for that to keep working after scrolling.

**Step 1 is LIVE AND UNMET today** (VERIFIED). The authored Combobox portals: `packages/design/src/components/combobox.tsx` names Portal among its nine adopted parts in the header and renders `<Primitive.Portal>` → `<Primitive.Positioner className={cn('isolate', STYLE.layer.overlay)}>` → `<Primitive.Popup>` at lines 177-216. `STYLE.layer.overlay` resolves to the `layer-overlay` utility (`packages/design/generated/style.ts:114-116`) — a z-index token, which is precisely the *"your z-index values"* the isolation root exists to make unnecessary. The `isolate` class on the Positioner creates a stacking context for the popup's own children; it does not raise the portaled subtree above page content. No isolation root exists: `apps/web/app/layout.tsx` renders `<body>` → Mocks → Providers → Page with no wrapper, and `apps/web/app/globals.css` contains no `isolation` declaration and no `body { position: relative }` (its only `position` rules are two `position: sticky` shell rules at lines 246 and 256).

So the stacking guarantee for the one shipped popup currently rests on a z-index token rather than on the documented mechanism. Either add `isolation: isolate` to an app root in `apps/web/app/globals.css` + `layout.tsx`, or record here why `layer-overlay` is accepted in its place and what would break it. Rule **R13** in §7 states the failing check that would settle it.

Step 2 is **dormant, for a stated reason**: the Combobox renders no `Backdrop`, and no authored component renders a dialog-style backdrop at all (VERIFIED — `combobox.tsx`, part list). It is not dormant because "there is no portaled adapter" — there is one. It becomes live with the first Backdrop.

---

# 2. Accessibility: the guarantee, the obligations, and where v4 stands

## 2.1 What Base UI guarantees

The accessibility overview makes five claims. Each is quoted precisely because loose paraphrase is exactly how a guarantee gets over-claimed.

**VERIFIED** — `overview_accessibility.md`, intro: components *"handle many complex accessibility details including ARIA attributes, role attributes, pointer interactions, keyboard navigation, and focus management."*

**VERIFIED** — same page, "Keyboard navigation": keyboard behaviour follows the WAI-ARIA Authoring Practices; *"many components support arrow keys, alphanumeric keys, Home, End, Enter and Esc."* Note the hedge — *many components*, not all, and the page does not say which.

**VERIFIED** — same page, "Focus management": focus is managed automatically following a user interaction, and *"some components expose `initialFocus` and `finalFocus` props to configure it."*

**VERIFIED** — same page, and `handbook_forms.md`: Form, Input, Field and Fieldset *automatically associate* form controls with their labels; `Field.Label` is *"An accessible label that is automatically associated with the field control"* (`components_field.md:421`); `Field.Description` *"automatically assigns an accessible description"* (`handbook_forms.md`, "Describing the control").

> **NOT DOCUMENTED — how a childless `Field.Error` behaves.** An earlier revision of this document claimed, twice and under a VERIFIED mark, that `Field.Error` with no children renders the browser's native validity message. **No page in the snapshot says that.** A grep for `validity message|native message|browser message|validationMessage` across all 78 pages returns zero hits. The Error section reads in full: *"An error message displayed if the field control fails validation. Renders a `<div>` element."* (`components_field.md:468-471`), and its `match` prop (`:477`) is documented purely as ValidityState-key/boolean gating of *visibility* — nothing about content. What a childless Error renders, and what a screen reader announces for it, is a browser measurement this repository owes, in the same category as Switch's role.

**VERIFIED** — same page, "Testing": components are tested on a broad spectrum of browsers, devices, platforms, screen readers and environments.

Two additional guarantees come from other pages and matter for correctness:

**VERIFIED** — `handbook_customization.md`: *"In most components, pressing the Esc key stops the propagation of the event so parent popups don't close simultaneously"* — nested popups close one level at a time by default, overridable with `eventDetails.allowPropagation()`.

**VERIFIED** — `components_dialog.md`, Root props and Popup: when `modal` is `true` (the default) a Dialog **traps focus, locks document scroll, and disables outside pointer interaction**; `'trap-focus'` traps focus without the scroll lock; `false` is fully non-modal. On open, focus moves to the first tabbable element inside the popup, **except when the dialog is opened by touch, in which case the popup itself is focused to avoid opening the virtual keyboard**. On close, `finalFocus` **returns focus to the trigger or the previously focused element** by default. That is the Dialog focus contract stated exactly; it is not "Dialog handles focus for you."

## 2.2 The consumer obligations the accessibility page lists

The page's own framing is *"some ways you need to augment the library"* (VERIFIED — `overview_accessibility.md`). There are three listed obligations, plus one conspicuous near-absence.

| # | Obligation (VERIFIED, `overview_accessibility.md` unless noted) | Where v4 meets it |
|---|---|---|
| **A1** | **Visible focus indicator.** Base UI manages focus but never draws it; style `:focus` / `:focus-visible`, WCAG 2.2 focus-appearance cited. ("Focus management") | **MET, unproved.** `packages/design/policy/interaction/focus.mjs` defines `FOCUS_INDICATOR` (outline, 2px minimum, focus-visible activation, 3:1 focused/unfocused change contrast, tokens `semantic.color.focus` / `size.ring` / `size.ring-offset`); `REFUSED_INDICATORS` rejects box-shadow, background-color and opacity as sole indicators; `FOCUS_SURVIVES` requires the ring to survive selected, pressed, error, hover and disabled. Every Base UI-backed adapter applies `STYLE.focus.ring` with `outline-none` resetting the UA outline. **Gap:** `assertFocusBrowserEvidence` and `assertFocusSurvives` both have no caller (VERIFIED — grep over the whole checkout returns only their definitions in `focus.mjs:385` and `:495` and two prose mentions in `policy/interaction/index.mjs:142-147`) — WCAG 2.4.7 (visible), 2.4.11 (not obscured, which the sticky Shell header makes a live question) and 2.4.13 (change contrast) are owed and unrun. |
| **A2** | **Colour contrast** between every foreground and its background; *"consider adhering to APCA"* unless strict compliance with current standards is required. ("Color contrast") | **MET, and stricter.** `packages/design/policy/interaction/accessibility.mjs` `ACCESSIBILITY_POLICY`: text 4.5:1, non-text UI 3:1, plus a **house 3:1 floor for inactive controls that declines WCAG's disabled-control exemption**. Measured instances recorded in components: `resource-boundary.tsx` uses default ink rather than muted on the danger tint because muted measured 4.31:1 (axe, 2026-09-04); `alert.tsx` records 0.5 CIEDE2000 between success and error for a deuteranope, which is why the redundant cue is an icon shape and not chroma. |
| **A3** | **Accessible names for custom controls** via `alt`, `aria-label` or `aria-labelledby`, or `<Field.Label>` / native `<label>`; WAI-ARIA 1.2 name calculation cited. ("Accessible labels"). Extended by `handbook_forms.md:1188-1226`, "Naming form controls", which splits controls into **two** lists. *Input controls* — named by `<Field.Label>` or a native `<label>` — are Input, NumberField, OTPField, Autocomplete, **Combobox (input outside popup)**, Checkbox, Radio, Switch. *Trigger-based controls* are **Combobox (input inside popup): `<Combobox.Label>`**; Select: `<Select.Label>`; Slider: `<Slider.Label>`, and **multi-thumb sliders additionally need `aria-label` on each `Slider.Thumb`**. The documented fallback for either list is *"If no visible label is rendered, provide `aria-label` on the actual form control."* | **MET structurally.** The authored `Field` makes `label` a **required string** and Base UI wires every id. v4's Combobox assembles the input *outside* the popup (`combobox.tsx:170-171` forwards `aria-label`/`aria-labelledby` onto the Input), so it is an **input control** — named by `Field.Label`, a native `<label>`, or `aria-label`, and **not** by `Combobox.Label`, which labels the Trigger. `Switch` forwards `aria-label` / `aria-labelledby` / `aria-describedby`; `Alert` and `Status` are named by the caller. **Gap:** nothing enforces that a control outside a `Field` carries a name — see rule R6 in §7. |
| **A4** | **Motion preferences — no prose guidance and no prop.** `prefers-reduced-motion` is not discussed on the accessibility page and not on any of the **six** handbook pages (VERIFIED by absence — `handbook_animation\|composition\|customization\|forms\|styling\|typescript`), and no component exposes a prop for it. It **does** appear in demo CSS on three component pages, which is the shape of the gate Base UI itself ships: `components_dialog.md:2225` `@media (prefers-reduced-motion: reduce) { transition: none; }` on the popup transition; `components_drawer.md:2158`, `:2192`, `:3273` `transition-duration: 0ms` / `transition: none` under the same query, including on the swipe transform; and `components_tabs.md:481` the opt-in form, `@media (prefers-reduced-motion: no-preference)` gating the indicator's `data-starting-style` translate. Honouring it is still entirely ours — but the pattern does not have to be invented, and the Tabs demo is the directly relevant precedent because it gates a `[data-starting-style]` transition (R10). | **PARTIAL.** `STYLE.motion.state` exists and is applied on inputs. **ASSUMED:** no reduced-motion gate is recorded in any policy module read. This is an open obligation, not a met one. |

Two further obligations come from other pages and belong in the same table because they are equally binding:

| # | Obligation | Where v4 meets it |
|---|---|---|
| **A5** | **Render a `Close` inside modal popups — for two different reasons.** *"When `modal` is `true` or `'trap-focus'` you must render `<Dialog.Close>` inside `<Dialog.Popup>`"* so touch screen readers can escape the popup (VERIFIED — `components_dialog.md`, Root `modal`). Popover's `modal` prop carries **both** clauses and they are not the same requirement (VERIFIED — `components_popover.md:2011`): *"When `modal` is `true`, focus trapping is enabled only if `<Popover.Close>` is rendered inside `<Popover.Popup>`"* — here the Close **enables** the trap — and *"When `modal` is `'trap-focus'`, render `<Popover.Close>` inside `<Popover.Popup>` so touch screen readers can escape the popup"* — here the trap already exists and the Close is the escape hatch. | **N/A today.** No authored modal exists; `keyboard.mjs` records the `modal` profile as DORMANT because Dialog is vendored and unexported. This obligation becomes live with the first Dialog or Popover adapter, and rule R9 in §7 states it — R9's assertion covers both cases. |
| **A6** | **`ref` forwarding.** Custom components passed to `render` must forward `ref` and spread all props (VERIFIED — `handbook_composition.md`); and *"For React Hook Form to focus invalid fields... you must ensure that any wrapping components forward the ref"* via `inputRef` or `ref` on input-rendering parts (VERIFIED — `handbook_forms.md`, "Integrate components"). | **N/A by construction.** No authored component accepts `render`, so no ref is ever handed to a consumer component. v4 uses no external form library. |

## 2.3 v4's policy modules, mapped to the guarantee

The five policy files under `packages/design/policy/interaction/` are v4's own accessibility contract. Their relationship to Base UI is worth stating explicitly, because it is what makes the division of labour legible (all VERIFIED — read of those files).

**`keyboard.mjs`** — the pivot. `KEYBOARD_SUPPLIERS` defines who supplies key handling, and the value `'library'` is glossed in the file as *"Base UI does it — focus traps, `aria-activedescendant`, listbox and option roles, arrow traversal"* (VERIFIED — `keyboard.mjs:63-64`).

> **That gloss is v4's own attribution, not a Base UI claim.** `aria-activedescendant` appears **zero** times across all 78 pages; `role="option"` appears zero times; "listbox" appears three times as prose only — `components_combobox.md:533` (*"accessibility features specific to a listbox without an input"*), `:8088` and `components_autocomplete.md:6156` (grid mode) — never as a documented role emission (all VERIFIED by grep). What *is* documented for Combobox is arrow traversal (`loopFocus`, `components_combobox.md:7225`) and polite announcement from Status and Empty (`:8007`, `:8037`). §5's Combobox row therefore says "Roles NOT DOCUMENTED", and §1's preamble rule applies: the page does not say, so this document does not say. The same wording appears in `combobox.tsx`'s own header under "Contract inherited from the adaptee"; it is a repository assertion in both places.

Profiles, and the subjects each declares:

| Profile | Keys | suppliedBy | Subject — **declared** in `keyboard.mjs` | Subject — **ASSUMED** (the profile an adapter would declare) |
|---|---|---|---|---|
| `none` | — | — | none | Alert, Field, layout components |
| `native-control` | Tab, Enter, Space | platform | `['every tab stop the product routes to']` (line 213) — a population, not a component | Button |
| `form-control` | Tab | platform | **`['TextInput', 'DateInput']`** (line 187) | — |
| `live-region` | — (takes no focus) | platform | none | Status |
| `composite` | Tab, ArrowUp, ArrowDown, Home, End, Enter, Escape; one tab stop; focus may stay put while `aria-activedescendant` moves | **library** | none | Combobox |
| `disclosure` | Escape; takes no focus, appears beside its trigger | **library** | none | DORMANT |
| `modal` | Tab, Escape; focus trapped on open, **returned to the trigger on close** | **library** | none | DORMANT |
| `composite-grid` | roving single tab stop, 2-D Arrow, Home/End, Enter, F2 | **this-system** — *"Base UI ships no grid. Every branch is written in this repository"* | none | DORMANT |

The split matters and is not cosmetic. `keyboard-coverage.test.ts` distinguishes the two kinds deliberately (`isComponentName`, line 37) because **only a declared component name can be held to a file**. `form-control` is the only row whose subjects are enforced; Button, Status, Alert, Field and Combobox appear nowhere in `keyboard.mjs` except inside prose strings (VERIFIED — grep of the file returns four hits, all inside `focus`/`gap` sentences, with 'Combobox' occurring only in the composite gap sentence at line 152). An undeclared subject is invisible to the one coverage check that can fail.

The three `library` profiles are the places where v4 leans on Base UI without a local check — and, per the box above, on behaviour Base UI does not document either. That is why §2.4 below is not optional.

**`focus.mjs`** — meets A1, and additionally records the "whether focus is obscured" question raised by the sticky `Shell` header.

**`states.mjs`** — five state axes (availability, interaction, process, selection, validation), `STATE_COLOR_ROLES`, and a `PROHIBITED_STATE_NAMES` list that bans `active` as a state name. This collides with Base UI exactly once: **`Tabs.Tab` and `NavigationMenu.Link` emit `data-active`** (VERIFIED — `components_tabs.md` Tab data attributes; `components_navigation-menu.md` Link data attributes). `stateFailures` walks token names only, so it cannot see the attribute. Adopting either primitive means deciding how the prohibited word maps to `pressed` / `selected` / `current` — see rule R11.

**`accessibility.mjs`** — meets A2; adds pointer target 24px (WCAG 2.5.8) and touch target 48px, with the rule that *density may compress presentation, never operability*.

**`assistive-technology.mjs`** — A11y-3 evidence: recorded screen-reader sessions on NVDA+Chrome **and** JAWS+Chrome (VoiceOver+Safari a recorded residual risk), each with at/browser name+version, os, ISO date, tester, and per-scenario verbatim "announced" text. **Base UI's own testing claim (VERIFIED, `overview_accessibility.md`) is not evidence under this policy.** The ledger path `.architecture/a11y-evidence.json` does not exist in this checkout and nothing runs `ledgerFailures` as a gate (VERIFIED — file absent).

## 2.4 What is proved, what is measured once, and what is only declared

This is the most important subsection in §2, and the honest answer is uncomfortable. Three categories, not two.

**Proved by test (VERIFIED — `packages/design/tests/`):**

- `switch.test.tsx` (SSR): `role="switch"`, `aria-checked="false"` + `data-unchecked` by default; controlled `checked` → `aria-checked="true"` + `data-checked=""` and no `data-unchecked`; `disabled` → `aria-disabled="true"`, `data-disabled`, `tabindex="-1"`.
- `switch.browser.test.tsx` (Chromium): click toggles and reports the new boolean; **Space toggles from the keyboard**; a controlled switch follows its prop not the click; `readOnly` renders `aria-readonly="true"` and refuses click and Space; `disabled` refuses both; the hidden checkbox carries `name`/`value`/`required` into `FormData` and `checkValidity`, with `aria-required="true"`.
- `combobox.test.tsx` (SSR): exactly **one** input with `role="combobox"` — Base UI also marks its trigger button with the role, which is why the count is asserted; `aria-expanded="false"`; no option in the DOM while closed; Root `disabled` propagates to the input.
- `combobox.browser.test.tsx` (Chromium): typing opens and filters; click selects and reports the id; the input shows the label and closes; Escape closes.

**Measured once, by hand, with nothing holding it:**

- **Field.** `Field.Root` carries `data-invalid` alongside `data-slot` and the class, *"measured in Chromium"* — recorded in a source comment, `packages/design/src/components/field.tsx:96-98`. **There is no Field test.** `packages/design/tests/` holds 26 files and none renders a Field; a grep for "field" across the directory returns only `adapter-schema.test.ts` and `tokens.test.ts`, and the `data-invalid` occurrences in the latter (`tokens.test.ts:1673-1679`) are string equality on generated class names (`interaction.invalid.border === 'data-invalid:not-data-disabled:border-error'`), not a DOM measurement. `field.stories.tsx` exists and mounts under `tests/stories.browser.test.tsx`, but that suite asserts only that a story renders. A header note recording a one-time observation is not a test and nothing re-runs it — which is precisely the distinction this subsection exists to draw, and it applies to the component this document elsewhere calls the owner of the whole accessible relationship.

**Declared but not proved (VERIFIED — every `PROFILE_COVERAGE` entry has `specs: []` and a gap sentence):**

- **`composite`** — *no browser suite exercises arrow traversal or `aria-activedescendant`* for the one authored composite. Typing, filtering, click-select and Escape are proved; the keyboard model that makes it a composite is not. Note the sharper framing forced by §2.3: because Base UI documents neither `aria-activedescendant` nor listbox/option roles anywhere, the open question is not "does Base UI's guarantee hold in this assembly" but **"does the behaviour exist in this assembly at all"**.
- **`form-control`** — no browser case types into a control or tabs through a Field; no case clears a date. The axe run that proved label wiring was a Playwright suite that was not carried over.
- **`live-region`** — whether a reader announces a region that did not exist a moment earlier varies by reader; axe finds the attribute present, which is not the question.
- **Focus ring** — no walk over every tab stop; `assertFocusSurvives` and `assertFocusBrowserEvidence` have no caller.
- **A11y-3** — no ledger file, no gate.
- **Field** — the browser evidence owed alongside the composite and form-control gaps: the `data-invalid` stamp, the accessible relationship (`for`, `aria-describedby`, `aria-invalid`), and what a childless `Field.Error` announces.

**Mechanisms named in the repository that do not exist:**

- `A11Y_LEVELS.mechanism` names `e2e/a11y-conformance.spec.ts` and `e2e/design-system-conformance.spec.ts`; **neither exists** (VERIFIED — files absent; `keyboard.mjs` records that the Playwright suites were not carried over).
- **`keyboard.mjs`'s own header claims a caller it does not have.** Lines 76 and 82-84 state, in the past tense, that `assertProfileKeyboard` *"is now called from `tests/unit/interaction-policy.test.ts`, proven by planting a ninth profile and watching it go red"*, and that *"'Cannot drift' described a function with no caller"* was the old state. `tests/unit/` contains exactly one file, `design-system-classes.test.ts`; no `interaction-policy*` file exists anywhere in the checkout; and `assertProfileKeyboard` appears in the nine module-scope calls in `policy/interaction/index.mjs:176-184` **not at all** — the list runs `assertAccessibilityPolicy`, `assertA11yLevels`, `assertAtPairings`, `assertStateAxes`, `assertProhibitedNames`, `assertStateLayers`, `assertStateColorRoles`, `assertFocusIndicator`, `assertKeyboardCoverage`. Its only caller is `assertKeyboardPolicy` (`keyboard.mjs:427`), which nothing calls either (all VERIFIED by grep). This is the strongest instance of the defect the closing note describes, because the prose claiming the escape is in the file that has not escaped.

**What `keyboard-coverage.test.ts` actually checks** (VERIFIED — read of the file, four `it` blocks):

- *Vacuous today:* "every spec it names is a file" (lines 46-52) iterates `row.specs`, and **all eight `specs` arrays are `[]`** (`keyboard.mjs` lines 155, 169, 176, 186, 194, 201, 208, 220). That assertion cannot fail while they stay empty.
- *Live, and has gone red:* "has rows to hold, and holds a component name somewhere" (lines 41-44) asserts more than three rows and that the flattened subjects contain `'TextInput'`. "every component it names is an authored component" (lines 54-63) asserts `existsSync(join(COMPONENTS, kebab(subject) + '.tsx'))` for every component-shaped subject — with `form-control.subjects = ['TextInput','DateInput']`, renaming or deleting either file turns it red today. "a subject a spec is said to cover through the stories has a story" (lines 65-79) holds story files. The file's own header records the red: *"`form-control` said its subjects were `Input` and `Textarea` … two components that never existed under src/components … Red before the row was corrected."*
- *The real gap, stated narrowly:* **no assertion requires a profile with a non-empty `gap` to eventually acquire a spec.** The debt can sit forever while the file stays green, and no spec exercises any profile's keyboard model.

**ASSUMED, and stated as the honest summary:** v4's accessibility position is *policy-complete and evidence-thin*. The policy modules describe a stricter standard than Base UI's own obligations list. What is missing is the browser evidence that the standard is met — and, for the `composite` profile, evidence that the behaviour exists at all. An agent building on this foundation should treat closing the `composite` and `form-control` gaps, and giving `assertProfileKeyboard` a caller, as higher-value work than adopting another primitive.

---

# 3. The handbook, distilled

For each area: the rule as documented, the pattern v4 standardises on, and what v4's STYLE-symbol model changes about it.

## 3.1 Styling

**The rule (VERIFIED — `handbook_styling.md`).** Four hooks on every rendering part: `className` (string or function of state), `style` (object or function of state), state data attributes, CSS variables. The same Menu renders identically under Tailwind, CSS Modules and CSS-in-JS; the pattern is one class or wrapper per part. Which parts render nothing is not enumerated by the handbook — see §1.1.

**v4's pattern.** Class strings assembled by `cva` recipes over `STYLE` symbols, applied by the adapter. **`className` and `style` never reach a screen**: `NativeProps<T>` is `ComponentProps<T>` minus `className` and `style`, and `packages/design/tests/adapter-schema.test.ts` refuses stray `className` in authored files (VERIFIED — `packages/design/src/lib/props.ts`; that test file).

**What the STYLE-symbol model changes.** Base UI's model is *"here are the attributes, write whatever CSS you like."* v4's model is *"the attribute is real, but the class that responds to it must be a named symbol."* Concretely: `data-invalid:border-error` cannot be hand-typed — **`tests/unit/design-system-classes.test.ts`** (repository root, not the design package) refuses it — so `interaction.invalid.border` was minted as a symbol before the invalid cue could exist (VERIFIED — `text-input.tsx` header, "THE INVALID CUE"). The measurement that forced it is recorded: an invalid input drew exactly like a valid one, `rgb(111,123,133)` either way, even though `aria-invalid` and `data-invalid` were both present.

Note the two enforcement locations, because they differ and the difference has bitten: `packages/design/tests/` holds the adapter, story and token checks; **`tests/unit/design-system-classes.test.ts` is at the repository root** and is the only file in `tests/unit/`. Both are picked up by the unit project through `include: ['**/tests/**/*.test.{ts,tsx,mjs}']` (`vitest.config.ts` line 25).

The generalisable rule: **Base UI's data attribute is the trigger; the STYLE symbol is the response.** A new state cue is a token change, not a class change.

## 3.2 Composition

**The rule (VERIFIED — `handbook_composition.md`).** `render` in element, nested and function forms; forward `ref` and spread props; the default element carries the intended semantics.

**v4's pattern: the Target boundary.** No authored component accepts `render`, `className`, `style`, `nativeButton`, or any type derived from the adaptee. `adapter-schema.test.ts` refuses `ComponentProps<'x'>` in an authored file and wholesale re-exports; its `typeof`-adaptee rule is narrower than that sentence implies and §7 R4 states the real scope. Base UI's own words are adopted **one at a time**: `SwitchProps` lists `checked`, `defaultChecked`, `disabled`, `name`, `onCheckedChange`, `readOnly`, `required`, `value` explicitly rather than re-exporting `Switch.Root.Props` (VERIFIED — `switch.tsx`).

The trade is deliberate and worth naming: v4 gives up polymorphism to gain a closed vocabulary. A `Button` cannot be rendered as an anchor from a screen, because `render` is not in `ComponentProps<'button'>` (VERIFIED — `button.tsx`). When a screen needs a link that looks like a button, that is a new authored word, not a `render` escape hatch. (`components_button.md` independently forbids routing `<a>` through Button — VERIFIED, "Rendering links as buttons".)

**Where composition still happens:** inside the adapter. `Combobox` assembles nine Base UI parts — Root, Input, Portal, Positioner, Popup, Empty, List, Item, ItemIndicator — once, and exposes an `options` array (VERIFIED — `combobox.tsx`). The composition surface is Base UI's; the API surface is v4's. It is also the reason §1.7's portal obligation is live.

## 3.3 Customization

**The rule (VERIFIED — `handbook_customization.md`).** Change events carry `eventDetails`; `cancel()` vetoes a change while keeping the component uncontrolled; `allowPropagation()` releases an event Base UI would stop; `reason` says why; `event.preventBaseUIHandler()` is the escape hatch *"in cases where there isn't a prop yet to customize the behavior"* — and *"in various cases, native events are used instead of React events, so this method has no effect."*

**v4's pattern.** `eventDetails` is **dropped at the boundary**, in every adapter, by wrapping the handler to one argument (VERIFIED — `switch.tsx`, `text-input.tsx`, `date-input.tsx`, `combobox.tsx`). A screen sees `onCheckedChange(checked: boolean)` and `onValueChange(value: string)`.

**ASSUMED, as guidance:** this is the right default and should hold until a real need arrives. When one does — a Dialog that must refuse to close while a form is dirty is the obvious candidate, and `components_dialog.md` documents exactly that pattern under "Close confirmation" — the correct move is to expose a **named** v4 prop whose implementation calls `eventDetails.cancel()` internally, never to widen the handler signature. The reason vocabulary is Base UI's and differs per component (§5); leaking it would put an upstream union in v4's public types.

## 3.4 Animation

**The rule (VERIFIED — `handbook_animation.md`).** Three paths:
- **CSS transitions** via `[data-starting-style]` / `[data-ending-style]` — *"recommended over CSS animations, because a transition can be smoothly cancelled midway."*
- **CSS keyframes** via `[data-open]` / `[data-closed]`.
- **JavaScript libraries**, which *"require control of the mounting and unmounting lifecycle"*.

The JavaScript path is **three recipes, not one**, and the page selects between them on a property of the component — whether the popup unmounts when closed — not on a preference (VERIFIED — the page's own section headings at lines 92, 272 and 444):

1. *"Animating components unmounted from DOM when closed with Motion"* — controlled `open`, `keepMounted` on Portal, `AnimatePresence`, and `actionsRef.current.unmount()`.
2. *"Animating components kept in DOM when closed with Motion"* — the opposite instruction: use the `render` prop to compose the `<Popup>` with `motion.div`, animate on the `open` state, **avoiding `<AnimatePresence>`**.
3. *"Animating Select component with Motion"* — Select *"is initially unmounted but remains mounted after interaction"*, so *"a mix of the two previous approaches is needed."*

An agent handed only the first recipe will wrap a kept-mounted popup in `AnimatePresence`, which the page tells you not to do.

Base UI *"relies on `element.getAnimations()` to detect if animations have finished"* and delays unmount until they do. The consequence is a genuinely surprising instruction: *"If opacity isn't part of your animation (such as in a translating drawer component), you should still animate it using a value close to 1 (such as `opacity: 0.9999`), so that Base UI can detect the animation."* (VERIFIED — same page.)

**v4's pattern.** Motion is a token (`STYLE.motion.state`), and no authored component animates entrance or exit. Upstream's entrance animation, side-aware slide and `min-w-[calc(...)]` were dropped when Combobox was normalised (VERIFIED — `combobox.tsx` header).

**What the STYLE model changes, and the open obligation.** Duration and easing are symbols; `data-starting-style` is a trigger the symbol responds to. **The `prefers-reduced-motion` gate is missing** (§2, A4) and belongs *in the symbol*, not in each recipe — one definition, honoured everywhere. Any agent adding the first entrance animation must add the gate in the same change, and should copy the shape from `components_tabs.md:481`, which gates a `data-starting-style` transition behind `@media (prefers-reduced-motion: no-preference)`.

## 3.5 Forms

**The rule (VERIFIED — `handbook_forms.md`, read whole, 4586 lines).**

- Field.Root owns `name`, `disabled`, `validate`, `validationMode`, `validationDebounceTime`, and accepts `invalid` / `touched` / `dirty` as controlled booleans *"useful when the field state is controlled by an external library."*
- Precedence is stated: **Root `name` over Control `name`; Root `disabled` over Control `disabled` and over Item `disabled`; Root `validationMode` over Form `validationMode`** (VERIFIED — `components_field.md`, props tables).
- Field.Control is optional — *"You can omit this part and use any Base UI input component instead"* (Input, Checkbox, Select, among others) (VERIFIED — `components_field.md`).
- `Field.Error` `match` accepts a `ValidityState` key, `'valid'`, or a boolean; *"Specifying `true` will always show the error message, and lets external libraries control the visibility"* (VERIFIED — `components_field.md:477`). What a **childless** Error renders is not documented (§2.1).
- `Field.Label` takes **`nativeLabel`** (`boolean`, default `true`): *"Whether the component renders a native `<label>` element when replacing it via the `render` prop. Set to `false` if the rendered element is not a label (for example, `<div>`). This is useful to avoid inheriting label behaviors on `<button>` controls (such as `<Select.Trigger>` and `<Combobox.Trigger>`), including avoiding `:hover` on the button when hovering the label, and preventing clicks on the label from firing on the button."* (VERIFIED — `components_field.md:428`; added in 1.1.0, #3723.) Base UI's own grouped-Select-in-a-Field demo writes `<Field.Label nativeLabel={false} render={<div />}>` (VERIFIED — `components_select.md:2151`, and `:1715`).
- `validate` runs only after native validations pass; *"Asynchronous functions are supported, but they do not prevent form submission when using `validationMode="onSubmit"`"* (VERIFIED — same page).
- **`<Form errors>` is shipped server-side error plumbing.** *"You can pass errors returned by (post-submission) server-side validation to the `errors` prop, which will be merged into the client-side field state for display… This should be an object with field names as keys, and an error string or array of strings as the value. **Once a field's value changes, any corresponding error in `errors` will be cleared from the field state.**"* (VERIFIED — `handbook_forms.md:1476ff`, "Server-side validation".) The page documents both the `useState` form and the React `useActionState` / Server Function form, `<Form action={formAction} errors={state.errors}>`.
- Six field state attributes propagate to every part and every control inside: `data-valid`, `data-invalid`, `data-dirty`, `data-touched`, `data-filled`, `data-focused` (VERIFIED — `components_field.md`, data-attribute tables). `valid` is `boolean | null` — **null is the not-yet-validated state, in which neither `data-valid` nor `data-invalid` is present** (VERIFIED — `Field.Root.State`).
- **NOT DOCUMENTED: `aria-invalid`.** The string does not appear on any of the 78 pages (VERIFIED by absence, snapshot-wide). The Field page names no ARIA mechanism at all — only the outcomes (association, validity). `aria-describedby`, by contrast, **is** documented, just not on the Field page: `components_otp-field.md:200` states it as a consumer obligation — *"Optionally, add `aria-describedby` when supporting text should be announced with the field"* — with the Root example at `:205` and nine further demo occurrences; `components_autocomplete.md:3256`/`:3837` use it, and `components_combobox.md:1800` documents the sibling `aria-description` obligation for chips.

**v4's pattern.** `Field` is the only authored component that names something it rendered **by reference**: Base UI generates and wires every id — the label's `for`, the control's `aria-describedby` pointing at description then error (rule, then complaint), `aria-invalid` following `invalid` (VERIFIED — `field.tsx` header; and note that the last of those three is v4's description of behaviour Base UI does not document, which is why §2.4 lists it as owed evidence).

Four v4 decisions depart from the handbook, each for a stated reason:

1. **`error?: string` is the whole invalid model.** Its presence *is* the invalid state — `invalid={error !== undefined}`. One fact, not a boolean plus a string (VERIFIED — `field.tsx`).
2. **`match={true}` always.** The server is the authority on validity: version conflicts, duplicate employee numbers, a date outside an employment period. Base UI's `ValidityState` matching is deliberately bypassed (VERIFIED — `field.tsx` header).
3. **`<Form errors>` is declined.** v4 does not use Base UI's `Form` and does not pass `errors`. **What declining costs, stated so it is not rediscovered as a bug:** `Form errors` clears a field's error automatically once that field's value changes; v4's `error?: string` does not — it stays until the caller sets it to `undefined`. That is a behavioural difference an agent must implement by hand in every screen, and today no adapter does.
4. **`label` is a required string** — never a placeholder, never a node that could contain a second focusable (VERIFIED — `field.tsx`).

The Error element is rendered only when there is an error, so no empty error region ever sits in the accessibility tree (VERIFIED — `field.tsx`).

**A precondition to record before Select is adopted.** The authored Field renders `<Primitive.Label>` unconditionally from a required string (VERIFIED — `field.tsx:180-185`), with no way for a caller to set `nativeLabel={false}`. Today the only control v4 nests in a Field is an input, so the `true` default is correct. The moment a trigger-based control goes inside a Field — Select, or a Combobox rendered with a Trigger rather than an Input — `nativeLabel={false}` is required, and that is a Target-boundary decision: either the Field grows the flag, or the trigger-based control supplies its own label part. Rule R3 in §7 records it as blocking.

**A hazard to record.** The vendored `ui/field.tsx` is **hand-rolled** — `fieldset`/`legend`/`label` with `data-[invalid=true]` and `group-data-[disabled=true]` — and does **not** use Base UI Field (VERIFIED — read of `ui/field.tsx` lines 1–60). It is a competing vocabulary: boolean-valued `data-invalid="true"` versus Base UI's presence-only `data-invalid`. It is unexported and unreachable, so it is an inert second source rather than a live one. It is exactly the defect pattern CLAUDE.md names — *a fact acquires a second source, and the two agree until they do not*.

## 3.6 TypeScript

**The rule.** Types are namespaces on each part. `Props` and `State` are the two core interfaces on every part; change events add `ChangeEventDetails` and `ChangeEventReason`; other exports include `Actions` (the `actionsRef` shape), `ToastObject`, and `useRender.ComponentProps` (VERIFIED — `handbook_typescript.md`, 81 lines, which is the whole of what that page says).

The canonical/alias rule is **not** on the handbook page. It lives in a per-page "Canonical Types" block, present on 39 pages of the snapshot — e.g. `utils_direction-provider.md` (*"Use Canonical when its namespace is already imported; otherwise use Alias"*, `DirectionProvider.Props` / `DirectionProviderProps`) and `components_combobox.md:8490-8502` (VERIFIED). So: `Tooltip.Root.Props` when the namespace is imported, `TooltipRootProps` otherwise.

Generic roots take type parameters, shown in the wrapper demos: `Autocomplete.Root.Props<any>` (`handbook_forms.md:532`), `Combobox.Root.Props<any, any>` (`:634`), `Select.Root.Props<any>` (`:967`), `Slider.Root.Props<any>` (`:1100`) — all VERIFIED. **`Combobox.Root.Props` gains a third `Item` parameter in 1.8.0**: `Combobox.Root.Props<Value, Multiple, Item>` (`components_combobox.md:617`), where *"The third `Item` type parameter is what lets the wrapper accept a `Combobox.createItems()` collection"* (`:623`). Two parameters is correct only on the 1.7.0 pin.

Two 1.7.0 changes touch this directly: render-callback `props` are now typed from the rendered element (#5104), and implementation-only types were stripped from published `.d.ts` files (#5165), so any type reached through internals may no longer be exported (VERIFIED — `overview_releases_v1-7-0.md`, "General changes").

**v4's pattern: the adaptee's types never cross the boundary.** `Props` is the type the handbook recommends for wrapper components — and v4 refuses it, by policy and partly by test (R4). Each prop is re-declared. Every optional is written `| undefined` because of `exactOptionalPropertyTypes` (VERIFIED — `field.tsx`).

Value shapes are v4's, not the adaptee's:

- **Combobox** maps a string id to and from Base UI's item object in both directions, and **throws** on an id the options do not contain: `"Combobox: value X is not one of the N options"` (VERIFIED — `combobox.tsx`). The reason is domain: a stale id or late-arriving options must not silently submit an empty field in a payroll form. `undefined` means uncontrolled, `null` means controlled-empty.
- **TextInput** refuses `type="date"` (that is `DateInput`, ADR-016) and `type="number"` (money is never a JS number, law 19) (VERIFIED — `text-input.tsx`).
- **DateInput** never defaults to today: civil dates derive from the legal entity's IANA zone, law 21 (VERIFIED — `date-input.tsx` header).

**ASSUMED:** this closed-vocabulary approach costs more per adapter than `Props`-spreading and buys a design system whose public types do not move when Base UI's do. Given a pinned version and a small authored surface, the trade is sound. It would not scale to fifty adapters, and if it ever does, that is the moment to revisit — not before.

---

# 4. Utilities and library hooks: when v4 needs each

## 4.1 `useRender`

**What it is (VERIFIED — `utils_use-render.md`).** A hook, `@base-ui/react/use-render`, that gives a custom component the same `render` prop every Base UI component has. It merges internal and consumer props, merges refs, converts `state` to `data-*` attributes, and returns the element (or `null` when `enabled: false`). Default tag is `div` unless `defaultTagName` is set. It is documented as the Base UI equivalent of Radix's `asChild` / `Slot`.

Key parameters: `render`, `ref` (single or array, merged with `props.ref`), `state` (auto-converted to `data-*`), `stateAttributesMapping`, `props`, `enabled`, `defaultTagName`. Types: `useRender.ComponentProps<'tag', State>` for public props, `useRender.ElementProps<'tag'>` for internal defaults.

**It adds no accessibility.** No role, no ARIA, no keyboard, no focus management — a component built on `useRender` owns its own semantics entirely (VERIFIED — the demos pass `type: 'button'` and `aria-label` through defaultProps themselves).

**When v4 needs it: not yet, and probably not soon.** v4's authored components accept no `render` prop by policy (§3.2), and `useRender` exists to implement `render`. The eight vendored files that use it — attachment, badge, breadcrumb, bubble, button-group, item, marker, sidebar — are unreachable (VERIFIED — grep of `ui/*.tsx`).

**ASSUMED — the one plausible future case:** if v4 ever needs a `Link` that a router's `Link` can substitute for, `useRender` with `defaultTagName: 'a'` is the sanctioned mechanism. That is a real need in a Next.js app and today's `Link` (a plain `<a href>`) cannot serve it. Any agent taking it on must read "Render prop and polymorphism" first: the component cannot know what element `render` will produce before hydration, so tag-specific defaults need an explicit signal — the pattern behind Button's `nativeButton`.

## 4.2 `mergeProps`

**What it is (VERIFIED — `utils_merge-props.md`).** `@base-ui/react/merge-props`. Merges up to five prop sets with `Object.assign` semantics (rightmost wins) except three cases: **event handlers are chained rightmost-first**, `className` strings are concatenated **rightmost-first** (`mergeProps({className:'a'},{className:'b'})` → `'b a'`), and `style` objects merge rightmost-wins. It adds `event.preventBaseUIHandler()` to React synthetic events. `mergePropsN` takes an array beyond five, on a documented slower path.

**`ref` is NOT merged** — rightmost ref wins, silently (VERIFIED — same page, "How merging works"). Merge refs through `useRender`'s `ref` array instead.

Two subtleties that bite: handlers returned from a function-form argument are **not** auto-prevented and must check `event.baseUIHandlerPrevented` themselves; and `preventBaseUIHandler()` only exists on React synthetic events — for non-synthetic events (custom value-change callbacks) there is no prevention and all handlers run (VERIFIED — same page).

**When v4 needs it: wherever internal defaults meet consumer props.** The documented trigger is two situations, not one: merging *"is useful when creating custom components, **as well as** inside the callback version of the `render` prop"* (VERIFIED — the "Merging props" section, carried at `utils_use-render.md:225`). Both `useRender` demos on that page call `mergeProps` in the *element* form, merging internal defaults with consumer props (`props: mergeProps<'p'>({ className: styles.Text }, otherProps)`), with no callback render anywhere — including the demo §4.1 reproduces. Since v4 authors no `useRender` component, `mergeProps` has no current call site; it becomes required the moment `useRender` does, in either form.

## 4.3 `DirectionProvider`

**What it is (VERIFIED — `utils_direction-provider.md`).** `@base-ui/react/direction-provider`. Tells descendant components the reading direction so keyboard arrows, positioning and scroll alignment follow RTL. One prop, `direction` (`TextDirection`, default `'ltr'`). **It renders no element and does not touch HTML or CSS** — *"The `dir=\"rtl\"` HTML attribute … must be set additionally"* by the consumer. `useDirection()` reads the current direction, and its documented purpose is re-applying direction to **portaled content**, which escapes the ancestor `dir` attribute.

One 1.7.0 change is worth knowing: *"Remove the implicit `dir` attribute from `<Accordion.Root>`"* (#5117) (VERIFIED — `overview_releases_v1-7-0.md:38`). **ASSUMED:** that no component writes `dir` for you — the release note covers one component in one release and no page states the universal. The operative rule needs no inference anyway: the provider page already says the consumer must set `dir` itself.

**When v4 needs it: when the product ships an RTL locale.** Not before. **ASSUMED:** an HR and payroll platform is a plausible RTL candidate (Arabic and Hebrew markets), so an agent should treat this as a foreseeable requirement rather than an exotic one. The hazard to record now: the provider enables behaviour only. Wrapping without also setting `dir` leaves layout LTR while keyboard behaviour goes RTL — a bug that looks like a Base UI defect and is not.

## 4.4 `CSPProvider`, and the separate `style-src-attr` question

**What it is (VERIFIED — `utils_csp-provider.md`).** `@base-ui/react/csp-provider`, added in v1.1.0. Two props: `nonce` (*"The nonce value to apply to inline `<style>` and `<script>` tags"*, line 110) and `disableStyleElements` (`boolean`, default `false`, line 109), which removes the inline `<style>` tags entirely in favour of your own CSS.

Which components inject inline `<style>` **elements**: `<ScrollArea.Viewport>`, and `<Select.Popup>` / `<Select.List>` when `alignItemWithTrigger` is enabled (line 66). Inline `<script>` tags are opt-in per component, are **not** affected by `disableStyleElements`, and have no disable flag — a nonce is required if any component uses one (the pre-hydration scripts for Tabs indicator and Slider). In 1.7.0 those were excluded from client bundles (#5003) and the Tabs pre-hydration indicator was moved inside streamed Suspense (#5171), so they are SSR-only (VERIFIED — `utils_csp-provider.md`; `overview_releases_v1-7-0.md`).

Under `disableStyleElements` the `.base-ui-disable-scrollbar` rules (`scrollbar-width: none`; `::-webkit-scrollbar { display: none }`) become yours to ship.

**Two triggers, not one — the page separates them and so must this document** (VERIFIED — `utils_csp-provider.md`, "Inline style attributes", lines 86-96):

1. **`CSPProvider` itself** (nonce, `disableStyleElements`) is triggered by adopting **ScrollArea or Select** under a strict CSP. None of the six Base UI-backed adapters injects a `<style>` element today.
2. **The `style-src-attr` policy is triggered by shipping a strict CSP at all, and is live now.** *"`CSPProvider` covers inline `<style>` and `<script>` tags rendered as elements, but it does not cover inline style attributes (for example, `<div style=\"...\">`). The `style-src-attr` directive in CSP governs inline style attributes encountered when parsing HTML from server pre-rendered components."* The remedies are relaxing `style-src-attr`, client-only rendering, or *"Manually unset inline styles… **Any component** can have its inline styles unset, such as `<ScrollArea.Viewport style={{ overflow: undefined }}>`. Note that you'll need to ensure you vet upgrades for any new inline styles added by Base UI components."* The authored Combobox renders a Positioner and a Popup, which position via inline style attributes, so this obligation applies to code already shipped. **ASSUMED:** it must be settled by reading the SSR'd HTML of the existing Combobox, not by assuming absence.

## 4.5 `Combobox.useFilter` and `Combobox.useFilteredItems`

**What they are (VERIFIED — `components_combobox.md`).** Two hooks exported alongside the component: `useFilter` (API reference at `:8326`, exported name at `:8485`, used in demos at `:3399`, `:4058`, `:4411`, `:5157`) and `useFilteredItems` (`:8342`, `:8486`, demos at `:6720`, `:7051`). They exist for the `filteredItems` prop (`:7215`): *"Filtered items to display in the list. When provided, the list uses these items instead of filtering the `items` prop internally. When `items` is also provided, this array must preserve its flat or grouped structure… Use when you want to control filtering logic externally with the `useFilter()` hook."*

The authored Combobox passes `items` and delegates all filtering to Base UI's internal default, so `filter`, `filter={null}`, `filteredItems`, `limit`, `locale` and `virtualized` are all unexposed (VERIFIED — `combobox.tsx`).

**When v4 needs them: when the option list stops being client-filterable** — server-side search, or a roster large enough to need `virtualized`. An employee picker over a large roster is precisely the documented `filter={null}` + `filteredItems` + `useFilteredItems` + `Combobox.Status` path, and it is the obvious HR case, so it is named here rather than left to be rediscovered.

Constraints to carry into that change (all VERIFIED — same page): `filteredItems` must preserve the flat-or-grouped structure of `items`; nullish entries are unsupported; with a `createItems()` collection you pass source items, not derived values; and **`Combobox.Status` and `Combobox.Empty` must remain mounted** — *"This component's root element must remain mounted in the DOM to announce changes consistently across screen readers. Avoid hiding or removing the component itself with `display: none`, `hidden`, `aria-hidden`, or conditional [rendering]"* (`:8007`, `:8037`). The authored adapter already satisfies the last one, by hiding Empty with `group-data-empty/popup:flex` rather than unmounting it (VERIFIED — `combobox.tsx:180-190`). **Record that as a deliberate constraint, not a styling accident** — a future refactor that "simplifies" it into a conditional render silently breaks announcement.

**Summary of the five: none is needed today, each has a named trigger.**

| Utility / hook | Trigger for v4 |
|---|---|
| `useRender` | Authoring a component that must accept `render` (most likely: router-aware Link) |
| `mergeProps` | Follows `useRender` — no independent trigger; needed in both the element and callback forms |
| `DirectionProvider` | Shipping an RTL locale |
| `CSPProvider` | Adopting ScrollArea or Select **under a strict CSP**. The separate `style-src-attr` decision is triggered by the strict CSP alone and applies to the shipped Combobox today (§4.4) |
| `Combobox.useFilter` / `useFilteredItems` | The option list stops being client-filterable — server-side search, or large enough to need `virtualized` |

---

# 5. The component catalog

Every component with a `components_*` page in the snapshot — 37 pages, 37 rows. Columns: parts (the ones that matter for assembly); the accessibility guarantee in one line, as stated; key data attributes and CSS variables; v4 status; and the interaction profile from `keyboard.mjs` that v4 would assign.

Two documented **components** live on `utils_*` pages and have their own small table at the end: `DirectionProvider` and `CSPProvider`. Two cross-component **families** — detached triggers, and Viewport content transitions — are catalogued as shared blocks after the tables, because they recur across more components than a per-row cell can carry.

Status values: **AUTHORED** (a v4 adapter sits on it), **VENDORED** (present in `packages/design/src/components/ui/`, unexported by the package exports map and imported by nothing), **NOT USED** (no file in v4 touches it).

All parts, attributes, variables and guarantees below are VERIFIED from the cited component page, **except where a cell is explicitly marked ASSUMED** (one cell: AlertDialog's modality consequence). Entries marked **(1.8.0)** are documented on the page but are newer than the 1.7.0 pin. All v4 statuses are VERIFIED from the file read. All profile assignments for non-authored components are **ASSUMED** — they are the profile a v4 adapter *would* declare, and nothing has declared them; §2.3 says which subjects `keyboard.mjs` actually declares.

### Form controls

| Component | Key parts | Accessibility guarantee (one line, as documented) | Key `data-*` | CSS vars | v4 status | Profile |
|---|---|---|---|---|---|---|
| **Field** | Root, Label, Control, Description, Error, Item, Validity | Label is *"An accessible label that is automatically associated with the field control"*; Description *"automatically assigns an accessible description"*; Error is *"An error message displayed if the field control fails validation. Renders a `<div>`"* — **what a childless Error renders is NOT DOCUMENTED**. `Field.Label` takes **`nativeLabel`** (default `true`); set `false` when the label renders a non-label element, and required when the labelled control is a button trigger (`Select.Trigger`, `Combobox.Trigger`). ARIA mechanism NOT DOCUMENTED. | `data-valid`, `data-invalid`, `data-dirty`, `data-touched`, `data-filled`, `data-focused`, `data-disabled` on every part; Error adds `data-starting-style`/`data-ending-style` | none | **AUTHORED** — Root, Label, Description, Error | `none` (the wiring component) |
| **Fieldset** | Root (`<fieldset>`), Legend (`<div>`) | Legend *"automatically associated with the fieldset"*; Legend is a `<div>`, not `<legend>` | none documented | none | NOT USED | `none` |
| **Form** | Form (`<form>`) | No form-specific a11y documented; naming and errors live in Field. Carries **`errors`** — server-side errors merged into field state, cleared per field on value change (`handbook_forms.md`, "Server-side validation") | none | none | NOT USED | `none` |
| **Input** | Input (`<input>`) | *"automatically works with Field"* — label association, validation, six state attributes, no configuration | `data-disabled`; + the six inside Field.Root | none | **AUTHORED** (TextInput, DateInput) | `form-control` |
| **Checkbox** | Root (`<span>` + hidden `<input>`), Indicator | Accessible name is the consumer's; Root is a `<span>` *"to support enclosing labels"*; hidden input carries `name`/`value`/`required` | `data-checked`, `data-unchecked`, `data-indeterminate`, `data-readonly`, `data-required`, `data-disabled` | none | VENDORED | `native-control` |
| **CheckboxGroup** | CheckboxGroup (element NOT DOCUMENTED) | Label with `aria-labelledby` + sibling, or Fieldset+Legend; group computes the parent's indeterminate state | `data-disabled` only | none | VENDORED | `native-control` |
| **Radio / RadioGroup** | RadioGroup (`<div>`), Radio.Root (`<span>` + hidden input), Indicator | *"Radio is always placed within Radio Group"*; selection is group-owned; Root has no `checked` prop | `data-checked`, `data-unchecked`, `data-readonly`, `data-required`, `data-disabled` | none | VENDORED | `native-control` |
| **Switch** | Root (`<span>` + hidden `<input>`), Thumb | Accessible name required and consumer-supplied; `readOnly` distinct from `disabled`. Role NOT DOCUMENTED on the page — **v4 measured `role="switch"` + `aria-checked`** | `data-checked`, `data-unchecked`, `data-readonly`, `data-required`, `data-disabled` | none | **AUTHORED** | `native-control` |
| **NumberField** | Root, Group, Decrement, Input, Increment, ScrubArea, ScrubAreaCursor | Arrow keys and Home/End step; Alt→`smallStep`, Shift→`largeStep`; Input's `aria-roledescription` is *"Number field"* and is **not a name** | `data-scrubbing` + the six field flags | none | VENDORED | `form-control` |
| **OTPField** | Root, Input (one per slot), Separator | `id` lands on the first input, later slots derive `{id}-2`…; first takes the field label, the rest get `aria-label` such as *"Character 2 of 6"*. **Consumer obligation:** *"Optionally, add `aria-describedby` when supporting text should be announced with the field"* (`:200`, example at `:205`) — the one place in the form-control set where the ARIA mechanism is named | `data-complete`, `data-filled`, `data-focused` (any slot) | none | VENDORED (input-otp lib) | `composite` |
| **Slider** | Root, Label, Value (`<output>`), Control, Track, Indicator, Thumb (`<div>` + nested `<input type=range>`) | `Slider.Label` auto-associated with the thumbs; **multi-thumb sliders additionally need `aria-label` per Thumb**; `step` on arrows, `largeStep` on PageUp/PageDown and Shift+Arrow | `data-dragging`, `data-orientation`, Thumb `data-index` | none | VENDORED | `form-control` |
| **Toggle** | Toggle (`<button>`) | Two-state button rendering `<button>`. `aria-pressed` and keys NOT DOCUMENTED | `data-pressed`, `data-disabled` (`:252` lists `data-pressed` as the only pressed-state attribute — there is no `data-[state=on]`) | none | VENDORED | `native-control` |
| **ToggleGroup** | ToggleGroup (element NOT DOCUMENTED) | Arrow keys move focus; `loopFocus` wraps; `multiple` allows several pressed | `data-orientation`, `data-multiple`, `data-disabled` | none | VENDORED | `composite` |
| **Button** | Button (`<button>`) | *"Enforces button semantics: role=button, keyboard interaction, disabled state"*; `type="submit"` must be explicit; **never render `<a>` through it** | `data-disabled` | none | **AUTHORED** | `native-control` |

### Overlays

| Component | Key parts | Accessibility guarantee (one line, as documented) | Key `data-*` | CSS vars | v4 status | Profile |
|---|---|---|---|---|---|---|
| **Dialog** | Root, Trigger, Portal, Backdrop, Viewport, Popup, Title (`<h2>`), Description (`<p>`), Close | `modal: true` (default) **traps focus, locks page scroll, disables outside pointer**; focus moves to the first tabbable element, or the popup itself when opened by touch; on close **focus returns to the trigger or the previously focused element**; a `Close` inside the Popup is **required** under `true`/`'trap-focus'` | `data-open`, `data-closed`, `data-nested`, `data-nested-dialog-open`, `data-starting-style`, `data-ending-style` | Popup `--nested-dialogs` | VENDORED | `modal` (DORMANT) |
| **AlertDialog** | same as Dialog | **VERIFIED:** Root exposes neither `modal` nor `disablePointerDismissal` — its props table is defaultOpen, open, onOpenChange, actionsRef, defaultTriggerId, handle, onOpenChangeComplete, triggerId, children, and the word "modal" does not appear anywhere on the page (grep count 0). **ASSUMED:** that this makes it permanently modal and never outside-dismissable — the focus-trap / scroll-lock / outside-pointer semantics are transplanted from Dialog's `modal` prop and the AlertDialog page states no consequence. A browser measurement is owed before any AlertDialog adapter is authored (see R9). | same as Dialog | `--nested-dialogs` | VENDORED | `modal` (DORMANT) |
| **Popover** | Root, Trigger, Portal, Backdrop, Positioner, Popup, Arrow, Viewport, Title, Description, Close | Non-modal by default (`modal` default `false`); under `modal: true` **focus trapping is enabled only if a `Popover.Close` is rendered inside the Popup**; under `modal: 'trap-focus'` the trap exists and a `Close` inside the Popup is required **so touch screen readers can escape**; hover open via Trigger `openOnHover`, delay 300ms | `data-popup-open`, `data-pressed`, `data-side`, `data-align`, `data-instant` (`click\|dismiss\|focus\|trigger-change`) | `--anchor-*`, `--available-*`, `--positioner-*`, `--popup-*`, `--transform-origin` | VENDORED | `disclosure` (DORMANT) |
| **Tooltip** | Provider, Root, Trigger, Portal, Positioner, Popup, Arrow, Viewport | **Visual only.** *"Tooltips are disabled on touch devices"*; the trigger **must** carry an `aria-label` closely matching the tooltip text; never place required information in one | `data-popup-open`, `data-trigger-disabled`, `data-instant` (`delay\|dismiss\|focus`) | `--anchor-*`, `--available-*`, `--positioner-*`, `--transform-origin` | VENDORED | `disclosure` (DORMANT) |
| **PreviewCard** | Root, Trigger (`<a>`), Portal, Backdrop, Positioner, Popup, Arrow, Viewport | The link is the only accessible interface; popup content is **not** touch/keyboard/screen-reader navigable and must not hold unique information | `data-popup-open`, `data-side`, `data-align` | `--anchor-*`, `--available-*`, `--positioner-*`, `--transform-origin` | VENDORED | `disclosure` |
| **Drawer** | Provider, Root, Trigger, SwipeArea, Portal, Backdrop, Viewport, Popup, Content, Title, Description, Close, VirtualKeyboardProvider, Indent, IndentBackground | Extends Dialog's modal/focus/dismissal model; adds swipe dismissal (reason `'swipe'`), snap points, and `--drawer-keyboard-inset` under `VirtualKeyboardProvider`. **Consumer attribute:** `data-base-ui-swipe-ignore` opts a descendant out of swipe dismissal for all input types (`:316`) | `data-swiping`, `data-swipe-direction`, `data-swipe-dismiss`, `data-expanded`, `data-nested-drawer-open`, `data-nested-drawer-swiping` | `--drawer-swipe-movement-x/y`, `--drawer-swipe-progress`, `--drawer-swipe-strength`, `--drawer-snap-point-offset`, `--drawer-height`, `--drawer-frontmost-height`, `--nested-drawers`, `--drawer-keyboard-inset` | VENDORED | `modal` |
| **Toast** | Provider, Portal, Viewport, Root, Content, Title, Description, Action, Close, Positioner, Arrow | Viewport is a **landmark region reachable with F6**; `priority: 'high'` announces urgently, and **only the `title`/`description` strings are announced** — *"Screen readers do not announce any extra content rendered inside `<Toast.Root>`"* (`:4651`). **Consumer attribute:** `data-base-ui-swipe-ignore` on elements inside a toast (`:460`) | `data-expanded`, `data-limited`, `data-swiping`, `data-swipe-direction`, `data-behind`, `data-type` | `--toast-index`, `--toast-offset-y`, `--toast-height`, `--toast-frontmost-height`, `--toast-swipe-movement-x/y`; the Positioner additionally carries `--anchor-*` / `--available-*` / `--transform-origin` | NOT USED | `live-region` |

### Menus and pickers

| Component | Key parts | Accessibility guarantee (one line, as documented) | Key `data-*` | CSS vars | v4 status | Profile |
|---|---|---|---|---|---|---|
| **Menu** | Root, Trigger, Portal, Backdrop, Positioner, Popup, Arrow, Item, LinkItem, Separator, SubmenuRoot/Trigger, Group, GroupLabel, RadioGroup/RadioItem/Indicator, CheckboxItem/Indicator, Viewport | Roving focus by arrow keys (`orientation` picks the axis, `loopFocus` wraps); typeahead via each item's `label`; Escape closes one level (`closeParentOnEsc` for the whole menu). Roles NOT DOCUMENTED | `data-highlighted`, `data-popup-open`, `data-pressed`, `data-side`, `data-align`, `data-instant` | `--anchor-*`, `--available-*`, `--positioner-*`, `--transform-origin`; Viewport `--popup-width` / `--popup-height` | VENDORED (dropdown-menu) | `composite` |
| **Menubar** | Menubar + nested Menu.Roots | `loopFocus`, `orientation`; vertical menubars default `side` to `inline-end` and `align` to `start` | `data-orientation`, `data-has-submenu-open`, `data-modal` | none | VENDORED | `composite` |
| **ContextMenu** | Root, Trigger (`<div>`), + Menu's part set | *"Users may not discover or be able to open a context menu, especially on touch devices or with assistive technology. Always provide visible controls for the actions"* | `data-popup-open`, `data-pressed`, `data-highlighted` | `--anchor-*`, `--available-*`, `--positioner-*`, `--transform-origin` | VENDORED | `composite` |
| **Select** | Root, Label, Trigger, Value, Icon, Portal, Positioner, Popup, ScrollUp/DownArrow, List, Item, ItemText, ItemIndicator, Group, GroupLabel, Separator | *"Prefer `<Select.Label>`, or provide an `aria-label` on `<Select.Trigger>`"*; basic keyboard typeahead; scroll arrows **do not render on touch**. Roles NOT DOCUMENTED. Nesting it in a Field requires `<Field.Label nativeLabel={false}>` (`:2151`) | `data-popup-open`, `data-popup-side`, `data-selected`, `data-highlighted`, `data-placeholder`, `data-side` (incl. **`'none'`**) | `--anchor-*`, `--available-*`, `--transform-origin` (no `--positioner-*`) | VENDORED | `composite` |
| **Combobox** | Root, Label, InputGroup, Input, Trigger, Icon, Clear, Value, Chips/Chip/ChipRemove, Portal, Backdrop, Positioner, Popup, Arrow, Status, Empty, List, Row, Item, ItemIndicator, Separator, Group, GroupLabel, Collection | *"Avoid when not rendering an input: Use Select instead"*. **Accessible name (`:534`):** *"If `<Combobox.Input>` is the form control, label it with a native `<label>` or `<Field.Label>`, or provide an `aria-label` when no visible label is rendered. `<Combobox.Label>` labels `<Combobox.Trigger>` and is intended for the input-inside-popup pattern, where the trigger is the form control."* — v4's assembly puts the input outside the popup, so it is the first case. **`loopFocus` (default `true`) controls only whether arrow traversal wraps at the first and last item; the input is always in the focus loop per the ARIA APG, regardless of the prop** (`:7225`). Status/Empty announce politely and **must remain mounted**. Roles NOT DOCUMENTED — **v4 measured `role="combobox"` on the Input and on the Trigger** | `data-highlighted`, `data-selected`, `data-empty`, `data-list-empty`, `data-popup-open`, `data-side`; `data-popup-side` on Trigger, Input and InputGroup **(1.8.0)**; `data-readonly` on Trigger **(1.8.0, #5418)** | `--anchor-*`, `--available-*`, `--transform-origin` | **AUTHORED** (9 of 27 parts) + VENDORED (20 parts) | `composite` |
| **Autocomplete** | Same as Combobox minus chips/Label/ItemIndicator, plus `mode` | Free-form text; `mode` *"accepts aria-autocomplete values list, both, inline, or none"*; `submitOnItemClick` off by default | `data-highlighted`, `data-empty`, `data-list-empty` | `--anchor-*`, `--available-*`, `--transform-origin` | VENDORED (+ cmdk) | `composite` |
| **NavigationMenu** | Root (`<nav>`), List (`<ul>`), Item (`<li>`), Trigger, Icon, Content, Link (`<a>`), Portal, Backdrop, Positioner, Popup (`<nav>`), Arrow, Viewport | Semantics come from native elements: `<nav>` landmark, real `<ul>`/`<li>`/`<a>`; `Content keepMounted` *"ensures the content is present during server-side rendering for web crawlers"* | `data-active` (Link), `data-activation-direction`, `data-popup-open`, `data-instant`; `data-disabled` on Trigger **(1.8.0, #5521)** | `--anchor-*`, `--available-*`, `--positioner-*`, `--popup-*`, `--transform-origin` | VENDORED | `composite` |

### Disclosure, layout and display

| Component | Key parts | Accessibility guarantee (one line, as documented) | Key `data-*` | CSS vars | v4 status | Profile |
|---|---|---|---|---|---|---|
| **Accordion** | Root, Item, Header (`<h3>`), Trigger (`<button>`), Panel | Header is a real `<h3>`; **roving focus was REMOVED per the APG update** — `loopFocus` and `orientation` are deprecated and *"no longer affect keyboard focus behavior"*; `hiddenUntilFound` makes find-in-page open a closed panel | `data-panel-open`, `data-open`, `data-index`, `data-starting-style`, `data-ending-style` | Panel `--accordion-panel-height`, `--accordion-panel-width` | VENDORED | `disclosure` |
| **Collapsible** | Root, Trigger, Panel | Trigger is a native `<button>`; closed panel hidden with `hidden`, or `hidden="until-found"` for find-in-page | `data-panel-open`, `data-open`, `data-closed`, `data-starting-style`, `data-ending-style` | Panel `--collapsible-panel-height`, `--collapsible-panel-width` | VENDORED | `disclosure` |
| **Tabs** | Root, List, Tab, Indicator, Panel | Arrow keys move focus; `activateOnFocus` false by default so **Enter or Space activates**; `loopFocus` wraps; **a disabled first tab is not skipped during SSR** — pass an enabled `defaultValue`. Roles NOT DOCUMENTED | **`data-active`** (Tab), `data-activation-direction`, `data-hidden`, `data-index` | Indicator `--active-tab-left/right/top/bottom/width/height` | VENDORED | `composite` |
| **Toolbar** | Root, Button, Link, Separator, Group, Input | Arrow keys navigate; disabled items **stay focusable** by default (`focusableWhenDisabled`); *"use only one [input] and place it as the last element of the toolbar"* | `data-orientation`, `data-focusable`, `data-disabled` | none | NOT USED | `composite` |
| **ScrollArea** | Root, Viewport, Content, Scrollbar, Thumb, Corner | Scrolling is native — the Viewport is the scroll container. No roles documented. **(1.8.0: scrollbars no longer take focus, #5430)** | `data-has-overflow-x/y`, `data-overflow-*-start/end`, `data-scrolling`, `data-hovering` | `--scroll-area-overflow-x/y-start/end`, `--scroll-area-thumb-height/width`, `--scroll-area-corner-height/width` | VENDORED | `none` |
| **Separator** | Separator (`<div>`) | *"A separator element accessible to screen readers"* with an `orientation`. Role NOT DOCUMENTED | `data-orientation` | none | VENDORED | `none` |
| **Avatar** | Root (`<span>`), Image (`<img>`), Fallback (`<span>`) | **(1.8.0, #5536)** With `keepMounted`, *"the image is hidden from assistive technology until then, so the fallback provides the accessible name on its own"* — the prop does not exist on the 1.7.0 pin | `data-error`, `data-loading`, `data-starting-style`, `data-ending-style` | none | VENDORED | `none` |
| **Meter** | Root, Label, Track, Indicator, Value | Root *"provides the value for screen readers"*; `getAriaValueText` gives a human-readable alternative for `aria-valuenow`. Role name NOT DOCUMENTED | **none — no data attributes on any part** | none | NOT USED | `none` |
| **Progress** | Root, Label, Track, Indicator, Value | Root *"provides the task completion status to screen readers"*; `value: null` means indeterminate. Role name NOT DOCUMENTED | `data-complete`, `data-indeterminate`, `data-progressing` on **every** part | none | VENDORED | `none` |

### Providers (documented on `utils_*` pages, no `components_*` page)

| Component | Props | State | Renders | v4 status |
|---|---|---|---|---|
| **DirectionProvider** | `direction` (`TextDirection`, default `'ltr'`) | empty | **no element**; does not touch HTML or CSS — the consumer must also set `dir` | NOT USED (§4.3) |
| **CSPProvider** | `nonce` (string), `disableStyleElements` (boolean, default `false`) | empty | **no element**; covers `<style>`/`<script>` **elements** only, never `style=""` attributes | NOT USED (§4.4) |

### Shared block: detached triggers

Seven pages document an imperative open/close API that no per-component row above carries, and it is the same API on each: **`createHandle()`**, the **`Handle`** object (`isOpen`, `open`, `openWithPayload`, `close`), the **`handle`** prop on Root and Trigger, **`payload`** with a function-as-child Root, and **`triggerId` / `defaultTriggerId`** to say which trigger a popup is associated with (VERIFIED — `createHandle` occurs 15× on `components_dialog.md`, 15× alert-dialog, 15× popover, 16× tooltip, 16× preview-card, 15× menu, 9× drawer; `triggerId` and `defaultTriggerId` are in every one of those Root props tables).

It is the mechanism for opening a popup from somewhere other than an adjacent trigger — a toolbar button, a row action, a keyboard shortcut. Nothing in v4 uses it, and no authored adapter could today, because `handle` is an adaptee type and would have to become a v4 word first (§3.2). Recorded here so that the next agent looking for "open this dialog from over there" finds the sanctioned API instead of hoisting `open` state.

### Shared block: Viewport content transitions

Popover, Tooltip, PreviewCard and Menu each expose a `Viewport` part, listed as a bare part name in the rows above. It is not decoration — it is the documented mechanism for direction-aware transitions when the *content* changes while the popup stays open (VERIFIED — `components_popover.md:1283-1292`):

- The Viewport renders a `div` with **`data-activation-direction`**, *"a space-separated set of up to two tokens (one per axis) — `left` or `right` for the horizontal axis and `up` or `down` for the vertical axis (for example, `right down`). Match a single token with the `~=` attribute selector, such as `[data-activation-direction~='right']`."*
- Inside it, the content is wrapped in `div`s carrying **`data-current`** (*"the currently visible content when no transitions are present or the incoming content"*) and **`data-previous`** (*"the outgoing content during a transition"*).
- The Popup's **`--popup-width` / `--popup-height`** are what freeze the box during the swap; the demos read them as `w-[var(--popup-width,auto)] h-[var(--popup-height,auto)]` on the Popup and `[&_[data-previous]]:w-[var(--popup-width)]` on the outgoing wrapper (VERIFIED — `components_popover.md:34`, `:76`, `:1338`; `components_menu.md:3638`, `:3653`, `:3662`).

The Positioner and Popup additionally carry `data-instant` to disable transitions during the initial positioning (`components_popover.md:1336`, `:2303`, `:1733`).

`data-activation-direction` also appears on NavigationMenu and Tabs, where the rows above already list it.

### The vendored tree, stated plainly

`packages/design/src/components/ui/` holds **59 files** (VERIFIED by count — the brief's figure of 55 is stale). Of those, 29 wrap a Base UI primitive, 8 use only `useRender` + `mergeProps`, 15 are plain elements, 7 wrap third-party libraries (react-day-picker, embla, recharts, cmdk, input-otp, react-resizable-panels, `@shadcn/react`).

**Nothing in it is reachable** (VERIFIED): `package.json` maps `"./components/ui/*": null`; no authored file imports it; `tokens.test.ts` computes the reachable closure from authored imports and records it **empty**; `stories.test.ts` refuses `components/ui/` in any story; `adapter-schema.test.ts` refuses `ComponentProps<'` and stray `className` in authored files.

Its styles are upstream's hand-typed Tailwind — `h-8`, `px-2.5`, `ring-3 ring-ring/50`, `disabled:opacity-50`, `bg-black/10` backdrops — every one of which v4's policy refuses (composited colour, opacity-as-disabled, box-shadow-style focus).

It also carries **dead Radix-era selectors Base UI never emits**. Found by grepping `data-\[state=` over `ui/*.tsx`, which returns nine hits in six files; **three are Radix leftovers on a Base UI-backed component** (VERIFIED):

- `ui/tooltip.tsx:53` — `data-[state=delayed-open]`
- `ui/navigation-menu.tsx:148` — `data-[state=hidden]` / `data-[state=visible]`
- `ui/toggle.tsx:9` — `aria-pressed:bg-muted data-[state=on]:bg-muted`, in a file whose line 3 is `import { Toggle as TogglePrimitive } from "@base-ui/react/toggle"`. Base UI's Toggle API table lists **`data-pressed`** as the only pressed-state attribute (`components_toggle.md:252`); `data-[state=on]` is Radix's word, and §5's Toggle row above is the correct one.

The remaining hits are component-authored states, not leftovers: `ui/attachment.tsx` (`data-[state=error|idle|done|processing|uploading]`, its own upload lifecycle), `ui/sidebar.tsx:310` (`peer-data-[state=collapsed]`, the sidebar's own state), `ui/table.tsx:60` (`data-[state=selected]`, the row's own). Stated as an enumeration with its method, because an exhaustive-looking list that is not exhaustive is the same second-source defect the closing note describes.

**ASSUMED, as directive guidance:** treat the vendored tree as a *reference for which Base UI parts exist and how they assemble*, never as a starting point for a v4 component. Adopting a primitive means authoring a new adapter on it with a v4 recipe. Copying a vendored file into the authored tree would import upstream's refused styling wholesale.

---

# 6. Version: 1.7.0 pinned, 1.8.0 shipped today

v4 pins **1.7.0** (VERIFIED — §1.6). **1.8.0 released 2026-09-04** (VERIFIED — `overview_releases.md`, "Timeline"), which is today. The documentation snapshot is 1.8.0's (see "How to read this document").

### 6.1 The headline

**Neither the 1.7.0 nor the 1.8.0 release notes contain a single labelled breaking change** (VERIFIED — `overview_releases_v1-7-0.md`, `overview_releases_v1-8-0.md`).

**Do not read that as "nothing has broken since 1.0."** Three later minors carry labelled breaking changes, and the per-release pages are the record — `overview_releases.md`'s Timeline shows breaking bullets only at v1.0.0-rc.0 because it is a summary (all VERIFIED):

- **1.3.0** (`:65`) — *"**Breaking change:** `Drawer` is no longer marked as preview"*; it *"should be imported as `{ Drawer } from '@base-ui/react/drawer'`"* (#4293).
- **1.5.0** (`:79`) — *"**Breaking change:** Rename `sanitizeValue()` to `normalizeValue()` and allow composing with validation"* (#4717).
- **1.6.0** (`:128`) — *"🚨 **Breaking change:** Unmark preview"* — the namespace export renamed `OTPFieldPreview` → `OTPField` (#5029).

The lesson for the next upgrade is procedural: read the minor's own notes, not the timeline.

Separately, several 1.8.0 fixes are **observable as breaking by a test or a CSS selector**, which is the category that matters to a design system with browser tests.

### 6.2 New APIs in 1.8.0

| Change | PR | Touches a pattern above? |
|---|---|---|
| Combobox `createItems` collection API (+ the third `Item` type parameter on `Combobox.Root.Props`) | #5326 | Yes — §3.6 value shapes and TypeScript generics. An alternative to v4's hand-rolled id↔object mapping. |
| `keepMounted` on `<Avatar.Image>` | #5536 | Yes — enables `next/image` and native lazy-loading. |
| Toast updates may be a function of the current toast | #5464, #5611, #5629 | No (Toast not used). |
| `data-disabled` on `<NavigationMenu.Trigger>` | #5521 | No (not used). |
| `data-readonly` on `<Combobox.Trigger>` | #5418 | Marked in §5; v4 exposes no `readOnly`. |

### 6.3 Behaviour changes between 1.7.0 and 1.8.0 that touch a pattern in this document

All entry titles VERIFIED — `overview_releases_v1-8-0.md`. Consequences beyond the quoted title are marked.

**Accessibility-tree changes (§2, §5):**
- `aria-orientation` **moved to the role owners** in Autocomplete, Combobox, Menu, Select (#5551).
- Group labels and scrollbars **hidden from the accessibility tree** in Autocomplete, Combobox, Menu, ScrollArea, Select (#5598).
- Groups render with role `rowgroup` in grid mode for Autocomplete/Combobox (#5564).
- Menubar a11y tree restructured to satisfy `aria-required-children` (#5058).
- **ScrollArea scrollbars no longer steal focus** (#5430).

**Field and forms (§3.5) — the densest cluster, and the one that touches the authored tree:**
- Custom validity ownership and validation lifecycle reworked (#5449).
- Controlled value changes now synced to field state (#5460).
- Stale/duplicated control IDs fixed for Checkbox/CheckboxGroup/Field (#5457).
- **Validate once on Enter inside a Form** (#5459).
- **Neutral validity published while async validation is in flight** (#5600) — i.e. `valid: null`, neither `data-valid` nor `data-invalid`.
- Controlled blur validation and stale `filled` state fixed for Checkbox/Field/Switch (#5563).
- Select root ID respected inside a Field (#5461).
- *"Fix `clearErrors` dropping updates when multiple fields change at once"* (#5446, line 85) — a **Form** fix, and therefore squarely in the machinery behind `<Form errors>` and the external-error round trip §3.5 describes. v4 declines `<Form errors>`, so it is not reached today; it becomes relevant the moment decision 3 in §3.5 is revisited.

**Overlays (§5):**
- Dialog/AlertDialog/Popover **ignore outside clicks from presses that began before open** (#5378).
- NavigationMenu keeps focus on the trigger when opening (#5479).
- *"Fix pointer-events lock when sweeping quickly across a trigger"* — NavigationMenu (#5454, line 99).

**Combobox (§5, AUTHORED):**
- Read-only popups may now **open and be browsed** in Autocomplete/Combobox/Select (#5541, #5531).
- `data-readonly` applied to the Trigger (#5418).
- Record label lookup no longer reads `Object.prototype` (#5518, also Select).
- Multiple selection anchored to the first selected item (#5573, #5613).
- Filter-preservation cancellation (#5362).

**General changes — unscoped, and the notes do not describe their blast radius:**
- *"Fix hover and focus interaction issues"* (#5037, line 21). Touches obligation A1 (the focus indicator) and R2's `data-highlighted` styling on the authored Combobox. **ASSUMED:** the entry names no component and no symptom.
- *"Reduce animation completion work"* (#5535, line 18). Touches the `element.getAnimations()` unmount-detection contract quoted in §3.4. **ASSUMED** as above.
- *"Register passive touch listeners"* (#5572, line 26) — that is the entry **in full**. It concerns listeners **Base UI registers**, not a consumer's own handlers, and the page states no consequence. **ASSUMED:** any claim that `preventDefault` in touch handlers is now ineffective. If that matters to v4 it is a browser measurement, not a documentation lookup.
- *"Improve trigger mount performance"* (#5426), *"Fix disabled anchor tracking on scroll"* (#5478), *"Remove duplicate options key from arrow middleware"* (#5606).
- **Prop and ref merging fixed for lazy `render` elements** (#5562) — touches `useRender` (§4.1).
- Transform origin fixed for start/end alignment (#5015) — touches the animation transform-origin pattern (§3.4).
- Label association when a control unregisters (#5456); roving focus when items are added or removed (#5447).
- *"Prevent update loops from unstable refs"* — Slider (#5441, line 131) and Tabs (#5441, line 139); *"Consider 3D transforms when positioning the indicator"* — Tabs (#4852, line 140), which touches §5's `--active-tab-*` variable family.

### 6.4 Recommendation

**Upgrade to 1.8.0, in a dedicated change, before authoring the next Base UI-backed component.**

The reasoning, in order:

1. **The Field cluster is a bug-fix cluster, and v4 sits squarely on Field.** #5457 (stale/duplicated control IDs for Field), #5563 (stale `filled` state), #5460 (controlled value sync) and #5459 (validate once on Enter) are all defects in exactly the machinery the authored `Field`, `TextInput`, `DateInput` and `Combobox` depend on. Staying on 1.7.0 keeps known defects for no benefit.

2. **The a11y-tree changes are net-positive and land in Combobox.** #5598 (group labels and scrollbars out of the tree) and #5551 (`aria-orientation` to role owners) improve exactly the surface v4's one composite exposes.

3. **Nothing is labelled breaking, and the suite's assertions do not intersect the *listed* changes — but the general fixes are unscoped.** Reading the changed behaviours against `switch.test.tsx`, `switch.browser.test.tsx`, `combobox.test.tsx` and `combobox.browser.test.tsx`: the Switch assertions are role, `aria-checked`, `data-checked`/`data-unchecked`, `data-disabled`, `tabindex`, `aria-readonly`, `aria-required` and FormData — none appears in the component-scoped 1.8.0 list. The Combobox assertions are the `role="combobox"` count, `aria-expanded`, absence of options while closed, disabled propagation, typing/filtering/click-select/Escape — of these only #5551 and #5598 touch Combobox's tree, and neither is asserted. **However**, #5037 (hover and focus interaction issues) and #5535 (animation completion work) are general fixes whose blast radius the notes do not describe, so the reading cannot settle it. **The browser project is the evidence, not the reading** — run it, do not assume it. That is the point of §7's rule R12.

4. **The one thing to verify by hand** is the read-only change (#5541, #5531). v4's Combobox does not expose `readOnly`, so it should not be reachable — but the change alters popup behaviour under a state the adapter does not currently model, and *"does not expose"* is not the same as *"cannot occur"*.

**Do not adopt `createItems` in the same change.** It is a genuine alternative to the hand-rolled id↔object mapping in `combobox.tsx`, and it carries its own constraints — `getValue` must return a unique primitive; items cannot have an `items` array property (it would be read as groups); the collection must be static at module scope or memoised; and the wrapper's `Combobox.Root.Props` needs the third `Item` parameter (VERIFIED — `components_combobox.md`, `:617-655`). Evaluating it is worth a separate decision, taken against the throw-on-unknown-id behaviour that the current mapping exists to provide.

**Upgrade procedure:** change the pinned version in `pnpm-workspace.yaml` (one line — the catalog is the single source), then run the fast loop with every exit code read directly: `pnpm exec biome ci .`, `pnpm exec tsc --noEmit -p tsconfig.json`, `pnpm exec vitest run --project unit`, `pnpm exec vitest run --project browser`, and `pnpm gen:tokens && git diff --exit-code packages/design/generated`. The browser project is the one that matters here, because the changes are behavioural.

---

# 7. How v4 uses Base UI: rules an agent can follow, checks that verify them

Each rule states the obligation, its source, and the mechanism that enforces it. Where the mechanism is **prose**, that is a gap and is labelled one.

---

**R1. Every interactive authored component sits on a Base UI part, or the file header documents why not.**

*Source:* Base UI owns ARIA, roles, pointer interaction, keyboard and focus management (VERIFIED — `overview_accessibility.md`). Reimplementing any of that is a second source for a fact Base UI already owns.

*Current state (VERIFIED):* six of twenty-three sit on Base UI. Seventeen do not, and every one of them is something Base UI **does not ship**: Status (Base UI ships no general-purpose live region — see §8 #9), Alert (no message region), Link (Base UI ships only `NavigationMenu.Link`), Heading, List, ListItem, Card, EmptyState, Shell, Grid, Stack, Page, Text, Code, Specimen, Swatch, ResourceBoundary. **No authored component reimplements a Base UI primitive.**

*Check:* **prose**, enforced by header convention. A check is buildable: a file exporting a component with an `on*` handler or a `disabled` prop, importing nothing from `@base-ui`, and lacking a header line naming the reason, is a violation. **ASSUMED:** worth writing when the authored surface next grows.

---

**R2. Styling reads `data-*` attributes and CSS variables, never internal state.**

*Source:* data attributes are *"designed for styling their states"* and CSS variables *"aid in styling"* (VERIFIED — `handbook_styling.md`). Nothing else in a Base UI component is a public styling surface.

*Current state (VERIFIED):* Switch styles `data-checked` / `data-unchecked` / `data-disabled`; Combobox styles `data-empty`, `data-highlighted`, `data-disabled`, `data-invalid`; TextInput and DateInput style `data-invalid`. `--anchor-width` and `--available-height` are consumed in Combobox as *layout plumbing with no design value* — the STYLE plane names neither, correctly.

*Check:* **`tests/unit/design-system-classes.test.ts`** (repository root — VERIFIED). A hand-typed `data-invalid:border-error` is refused: `isThemed` (lines 375-382) strips everything before the last `:`, so the literal reduces to `border-error`, matches the `border-` prefix, is not in `BUILT_IN`, and is reported by the `writes no design-bearing class literal` case at line 468. The class must come from a STYLE symbol. This is the strongest check in the repository and it is why the invalid-cue fix produced a token rather than a class.

---

**R3. Forms use Field for labelling and error wiring. Nothing hand-writes an id, a `for`, an `aria-describedby` or an `aria-invalid`.**

*Source:* Field *"automatically associates"* the label, description and error (VERIFIED — `overview_accessibility.md`; `components_field.md`).

*Current state (VERIFIED):* the authored `Field` is the only place labels are wired, and it names Base UI's output by reference — it writes no id. `label` is a required string.

*Blocking decision before Select is adopted:* the authored Field renders `<Primitive.Label>` unconditionally with `nativeLabel` at its default `true` (`field.tsx:180-185`). Any trigger-based control placed inside it — `Select.Trigger`, or a Combobox rendered with a Trigger — requires `nativeLabel={false}`, or the label's click and `:hover` behaviour leaks onto the button (VERIFIED — `components_field.md:428`; Base UI's own demo at `components_select.md:2151`). Either the Field grows the flag, or the trigger-based control supplies its own label part. That is a Target-boundary decision and must be taken before, not during, the Select adapter.

*Check:* **prose.** A grep-level check is easy and worth having: `htmlFor=`, `aria-describedby=` or `aria-invalid=` in an authored component file is a violation unless the file *is* `field.tsx`. **ASSUMED** — not present today.

---

**R4. `className`, `style`, `render` and adaptee-derived types never cross into a screen's vocabulary.**

*Source:* v4's Target boundary (§3.2).

*Check:* `packages/design/tests/adapter-schema.test.ts` — **partial, and the parts differ in strength** (VERIFIED — read of the file):

- *Live for every authored file:* `className` may appear only as the attribute the adapter sets (the `\bclassName\b(?!=)` case), and intrinsic props must come through `NativeProps`, never `ComponentProps<'…'>` (the `/ComponentProps<'/` case at line 109 — note it matches only the **quoted intrinsic** form). Wholesale `export * from '#components/ui/…'` is refused.
- *Blind for the six files it is most about:* the typeof-adaptee rule builds its identifier list from `adapteeIdentifiers` (lines 37-57), which matches **only `import … from '#components/ui/…'`**, and the case returns early when that list is empty (lines 72-74). None of the six Base UI-backed adapters imports from `#components/ui/`, so `export type SwitchProps = ComponentProps<typeof Primitive>` in `switch.tsx` would pass every case in the file.
- *Not checked at all:* `render` and `nativeButton`. They are excluded from Button structurally by `NativeProps<'button'>` (`lib/props.ts:13`), and from Switch and Combobox **only by hand-declared prop lists**.

*The one-line fix that would make R4 as strong as this document once claimed:* widen `adapteeIdentifiers` to also match `@base-ui/react` specifiers. Watch it go red on a planted `typeof Primitive` export first.

---

**R5. Every forwarded Base UI handler is wrapped to one argument.**

*Source:* Base UI calls handlers `(value, eventDetails)` (VERIFIED — `handbook_customization.md`). Forwarding a callback straight through leaks `eventDetails` into v4's public API.

*Current state (VERIFIED):* done in all four adapters that forward a handler.

*Check:* **prose.** A type-level check is available and cheap: declaring the v4 prop as `(value: T) => void` makes a straight pass-through a type error under `strictFunctionTypes`. **ASSUMED:** the current adapters get this right by wrapping; the type already documents the intent.

---

**R6. Every interactive control has an accessible name, from a Field label or an explicitly forwarded ARIA attribute.**

*Source:* obligation A3 (VERIFIED — `overview_accessibility.md`, "Accessible labels"; `handbook_forms.md:1188-1226` for which mechanism names which control).

*Current state (VERIFIED):* Combobox forwards `aria-label` / `aria-labelledby` onto its Input — which is the documented mechanism for its shape, since the input is outside the popup; `Combobox.Label` would name a Trigger v4 does not render. Switch forwards `aria-label` / `aria-labelledby` / `aria-describedby`; TextInput and DateInput pick `aria-label` / `aria-labelledby` / `aria-describedby` from the native input. Field supplies the name when present.

*Check:* **prose, and this is the sharpest gap in §7.** Nothing prevents a screen rendering a bare `<Switch>` with no label and no `Field`. The right mechanism is an axe run over the rendered stories in the browser project — which also closes part of the A11y-2 gap. **ASSUMED** — not present.

---

**R7. Base UI is imported from the package root unless the subpath is known pre-bundled.**

*Source:* the second-React failure (VERIFIED — `text-input.tsx` header). Root: Input, Field, Combobox. Subpath: Button, Switch.

*Check:* **prose, held by four file headers.** This is the highest-risk unguarded rule in the document: the failure mode is sixteen red story mounts with an error (*"Invalid hook call"*) that names neither Base UI nor the import. Two mechanisms would work — a lint rule refusing `@base-ui/react/` subpaths outside an allowlist, or `optimizeDeps.include` in `vitest.config.ts` listing every subpath used. **ASSUMED: the lint rule is preferable**, because it fails at author time with a message that names the cause, whereas `optimizeDeps` fixes the symptom and leaves the next agent to rediscover the rule.

---

**R8. Every Base UI-backed adapter carries `'use client'`.**

*Source:* every stateful Base UI demo is a `'use client'` file (VERIFIED — `handbook_animation.md`, `handbook_forms.md` demos).

*Current state (VERIFIED — line 1 of all 23 authored components printed):* present on **five of the six** Base UI-backed adapters — Combobox, DateInput, Field, Switch, TextInput — plus ResourceBoundary. **`button.tsx` is the exception and the one live violation:** its line 1 is `import { Button as Primitive } from '@base-ui/react/button'`, and the directive appears nowhere in the file. Button is not a native-element adapter; it sits on `@base-ui/react/button`.

*Why nothing has broken:* the installed package ships its own boundary — `packages/design/node_modules/@base-ui/react/button/Button.mjs` begins with `'use client'` (VERIFIED) — so the adapter inherits a client boundary it does not declare. That inheritance is real but undeclared, and it is what the rule exists to make explicit.

*Check:* **prose.** Trivially checkable: a file importing `@base-ui` without `'use client'` on line 1 is a violation. Writing it is therefore a **RED-first change** — the check goes red on `button.tsx` the moment it exists, and the commit that adds the directive is the one that turns it green. **ASSUMED** — worth adding, in that order.

---

**R9. A modal popup renders its Close inside its Popup.**

*Source:* required under `modal: true` or `'trap-focus'` for touch screen readers (VERIFIED — `components_dialog.md`); for Popover, `true` makes the Close the thing that *enables* the trap and `'trap-focus'` makes it the escape hatch (VERIFIED — `components_popover.md:2011`, both clauses).

*Current state:* not applicable — `modal` and `disclosure` are DORMANT profiles (VERIFIED — `keyboard.mjs`).

*Check:* **prose, and it must become a browser assertion when the first Dialog, AlertDialog or Popover adapter lands.** The assertion is concrete and covers both Popover cases: open the popup, assert a focusable Close exists within the Popup subtree, Tab to the end and assert focus does not escape, close and assert focus is on the trigger. For AlertDialog it additionally settles the ASSUMED cell in §5 — whether a Root with no `modal` prop is in fact permanently modal and non-dismissable.

---

**R10. Base UI's animation attributes are triggers; the response is a STYLE symbol, and the reduced-motion gate lives in the symbol.**

*Source:* `[data-starting-style]` / `[data-ending-style]` are the sanctioned transition mechanism (VERIFIED — `handbook_animation.md`). Base UI publishes **no prose guidance and no prop** for `prefers-reduced-motion`, but its own demos ship the gate — `components_dialog.md:2225`, `components_drawer.md:2158`/`:2192`/`:3273`, and, most relevantly, `components_tabs.md:481`, which gates a `data-starting-style` translate behind `@media (prefers-reduced-motion: no-preference)` (VERIFIED). Copy that shape rather than inventing one.

*Check:* **`tests/unit/design-system-classes.test.ts`** covers the symbol half. The reduced-motion half is **prose and unmet** (§2, A4).

---

**R11. `data-active` is Base UI's word, not v4's, and adopting a primitive that emits it requires an explicit mapping.**

*Source:* `states.mjs` `PROHIBITED_STATE_NAMES` bans `active`; `Tabs.Tab` and `NavigationMenu.Link` emit `data-active` (VERIFIED — both pages).

*Current state (VERIFIED):* no authored component touches either primitive, so no collision exists yet. `stateFailures` walks token names only and **cannot see the attribute** — this is a check that is blind to the case it would need to catch.

*Check:* **prose.** The mapping decision (`pressed`, `selected`, or `current`) must be made in the adapter and recorded in its header before either primitive is adopted.

---

**R12. After any change that can alter a verdict, the fast loop runs on this machine with every exit code read directly.**

*Source:* CLAUDE.md, "Verification". `pnpm exec biome ci .`; `pnpm exec tsc --noEmit -p tsconfig.json`; `pnpm exec vitest run --project unit`; `pnpm exec vitest run --project browser` when behaviour changes; `pnpm gen:tokens && git diff --exit-code packages/design/generated`.

Two habits the loop cannot enforce and that apply directly to work on this foundation: a new check is **watched failing** before the code that satisfies it lands (R8 and R13 are both available as RED-first changes today), and exit codes are never read through a pipe into `grep` or `tail` — `cmd | tail -1` reports `tail`'s status.

One repository-specific hazard: this checkout is shared, and the fast loop can go red from a peer's in-flight files. Scope checks to your paths and say whose red it is.

---

**R13. A portaled popup paints above page content because of the app's isolation root, not because of a z-index token.**

*Source:* *"Wrap the app in a root element with `isolation: isolate`"* so portaled popups sit above page content *"and none of your z-index values interfere"* (VERIFIED — `overview_quick-start.md`, "Set up > Portals").

*Current state (VERIFIED, and it is a live violation — §1.7):* the authored Combobox renders `Portal > Positioner > Popup` (`combobox.tsx:177-216`); the Positioner carries `cn('isolate', STYLE.layer.overlay)`, i.e. a stacking context for the popup's own children plus a z-index token (`generated/style.ts:114-116`); and no isolation root exists — `apps/web/app/layout.tsx` wraps nothing, and `apps/web/app/globals.css` declares no `isolation`.

*Check:* **a browser assertion, and it should be written RED first.** Render the Combobox inside a page element that establishes its own stacking context (a `transform`, a `filter`, or a positioned element with a competing `z-index`), open the popup, and assert it still paints above. On today's tree that is the failing case; adding `isolation: isolate` to an app root is the change that turns it green. If instead the decision is to keep `layer-overlay`, §1.7 must record why, and what would break it.

---

**Summary of enforcement.** Thirteen rules. **Two and a half have real checks:** R2 (whole), R12's regeneration diff (whole), and R4 (partial — the `className` and intrinsic-`ComponentProps` halves are live for every authored file; the adaptee-type half is blind to `@base-ui/react` imports, and `render`/`nativeButton` are held by hand-written prop unions alone). The rest are prose.

The prose rules whose failure modes are worst are **R7** (second React — an inscrutable error, sixteen red mounts) and **R6** (unnamed control — a silent accessibility defect that no current test can see). Two more are cheap and are **available as RED-first changes today, which makes them the best next commits**: **R8** (goes red on `button.tsx` immediately) and **R13** (goes red on the shipped Combobox immediately). If an agent has budget for one check, R7 is the cheapest fix; if for evidence, R6's axe run also pays down the `form-control` and focus-ring gaps in §2.4.

---

# 8. What Base UI does NOT do, that we must

The union of every `leftToUs` across the documentation groups, deduplicated, each with its home in v4. These are obligations no upgrade will satisfy — with the caveat that "Base UI does not do this" is a claim that must be re-checked against the pages, not inherited; entry #19 was wrong in an earlier revision for exactly that reason.

### Visual and stylistic

| # | Obligation | Source | Where it lives in v4 |
|---|---|---|---|
| 1 | **The visible focus indicator.** Base UI manages focus, never draws it. | `overview_accessibility.md`, "Focus management" | `policy/interaction/focus.mjs` + `STYLE.focus.ring`, applied by every recipe. **Browser evidence owed; `assertFocusSurvives` and `assertFocusBrowserEvidence` have no caller.** |
| 2 | **Colour contrast** of every foreground/background pair. | `overview_accessibility.md`, "Color contrast" | `policy/interaction/accessibility.mjs`; measured instances recorded in `alert.tsx`, `resource-boundary.tsx`. |
| 3 | **All visual styling** — every dimension, colour, radius, hover and pressed tint, disabled treatment, dark mode. | `overview_about.md`, "Headless" | The STYLE plane; `cva` recipes in each adapter; `tests/unit/design-system-classes.test.ts`. |
| 4 | **All motion** — enter/exit transitions, transform-origin, freezing sizes during content transitions, `[data-instant]` no-transition rules. | `handbook_animation.md`; §5's Viewport block | `STYLE.motion.state`. No authored component animates today. |
| 5 | **`prefers-reduced-motion`** — **no prose guidance and no prop** anywhere in the docs; the gate appears only in demo CSS on Dialog, Drawer and Tabs. | absence of guidance across all pages; `components_dialog.md:2225`, `components_drawer.md:2158`/`:2192`/`:3273`, `components_tabs.md:481` | **UNMET.** Belongs in the motion symbol (§3.4); copy the Tabs `no-preference` shape. |
| 6 | **UA resets** — fieldset border/margin/padding, input `font-family: inherit` and `box-sizing`, coarse-pointer font sizes. | demo CSS across the form pages | Recipes; `Page` sets family and body role once. |
| 7 | **Icons and arrow geometry**, and `z-index` on positioners. | menu/overlay demo CSS | lucide icons in `alert.tsx`, `combobox.tsx`; `STYLE.layer.overlay` on the Combobox Positioner — which is currently doing the isolation root's job as well (§1.7, R13). |

### Semantic and accessibility

| # | Obligation | Source | Where it lives in v4 |
|---|---|---|---|
| 8 | **Accessible names for custom controls** — `aria-label`, `aria-labelledby`, `Field.Label`, `Select.Label`, `Combobox.Label` for the input-inside-popup pattern only, per-thumb Slider labels. | `overview_accessibility.md`; `handbook_forms.md:1188-1226` | `Field` (required label); forwarded ARIA on Switch and Combobox. **Rule R6 unenforced.** |
| 9 | **A general-purpose live region.** Base UI ships none — **Toast is its only announcing component**, and it is a queued transient notification, not a region a page marks as busy. Toast announces `title`/`description` only, and nothing else inside `Toast.Root`. | absence of a region component in the catalog; `components_toast.md:4651`, `:3744-3745` | `status.tsx` (`role=status`, `aria-live=polite`, `aria-busy=true`, all three removed from the prop type so a caller cannot override through the spread); `alert.tsx` (`role` from the tone table, no `aria-live` — an explicit duplicate double-speaks in VoiceOver on iOS). |
| 10 | **Every AT-facing string, and its translation** — `aria-label` on icon-only controls, `aria-description` on chips and multi-select inputs, Status/Empty copy. *"Base UI does not ship these strings."* | `components_combobox.md`, multiple-selection a11y notes | Screens and adapters. No i18n layer exists yet. |
| 11 | **Two-dimensional grid navigation.** *"Base UI ships no grid. Every branch is written in this repository."* | `keyboard.mjs` `PROFILE_KEYBOARD['composite-grid']` | `keyboard.mjs`, suppliedBy `this-system`. DORMANT. |
| 12 | **Verifying undocumented behaviour before relying on it** — roles and keys for Switch and Toggle, OTP slot navigation, ToggleGroup's rendered element, **`aria-activedescendant` and listbox/option roles anywhere** (zero documented occurrences), **what a childless `Field.Error` renders**. | NOT DOCUMENTED across those pages | `switch.test.tsx` / `switch.browser.test.tsx` did this for Switch (role, `aria-checked`, Space). Combobox's arrow traversal and Field's relationship are the open cases. |
| 13 | **Screen-reader evidence.** Base UI's own testing claim is not evidence under v4's policy. | `assistive-technology.mjs` `REQUIRED_PAIRINGS` | NVDA+Chrome and JAWS+Chrome sessions with verbatim announced text. **No ledger file; no gate.** |
| 14 | **Target sizes** — 24px pointer (WCAG 2.5.8), 48px touch; density may compress presentation, never operability. | `accessibility.mjs` | `STYLE.component.switch.trackHeight` (24px floor), 40px control height. |

### Structural and behavioural

| # | Obligation | Source | Where it lives in v4 |
|---|---|---|---|
| 15 | **App-level portal setup** — `isolation: isolate` around the app. | `overview_quick-start.md`, "Set up" | **LIVE AND UNMET.** The authored Combobox portals to `<body>` today (`combobox.tsx:177-216`); no isolation root exists in `apps/web`; the stacking guarantee currently rests on the `layer-overlay` z-index token. Rule **R13**. |
| 16 | **iOS 26+ Safari** — `body { position: relative }` and `position: absolute` backdrops. | `overview_quick-start.md`, "Set up" | **Dormant for a stated reason:** the Combobox renders no `Backdrop`, and no authored component renders one. Live with the first Backdrop — not with the first Portal, which already exists. |
| 17 | **`ref` forwarding** into any component passed to `render`; the spread in the callback form. | `handbook_composition.md` | N/A — no authored component accepts `render`. |
| 18 | **Positioning hidden inputs** — a `name` plus a relatively positioned container so the native validation bubble points correctly. | `handbook_forms.md`, "Constraint validation" | `Field` passes `name` to Root. Container positioning is **prose**. |
| 19 | **Message localisation — but NOT server-side error plumbing.** Base UI **ships** `<Form errors>`: an object keyed by field name, merged into client-side field state, and *"Once a field's value changes, any corresponding error in `errors` will be cleared from the field state"* — including the `useActionState` / Server Function form. **v4 declines it.** | `handbook_forms.md:1476ff`, "Server-side validation" | `Field`'s `error?: string` + `match={true}`; the server is the authority. **What declining costs:** v4's error does not clear itself when the value changes. That is behaviour an agent must implement by hand in every screen, and today no adapter does (§3.5, decision 3). Localisation genuinely is ours. |
| 20 | **Choosing when to override the default element** — recommended case-by-case only. | `handbook_composition.md` | Removed from the decision space: `render` is not in the Target. |
| 21 | **Reading each API reference for the complete attribute and variable list.** The handbook gives examples only. | `handbook_styling.md` | **§5 is the assembly-level index** — 37 component rows, the two provider rows, and the shared blocks for detached triggers and Viewport content transitions. **The per-component API reference remains the authority for the complete list**; §5 does not carry `nativeButton` defaults (§1.2), every `reason` union, or every prop. |
| 22 | **Client boundaries** — `'use client'` on every interactive file. | demos across the handbook | Present on **five of the six** Base UI-backed adapters plus ResourceBoundary; **`button.tsx` is the exception** and relies on the boundary the installed package declares in `@base-ui/react/button/Button.mjs`. Rule R8, RED-first. |
| 23 | **RTL:** setting `dir` yourself; re-applying direction to portaled content via `useDirection()`. | `utils_direction-provider.md` | Not needed until an RTL locale ships (§4.3). |
| 24 | **CSP, in two separable pieces:** (a) generating the per-request nonce and emitting `script-src` / `style-src-elem` for Base UI's inline `<style>`/`<script>` **elements**; (b) deciding the **`style-src-attr`** policy for inline `style=""` attributes, which `CSPProvider` does **not** cover and which *"any component"* can produce. | `utils_csp-provider.md`, lines 66 and 86-96 | (a) Not needed until ScrollArea or Select is adopted under a strict CSP. **(b) is triggered by shipping a strict CSP at all and applies today** — the authored Combobox's Positioner and Popup position via inline style attributes. Settle it by reading the SSR'd HTML, not by assuming absence (§4.4). |
| 25 | **Ref merging through `useRender`'s `ref` array** — `mergeProps` drops all but the rightmost. | `utils_merge-props.md` | N/A today; becomes live with `useRender`. |
| 26 | **Accepting that non-synthetic events cannot be vetoed** — `preventBaseUIHandler()` exists only on React synthetic events. | `utils_merge-props.md` | Documented constraint; no current dependency. |
| 27 | **Value shapes and refusals** — the domain rules Base UI has no opinion about. | law 19, law 21, ADR-016 | `Combobox` throws on an unknown id; `DateInput` never defaults to today; `TextInput` refuses `type="number"`. |
| 28 | **The Target boundary itself** — deciding which of Base UI's words become v4's words. | v4 decision | Each adapter's prop type; `adapter-schema.test.ts`, with the scope R4 states. |
| 29 | **Running v4's own policy validators.** A validator that nothing calls proves nothing, and a header that says it is called is a second source for a fact the tree contradicts. | `policy/interaction/keyboard.mjs:74-84`, `focus.mjs:385`/`:495`, `policy/interaction/index.mjs:176-184` | **UNMET.** `assertProfileKeyboard`, `assertKeyboardPolicy`, `assertFocusSurvives` and `assertFocusBrowserEvidence` have **no caller**; `keyboard.mjs`'s header claims one — `tests/unit/interaction-policy.test.ts` — that does not exist in this checkout. The nine module-scope calls in `index.mjs` do not include them. Either write the caller (and watch it go red by planting a ninth profile, as the header describes) or delete the claim. |

---

## Closing note for the next agent

Three things about this foundation are worth carrying forward, in order of how easily they are forgotten.

**The gap between policy and evidence is the real state of the system.** v4's accessibility policy is stricter than Base UI's obligations list, and that is genuinely good. The sharpest instance of the gap is not the coverage test — that test has four assertions, three of them live, and it has gone red once already (its own header records the day). The sharpest instance is `keyboard.mjs`'s header, which states in the past tense that `assertProfileKeyboard` *"is now called from `tests/unit/interaction-policy.test.ts`, proven by planting a ninth profile and watching it go red"*. That file does not exist and the function has no caller. A module documenting its own escape from the defect it describes, while still inside it, is what "decoration" means here. Close behind it: no assertion anywhere requires a profile with a non-empty `gap` to eventually acquire a spec, so the composite debt can sit forever while everything stays green. The single highest-value piece of work available is still not adopting another primitive; it is proving arrow traversal and `aria-activedescendant` on the one authored composite — and note the sharpened framing, that Base UI documents neither of those anywhere, so what is unproven is not "their guarantee in our assembly" but the behaviour itself.

**Base UI's silence is not a guarantee — and neither is a confident sentence in this document.** Many pages do not enumerate roles or key tables. Where this document says NOT DOCUMENTED, it means the page does not say — not that the behaviour is absent. Switch's `role="switch"` is a measured fact in this repository because a test asserted it, not because a page claimed it. The reverse failure has also happened here: an earlier revision asserted, under a VERIFIED mark and twice, that a childless `Field.Error` renders the native validity message. No page says it; nothing measured it; it sat between two genuine quotations and read exactly like them. Any new claim about a role or an announcement belongs in a browser test first and in this document second.

**Ask what else holds a copy of the fact.** The vendored `ui/field.tsx` hand-rolls a Field vocabulary that disagrees with the one the authored `Field` uses — boolean `data-invalid="true"` versus Base UI's presence-only `data-invalid`. `ui/toggle.tsx` styles `data-[state=on]` on a component whose API table publishes only `data-pressed`. Both are unreachable, so both are inert. They are also exactly the shape of the defect CLAUDE.md documents five times over: two sources for one fact, agreeing right up until they do not — and this revision found three more of the same shape inside the checkout's own prose (`keyboard.mjs`'s claimed caller, `A11Y_LEVELS`'s two named specs, this document's "proved by test" line for a component with no test). When you fix something here, the last question is always where else that fact lives.

REJECTED FINDINGS:
- none
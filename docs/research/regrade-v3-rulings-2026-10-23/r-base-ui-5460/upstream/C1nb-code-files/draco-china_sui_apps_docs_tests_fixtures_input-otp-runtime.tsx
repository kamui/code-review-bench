import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { Window as HappyWindow } from "happy-dom";
import { scenario } from "../support/scenario";

const window = new HappyWindow({
  url: "http://localhost/docs?preview=1#example",
}) as unknown as Window &
  typeof globalThis & {
    happyDOM: {
      close: () => Promise<void>;
      settings: { device: { prefersReducedMotion: string } };
    };
  };
const document = window.document;
const hidden = false;
Object.defineProperty(document, "hidden", {
  configurable: true,
  get: () => hidden,
});
Object.assign(globalThis, {
  window,
  document,
  HTMLElement: window.HTMLElement,
  Element: window.Element,
  Node: window.Node,
  Document: window.Document,
  ShadowRoot: window.ShadowRoot,
  SVGElement: window.SVGElement,
  HTMLIFrameElement: window.HTMLIFrameElement,
  HTMLInputElement: window.HTMLInputElement,
  HTMLButtonElement: window.HTMLButtonElement,
  HTMLFormElement: window.HTMLFormElement,
  MutationObserver: window.MutationObserver,
  getComputedStyle: window.getComputedStyle.bind(window),
  requestAnimationFrame: window.requestAnimationFrame.bind(window),
  cancelAnimationFrame: window.cancelAnimationFrame.bind(window),
  IS_REACT_ACT_ENVIRONMENT: true,
});
Object.defineProperty(globalThis, "navigator", {
  value: window.navigator,
  configurable: true,
});
class Observer {
  observe() {}
  unobserve() {}
  disconnect() {}
}
Object.assign(globalThis, {
  ResizeObserver: Observer,
  IntersectionObserver: Observer,
});
const frames = new Map<number, FrameRequestCallback>();
let nextFrame = 0;
const requestFrame = (callback: FrameRequestCallback) => {
  const id = ++nextFrame;
  frames.set(id, callback);
  return id;
};
const cancelFrame = (id: number) => {
  frames.delete(id);
};
Object.assign(globalThis, {
  requestAnimationFrame: requestFrame,
  cancelAnimationFrame: cancelFrame,
});
window.requestAnimationFrame = requestFrame;
window.cancelAnimationFrame = cancelFrame;
const listeners = new Set<() => void>();
let reduced = false;
window.matchMedia = (media) =>
  ({
    get matches() {
      return media.includes("prefers-reduced-motion") && reduced;
    },
    media,
    addEventListener: (_type: string, listener: () => void) =>
      listeners.add(listener),
    removeEventListener: (_type: string, listener: () => void) =>
      listeners.delete(listener),
  }) as unknown as MediaQueryList;

Object.assign(globalThis, { matchMedia: window.matchMedia });

const stylesheet = document.createElement("style");
stylesheet.textContent = readFileSync(
  new URL(
    "../../../../packages/ui/src/styles/components/input-otp.css",
    import.meta.url,
  ),
  "utf8",
);
document.head.append(stylesheet);
const restoreFrames = [...(stylesheet.sheet?.cssRules ?? [])].find(
  (rule) => (rule as CSSKeyframesRule).name === "input-otp-restore",
) as CSSKeyframesRule;
assert.ok(restoreFrames);
const from = restoreFrames.cssRules[0] as CSSKeyframeRule;
const to = restoreFrames.cssRules[1] as CSSKeyframeRule;
assert.match(from.style.transform, /--otp-collapse-x/);
assert.match(from.style.transform, /--otp-collapse-y/);
assert.match(from.style.transform, /scale\(0\.86\)/);
assert.match(to.style.transform, /translate\(0, 0\) scale\(1\)/);
assert.equal(from.style.opacity, "0");
assert.equal(to.style.opacity, "1");
const nativeStyle = getComputedStyle;
Object.assign(globalThis, {
  getComputedStyle: (element: Element) => {
    const computed = nativeStyle(element);
    // Happy DOM does not expand animation shorthand containing CSS variables.
    if (
      element.getAttribute("data-slot") === "input-otp-feedback-clock" &&
      !computed.animationName
    )
      return { animationName: "input-otp-feedback" };
    return computed;
  },
});
const { act, createRef, useState } = await import("react");
const { hydrateRoot } = await import("react-dom/client");
const { renderToString } = await import("react-dom/server");
const { InputOTP, InputOTPGroup, InputOTPSlot, InputOTPSeparator } =
  await import("@workspace/ui/components/input-otp");
const requireUI = createRequire(
  new URL("../../../../packages/ui/package.json", import.meta.url),
);
const { Field } = (await import(
  requireUI.resolve("@base-ui/react/field").replace(/\.js$/, ".mjs")
)) as typeof import("@base-ui/react/field");
const { Form } = (await import(
  requireUI.resolve("@base-ui/react/form").replace(/\.js$/, ".mjs")
)) as typeof import("@base-ui/react/form");
const rootRef = createRef<HTMLDivElement>();
const inputRef = createRef<HTMLInputElement>();
const changes: string[] = [];
const completed: string[] = [];
const statusRequests: string[] = [];
const invalid: string[] = [];
let setExternal: (value: string) => void = () => {};
let setStatus: (value: "idle" | "loading" | "success" | "error") => void =
  () => {};
let updateFeedbackDuration: (value: number) => void = () => {};
let updateMask: (value: boolean) => void = () => {};
let submits = 0;
function App() {
  const [value, setValue] = useState("");
  const [status, changeStatus] = useState<
    "idle" | "loading" | "success" | "error"
  >("idle");
  const [duration, changeDuration] = useState(1250);
  const [mask, changeMask] = useState(false);
  updateMask = changeMask;
  setExternal = setValue;
  setStatus = changeStatus;
  updateFeedbackDuration = changeDuration;
  return (
    <>
      <p role="status" id="external-feedback">
        {
          {
            idle: "",
            loading: "Verifying code",
            success: "Code verified",
            error: "Verification failed",
          }[status]
        }
      </p>
      <Form
        id="form"
        onSubmit={(event) => {
          event.preventDefault();
          submits++;
        }}
      >
        <Field.Root name="code">
          <Field.Label>Verification code</Field.Label>
          <InputOTP
            ref={rootRef}
            length={6}
            value={value}
            onValueChange={(value) => {
              changes.push(value);
              setValue(value);
            }}
            onValueComplete={(value) => completed.push(value)}
            onValueInvalid={(value) => invalid.push(value)}
            status={status}
            feedbackDuration={duration}
            onStatusChange={(status) => {
              statusRequests.push(status);
              changeStatus(status);
            }}
            required
            mask={mask}
          >
            <InputOTPGroup>
              <InputOTPSlot ref={inputRef} />
              <InputOTPSlot />
              <InputOTPSlot />
            </InputOTPGroup>
            <InputOTPSeparator />
            <InputOTPGroup>
              <InputOTPSlot />
              <InputOTPSlot />
              <InputOTPSlot />
            </InputOTPGroup>
          </InputOTP>
          <Field.Error match="valueMissing">Enter a complete code</Field.Error>
        </Field.Root>
        <button type="submit">Submit</button>
      </Form>
      <label htmlFor="local">Local code</label>
      <InputOTP id="local" length={4} name="local-code" defaultValue="12">
        <InputOTPGroup>
          <InputOTPSlot />
          <InputOTPSlot />
          <InputOTPSlot />
          <InputOTPSlot />
        </InputOTPGroup>
      </InputOTP>
      <label htmlFor="disabled">Disabled code</label>
      <InputOTP id="disabled" length={2} disabled defaultValue="12">
        <InputOTPGroup>
          <InputOTPSlot />
          <InputOTPSlot />
        </InputOTPGroup>
      </InputOTP>
      <label htmlFor="readonly">Read-only code</label>
      <InputOTP id="readonly" length={2} readOnly defaultValue="12">
        <InputOTPGroup>
          <InputOTPSlot />
          <InputOTPSlot />
        </InputOTPGroup>
      </InputOTP>
      <label htmlFor="alpha">Recovery code</label>
      <InputOTP
        id="alpha"
        length={4}
        validationType="alphanumeric"
        normalizeValue={(value) => value.toUpperCase()}
      >
        <InputOTPGroup>
          <InputOTPSlot />
          <InputOTPSlot />
          <InputOTPSlot />
          <InputOTPSlot />
        </InputOTPGroup>
      </InputOTP>
      <label htmlFor="masked">Masked code</label>
      <InputOTP
        id="masked"
        name="masked-code"
        form="form"
        length={2}
        mask
        defaultValue="12"
      >
        <InputOTPGroup>
          <InputOTPSlot />
          <InputOTPSlot />
        </InputOTPGroup>
      </InputOTP>
      <label htmlFor="styled">Styled code</label>
      <InputOTP id="styled" length={2}>
        <InputOTPGroup>
          <InputOTPSlot
            style={(state) => ({ opacity: state.filled ? 0.9 : 0.8 })}
          />
          <InputOTPSlot />
        </InputOTPGroup>
      </InputOTP>
      <button id="unrelated" type="button">
        Another control
      </button>
    </>
  );
}

scenario("server markup hydrates without recoverable errors");
const host = document.createElement("div");
document.body.append(host);
host.innerHTML = renderToString(<App />);
const errors: unknown[] = [];
let root: ReturnType<typeof hydrateRoot>;
await act(async () => {
  root = hydrateRoot(host, <App />, {
    onRecoverableError: (error) => errors.push(error),
  });
  await new Promise((resolve) => setTimeout(resolve, 30));
});
assert.deepEqual(errors, []);

scenario("slots are native inputs with the field's label and validation");
const otpRoot = rootRef.current;
assert.ok(otpRoot);
const slots = () => [
  ...otpRoot.querySelectorAll<HTMLInputElement>(
    'input[data-slot="input-otp-slot"]',
  ),
];
assert.equal(slots().length, 6);
assert.equal(otpRoot.tagName, "DIV");
assert.equal(inputRef.current, slots()[0]);
assert.equal(inputRef.current?.tagName, "INPUT");
assert.equal(inputRef.current?.getAttribute("aria-label"), null);
assert.match(
  inputRef.current?.labels?.[0]?.textContent ?? "",
  /Verification code/,
);
assert.equal(otpRoot.querySelector("label"), null);
assert.equal(otpRoot.querySelector('[role="status"]'), null);
const validationInput = document.querySelector(
  'input[name="code"]',
) as HTMLInputElement;
assert.equal(validationInput.required, true);
assert.equal(validationInput.minLength, 6);
assert.equal(validationInput.maxLength, 6);
assert.equal(validationInput.checkValidity(), false);

scenario("only the first slot offers one-time-code autofill and a tab stop");
assert.equal(inputRef.current?.getAttribute("autocomplete"), "one-time-code");
assert.equal(slots()[1]?.getAttribute("autocomplete"), "off");
assert.equal(slots()[1]?.getAttribute("aria-label"), null);
assert.equal(
  slots()[1]?.getAttribute("aria-labelledby"),
  slots()[0]?.getAttribute("aria-labelledby"),
);
assert.equal(slots()[1]?.getAttribute("tabindex"), "-1");
assert.ok(otpRoot.querySelector('[data-slot="input-otp-separator"]'));
async function interact(callback: () => void) {
  await act(async () => {
    callback();
    await new Promise((resolve) => setTimeout(resolve, 15));
  });
}
async function frame(time: number) {
  const pending = [...frames.values()];
  frames.clear();
  await act(async () => {
    for (const callback of pending) callback(time);
  });
}
let now = 100;
async function settle() {
  for (let i = 0; i < 120 && frames.size; i++) {
    now += 16;
    await frame(now);
  }
}
function edit(input: HTMLInputElement, value: string) {
  const set = Object.getOwnPropertyDescriptor(
    window.HTMLInputElement.prototype,
    "value",
  )?.set;
  assert.ok(set);
  set.call(input, value);
  input.dispatchEvent(new window.Event("input", { bubbles: true }));
}
function paste(input: HTMLInputElement, value: string) {
  const event = new window.Event("paste", { bubbles: true, cancelable: true });
  Object.defineProperty(event, "clipboardData", {
    value: { getData: () => value },
  });
  input.dispatchEvent(event);
}

scenario("typing advances focus and rejects characters outside the pattern");
await interact(() => inputRef.current?.focus());
await interact(() => edit(slots()[0] as HTMLInputElement, "1"));
await settle();
assert.equal(changes.at(-1), "1");
assert.equal(document.activeElement, slots()[1]);
await interact(() => edit(slots()[1] as HTMLInputElement, "x"));
assert.equal(invalid.at(-1), "x");
assert.equal(changes.at(-1), "1");

scenario("a paste fills every slot, completes the code and submits one value");
await interact(() => paste(slots()[1] as HTMLInputElement, "2 3-456"));
await settle();
assert.equal(changes.at(-1), "123456");
assert.equal(completed.at(-1), "123456");
assert.equal(otpRoot.hasAttribute("data-complete"), true);
assert.equal(
  new window.FormData(document.querySelector("#form") as HTMLFormElement).get(
    "code",
  ),
  "123456",
);
await interact(() =>
  (document.querySelector("#form button") as HTMLButtonElement).click(),
);
assert.equal(submits, 1);
assert.equal(validationInput.checkValidity(), true);
assert.equal(
  new window.FormData(
    document.querySelector("#form") as HTMLFormElement,
  ).getAll("code").length,
  1,
);

scenario("Backspace removes the last character");
await interact(() =>
  (slots()[5] as HTMLElement).dispatchEvent(
    new window.KeyboardEvent("keydown", {
      key: "Backspace",
      bubbles: true,
      cancelable: true,
    }),
  ),
);
await settle();
assert.equal(changes.at(-1), "12345");

scenario("an external value fills the slots");
await interact(() => setExternal("654321"));
assert.equal(
  slots()
    .map((slot) => slot.value)
    .join(""),
  "654321",
);

scenario("loading makes masked slots read-only while the form keeps the value");
await interact(() => inputRef.current?.focus());
await interact(() => updateMask(true));
await interact(() => setStatus("loading"));
assert.equal(otpRoot.getAttribute("aria-busy"), "true");
assert.equal(
  document.querySelector("#external-feedback")?.textContent,
  "Verifying code",
);
assert.equal(otpRoot.textContent, "");
assert.ok(slots().every((slot) => slot.readOnly));
assert.ok(slots().every((slot) => !slot.disabled));
assert.equal(
  new window.FormData(document.querySelector("#form") as HTMLFormElement).get(
    "code",
  ),
  "654321",
);

scenario("an error morphs the loading glyph in place");
const glyph = otpRoot.querySelector('[data-slot="input-otp-state"] svg path');
assert.ok(glyph);
const loader = glyph.getAttribute("d");
await interact(() => setStatus("error"));
await settle();
assert.equal(
  otpRoot.querySelector('[data-slot="input-otp-state"] svg path'),
  glyph,
);
assert.notEqual(glyph.getAttribute("d"), loader);
assert.equal(otpRoot.getAttribute("data-feedback"), "active");
const marker = otpRoot.querySelector('[data-slot="input-otp-feedback-clock"]');
assert.ok(marker);
function completeAnimation(element: Element, name = "input-otp-feedback") {
  const event = new window.Event("animationend", { bubbles: true });
  Object.defineProperty(event, "animationName", {
    value: name,
  });
  element.dispatchEvent(event);
}

scenario("after error feedback the slots restore, refocus and stay editable");
const beforeError = statusRequests.length;
await interact(() => completeAnimation(marker));
assert.equal(otpRoot.getAttribute("data-feedback"), "restoring");
assert.ok(slots().every((slot) => slot.readOnly));
assert.match(
  nativeStyle(slots()[0] as HTMLInputElement).animation,
  /input-otp-restore/,
);
assert.match(
  nativeStyle(otpRoot.querySelector('[data-slot="input-otp-state"]') as Element)
    .animation,
  /input-otp-dismiss/,
);
assert.equal(statusRequests.length, beforeError);
await interact(() => completeAnimation(marker, "input-otp-restoration"));
assert.equal(statusRequests.at(-1), "idle");
assert.equal(otpRoot.getAttribute("data-status"), "idle");
assert.equal(otpRoot.getAttribute("data-feedback"), "restored");
assert.ok(slots().every((slot) => !slot.readOnly));
assert.ok(slots().every((slot) => slot.type === "password"));
assert.equal(
  new window.FormData(document.querySelector("#form") as HTMLFormElement).get(
    "code",
  ),
  "654321",
);
assert.equal(
  slots()
    .map((slot) => slot.value)
    .join(""),
  "654321",
);
assert.equal(document.activeElement, slots()[0]);
await interact(() => edit(slots()[0] as HTMLInputElement, "1"));
assert.equal(changes.at(-1), "154321");

scenario("success stays until the status changes without asking for idle");
await interact(() => setStatus("loading"));
const beforeSuccess = statusRequests.length;
await interact(() => setStatus("success"));
await settle();
assert.equal(
  otpRoot.querySelectorAll('[data-slot="input-otp-particles"] span').length,
  28,
);
const successMarker = otpRoot.querySelector(
  '[data-slot="input-otp-feedback-clock"]',
);
assert.ok(successMarker);
await interact(() => completeAnimation(successMarker));
assert.equal(
  statusRequests.length,
  beforeSuccess,
  "success never requests an idle reset",
);
assert.ok(otpRoot.querySelector('[data-slot="input-otp-state"]'));
assert.equal(otpRoot.getAttribute("data-status"), "success");
assert.ok(slots().every((slot) => slot.readOnly));
await interact(() => setStatus("idle"));
assert.equal(otpRoot.querySelector('[data-slot="input-otp-state"]'), null);

scenario("error feedback does not steal focus from another control");
await interact(() => setStatus("loading"));
await interact(() =>
  (document.querySelector("#unrelated") as HTMLElement).focus(),
);
await interact(() => setStatus("error"));
const blurMarker = otpRoot.querySelector(
  '[data-slot="input-otp-feedback-clock"]',
);
assert.ok(blurMarker);
await interact(() => completeAnimation(blurMarker));
await interact(() => completeAnimation(blurMarker, "input-otp-restoration"));
assert.equal(
  document.activeElement,
  document.querySelector("#unrelated"),
  "feedback does not steal another instance's focus",
);

scenario("uncontrolled, disabled and read-only instances keep their values");
const local = document.querySelector("#local") as HTMLInputElement;
assert.equal(local.value, "1");
await interact(() => local.focus());
await interact(() => paste(local, "4321"));
await settle();
assert.equal(
  [...document.querySelectorAll<HTMLInputElement>('[data-slot="input-otp"]')]
    .length,
  7,
);
assert.equal(
  (document.querySelector('input[name="local-code"]') as HTMLInputElement)
    .value,
  "4321",
);
assert.equal(
  (document.querySelector("#disabled") as HTMLInputElement).disabled,
  true,
);
assert.equal(
  (document.querySelector("#readonly") as HTMLInputElement).readOnly,
  true,
);

scenario("alphanumeric codes normalize pasted characters");
const alpha = document.querySelector("#alpha") as HTMLInputElement;
await interact(() => alpha.focus());
await interact(() => paste(alpha, "a7-c9"));
await settle();
assert.equal(
  [
    alpha,
    document.querySelector("#alpha-2"),
    document.querySelector("#alpha-3"),
    document.querySelector("#alpha-4"),
  ]
    .map((node) => (node as HTMLInputElement).value)
    .join(""),
  "A7C9",
);

scenario("masked slots submit their value through the form attribute");
assert.equal(
  (document.querySelector("#masked") as HTMLInputElement).type,
  "password",
);
assert.equal(
  new window.FormData(document.querySelector("#form") as HTMLFormElement).get(
    "masked-code",
  ),
  "12",
);

scenario("unmasking switches the slots back to text");
await interact(() => updateMask(false));
assert.ok(slots().every((slot) => slot.type === "text"));
assert.equal(
  slots()
    .map((slot) => slot.value)
    .join(""),
  "154321",
);

scenario("slot style callbacks receive the filled state");
assert.equal(
  document.querySelector("#styled")?.hasAttribute("data-glass"),
  false,
);
const styledSlot = document.querySelector("#styled") as HTMLInputElement;
assert.equal(styledSlot.style.opacity, "0.8");
await interact(() => paste(styledSlot, "42"));
assert.equal(styledSlot.style.opacity, "0.9");

scenario("cancelled loading leaves no stale focus ownership");
await interact(() => inputRef.current?.focus());
await interact(() => setStatus("loading"));
await interact(() => setStatus("idle"));
await interact(() => (document.activeElement as HTMLElement).blur());
await interact(() => setStatus("error"));
const cancelledMarker = otpRoot.querySelector(
  '[data-slot="input-otp-feedback-clock"]',
);
assert.ok(cancelledMarker);
await interact(() => completeAnimation(cancelledMarker));
await interact(() =>
  completeAnimation(cancelledMarker, "input-otp-restoration"),
);
assert.equal(
  document.activeElement,
  document.body,
  "cancelled loading does not leave stale focus ownership",
);

scenario("reduced motion restores after the feedback duration");
reduced = true;
for (const listener of listeners) listener();
await interact(() => inputRef.current?.focus());
await interact(() => updateFeedbackDuration(60));
await interact(() => setStatus("error"));
assert.equal(otpRoot.getAttribute("data-status"), "error");
assert.ok(otpRoot.querySelector('[data-slot="input-otp-state"]'));
await act(async () => {
  await new Promise((resolve) => setTimeout(resolve, 70));
});
assert.equal(otpRoot.getAttribute("data-status"), "idle");
assert.equal(otpRoot.querySelector('[data-slot="input-otp-state"]'), null);
assert.equal(document.activeElement, slots()[0]);
reduced = false;

scenario("a timeout restores the input when animationend never fires");
await interact(() => updateFeedbackDuration(10));
await interact(() => setStatus("error"));
await interact(() => {});
await act(async () => {
  await new Promise((resolve) => setTimeout(resolve, 100));
});
assert.equal(otpRoot.getAttribute("data-feedback"), "restoring");
await act(async () => {
  await new Promise((resolve) => setTimeout(resolve, 470));
});
assert.equal(
  otpRoot.getAttribute("data-status"),
  "idle",
  "fallback restores input if animationend was suppressed",
);

scenario("success with a zero duration stays until the status changes");
await interact(() => updateFeedbackDuration(0));
await interact(() => setStatus("success"));
assert.equal(otpRoot.getAttribute("data-status"), "success");
assert.ok(otpRoot.querySelector('[data-slot="input-otp-state"]'));
await interact(() => setStatus("idle"));

scenario("unmount during feedback requests nothing and releases listeners");
await interact(() => updateFeedbackDuration(1250));
await interact(() => setStatus("error"));
const beforeUnmount = statusRequests.length;
await interact(() => root.unmount());
await settle();
await new Promise((resolve) => setTimeout(resolve, 10));
assert.equal(statusRequests.length, beforeUnmount);
assert.equal(frames.size, 0);
assert.equal(listeners.size, 0);
await window.happyDOM.close();

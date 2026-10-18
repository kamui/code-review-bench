import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

// Probe C1: the statements of one comment about the controlled Field.Control, checked one by one.
// Observations only. Each case prints one PROBE line and asserts nothing.
// Renders are counted as the pull request's own test counts them: a `render` prop on the control.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

interface Seen {
  renders: number;
  validate: unknown[];
  validity: { valid: boolean | null; valueMissing: boolean } | null;
}

function seen(): Seen {
  return { renders: 0, validate: [], validity: null };
}

function snapshot(s: Seen) {
  const root = screen.getByTestId('root');
  return {
    inputText: screen.getByTestId<HTMLInputElement>('control').value,
    dataDirty: root.hasAttribute('data-dirty'),
    dataFilled: root.hasAttribute('data-filled'),
    dataInvalid: root.hasAttribute('data-invalid'),
    validity: s.validity,
    validateCalls: [...s.validate],
  };
}

type Setter = (next: string, set: React.Dispatch<React.SetStateAction<string>>) => void;

function Harness(props: {
  s: Seen;
  controlled: boolean;
  initial?: string;
  rootProps?: Record<string, unknown>;
  controlProps?: Record<string, unknown>;
  setter?: Setter;
}) {
  const { s, controlled, initial = '', rootProps, controlProps, setter } = props;
  const [value, setValue] = React.useState(initial);
  const onValueChange = (next: string) => (setter ? setter(next, setValue) : setValue(next));
  return (
    <Field.Root
      data-testid="root"
      name="field"
      validate={(v: unknown) => {
        s.validate.push(v);
        return null;
      }}
      {...rootProps}
    >
      <Field.Control
        data-testid="control"
        {...(controlled ? { value, onValueChange } : { defaultValue: initial })}
        {...controlProps}
        render={(p) => {
          s.renders += 1;
          return <input {...p} />;
        }}
      />
      <Field.Validity>
        {(validity) => {
          s.validity = { valid: validity.state.valid, valueMissing: validity.state.valueMissing };
          return null;
        }}
      </Field.Validity>
    </Field.Root>
  );
}

function typeEach(s: Seen, texts: string[]) {
  return texts.map((text) => {
    const before = s.renders;
    const calls = s.validate.length;
    fireEvent.change(screen.getByTestId('control'), { target: { value: text } });
    const after = snapshot(s);
    return {
      typed: text,
      controlRenders: s.renders - before,
      validateCallsThisKeystroke: s.validate.length - calls,
      dataDirty: after.dataDirty,
      dataFilled: after.dataFilled,
    };
  });
}

async function wait(ms: number) {
  await act(async () => {
    await new Promise((resolve) => {
      setTimeout(resolve, ms);
    });
  });
}

function debounced(ms: number): Setter {
  let timer: ReturnType<typeof setTimeout> | undefined;
  return (next, set) => {
    clearTimeout(timer);
    timer = setTimeout(() => set(next), ms);
  };
}

// A raw input event outside act(), so that React schedules work as it would in a browser and the
// state between the event and a deferred update can be read.
function rawType(text: string) {
  const input = screen.getByTestId<HTMLInputElement>('control');
  const setValue = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!;
  setValue.call(input, text);
  input.dispatchEvent(new Event('input', { bubbles: true }));
}

describe('C1 probe', () => {
  const { render } = createRenderer({ strict: false });

  it('C1-1 renders per keystroke, controlled, default mode: dirty and filled flip on "a", "" and "x"', async () => {
    const s = seen();
    await render(<Harness s={s} controlled />);
    report('C1-1-controlled-default-renders', { keystrokes: typeEach(s, ['a', 'ab', 'abc', '', 'x', 'xy']) });
  });

  it('C1-2 renders per keystroke, controlled, validationMode="onChange"', async () => {
    const s = seen();
    await render(<Harness s={s} controlled rootProps={{ validationMode: 'onChange' }} />);
    report('C1-2-controlled-onChange-renders', { keystrokes: typeEach(s, ['a', 'ab', 'abc', '', 'x', 'xy']) });
  });

  it('C1-3 renders per keystroke, controlled, default mode, starting from "abc": only dirty flips', async () => {
    const s = seen();
    await render(<Harness s={s} controlled initial="abc" />);
    report('C1-3-controlled-default-dirty-only', { keystrokes: typeEach(s, ['abcd', 'abcde', 'abc', 'ab']) });
  });

  it('C1-4 renders per keystroke, uncontrolled, default mode (for comparison)', async () => {
    const s = seen();
    await render(<Harness s={s} controlled={false} />);
    report('C1-4-uncontrolled-default-renders', { keystrokes: typeEach(s, ['a', 'ab', 'abc', '', 'x', 'xy']) });
  });

  it('C1-5 a debounced setter (30 ms), default mode: before and after the update lands', async () => {
    const s = seen();
    await render(<Harness s={s} controlled setter={debounced(30)} />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a' } });
    const rightAfterA = snapshot(s);
    await wait(60);
    const afterDelayA = snapshot(s);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'ab' } });
    const rightAfterAb = snapshot(s);
    await wait(60);
    report('C1-5-debounced-default', { rightAfterA, afterDelayA, rightAfterAb, afterDelayAb: snapshot(s) });
  });

  it('C1-6 a debounced setter (30 ms), validationMode="onChange": before and after the update lands', async () => {
    const s = seen();
    await render(<Harness s={s} controlled setter={debounced(30)} rootProps={{ validationMode: 'onChange' }} />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a' } });
    const rightAfterA = snapshot(s);
    await wait(60);
    report('C1-6-debounced-onChange', { rightAfterA, afterDelayA: snapshot(s) });
  });

  it('C1-7 a setter wrapped in startTransition, raw input event outside act()', async () => {
    const s = seen();
    await render(
      <Harness
        s={s}
        controlled
        rootProps={{ validationMode: 'onChange' }}
        setter={(next, set) => React.startTransition(() => set(next))}
      />,
    );
    const g = globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean };
    const previous = g.IS_REACT_ACT_ENVIRONMENT;
    g.IS_REACT_ACT_ENVIRONMENT = false;
    let synchronouslyAfterEvent;
    let afterMicrotask;
    let after50ms;
    try {
      rawType('a');
      synchronouslyAfterEvent = snapshot(s);
      await Promise.resolve();
      afterMicrotask = snapshot(s);
      await new Promise((resolve) => {
        setTimeout(resolve, 50);
      });
      after50ms = snapshot(s);
    } finally {
      g.IS_REACT_ACT_ENVIRONMENT = previous;
    }
    report('C1-7-startTransition', { synchronouslyAfterEvent, afterMicrotask, after50ms });
  });

  it('C1-8 a setter that rejects anything but digits, empty field, default mode', async () => {
    const s = seen();
    await render(
      <Harness s={s} controlled setter={(next, set) => (/^\d*$/.test(next) ? set(next) : undefined)} />,
    );
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'x' } });
    const afterRejectedX = snapshot(s);
    fireEvent.change(screen.getByTestId('control'), { target: { value: '1' } });
    const afterAccepted1 = snapshot(s);
    fireEvent.change(screen.getByTestId('control'), { target: { value: '1x' } });
    report('C1-8-reject-digits-only', { afterRejectedX, afterAccepted1, afterRejected1x: snapshot(s) });
  });

  it('C1-9 a maxLength-style setter that returns the previous value past 3 characters, onChange mode', async () => {
    const s = seen();
    await render(
      <Harness
        s={s}
        controlled
        rootProps={{ validationMode: 'onChange' }}
        setter={(next, set) => set((previous) => (next.length > 3 ? previous : next))}
      />,
    );
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'abc' } });
    const afterAbc = snapshot(s);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'abcd' } });
    report('C1-9-reject-maxlength', { afterAbc, afterRejectedAbcd: snapshot(s) });
  });

  it('C1-10 a setter that rewrites the text to upper case, onChange mode', async () => {
    const s = seen();
    await render(
      <Harness
        s={s}
        controlled
        rootProps={{ validationMode: 'onChange' }}
        setter={(next, set) => set(next.toUpperCase())}
      />,
    );
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a' } });
    report('C1-10-transform-uppercase', { afterTypingA: snapshot(s) });
  });

  it('C1-11 required field, onBlur mode, digits-only setter: a rejected keystroke, then blur', async () => {
    const s = seen();
    await render(
      <Harness
        s={s}
        controlled
        rootProps={{ validationMode: 'onBlur' }}
        controlProps={{ required: true }}
        setter={(next, set) => (/^\d*$/.test(next) ? set(next) : undefined)}
      />,
    );
    const control = screen.getByTestId('control');
    fireEvent.focus(control);
    fireEvent.change(control, { target: { value: 'x' } });
    const afterRejectedX = snapshot(s);
    fireEvent.blur(control);
    report('C1-11-required-rejected-then-blur', { afterRejectedX, afterBlur: snapshot(s) });
  });

  it('C1-12 required field, onBlur mode, debounced setter (30 ms): type, blur before the update lands', async () => {
    const s = seen();
    await render(
      <Harness
        s={s}
        controlled
        rootProps={{ validationMode: 'onBlur' }}
        controlProps={{ required: true }}
        setter={debounced(30)}
      />,
    );
    const control = screen.getByTestId('control');
    fireEvent.focus(control);
    fireEvent.change(control, { target: { value: 'a' } });
    const rightAfterA = snapshot(s);
    fireEvent.blur(control);
    const afterBlur = snapshot(s);
    await wait(60);
    report('C1-12-required-debounced-blur-early', { rightAfterA, afterBlur, afterDelay: snapshot(s) });
  });

  it('C1-13 required field, onBlur mode, ordinary setter: type "a", empty it, blur', async () => {
    const s = seen();
    await render(
      <Harness s={s} controlled rootProps={{ validationMode: 'onBlur' }} controlProps={{ required: true }} />,
    );
    const control = screen.getByTestId('control');
    fireEvent.focus(control);
    fireEvent.change(control, { target: { value: 'a' } });
    fireEvent.change(control, { target: { value: '' } });
    fireEvent.blur(control);
    report('C1-13-required-sync-empty-blur', { afterBlur: snapshot(s) });
  });
});

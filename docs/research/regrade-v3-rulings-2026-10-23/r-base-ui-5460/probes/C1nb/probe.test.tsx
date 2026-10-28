import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

// Probe C1nb: what app code can observe of the render in which a controlled control already holds
// the new text and the field state does not yet reflect it. The app renders the input through the
// control's `render` prop, as Base UI documents, and its component reads the `state` argument.
// It records:
// - every render (text and state);
// - a passive effect and a layout effect that depend on the text only ([value]);
// - a passive effect that runs after every commit (no dependency list);
// - a passive effect that depends on the marks only ([dirty, filled]);
// - the field's data attributes read synchronously right after the change event returns, before
//   the test yields to the browser.
// Case N3 adds an app that refuses a keystroke, to show what each smallest fix does with it.
// Observations only. Each case prints one PROBE line and asserts nothing.
// An entry is written "text|D<dirty>F<filled>V<valid>", with V- for "not yet validated" (null).
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

interface FieldStateLike {
  dirty: boolean;
  filled: boolean;
  valid: boolean | null;
}

interface Log {
  renders: string[];
  effectOnText: string[];
  layoutEffectOnText: string[];
  effectEveryCommit: string[];
  effectOnMarks: string[];
}

function newLog(): Log {
  return {
    renders: [],
    effectOnText: [],
    layoutEffectOnText: [],
    effectEveryCommit: [],
    effectOnMarks: [],
  };
}

function snap(value: unknown, state: FieldStateLike) {
  let valid = '-';
  if (state.valid === true) {
    valid = '1';
  } else if (state.valid === false) {
    valid = '0';
  }
  return `${String(value)}|D${state.dirty ? 1 : 0}F${state.filled ? 1 : 0}V${valid}`;
}

function AppInput(props: { p: Record<string, unknown>; s: FieldStateLike; log: Log }) {
  const { p, s, log } = props;
  const entry = snap(p.value, s);
  log.renders.push(entry);
  const latest = React.useRef(entry);
  latest.current = entry;
  React.useEffect(() => {
    log.effectOnText.push(latest.current);
  }, [p.value, log]);
  React.useLayoutEffect(() => {
    log.layoutEffectOnText.push(latest.current);
  }, [p.value, log]);
  React.useEffect(() => {
    log.effectEveryCommit.push(latest.current);
  });
  React.useEffect(() => {
    log.effectOnMarks.push(latest.current);
  }, [s.dirty, s.filled, log]);
  return <input {...p} />;
}

function Controlled(props: {
  log: Log;
  rootProps?: Record<string, unknown>;
  accept?: (next: string) => boolean;
}) {
  const { log, rootProps, accept = () => true } = props;
  const [value, setValue] = React.useState('');
  return (
    <Field.Root data-testid="root" name="field" {...rootProps}>
      <Field.Control
        data-testid="control"
        value={value}
        onValueChange={(next: string) => {
          if (accept(next)) {
            setValue(next);
          }
        }}
        render={(p, s) => <AppInput p={p as Record<string, unknown>} s={s} log={log} />}
      />
    </Field.Root>
  );
}

function type(log: Log, texts: string[]) {
  return texts.map((text) => {
    const r0 = log.renders.length;
    const e0 = log.effectOnText.length;
    const l0 = log.layoutEffectOnText.length;
    const a0 = log.effectEveryCommit.length;
    const m0 = log.effectOnMarks.length;
    fireEvent.change(screen.getByTestId('control'), { target: { value: text } });
    const root = screen.getByTestId('root');
    const input = screen.getByTestId('control') as HTMLInputElement;
    return {
      typed: text,
      renders: log.renders.slice(r0),
      effectOnText: log.effectOnText.slice(e0),
      layoutEffectOnText: log.layoutEffectOnText.slice(l0),
      effectEveryCommit: log.effectEveryCommit.slice(a0),
      effectOnMarks: log.effectOnMarks.slice(m0),
      domRightAfterEvent: `${input.value}|D${root.hasAttribute('data-dirty') ? 1 : 0}F${root.hasAttribute('data-filled') ? 1 : 0}I${root.hasAttribute('data-invalid') ? 1 : 0}`,
    };
  });
}

describe('C1nb probe', () => {
  const { render } = createRenderer({ strict: false });

  it('N1 controlled, default mode, no validator', async () => {
    const log = newLog();
    await render(<Controlled log={log} />);
    report('N1-default', { keystrokes: type(log, ['a', 'ab', 'abc', '', 'x']) });
  });

  it('N2 controlled, validationMode="onChange", validator rejects under 3 characters', async () => {
    const log = newLog();
    await render(
      <Controlled
        log={log}
        rootProps={{
          validationMode: 'onChange',
          validate: (v: unknown) => (String(v).length >= 3 ? null : 'Too short'),
        }}
      />,
    );
    report('N2-onChange-rejects-short', { keystrokes: type(log, ['a', 'ab', 'abc', 'abcd']) });
  });

  it('N3 controlled, default mode, the app refuses anything but digits; type "x", then "1"', async () => {
    const log = newLog();
    await render(<Controlled log={log} accept={(next) => /^\d*$/.test(next)} />);
    report('N3-default-refuses-non-digits', { keystrokes: type(log, ['x', '1']) });
  });
});

import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Probe C1: for each keystroke, every render of the control and of Field.Root with the state that
// render carried, and the renders of a part that reads the field's context (Field.Description).
// Observations only. Each case prints one PROBE line and asserts nothing. Renders of the control
// are counted as the pull request's own test counts them: a `render` prop on the control.
// A render is written "text|D<dirty>F<filled>V<valid>", with V- for "not yet validated" (null).
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
  control: string[];
  root: string[];
  description: number;
  validate: unknown[];
}

function newLog(): Log {
  return { control: [], root: [], description: 0, validate: [] };
}

function snap(value: unknown, state: FieldStateLike) {
  let valid = '-';
  if (state.valid === true) {
    valid = '1';
  } else if (state.valid === false) {
    valid = '0';
  }
  return `${value === undefined ? '?' : String(value)}|D${state.dirty ? 1 : 0}F${state.filled ? 1 : 0}V${valid}`;
}

function type(log: Log, texts: string[]) {
  return texts.map((text) => {
    const c0 = log.control.length;
    const r0 = log.root.length;
    const d0 = log.description;
    const v0 = log.validate.length;
    fireEvent.change(screen.getByTestId('control'), { target: { value: text } });
    const root = screen.getByTestId('root');
    return {
      typed: text,
      controlRenders: log.control.slice(c0),
      rootRenders: log.root.slice(r0),
      descriptionRenders: log.description - d0,
      validateCalls: log.validate.length - v0,
      domAfter: `D${root.hasAttribute('data-dirty') ? 1 : 0}F${root.hasAttribute('data-filled') ? 1 : 0}I${root.hasAttribute('data-invalid') ? 1 : 0}`,
      error: screen.queryByTestId('error')?.textContent ?? null,
    };
  });
}

function Parts(props: {
  log: Log;
  controlProps: Record<string, unknown>;
  rootProps?: Record<string, unknown>;
}) {
  const { log, controlProps, rootProps } = props;
  return (
    <Field.Root
      data-testid="root"
      name="field"
      {...rootProps}
      render={(p, s) => {
        log.root.push(snap(undefined, s));
        return <div {...p} />;
      }}
    >
      <Field.Control
        data-testid="control"
        {...controlProps}
        render={(p, s) => {
          log.control.push(snap((p as { value?: unknown }).value, s));
          return <input {...p} />;
        }}
      />
      <Field.Description
        render={(p) => {
          log.description += 1;
          return <p {...p} />;
        }}
      >
        Hint
      </Field.Description>
      <Field.Error data-testid="error" />
    </Field.Root>
  );
}

function Controlled(props: { log: Log; initial?: string; rootProps?: Record<string, unknown> }) {
  const { log, initial = '', rootProps } = props;
  const [value, setValue] = React.useState(initial);
  return (
    <Parts log={log} rootProps={rootProps} controlProps={{ value, onValueChange: setValue }} />
  );
}

function recording(log: Log, verdict: (v: unknown) => string | null) {
  return (v: unknown) => {
    log.validate.push(v);
    return verdict(v);
  };
}

describe('C1 probe', () => {
  const { render } = createRenderer({ strict: false });

  it('C1-1 controlled, default mode, no validator', async () => {
    const log = newLog();
    await render(<Controlled log={log} />);
    report('C1-1-controlled-default', { keystrokes: type(log, ['a', 'ab', 'abc', '', 'x']) });
  });

  it('C1-2 controlled, validationMode="onChange", validator accepts', async () => {
    const log = newLog();
    await render(
      <Controlled
        log={log}
        rootProps={{ validationMode: 'onChange', validate: recording(log, () => null) }}
      />,
    );
    report('C1-2-controlled-onChange-accepts', { keystrokes: type(log, ['a', 'ab', 'abc']) });
  });

  it('C1-3 controlled, validationMode="onChange", validator rejects under 3 characters', async () => {
    const log = newLog();
    await render(
      <Controlled
        log={log}
        rootProps={{
          validationMode: 'onChange',
          validate: recording(log, (v) => (String(v).length >= 3 ? null : 'Too short')),
        }}
      />,
    );
    report('C1-3-controlled-onChange-rejects-short', {
      keystrokes: type(log, ['a', 'ab', 'abc', 'abcd']),
    });
  });

  it('C1-4 controlled, default mode in a Form, before and after a submit attempt', async () => {
    const log = newLog();
    await render(
      <Form>
        <Controlled log={log} rootProps={{ validate: recording(log, () => null) }} />
        <button type="submit">Send</button>
      </Form>,
    );
    const beforeSubmit = type(log, ['a', 'ab']);
    fireEvent.click(screen.getByText('Send'));
    report('C1-4-controlled-default-after-submit', {
      beforeSubmit,
      afterSubmit: type(log, ['abc', 'abcd']),
    });
  });

  it('C1-5 controlled, default mode, Field.Root invalid', async () => {
    const log = newLog();
    await render(<Controlled log={log} rootProps={{ invalid: true }} />);
    report('C1-5-controlled-root-invalid', { keystrokes: type(log, ['a', 'ab', 'abc']) });
  });

  it('C1-6 uncontrolled, default mode, no validator', async () => {
    const log = newLog();
    await render(<Parts log={log} controlProps={{ defaultValue: '' }} />);
    report('C1-6-uncontrolled-default', { keystrokes: type(log, ['a', 'ab', 'abc', '', 'x']) });
  });
});

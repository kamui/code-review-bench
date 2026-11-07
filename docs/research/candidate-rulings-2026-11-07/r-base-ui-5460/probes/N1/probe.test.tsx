import * as React from 'react';
import { describe, it } from 'bun:test';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Field } from '../index';
import { Form } from '../../form';

// Probe N1: renders of a controlled Field.Control on the first keystroke and on the keystrokes
// after it, in each validation mode. Observations only: each case prints one PROBE line and
// asserts nothing. Renders are counted as the pull request's own test counts them, with a
// `render` prop on the control. Copied to packages/react/src/field/control/zzprobe.test.tsx.
function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

interface Counters {
  control: number;
  validate: unknown[];
}

function type(c: Counters, texts: string[]) {
  return texts.map((text) => {
    const renders = c.control;
    const calls = c.validate.length;
    fireEvent.change(screen.getByTestId('control'), { target: { value: text } });
    const root = screen.getByTestId('root');
    return {
      typed: text,
      controlRenders: c.control - renders,
      validateCalls: c.validate.length - calls,
      dirty: root.hasAttribute('data-dirty'),
      filled: root.hasAttribute('data-filled'),
      invalid: root.hasAttribute('data-invalid'),
      error: screen.queryByTestId('error')?.textContent || null,
    };
  });
}

function Counted(props: { c: Counters; rootProps?: Record<string, unknown> }) {
  const { c, rootProps } = props;
  const [value, setValue] = React.useState('');
  return (
    <Field.Root data-testid="root" name="field" {...rootProps}>
      <Field.Control
        data-testid="control"
        value={value}
        onValueChange={setValue}
        render={(p) => {
          c.control += 1;
          return <input {...p} />;
        }}
      />
      <Field.Error data-testid="error" />
    </Field.Root>
  );
}

function UncontrolledCounted(props: { c: Counters }) {
  const { c } = props;
  return (
    <Field.Root data-testid="root" name="field">
      <Field.Control
        data-testid="control"
        render={(p) => {
          c.control += 1;
          return <input {...p} />;
        }}
      />
    </Field.Root>
  );
}

function recording(c: Counters, verdict: (v: unknown) => string | null) {
  return (v: unknown) => {
    c.validate.push(v);
    return verdict(v);
  };
}

describe('N1 probe', () => {
  it('P1 default mode, no validator, before any submit (the setup of the change own test)', () => {
    const c: Counters = { control: 0, validate: [] };
    render(<Counted c={c} />);
    report('P1-default-no-validator', { keystrokes: type(c, ['a', 'ab', 'abc']) });
    cleanup();
  });

  it('P2 default mode with a validator, before any submit', () => {
    const c: Counters = { control: 0, validate: [] };
    render(<Counted c={c} rootProps={{ validate: recording(c, () => null) }} />);
    report('P2-default-validator-before-submit', { keystrokes: type(c, ['a', 'ab', 'abc']) });
    cleanup();
  });

  it('P3 validationMode="onChange", validator accepts', () => {
    const c: Counters = { control: 0, validate: [] };
    render(
      <Counted c={c} rootProps={{ validationMode: 'onChange', validate: recording(c, () => null) }} />,
    );
    report('P3-onChange-accepts', { keystrokes: type(c, ['a', 'ab', 'abc']) });
    cleanup();
  });

  it('P4 validationMode="onChange", validator rejects', () => {
    const c: Counters = { control: 0, validate: [] };
    render(
      <Counted c={c} rootProps={{ validationMode: 'onChange', validate: recording(c, () => 'Wrong') }} />,
    );
    report('P4-onChange-rejects', { keystrokes: type(c, ['a', 'ab', 'abc']) });
    cleanup();
  });

  it('P5 validationMode="onBlur", before any blur', () => {
    const c: Counters = { control: 0, validate: [] };
    render(
      <Counted c={c} rootProps={{ validationMode: 'onBlur', validate: recording(c, () => null) }} />,
    );
    report('P5-onBlur-before-blur', { keystrokes: type(c, ['a', 'ab', 'abc']) });
    cleanup();
  });

  it('P6 Field.Root invalid', () => {
    const c: Counters = { control: 0, validate: [] };
    render(<Counted c={c} rootProps={{ invalid: true }} />);
    report('P6-root-invalid', { keystrokes: type(c, ['a', 'ab', 'abc']) });
    cleanup();
  });

  it('P7 default mode inside a Form, first keystroke after a submit attempt on an empty field', () => {
    const c: Counters = { control: 0, validate: [] };
    render(
      <Form>
        <Counted c={c} rootProps={{ validate: recording(c, () => null) }} />
        <button type="submit">Send</button>
      </Form>,
    );
    fireEvent.click(screen.getByText('Send'));
    report('P7-default-after-submit', { keystrokes: type(c, ['a', 'ab', 'abc']) });
    cleanup();
  });

  it('P8 uncontrolled control, default mode', () => {
    const c: Counters = { control: 0, validate: [] };
    render(<UncontrolledCounted c={c} />);
    report('P8-uncontrolled-default', { keystrokes: type(c, ['a', 'ab', 'abc']) });
    cleanup();
  });
});

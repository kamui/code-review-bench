import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Probe C1na: renders of a controlled Field.Control per keystroke, in the cases where the field's
// dirty or filled mark flips (this group) and in the failing cases of known problem GT-r10.
// Observations only. Each case prints one PROBE line and asserts nothing. Run at the base, the
// head, and the head with each smallest fix applied. Renders are counted as the pull request's own
// test counts them: a `render` prop on the control.
function report(name: string, data: Record<string, unknown>) {
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
      error: screen.queryByTestId('error')?.textContent ?? null,
    };
  });
}

function Counted(props: { c: Counters; initial?: string; rootProps?: Record<string, unknown> }) {
  const { c, initial = '', rootProps } = props;
  const [value, setValue] = React.useState(initial);
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

function recording(c: Counters, verdict: (v: unknown) => string | null) {
  return (v: unknown) => {
    c.validate.push(v);
    return verdict(v);
  };
}

describe('C1na probe', () => {
  const { render } = createRenderer({ strict: false });

  it('G1 default mode, no validator: dirty and filled flip on "a", "" and "x"', async () => {
    const c: Counters = { control: 0, validate: [] };
    await render(<Counted c={c} />);
    report('G1-default-flips', { keystrokes: type(c, ['a', 'ab', 'abc', '', 'x', 'xy']) });
  });

  it('G2 default mode, no validator, starting from "abc": only dirty flips', async () => {
    const c: Counters = { control: 0, validate: [] };
    await render(<Counted c={c} initial="abc" />);
    report('G2-default-dirty-only-flips', { keystrokes: type(c, ['abcd', 'abcde', 'abc', 'ab']) });
  });

  it('G3 validationMode="onChange", validator accepts (GT-r10)', async () => {
    const c: Counters = { control: 0, validate: [] };
    await render(
      <Counted c={c} rootProps={{ validationMode: 'onChange', validate: recording(c, () => null) }} />,
    );
    report('G3-onChange-accepts', { keystrokes: type(c, ['a', 'ab', 'abc']) });
  });

  it('G4 validationMode="onChange", validator rejects (GT-r10)', async () => {
    const c: Counters = { control: 0, validate: [] };
    await render(
      <Counted c={c} rootProps={{ validationMode: 'onChange', validate: recording(c, () => 'Wrong') }} />,
    );
    report('G4-onChange-rejects', { keystrokes: type(c, ['a', 'ab', 'abc']) });
  });

  it('G5 default mode inside a Form, after a submit attempt (GT-r10)', async () => {
    const c: Counters = { control: 0, validate: [] };
    await render(
      <Form>
        <Counted c={c} rootProps={{ validate: recording(c, () => null) }} />
        <button type="submit">Send</button>
      </Form>,
    );
    const beforeSubmit = type(c, ['a', 'ab']);
    fireEvent.click(screen.getByText('Send'));
    report('G5-default-after-submit', { beforeSubmit, afterSubmit: type(c, ['abc', 'abcd', 'abcde']) });
  });

  it('G6 default mode, Field.Root invalid (GT-r10)', async () => {
    const c: Counters = { control: 0, validate: [] };
    await render(<Counted c={c} rootProps={{ invalid: true }} />);
    report('G6-default-root-invalid', { keystrokes: type(c, ['a', 'ab', 'abc']) });
  });

  it('G7 validationMode="onBlur": a field made invalid by blur, then typing (GT-r10)', async () => {
    const c: Counters = { control: 0, validate: [] };
    await render(
      <Counted
        c={c}
        rootProps={{
          validationMode: 'onBlur',
          validate: recording(c, (v) => (String(v).length >= 5 ? null : 'Too short')),
        }}
      />,
    );
    const firstTyping = type(c, ['a', 'ab']);
    fireEvent.focus(screen.getByTestId('control'));
    fireEvent.blur(screen.getByTestId('control'));
    report('G7-onBlur-invalid-then-typing', {
      firstTyping,
      typingWhileInvalid: type(c, ['abc', 'abcd', 'abcde']),
    });
  });
});

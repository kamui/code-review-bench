import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Observations only. Each case prints one PROBE line.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(calls: unknown[]) {
  const root = screen.getByTestId('root');
  return {
    domValue: screen.getByTestId<HTMLInputElement>('control').value,
    validateCalls: calls,
    rootFilled: root.hasAttribute('data-filled'),
    rootDirty: root.hasAttribute('data-dirty'),
    rootInvalid: root.hasAttribute('data-invalid'),
  };
}

function Harness(props: {
  calls: unknown[];
  type?: string;
  initial: string;
  next: string;
  rewrite?: (typed: string) => string;
  min?: number;
  max?: number;
}) {
  const { calls, type = 'text', initial, next, rewrite, min, max } = props;
  const [value, setValue] = React.useState(initial);
  return (
    <Form>
      <Field.Root
        data-testid="root"
        name="field"
        validationMode="onChange"
        validate={(v, formValues) => {
          calls.push({ validateArgument: v, formValueOfSameField: formValues.field });
          return null;
        }}
      >
        <Field.Control
          data-testid="control"
          type={type}
          min={min}
          max={max}
          value={value}
          onValueChange={(typed) => setValue(rewrite ? rewrite(typed) : typed)}
        />
      </Field.Root>
      <button type="button" onClick={() => setValue(next)}>
        set
      </button>
    </Form>
  );
}

describe('B4 probe: the value the validator gets versus the value the input holds', () => {
  const { render } = createRenderer();

  it('B4-1 text input, code sets "a\\n" (the browser strips the line break)', async () => {
    const calls: unknown[] = [];
    await render(<Harness calls={calls} initial="" next={'a\n'} />);
    fireEvent.click(screen.getByText('set'));
    report('B4-1-text-code-sets-a-newline', snapshot(calls));
  });

  it('B4-2 text input, code sets "\\n" only (the input ends up empty)', async () => {
    const calls: unknown[] = [];
    await render(<Harness calls={calls} initial="" next={'\n'} />);
    fireEvent.click(screen.getByText('set'));
    report('B4-2-text-code-sets-newline-only', snapshot(calls));
  });

  it('B4-3 text input, the person types "a" and the consumer rewrites it to "a\\n"', async () => {
    const calls: unknown[] = [];
    await render(<Harness calls={calls} initial="" next="" rewrite={(typed) => `${typed}\n`} />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a' } });
    report('B4-3-text-user-edit-rewritten-with-newline', snapshot(calls));
  });

  it('B4-4 range input min 0 max 100, code sets "200" (the browser clamps to 100)', async () => {
    const calls: unknown[] = [];
    await render(<Harness calls={calls} type="range" min={0} max={100} initial="50" next="200" />);
    fireEvent.click(screen.getByText('set'));
    report('B4-4-range-code-sets-200', snapshot(calls));
  });

  it('B4-5 reference: text input, code sets "abc" (nothing to sanitize)', async () => {
    const calls: unknown[] = [];
    await render(<Harness calls={calls} initial="" next="abc" />);
    fireEvent.click(screen.getByText('set'));
    report('B4-5-ref-text-code-sets-abc', snapshot(calls));
  });
});

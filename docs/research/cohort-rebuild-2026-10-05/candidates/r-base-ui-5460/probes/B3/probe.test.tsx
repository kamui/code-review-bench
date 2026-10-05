import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Observations only. Each case prints one PROBE line.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function describeValue(v: unknown) {
  return { type: Array.isArray(v) ? 'array' : typeof v, value: v };
}

describe('B3 probe: what the validate callback receives for a non-string controlled value', () => {
  const { render } = createRenderer();

  it('B3-1 value={5}: Form submit, actionsRef.validate(), typing, blur, and Field.Validity', async () => {
    const calls: Record<string, unknown>[] = [];
    let phase = 'mount';
    let lastValidity: Record<string, unknown> = {};
    const actionsRef = React.createRef<Field.Root.Actions>();
    function App() {
      const [value, setValue] = React.useState<number | string>(5);
      return (
        <Form onFormSubmit={() => {}}>
          <Field.Root
            name="amount"
            actionsRef={actionsRef}
            validate={(v) => {
              calls.push({ phase, ...describeValue(v) });
              // A validator written for the number the consumer passed in.
              return typeof v === 'number' && v > 3 ? null : 'Must be a number above 3';
            }}
          >
            <Field.Control data-testid="control" value={value} onValueChange={setValue} />
            <Field.Error data-testid="error" />
            <Field.Validity>
              {(validity) => {
                lastValidity = {
                  value: describeValue(validity.value),
                  initialValue: describeValue(validity.initialValue),
                };
                return null;
              }}
            </Field.Validity>
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    const validityAtMount = lastValidity;

    phase = 'form-submit';
    fireEvent.click(screen.getByText('Send'));
    const errorAfterSubmit = screen.queryByTestId('error')?.textContent ?? null;
    const controlInvalidAfterSubmit = screen.getByTestId('control').getAttribute('aria-invalid');

    phase = 'actionsRef.validate';
    await act(async () => {
      actionsRef.current?.validate();
    });

    phase = 'typing';
    fireEvent.change(screen.getByTestId('control'), { target: { value: '6' } });

    report('B3-1-number-value', {
      validityAtMount,
      calls,
      errorAfterSubmit,
      controlInvalidAfterSubmit,
    });
  });

  it('B3-2 value={5}, validationMode="onBlur": blur', async () => {
    const calls: Record<string, unknown>[] = [];
    function App() {
      const [value, setValue] = React.useState<number | string>(5);
      return (
        <Field.Root
          name="amount"
          validationMode="onBlur"
          validate={(v) => {
            calls.push(describeValue(v));
            return null;
          }}
        >
          <Field.Control data-testid="control" value={value} onValueChange={setValue} />
        </Field.Root>
      );
    }
    await render(<App />);
    const control = screen.getByTestId('control');
    fireEvent.focus(control);
    fireEvent.blur(control);
    report('B3-2-number-value-blur', { calls });
  });

  it('B3-3 value={["a","b"]}: Form submit', async () => {
    const calls: Record<string, unknown>[] = [];
    function App() {
      const [value] = React.useState<string[]>(['a', 'b']);
      return (
        <Form onFormSubmit={() => {}}>
          <Field.Root
            name="tags"
            validate={(v) => {
              calls.push(describeValue(v));
              return null;
            }}
          >
            <Field.Control data-testid="control" value={value} onValueChange={() => {}} />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('Send'));
    report('B3-3-array-value-submit', { calls });
  });

  it('B3-4 reference, value="5" (a string): Form submit', async () => {
    const calls: Record<string, unknown>[] = [];
    function App() {
      const [value, setValue] = React.useState('5');
      return (
        <Form onFormSubmit={() => {}}>
          <Field.Root
            name="amount"
            validate={(v) => {
              calls.push(describeValue(v));
              return null;
            }}
          >
            <Field.Control data-testid="control" value={value} onValueChange={setValue} />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('Send'));
    report('B3-4-ref-string-value-submit', { calls });
  });
});

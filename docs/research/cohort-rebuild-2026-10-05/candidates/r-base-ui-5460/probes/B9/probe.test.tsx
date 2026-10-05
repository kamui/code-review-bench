import * as React from 'react';
import { vi } from 'vitest';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

const SERVER_ERRORS = { message: 'Server error' };

// Observations only. Each case prints one PROBE line.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(calls: unknown[]) {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>('control');
  return {
    inputValue: control.value,
    validateCalls: [...calls],
    ariaInvalid: control.getAttribute('aria-invalid'),
    rootInvalid: root.hasAttribute('data-invalid'),
    rootDirty: root.hasAttribute('data-dirty'),
    rootFilled: root.hasAttribute('data-filled'),
    serverErrorShown: screen.queryByText('Server error') !== null,
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

// `null` is not in the declared type of the `value` prop, so the probe has to cast.
type AnyValue = any;

describe('B9 probe: a controlled Field.Control whose value becomes null', () => {
  const { render } = createRenderer();

  it('B9-1 the person deletes the text and the consumer maps empty text to null', async () => {
    // React itself warns about value={null}; this repository's tests fail on console.error,
    // so capture the warnings instead.
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState<string | null>('abc');
      return (
        <Form errors={SERVER_ERRORS}>
          <Field.Root
            data-testid="root"
            name="message"
            validationMode="onChange"
            validate={(v) => {
              calls.push(v);
              return null;
            }}
          >
            <Field.Control
              data-testid="control"
              required
              value={value as AnyValue}
              onValueChange={(next) => setValue(next || null)}
            />
            <Field.Error data-testid="error" />
          </Field.Root>
        </Form>
      );
    }
    await render(<App />);
    const atMount = snapshot(calls);
    fireEvent.change(screen.getByTestId('control'), { target: { value: '' } });
    const afterUserClears = snapshot(calls);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'x' } });
    const afterTypingAgain = snapshot(calls);
    report('B9-1-user-clears-mapped-to-null', {
      atMount,
      afterUserClears,
      afterTypingAgain,
      reactWarnings: consoleError.mock.calls.map((args) => String(args[0]).slice(0, 160)),
    });
  });

  it('B9-2 reference: the same, but empty text stays an empty string', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('abc');
      return (
        <Form errors={SERVER_ERRORS}>
          <Field.Root
            data-testid="root"
            name="message"
            validationMode="onChange"
            validate={(v) => {
              calls.push(v);
              return null;
            }}
          >
            <Field.Control data-testid="control" required value={value} onValueChange={setValue} />
            <Field.Error data-testid="error" />
          </Field.Root>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: '' } });
    report('B9-2-ref-user-clears-empty-string', { afterUserClears: snapshot(calls) });
  });

  it('B9-3 code resets the value from "abc" to null', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState<string | null>('');
      return (
        <Form errors={SERVER_ERRORS}>
          <Field.Root
            data-testid="root"
            name="message"
            validationMode="onChange"
            validate={(v) => {
              calls.push(v);
              return (v as string).length < 5 ? 'Too short' : null;
            }}
          >
            <Field.Control
              data-testid="control"
              value={value as AnyValue}
              onValueChange={(next) => setValue(next)}
            />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="button" onClick={() => setValue(null)}>
            reset
          </button>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'abc' } });
    const afterTyping = snapshot(calls);
    fireEvent.click(screen.getByText('reset'));
    report('B9-3-code-resets-to-null', {
      afterTyping,
      afterReset: snapshot(calls),
      reactWarnings: consoleError.mock.calls.map((args) => String(args[0]).slice(0, 160)),
    });
  });
});

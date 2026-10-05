import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { NumberField } from '@base-ui/react/number-field';

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
    inputDisabled: control.disabled,
    validateCalls: [...calls],
    controlAriaInvalid: control.getAttribute('aria-invalid'),
    controlDataInvalid: control.hasAttribute('data-invalid'),
    rootInvalid: root.hasAttribute('data-invalid'),
    rootDirty: root.hasAttribute('data-dirty'),
    rootFilled: root.hasAttribute('data-filled'),
    rootDisabled: root.hasAttribute('data-disabled'),
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

describe('B7 probe: a code-driven value change on a disabled controlled Field.Control', () => {
  const { render } = createRenderer();

  it('B7-1 control-level disabled, onChange mode, failing validator: code changes foo -> bar', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('foo');
      const [disabled, setDisabled] = React.useState(true);
      return (
        <Field.Root
          data-testid="root"
          name="field"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return 'Always wrong';
          }}
        >
          <Field.Control data-testid="control" value={value} onValueChange={setValue} disabled={disabled} />
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue('bar')}>
            bar
          </button>
          <button type="button" onClick={() => setValue('foo')}>
            foo
          </button>
          <button type="button" onClick={() => setDisabled(false)}>
            enable
          </button>
        </Field.Root>
      );
    }
    await render(<App />);
    const atMount = snapshot(calls);
    fireEvent.click(screen.getByText('bar'));
    const afterCodeChangeWhileDisabled = snapshot(calls);
    fireEvent.click(screen.getByText('enable'));
    fireEvent.click(screen.getByText('foo'));
    const afterEnableAndBackToInitial = snapshot(calls);
    report('B7-1-control-disabled', {
      atMount,
      afterCodeChangeWhileDisabled,
      afterEnableAndBackToInitial,
    });
  });

  it('B7-2 Field.Root disabled, onChange mode, failing validator: code changes foo -> bar', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('foo');
      return (
        <Field.Root
          data-testid="root"
          name="field"
          disabled
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return 'Always wrong';
          }}
        >
          <Field.Control data-testid="control" value={value} onValueChange={setValue} />
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue('bar')}>
            bar
          </button>
        </Field.Root>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('bar'));
    report('B7-2-root-disabled', { afterCodeChangeWhileDisabled: snapshot(calls) });
  });

  it('B7-3 reference: the same control enabled, code changes foo -> bar', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('foo');
      return (
        <Field.Root
          data-testid="root"
          name="field"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return 'Always wrong';
          }}
        >
          <Field.Control data-testid="control" value={value} onValueChange={setValue} />
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue('bar')}>
            bar
          </button>
        </Field.Root>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('bar'));
    report('B7-3-ref-enabled', { afterCodeChange: snapshot(calls) });
  });

  it('B7-4 sibling NumberField, control-level disabled: code changes 1 -> 2', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState<number | null>(1);
      return (
        <Field.Root
          data-testid="root"
          name="field"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return 'Always wrong';
          }}
        >
          <NumberField.Root value={value} onValueChange={setValue} disabled>
            <NumberField.Input data-testid="control" />
          </NumberField.Root>
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue(2)}>
            two
          </button>
        </Field.Root>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('two'));
    report('B7-4-sibling-NumberField-disabled', { afterCodeChangeWhileDisabled: snapshot(calls) });
  });

  it('B7-5 inside a Form: a disabled control showing a code-driven error, then submit', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('foo');
      return (
        <Form validationMode="onChange" onFormSubmit={(values) => submitted.push(values)}>
          <Field.Root
            data-testid="root"
            name="field"
            validate={(v) => {
              calls.push(v);
              return 'Always wrong';
            }}
          >
            <Field.Control data-testid="control" value={value} onValueChange={setValue} disabled />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="button" onClick={() => setValue('bar')}>
            bar
          </button>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('bar'));
    const afterCodeChangeWhileDisabled = snapshot(calls);
    fireEvent.click(screen.getByText('Send'));
    report('B7-5-disabled-in-form-then-submit', {
      afterCodeChangeWhileDisabled,
      afterSubmit: snapshot(calls),
      submitted,
    });
  });
});

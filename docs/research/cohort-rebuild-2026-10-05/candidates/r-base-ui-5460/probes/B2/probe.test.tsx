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

function snapshot() {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>('control');
  return {
    inputValue: control.value,
    ariaInvalid: control.getAttribute('aria-invalid'),
    rootInvalid: root.hasAttribute('data-invalid'),
    rootValid: root.hasAttribute('data-valid'),
    rootDirty: root.hasAttribute('data-dirty'),
    rootFilled: root.hasAttribute('data-filled'),
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

describe('B2 probe: resetting a required controlled Field.Control to an empty value from code', () => {
  const { render } = createRenderer();

  it('B2-1 default mode: type, submit successfully, the submit handler clears the field', async () => {
    const submitted: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Form
          onFormSubmit={(values) => {
            submitted.push(values);
            setValue('');
          }}
        >
          <Field.Root data-testid="root" name="message">
            <Field.Control data-testid="control" required value={value} onValueChange={setValue} />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'hello' } });
    const afterTyping = snapshot();
    fireEvent.click(screen.getByText('Send'));
    report('B2-1-onSubmit-mode-submit-then-clear', { submitted, afterTyping, afterReset: snapshot() });
  });

  it('B2-2 onChange mode: type, then a Reset button sets the value back to empty', async () => {
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Field.Root data-testid="root" name="message" validationMode="onChange">
          <Field.Control data-testid="control" required value={value} onValueChange={setValue} />
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue('')}>
            Reset
          </button>
        </Field.Root>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'x' } });
    const afterTyping = snapshot();
    fireEvent.click(screen.getByText('Reset'));
    report('B2-2-onChange-mode-type-then-reset', { afterTyping, afterReset: snapshot() });
  });

  it('B2-3 reference, onChange mode: the person deletes the text by hand', async () => {
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Field.Root data-testid="root" name="message" validationMode="onChange">
          <Field.Control data-testid="control" required value={value} onValueChange={setValue} />
          <Field.Error data-testid="error" />
        </Field.Root>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'x' } });
    fireEvent.change(screen.getByTestId('control'), { target: { value: '' } });
    report('B2-3-ref-onChange-mode-manual-clear', { afterManualClear: snapshot() });
  });

  it('B2-4 default mode, no submit yet: type, then Reset', async () => {
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Form>
          <Field.Root data-testid="root" name="message">
            <Field.Control data-testid="control" required value={value} onValueChange={setValue} />
            <Field.Error data-testid="error" />
            <button type="button" onClick={() => setValue('')}>
              Reset
            </button>
          </Field.Root>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'x' } });
    fireEvent.click(screen.getByText('Reset'));
    report('B2-4-onSubmit-mode-no-submit-type-then-reset', { afterReset: snapshot() });
  });

  it('B2-5 onChange mode, never typed: code sets a value then code clears it', async () => {
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Field.Root data-testid="root" name="message" validationMode="onChange">
          <Field.Control data-testid="control" required value={value} onValueChange={setValue} />
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue('loaded')}>
            Load
          </button>
          <button type="button" onClick={() => setValue('')}>
            Reset
          </button>
        </Field.Root>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('Load'));
    const afterLoad = snapshot();
    fireEvent.click(screen.getByText('Reset'));
    report('B2-5-onChange-mode-code-load-then-code-reset', { afterLoad, afterReset: snapshot() });
  });

  // Sibling control the pull request says Field.Control now matches.
  it('B2-6 sibling NumberField, default mode: type, submit successfully, handler clears', async () => {
    const submitted: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState<number | null>(null);
      return (
        <Form
          onFormSubmit={(values) => {
            submitted.push(values);
            setValue(null);
          }}
        >
          <Field.Root data-testid="root" name="amount">
            <NumberField.Root required value={value} onValueChange={setValue}>
              <NumberField.Input data-testid="control" />
            </NumberField.Root>
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    const control = screen.getByTestId('control');
    fireEvent.change(control, { target: { value: '5' } });
    fireEvent.blur(control);
    const afterTyping = snapshot();
    fireEvent.click(screen.getByText('Send'));
    report('B2-6-sibling-NumberField-submit-then-clear', {
      submitted,
      afterTyping,
      afterReset: snapshot(),
    });
  });
});

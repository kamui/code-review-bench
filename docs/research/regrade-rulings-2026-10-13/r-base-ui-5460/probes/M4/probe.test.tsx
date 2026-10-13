// Probe M4: a field that shows an error from the Form's `errors` prop (a server error) while it
// is disabled, and whose value the app then changes from code.
// Observations only. Each case prints one PROBE line and asserts nothing.
import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { Switch } from '@base-ui/react/switch';
import { NumberField } from '@base-ui/react/number-field';

function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(calls: unknown[]) {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>('control');
  return {
    inputValue: control.value ?? null,
    rootDisabled: root.hasAttribute('data-disabled'),
    rootInvalid: root.hasAttribute('data-invalid'),
    rootDirty: root.hasAttribute('data-dirty'),
    controlAriaInvalid: control.getAttribute('aria-invalid'),
    validateCalls: [...calls],
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

type Where = 'root' | 'control' | 'none';

// One object for the life of the test, as when an app keeps the server's errors in state. A new
// object on every render makes Form copy it into its own state again each time, which puts a
// cleared error straight back (seen in a first version of this probe, which passed
// `errors={{ message: 'Server error' }}` inline: the error text never went away at either commit).
const SERVER_ERRORS = { message: 'Server error' };

function TextApp(props: {
  calls: unknown[];
  submitted: unknown[];
  where: Where;
  validationMode?: 'onChange' | 'onSubmit';
  withValidator?: boolean;
}) {
  const { calls, submitted, where, validationMode, withValidator } = props;
  const [value, setValue] = React.useState('old');
  return (
    <Form errors={SERVER_ERRORS} onFormSubmit={(values) => submitted.push(values)}>
      <Field.Root
        data-testid="root"
        name="message"
        disabled={where === 'root' ? true : undefined}
        validationMode={validationMode}
        validate={
          withValidator
            ? (v) => {
                calls.push(v);
                return null;
              }
            : undefined
        }
      >
        <Field.Control
          data-testid="control"
          value={value}
          onValueChange={setValue}
          disabled={where === 'control' ? true : undefined}
        />
        <Field.Error data-testid="error" />
      </Field.Root>
      <button type="button" onClick={() => setValue('new')}>
        code sets new
      </button>
      <button type="submit">Send</button>
    </Form>
  );
}

describe('M4 probe: a server error on a disabled field whose value changes from code', () => {
  const { render } = createRenderer();

  for (const where of ['root', 'control', 'none'] as Where[]) {
    it(`M4-1 Field.Control, disabled on ${where}, onChange mode with a validator that accepts`, async () => {
      const calls: unknown[] = [];
      const submitted: unknown[] = [];
      await render(
        <TextApp calls={calls} submitted={submitted} where={where} validationMode="onChange" withValidator />,
      );
      const atMount = snapshot(calls);
      fireEvent.click(screen.getByText('code sets new'));
      const afterCodeChange = snapshot(calls);
      fireEvent.click(screen.getByText('Send'));
      report(`M4-1-disabled-${where}-onChange`, {
        atMount,
        afterCodeChange,
        afterSend: snapshot(calls),
        submitted,
      });
    });

    it(`M4-2 Field.Control, disabled on ${where}, default mode, no validator`, async () => {
      const calls: unknown[] = [];
      const submitted: unknown[] = [];
      await render(<TextApp calls={calls} submitted={submitted} where={where} />);
      const atMount = snapshot(calls);
      fireEvent.click(screen.getByText('code sets new'));
      const afterCodeChange = snapshot(calls);
      report(`M4-2-disabled-${where}-default`, { atMount, afterCodeChange });
    });
  }

  it('M4-3 Field.Control, disabled on root: Send without any value change', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<TextApp calls={calls} submitted={submitted} where="root" />);
    fireEvent.click(screen.getByText('Send'));
    report('M4-3-disabled-root-send-with-server-error', { afterSend: snapshot(calls), submitted });
  });

  for (const where of ['root', 'control'] as Where[]) {
    it(`M4-4 sibling Switch, disabled on ${where}: the app flips it from code`, async () => {
      const calls: unknown[] = [];
      function App() {
        const [checked, setChecked] = React.useState(false);
        return (
          <Form errors={SERVER_ERRORS}>
            <Field.Root data-testid="root" name="message" disabled={where === 'root' ? true : undefined}>
              <Switch.Root
                data-testid="control"
                checked={checked}
                onCheckedChange={setChecked}
                disabled={where === 'control' ? true : undefined}
              />
              <Field.Error data-testid="error" />
            </Field.Root>
            <button type="button" onClick={() => setChecked(true)}>
              code sets new
            </button>
          </Form>
        );
      }
      await render(<App />);
      const atMount = snapshot(calls);
      fireEvent.click(screen.getByText('code sets new'));
      report(`M4-4-sibling-Switch-disabled-${where}`, { atMount, afterCodeChange: snapshot(calls) });
    });

    it(`M4-5 sibling NumberField, disabled on ${where}: the app changes 1 to 2 from code`, async () => {
      const calls: unknown[] = [];
      function App() {
        const [value, setValue] = React.useState<number | null>(1);
        return (
          <Form errors={SERVER_ERRORS}>
            <Field.Root data-testid="root" name="message" disabled={where === 'root' ? true : undefined}>
              <NumberField.Root
                value={value}
                onValueChange={setValue}
                disabled={where === 'control' ? true : undefined}
              >
                <NumberField.Input data-testid="control" />
              </NumberField.Root>
              <Field.Error data-testid="error" />
            </Field.Root>
            <button type="button" onClick={() => setValue(2)}>
              code sets new
            </button>
          </Form>
        );
      }
      await render(<App />);
      const atMount = snapshot(calls);
      fireEvent.click(screen.getByText('code sets new'));
      report(`M4-5-sibling-NumberField-disabled-${where}`, {
        atMount,
        afterCodeChange: snapshot(calls),
      });
    });
  }
});

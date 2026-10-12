import * as React from 'react';
import { act, createRenderer, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { Switch } from '@base-ui/react/switch';

// Observations only. Each case prints one PROBE line and asserts nothing.
// The pull request description as it read at the last push says, under "Open question": "The
// sibling controls also call `clearErrors(name)` when their value changes. This PR does not, so a
// programmatic change clears a server error on a Switch but not on an Input. I can align it
// either way." These cases record what a value set from code does to a server error on an Input
// (`Field.Control`) and on a Switch.

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

const ERRORS = { message: 'Server error', toggle: 'Server error' };

describe('D2 probe: a value set from code and a server error', () => {
  const { render } = createRenderer();

  for (const mode of [undefined, 'onChange'] as const) {
    const modeName = mode ?? 'default';

    it(`D2-1 ${modeName}: a server error is showing, then code sets the value`, async () => {
      let setValueFromCode: (next: string) => void = () => {};
      let setCheckedFromCode: (next: boolean) => void = () => {};
      function App() {
        const [value, setValue] = React.useState('abc');
        const [checked, setChecked] = React.useState(false);
        setValueFromCode = setValue;
        setCheckedFromCode = setChecked;
        return (
          <Form errors={ERRORS}>
            <Field.Root name="message" validationMode={mode}>
              <Field.Control data-testid="control" value={value} onValueChange={setValue} />
              <Field.Error data-testid="input-error" />
            </Field.Root>
            <Field.Root name="toggle" validationMode={mode}>
              <Switch.Root data-testid="toggle" checked={checked} onCheckedChange={setChecked} />
              <Field.Error data-testid="switch-error" />
            </Field.Root>
          </Form>
        );
      }
      await render(<App />);
      const read = () => ({
        inputShows: (screen.getByTestId('control') as HTMLInputElement).value,
        inputError: screen.queryByTestId('input-error')?.textContent ?? null,
        switchError: screen.queryByTestId('switch-error')?.textContent ?? null,
      });
      const atMount = read();
      await act(async () => {
        setValueFromCode('abc');
      });
      const afterCodeSetsTheSameValue = read();
      await act(async () => {
        setValueFromCode('xyz');
      });
      const afterCodeSetsInput = read();
      await act(async () => {
        setCheckedFromCode(true);
      });
      const afterCodeSetsSwitch = read();
      report(`D2-1-error-showing-then-code-sets-value mode=${modeName}`, {
        atMount,
        afterCodeSetsTheSameValue,
        afterCodeSetsInput,
        afterCodeSetsSwitch,
      });
    });

    it(`D2-2 ${modeName}: the app stores a server error and a new value in the same update`, async () => {
      let respond: () => void = () => {};
      function App() {
        const [value, setValue] = React.useState('abc');
        const [checked, setChecked] = React.useState(false);
        const [errors, setErrors] = React.useState<Form.Props['errors']>({});
        respond = () => {
          setErrors({ message: 'Server error', toggle: 'Server error' });
          setValue('ABC');
          setChecked(true);
        };
        return (
          <Form errors={errors}>
            <Field.Root name="message" validationMode={mode}>
              <Field.Control data-testid="control" value={value} onValueChange={setValue} />
              <Field.Error data-testid="input-error" />
            </Field.Root>
            <Field.Root name="toggle" validationMode={mode}>
              <Switch.Root data-testid="toggle" checked={checked} onCheckedChange={setChecked} />
              <Field.Error data-testid="switch-error" />
            </Field.Root>
          </Form>
        );
      }
      await render(<App />);
      await act(async () => {
        respond();
      });
      await act(async () => {});
      report(`D2-2-error-and-value-in-one-update mode=${modeName}`, {
        inputShows: (screen.getByTestId('control') as HTMLInputElement).value,
        inputError: screen.queryByTestId('input-error')?.textContent ?? null,
        switchError: screen.queryByTestId('switch-error')?.textContent ?? null,
      });
    });
  }
});

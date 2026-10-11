import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { Switch } from '@base-ui/react/switch';
import { Checkbox } from '@base-ui/react/checkbox';

// Observations only. Each case prints one PROBE line and asserts nothing.
// Three things the earlier probe did not cover:
//   P1  how the browser itself treats a disabled native input in constraint validation;
//   P2  sibling Base UI controls (Switch, Checkbox) disabled at control level, value changed from code;
//   P3  a Field.Control that is already invalid from a submit and is then disabled at control level,
//       with no value change: does Field.Root keep the invalid mark and the error text?

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

describe('B7 platform and sibling probe', () => {
  const { render } = createRenderer();

  it('P1 a native input: required and empty, then a custom error, enabled and disabled', async () => {
    await render(
      <form data-testid="form">
        <input data-testid="native" required defaultValue="" />
      </form>,
    );
    const input = screen.getByTestId('native') as HTMLInputElement;
    const form = screen.getByTestId('form') as HTMLFormElement;
    const read = () => ({
      disabled: input.disabled,
      willValidate: input.willValidate,
      checkValidity: input.checkValidity(),
      matchesInvalid: input.matches(':invalid'),
      matchesValid: input.matches(':valid'),
      valueMissing: input.validity.valueMissing,
      customError: input.validity.customError,
      validationMessageEmpty: input.validationMessage === '',
      formCheckValidity: form.checkValidity(),
    });
    const enabledRequiredEmpty = read();
    input.disabled = true;
    const disabledRequiredEmpty = read();
    input.setCustomValidity('Always wrong');
    const disabledWithCustomError = read();
    input.disabled = false;
    const enabledAgainWithCustomError = read();
    report('P1-native-disabled-input', {
      enabledRequiredEmpty,
      disabledRequiredEmpty,
      disabledWithCustomError,
      enabledAgainWithCustomError,
    });
  });

  it('P2 sibling Switch and Checkbox, disabled at control level, value changed from code', async () => {
    const switchCalls: unknown[] = [];
    const checkboxCalls: unknown[] = [];
    let change: () => void = () => {};
    function App() {
      const [on, setOn] = React.useState(false);
      change = () => setOn(true);
      return (
        <div>
          <Field.Root
            data-testid="switch-root"
            validationMode="onChange"
            validate={(value) => {
              switchCalls.push(value);
              return 'Always wrong';
            }}
          >
            <Switch.Root data-testid="switch" checked={on} onCheckedChange={() => {}} disabled />
            <Field.Error data-testid="switch-error" />
          </Field.Root>
          <Field.Root
            data-testid="checkbox-root"
            validationMode="onChange"
            validate={(value) => {
              checkboxCalls.push(value);
              return 'Always wrong';
            }}
          >
            <Checkbox.Root data-testid="checkbox" checked={on} onCheckedChange={() => {}} disabled />
            <Field.Error data-testid="checkbox-error" />
          </Field.Root>
        </div>
      );
    }
    await render(<App />);
    await act(async () => {
      change();
    });
    await act(async () => {});
    const read = (name: string, calls: unknown[]) => {
      const root = screen.getByTestId(`${name}-root`);
      return {
        validateCalls: calls,
        rootInvalid: root.hasAttribute('data-invalid'),
        rootDirty: root.hasAttribute('data-dirty'),
        errorText: screen.queryByTestId(`${name}-error`)?.textContent ?? null,
      };
    };
    report('P2-siblings-disabled-code-change', {
      switch: read('switch', switchCalls),
      checkbox: read('checkbox', checkboxCalls),
    });
  });

  it('P3 a Field.Control invalid from a submit, then disabled at control level, no value change', async () => {
    let disable: () => void = () => {};
    function App() {
      const [disabled, setDisabled] = React.useState(false);
      disable = () => setDisabled(true);
      return (
        <Form>
          <Field.Root data-testid="root" name="field" validate={() => 'Always wrong'}>
            <Field.Control data-testid="control" defaultValue="foo" disabled={disabled} />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    const read = () => {
      const root = screen.getByTestId('root');
      const control = screen.getByTestId('control') as HTMLInputElement;
      return {
        inputDisabled: control.disabled,
        controlAriaInvalid: control.getAttribute('aria-invalid'),
        rootInvalid: root.hasAttribute('data-invalid'),
        errorText: screen.queryByTestId('error')?.textContent ?? null,
      };
    };
    fireEvent.click(screen.getByText('Send'));
    await act(async () => {});
    const afterSubmit = read();
    await act(async () => {
      disable();
    });
    await act(async () => {});
    const afterDisable = read();
    report('P3-invalid-then-control-disabled', { afterSubmit, afterDisable });
  });
});

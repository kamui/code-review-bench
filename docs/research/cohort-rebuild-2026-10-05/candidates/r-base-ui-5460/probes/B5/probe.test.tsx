import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

const SERVER_ERRORS = { agree: 'Server error' };

// Observations only. Each case prints one PROBE line.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(calls: unknown[], testId = 'control') {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>(testId);
  return {
    checked: control.checked,
    validateCalls: calls,
    ariaInvalid: control.getAttribute('aria-invalid'),
    rootInvalid: root.hasAttribute('data-invalid'),
    rootDirty: root.hasAttribute('data-dirty'),
    serverErrorShown: screen.queryByText('Server error') !== null,
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

describe('B5 probe: Field.Control as a native checkbox or radio with a constant value attribute', () => {
  const { render } = createRenderer();

  it('B5-1 checkbox value="yes", onChange mode, server error present: click it', async () => {
    const calls: unknown[] = [];
    await render(
      <Form errors={SERVER_ERRORS}>
        <Field.Root
          data-testid="root"
          name="agree"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return null;
          }}
        >
          <Field.Control data-testid="control" type="checkbox" value="yes" required />
          <Field.Error data-testid="error" />
        </Field.Root>
      </Form>,
    );
    fireEvent.click(screen.getByTestId('control'));
    report('B5-1-checkbox-with-value-click', snapshot(calls));
  });

  it('B5-2 reference: the same checkbox without a value attribute', async () => {
    const calls: unknown[] = [];
    await render(
      <Form errors={SERVER_ERRORS}>
        <Field.Root
          data-testid="root"
          name="agree"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return null;
          }}
        >
          <Field.Control data-testid="control" type="checkbox" required />
          <Field.Error data-testid="error" />
        </Field.Root>
      </Form>,
    );
    fireEvent.click(screen.getByTestId('control'));
    report('B5-2-ref-checkbox-without-value-click', snapshot(calls));
  });

  it('B5-3 default mode: submit with the required checkbox unticked, then tick it, then submit', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(
      <Form onFormSubmit={(values) => submitted.push(values)}>
        <Field.Root
          data-testid="root"
          name="agree"
          validate={(v) => {
            calls.push(v);
            return null;
          }}
        >
          <Field.Control data-testid="control" type="checkbox" value="yes" required />
          <Field.Error data-testid="error" />
        </Field.Root>
        <button type="submit">Send</button>
      </Form>,
    );
    fireEvent.click(screen.getByText('Send'));
    const afterFirstSubmit = snapshot([...calls]);
    fireEvent.click(screen.getByTestId('control'));
    const afterTicking = snapshot([...calls]);
    fireEvent.click(screen.getByText('Send'));
    const afterSecondSubmit = snapshot([...calls]);
    report('B5-3-required-checkbox-submit-tick-submit', {
      afterFirstSubmit,
      afterTicking,
      afterSecondSubmit,
      submitted,
    });
  });

  it('B5-4 radio pair with value="a" / value="b", onChange mode: pick one', async () => {
    const calls: unknown[] = [];
    await render(
      <Form errors={{ plan: 'Server error' }}>
        <Field.Root
          data-testid="root"
          name="plan"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return null;
          }}
        >
          <Field.Control data-testid="control" type="radio" value="a" />
          <Field.Error data-testid="error" />
        </Field.Root>
      </Form>,
    );
    fireEvent.click(screen.getByTestId('control'));
    report('B5-4-radio-with-value-click', snapshot(calls));
  });

  it('B5-5 server error on the checkbox field: untick/tick it, then try to submit again', async () => {
    const submitted: unknown[] = [];
    function App() {
      // The server rejected the first submission; the app stores the errors it got back.
      const [errors] = React.useState<Record<string, string>>({ agree: 'Server error' });
      return (
        <Form errors={errors} onFormSubmit={(values) => submitted.push(values)}>
          <Field.Root data-testid="root" name="agree">
            <Field.Control data-testid="control" type="checkbox" value="yes" defaultChecked />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    const control = screen.getByTestId('control');
    fireEvent.click(control);
    fireEvent.click(control);
    const afterToggling = snapshot([]);
    fireEvent.click(screen.getByText('Send'));
    report('B5-5-server-error-then-toggle-then-resubmit', {
      afterToggling,
      afterResubmit: snapshot([]),
      submitted,
    });
  });

  it('B5-6 reference: the same with a text field, the person edits it then resubmits', async () => {
    const submitted: unknown[] = [];
    function App() {
      const [errors] = React.useState<Record<string, string>>({ agree: 'Server error' });
      const [value, setValue] = React.useState('abc');
      return (
        <Form errors={errors} onFormSubmit={(values) => submitted.push(values)}>
          <Field.Root data-testid="root" name="agree">
            <Field.Control data-testid="control" value={value} onValueChange={setValue} />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'abcd' } });
    fireEvent.click(screen.getByText('Send'));
    const control = screen.getByTestId<HTMLInputElement>('control');
    report('B5-6-ref-text-server-error-edit-resubmit', {
      serverErrorShown: screen.queryByText('Server error') !== null,
      inputValue: control.value,
      submitted,
    });
  });

  it('B5-7 context: what the Form reports for an UNTICKED native checkbox Field.Control', async () => {
    const submitted: unknown[] = [];
    const validateArgs: unknown[] = [];
    await render(
      <Form onFormSubmit={(values) => submitted.push(values)}>
        <Field.Root
          data-testid="root"
          name="agree"
          validate={(v) => {
            validateArgs.push(v);
            return null;
          }}
        >
          <Field.Control data-testid="control" type="checkbox" value="yes" />
        </Field.Root>
        <button type="submit">Send</button>
      </Form>,
    );
    fireEvent.click(screen.getByText('Send'));
    report('B5-7-context-unticked-checkbox-form-value', {
      checked: screen.getByTestId<HTMLInputElement>('control').checked,
      submitted,
      validateArgs,
    });
  });
});

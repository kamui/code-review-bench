import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { NumberField } from '@base-ui/react/number-field';

const SERVER_ERRORS = { message: 'Server error' };

// Observations only. Each case prints one PROBE line.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(validateCalls: unknown[]) {
  const root = screen.getByTestId('root');
  return {
    validateCalls,
    inputValue: screen.getByTestId<HTMLInputElement>('control').value,
    rootDirty: root.hasAttribute('data-dirty'),
    rootFilled: root.hasAttribute('data-filled'),
    rootInvalid: root.hasAttribute('data-invalid'),
    serverErrorShown: screen.queryByText('Server error') !== null,
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

describe('B6 probe: details.cancel() in onValueChange for an uncontrolled Field.Control', () => {
  const { render } = createRenderer();

  it('B6-1 uncontrolled, cancel(), typing into an empty field', async () => {
    const calls: unknown[] = [];
    await render(
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
          <Field.Control data-testid="control" onValueChange={(value, details) => details.cancel()} />
          <Field.Error data-testid="error" />
        </Field.Root>
      </Form>,
    );
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a' } });
    report('B6-1-uncontrolled-cancel', snapshot(calls));
  });

  it('B6-2 reference: uncontrolled, no cancel', async () => {
    const calls: unknown[] = [];
    await render(
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
          <Field.Control data-testid="control" />
          <Field.Error data-testid="error" />
        </Field.Root>
      </Form>,
    );
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a' } });
    report('B6-2-ref-uncontrolled-no-cancel', snapshot(calls));
  });

  it('B6-3 uncontrolled required, cancel(): type then delete, the validity that is shown', async () => {
    const calls: unknown[] = [];
    await render(
      <Field.Root
        data-testid="root"
        name="message"
        validationMode="onChange"
        validate={(v) => {
          calls.push(v);
          return (v as string).length < 3 ? 'Too short' : null;
        }}
      >
        <Field.Control data-testid="control" onValueChange={(value, details) => details.cancel()} />
        <Field.Error data-testid="error" />
      </Field.Root>,
    );
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'ab' } });
    report('B6-3-uncontrolled-cancel-invalid-value', snapshot(calls));
  });

  // What cancel() does in a sibling control whose value Base UI owns.
  it('B6-4 sibling NumberField uncontrolled, cancel()', async () => {
    const calls: unknown[] = [];
    await render(
      <Field.Root
        data-testid="root"
        name="amount"
        validationMode="onChange"
        validate={(v) => {
          calls.push(v);
          return null;
        }}
      >
        <NumberField.Root onValueChange={(value, details) => details.cancel()}>
          <NumberField.Input data-testid="control" />
        </NumberField.Root>
      </Field.Root>,
    );
    const control = screen.getByTestId('control');
    fireEvent.change(control, { target: { value: '5' } });
    fireEvent.blur(control);
    report('B6-4-sibling-NumberField-cancel', snapshot(calls));
  });
});

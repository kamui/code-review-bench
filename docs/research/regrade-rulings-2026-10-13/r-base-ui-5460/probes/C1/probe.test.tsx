import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Observations only. Each case prints one PROBE line and asserts nothing.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function describeValue(v: unknown) {
  return { type: Array.isArray(v) ? 'array' : typeof v, value: v };
}

// The validator the comment quotes, word for word, with the verdict recorded beside its input.
function quotedValidator(calls: Record<string, unknown>[], phase: () => string) {
  return (v: unknown) => {
    const verdict = typeof v === 'number' && v > 3 ? null : 'err';
    calls.push({ phase: phase(), received: describeValue(v), returned: verdict });
    return verdict;
  };
}

describe('C1 probe: the statements of the comment, one by one', () => {
  const { render } = createRenderer();

  it('C1-1 the quoted validator with <Field.Control value={5} />: Form submit', async () => {
    const calls: Record<string, unknown>[] = [];
    const submitted: unknown[] = [];
    let lastValidity: Record<string, unknown> = {};
    await render(
      <Form onFormSubmit={(values) => submitted.push(values)}>
        <Field.Root name="amount" validate={quotedValidator(calls, () => 'form-submit')}>
          <Field.Control data-testid="control" value={5} />
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
      </Form>,
    );
    const validityAtMount = lastValidity;
    fireEvent.click(screen.getByText('Send'));
    report('C1-1-quoted-validator-submit', {
      fieldValidityAtMount: validityAtMount,
      validateCalls: calls,
      errorShown: screen.queryByTestId('error')?.textContent ?? null,
      controlAriaInvalid: screen.getByTestId('control').getAttribute('aria-invalid'),
      onFormSubmitCalls: submitted,
    });
  });

  it('C1-2 the quoted validator with <Field.Control value={5} />: actionsRef.current.validate()', async () => {
    const calls: Record<string, unknown>[] = [];
    const actionsRef = React.createRef<Field.Root.Actions>();
    await render(
      <Field.Root
        name="amount"
        actionsRef={actionsRef}
        validate={quotedValidator(calls, () => 'actionsRef.validate')}
      >
        <Field.Control data-testid="control" value={5} />
        <Field.Error data-testid="error" />
      </Field.Root>,
    );
    await act(async () => {
      actionsRef.current?.validate();
    });
    report('C1-2-quoted-validator-actionsRef', {
      validateCalls: calls,
      errorShown: screen.queryByTestId('error')?.textContent ?? null,
      controlAriaInvalid: screen.getByTestId('control').getAttribute('aria-invalid'),
    });
  });

  it('C1-3 the same field with the app storing Number(next): submit, then the person types 6, then submit', async () => {
    const calls: Record<string, unknown>[] = [];
    const submitted: unknown[] = [];
    let phase = 'first-submit';
    function App() {
      const [value, setValue] = React.useState(5);
      return (
        <Form onFormSubmit={(values) => submitted.push(values)}>
          <Field.Root name="amount" validate={quotedValidator(calls, () => phase)}>
            <Field.Control
              data-testid="control"
              value={value}
              onValueChange={(next) => setValue(Number(next))}
            />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('Send'));
    const errorAfterFirstSubmit = screen.queryByTestId('error')?.textContent ?? null;
    phase = 'typing-6';
    fireEvent.change(screen.getByTestId('control'), { target: { value: '6' } });
    const errorAfterTyping = screen.queryByTestId('error')?.textContent ?? null;
    phase = 'second-submit';
    fireEvent.click(screen.getByText('Send'));
    report('C1-3-quoted-validator-submit-type-submit', {
      validateCalls: calls,
      errorAfterFirstSubmit,
      errorAfterTyping,
      errorAfterSecondSubmit: screen.queryByTestId('error')?.textContent ?? null,
      onFormSubmitCalls: submitted,
    });
  });

  it("C1-4 an array value ['a','b']: what validate receives on submit", async () => {
    const calls: Record<string, unknown>[] = [];
    let lastValidity: Record<string, unknown> = {};
    await render(
      <Form onFormSubmit={() => {}}>
        <Field.Root
          name="tags"
          validate={(v) => {
            calls.push(describeValue(v));
            return null;
          }}
        >
          <Field.Control data-testid="control" value={['a', 'b']} onValueChange={() => {}} />
          <Field.Validity>
            {(validity) => {
              lastValidity = { initialValue: describeValue(validity.initialValue) };
              return null;
            }}
          </Field.Validity>
        </Field.Root>
        <button type="submit">Send</button>
      </Form>,
    );
    fireEvent.click(screen.getByText('Send'));
    report('C1-4-array-submit', { validateCalls: calls, fieldValidity: lastValidity });
  });

  it("C1-5 code switches the value between ['a,b'] and ['a','b'], validationMode onChange", async () => {
    const calls: Record<string, unknown>[] = [];
    let setFromOutside: (next: string[]) => void = () => {};
    function App() {
      const [value, setValue] = React.useState<string[]>(['a,b']);
      setFromOutside = setValue;
      return (
        <Field.Root
          data-testid="root"
          name="tags"
          validationMode="onChange"
          validate={(v) => {
            calls.push(describeValue(v));
            return null;
          }}
        >
          <Field.Control data-testid="control" value={value} onValueChange={() => {}} />
        </Field.Root>
      );
    }
    await render(<App />);
    const snapshot = () => ({
      inputText: screen.getByTestId<HTMLInputElement>('control').value,
      rootDirty: screen.getByTestId('root').hasAttribute('data-dirty'),
      validateCalls: calls.length,
    });
    const atMount = snapshot();
    await act(async () => {
      setFromOutside(['a', 'b']);
    });
    const afterSameText = snapshot();
    await act(async () => {
      setFromOutside(['a', 'c']);
    });
    const afterOtherText = snapshot();
    report('C1-5-array-same-text-form', {
      atMount,
      afterSwitchToArrayWithSameText: afterSameText,
      afterSwitchToArrayWithOtherText: afterOtherText,
      validateCalls: calls,
    });
  });
});

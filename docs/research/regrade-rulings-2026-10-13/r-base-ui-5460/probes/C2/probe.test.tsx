import * as React from 'react';
import { vi } from 'vitest';
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

function fieldSnapshot() {
  const root = screen.getByTestId('root');
  return {
    inputText: screen.getByTestId<HTMLInputElement>('control').value,
    rootDirty: root.hasAttribute('data-dirty'),
    rootFilled: root.hasAttribute('data-filled'),
    rootInvalid: root.hasAttribute('data-invalid'),
  };
}

describe('C2 probe: the statements of the comment, one by one', () => {
  const { render } = createRenderer();

  it('C2-1 value={5}: the registered value as Field.Validity and validate see it', async () => {
    const calls: Record<string, unknown>[] = [];
    const submitted: unknown[] = [];
    let phase = 'form-submit';
    let lastValidity: Record<string, unknown> = {};
    const actionsRef = React.createRef<Field.Root.Actions>();
    await render(
      <Form onFormSubmit={(values) => submitted.push(values)}>
        <Field.Root
          data-testid="root"
          name="amount"
          actionsRef={actionsRef}
          validate={(v) => {
            calls.push({ phase, ...describeValue(v) });
            return null;
          }}
        >
          <Field.Control data-testid="control" value={5} onValueChange={() => {}} />
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
    const validityAtMount = lastValidity;
    fireEvent.click(screen.getByText('Send'));
    phase = 'actionsRef.validate';
    await act(async () => {
      actionsRef.current?.validate();
    });
    report('C2-1-number-registered-value', {
      fieldValidityAtMount: validityAtMount,
      validateCalls: calls,
      onFormSubmitCalls: submitted,
    });
  });

  it('C2-2 an object value and an array value: input text, initial value, what validate receives on submit', async () => {
    const results: Record<string, unknown> = {};
    for (const [label, value] of [
      ['object', { id: 1 }],
      ['array', ['a', 'b']],
    ] as const) {
      const calls: Record<string, unknown>[] = [];
      let lastValidity: Record<string, unknown> = {};
      const view = await render(
        <Form onFormSubmit={() => {}}>
          <Field.Root
            name="thing"
            validate={(v) => {
              calls.push(describeValue(v));
              return null;
            }}
          >
            <Field.Control data-testid="control" value={value as any} onValueChange={() => {}} />
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
      const inputText = screen.getByTestId<HTMLInputElement>('control').value;
      fireEvent.click(screen.getByText('Send'));
      results[label] = { inputText, fieldValidity: lastValidity, validateCalls: calls };
      view.unmount();
    }
    report('C2-2-object-and-array', results);
  });

  it('C2-3 code replaces one object with a different object, validationMode onChange', async () => {
    const calls: Record<string, unknown>[] = [];
    let setFromOutside: (next: unknown) => void = () => {};
    function App() {
      const [value, setValue] = React.useState<unknown>({ id: 1 });
      setFromOutside = setValue;
      return (
        <Field.Root
          data-testid="root"
          name="thing"
          validationMode="onChange"
          validate={(v) => {
            calls.push(describeValue(v));
            return null;
          }}
        >
          <Field.Control data-testid="control" value={value as any} onValueChange={() => {}} />
        </Field.Root>
      );
    }
    await render(<App />);
    const atMount = fieldSnapshot();
    await act(async () => {
      setFromOutside({ id: 2 });
    });
    const afterOtherObject = fieldSnapshot();
    const callsAfterOtherObject = [...calls];
    await act(async () => {
      setFromOutside('text');
    });
    const afterString = fieldSnapshot();
    report('C2-3-two-objects', {
      atMount,
      afterReplacingWithAnotherObject: afterOtherObject,
      validateCallsAfterAnotherObject: callsAfterOtherObject,
      afterReplacingWithAString: afterString,
      validateCalls: calls,
    });
  });

  it('C2-4 code sets a controlled value to null, validationMode onChange', async () => {
    // React prints a console.error for a null `value`, and this repository's test setup fails a
    // test on any console.error. Record the message instead.
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const calls: Record<string, unknown>[] = [];
    let setFromOutside: (next: string | null) => void = () => {};
    function App() {
      const [value, setValue] = React.useState<string | null>('');
      setFromOutside = setValue;
      return (
        <Field.Root
          data-testid="root"
          name="thing"
          validationMode="onChange"
          validate={(v) => {
            calls.push(describeValue(v));
            return null;
          }}
        >
          <Field.Control
            data-testid="control"
            value={value as any}
            onValueChange={(next) => setValue(next)}
          />
        </Field.Root>
      );
    }
    await render(<App />);
    const atMount = fieldSnapshot();
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'abc' } });
    const afterTyping = fieldSnapshot();
    const callsAfterTyping = [...calls];
    await act(async () => {
      setFromOutside(null);
    });
    const afterNull = fieldSnapshot();
    report('C2-4-null-from-code', {
      atMount,
      afterTypingAbc: afterTyping,
      validateCallsAfterTyping: callsAfterTyping,
      afterCodeSetsNull: afterNull,
      validateCalls: calls,
      consoleErrors: consoleError.mock.calls.map((args) => String(args[0]).slice(0, 200)),
    });
    consoleError.mockRestore();
  });

  it('C2-5 the person empties the field and the app stores null for an empty text, validationMode onChange', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    const calls: Record<string, unknown>[] = [];
    function App() {
      const [value, setValue] = React.useState<string | null>('abc');
      return (
        <Field.Root
          data-testid="root"
          name="thing"
          validationMode="onChange"
          validate={(v) => {
            calls.push(describeValue(v));
            return null;
          }}
        >
          <Field.Control
            data-testid="control"
            value={value as any}
            onValueChange={(next) => setValue(next === '' ? null : next)}
          />
        </Field.Root>
      );
    }
    await render(<App />);
    const atMount = fieldSnapshot();
    fireEvent.change(screen.getByTestId('control'), { target: { value: '' } });
    report('C2-5-null-from-emptying', {
      atMount,
      afterEmptying: fieldSnapshot(),
      validateCalls: calls,
      consoleErrors: consoleError.mock.calls.map((args) => String(args[0]).slice(0, 200)),
    });
    consoleError.mockRestore();
  });
});

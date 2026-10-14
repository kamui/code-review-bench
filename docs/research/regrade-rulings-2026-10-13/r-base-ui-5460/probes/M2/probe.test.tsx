// Probe M2: NumberField.Input rendered through the field-aware Input (which is Field.Control),
// inside a Field.Root with a validator written for the number.
// Observations only. Each case prints one PROBE line and asserts nothing.
import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { Input } from '@base-ui/react/input';
import { NumberField } from '@base-ui/react/number-field';

function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(calls: unknown[]) {
  const root = screen.getByTestId('root');
  const input = screen.getByTestId<HTMLInputElement>('input');
  return {
    inputText: input.value,
    validateCalls: [...calls],
    inputAriaInvalid: input.getAttribute('aria-invalid'),
    rootInvalid: root.hasAttribute('data-invalid'),
    rootValid: root.hasAttribute('data-valid'),
    rootDirty: root.hasAttribute('data-dirty'),
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => {
      setTimeout(resolve, 20);
    });
  });
}

// "Too big" above 1000: a validator written for the number, in the loose form `value > 1000`.
const tooBig = (v: unknown) => ((v as number) > 1000 ? 'Too big' : null);
// A validator that checks the type first, as GT-r7's probe validator does for the combobox item.
const mustBeNumber = (v: unknown) =>
  v === null || typeof v === 'number' ? null : 'Enter a number';

function App(props: {
  calls: unknown[];
  submitted: unknown[];
  fieldAware: boolean;
  validator?: (v: unknown) => string | null;
  validationMode?: 'onChange' | 'onSubmit' | 'onBlur';
  initial?: number | null;
}) {
  const {
    calls,
    submitted,
    fieldAware,
    validator = tooBig,
    validationMode = 'onChange',
    initial = 5,
  } = props;
  const [value, setValue] = React.useState<number | null>(initial);
  return (
    <Form onFormSubmit={(values) => submitted.push(values)}>
      <Field.Root
        data-testid="root"
        name="amount"
        validationMode={validationMode}
        validate={(v) => {
          calls.push(v);
          return validator(v);
        }}
      >
        <NumberField.Root value={value} onValueChange={setValue}>
          <NumberField.Decrement data-testid="decrement" />
          {fieldAware ? (
            <NumberField.Input render={<Input data-testid="input" />} />
          ) : (
            <NumberField.Input data-testid="input" />
          )}
          <NumberField.Increment data-testid="increment" />
        </NumberField.Root>
        <Field.Error data-testid="error" />
      </Field.Root>
      <button type="button" onClick={() => setValue(2000)}>
        set 2000 from code
      </button>
      <button type="button" onClick={() => setValue(7)}>
        set 7 from code
      </button>
      <button type="submit">Send</button>
    </Form>
  );
}

describe('M2 probe: NumberField.Input rendered as the field-aware Input', () => {
  const { render } = createRenderer();

  for (const fieldAware of [true, false]) {
    const kind = fieldAware ? 'fieldaware' : 'plain';

    it(`M2-1 ${kind}, onChange mode, "Too big" validator: the app sets 2000 from code, then Send`, async () => {
      const calls: unknown[] = [];
      const submitted: unknown[] = [];
      await render(<App calls={calls} submitted={submitted} fieldAware={fieldAware} />);
      fireEvent.click(screen.getByText('set 2000 from code'));
      await settle();
      const afterCodeSet = snapshot(calls);
      fireEvent.click(screen.getByText('Send'));
      await settle();
      report(`M2-1-${kind}-code-set-2000-then-submit`, {
        afterCodeSet,
        afterSubmit: snapshot(calls),
        submitted,
      });
    });

    it(`M2-2 ${kind}, onChange mode, "Too big" validator: the person clicks the increment button from 1000`, async () => {
      const calls: unknown[] = [];
      const submitted: unknown[] = [];
      const { user } = await render(
        <App calls={calls} submitted={submitted} fieldAware={fieldAware} initial={1000} />,
      );
      await user.click(screen.getByTestId('increment'));
      await settle();
      const afterIncrement = snapshot(calls);
      await user.click(screen.getByText('Send'));
      await settle();
      report(`M2-2-${kind}-increment-to-1001-then-submit`, {
        afterIncrement,
        afterSubmit: snapshot(calls),
        submitted,
      });
    });

    it(`M2-3 ${kind}, onChange mode, "Too big" validator: the person types 2000 and leaves the field`, async () => {
      const calls: unknown[] = [];
      const submitted: unknown[] = [];
      const { user } = await render(
        <App calls={calls} submitted={submitted} fieldAware={fieldAware} initial={null} />,
      );
      await user.click(screen.getByTestId('input'));
      await user.keyboard('2000');
      await settle();
      const afterTyping = snapshot(calls);
      await user.tab();
      await settle();
      const afterBlur = snapshot(calls);
      await user.click(screen.getByText('Send'));
      await settle();
      report(`M2-3-${kind}-typed-2000-blur-then-submit`, {
        afterTyping,
        afterBlur,
        afterSubmit: snapshot(calls),
        submitted,
      });
    });

    it(`M2-4 ${kind}, onChange mode, type-checking validator: the app sets 7 from code, then Send`, async () => {
      const calls: unknown[] = [];
      const submitted: unknown[] = [];
      await render(
        <App calls={calls} submitted={submitted} fieldAware={fieldAware} validator={mustBeNumber} />,
      );
      fireEvent.click(screen.getByText('set 7 from code'));
      await settle();
      const afterCodeSet = snapshot(calls);
      fireEvent.click(screen.getByText('Send'));
      await settle();
      report(`M2-4-${kind}-typecheck-code-set-7-then-submit`, {
        afterCodeSet,
        afterSubmit: snapshot(calls),
        submitted,
      });
    });

    it(`M2-5 ${kind}, default (onSubmit) mode, "Too big" validator: the app sets 2000 from code, then Send`, async () => {
      const calls: unknown[] = [];
      const submitted: unknown[] = [];
      await render(
        <App calls={calls} submitted={submitted} fieldAware={fieldAware} validationMode="onSubmit" />,
      );
      fireEvent.click(screen.getByText('set 2000 from code'));
      await settle();
      const afterCodeSet = snapshot(calls);
      fireEvent.click(screen.getByText('Send'));
      await settle();
      report(`M2-5-${kind}-default-mode-code-set-2000-then-submit`, {
        afterCodeSet,
        afterSubmit: snapshot(calls),
        submitted,
      });
    });

    it(`M2-6 ${kind}, onBlur mode, "Too big" validator: invalid after blur at 2000, then the app sets 3000 from code`, async () => {
      const calls: unknown[] = [];
      const submitted: unknown[] = [];
      function Late() {
        const [value, setValue] = React.useState<number | null>(2000);
        return (
          <Field.Root
            data-testid="root"
            name="amount"
            validationMode="onBlur"
            validate={(v) => {
              calls.push(v);
              return tooBig(v);
            }}
          >
            <NumberField.Root value={value} onValueChange={setValue}>
              {fieldAware ? (
                <NumberField.Input render={<Input data-testid="input" />} />
              ) : (
                <NumberField.Input data-testid="input" />
              )}
            </NumberField.Root>
            <Field.Error data-testid="error" />
            <button type="button" onClick={() => setValue(3000)}>
              set 3000 from code
            </button>
          </Field.Root>
        );
      }
      const { user } = await render(<Late />);
      await user.click(screen.getByTestId('input'));
      await user.tab();
      await settle();
      const afterBlur = snapshot(calls);
      fireEvent.click(screen.getByText('set 3000 from code'));
      await settle();
      report(`M2-6-${kind}-onBlur-invalid-then-code-set-3000`, {
        afterBlur,
        afterCodeSet: snapshot(calls),
        submitted,
      });
    });
  }
});

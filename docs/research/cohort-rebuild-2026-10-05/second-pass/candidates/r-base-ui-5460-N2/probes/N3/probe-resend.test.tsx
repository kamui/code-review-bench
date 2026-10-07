import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { Switch } from '@base-ui/react/switch';
import { Checkbox } from '@base-ui/react/checkbox';
import { NumberField } from '@base-ui/react/number-field';

// Observations only. Each case prints one PROBE line and asserts nothing.
// What a person has to do to send a form again once a field shows a server error,
// when the app's onValueChange can reject or rewrite what is typed.

type Decide = (typed: string, current: string) => string;
type Step = { type: string } | { send: true } | { appClearsErrors: true };

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

interface Options {
  mode?: 'onChange';
  decide: Decide;
  initial: string;
  steps: Step[];
  controlled?: boolean;
  clearErrorsWhenRejecting?: boolean;
}

async function run(render: ReturnType<typeof createRenderer>['render'], options: Options) {
  const { mode, decide, initial, steps, controlled = true, clearErrorsWhenRejecting = false } = options;
  const validateCalls: string[] = [];
  const submitted: string[] = [];
  let phase = 'mount';

  function App() {
    const [value, setValue] = React.useState(initial);
    // Server errors kept in state, as the forms handbook shows.
    const [errors, setErrors] = React.useState<Form.Props['errors']>({ field: 'Server error' });
    return (
      <Form
        errors={errors}
        onFormSubmit={(values) => submitted.push(`${phase}: ${JSON.stringify(values.field)}`)}
      >
        <Field.Root
          name="field"
          data-testid="root"
          validationMode={mode}
          validate={(candidate) => {
            validateCalls.push(`${phase}: ${JSON.stringify(candidate)}`);
            return null;
          }}
        >
          {controlled ? (
            <Field.Control
              data-testid="control"
              value={value}
              onValueChange={(next) => {
                const decided = decide(next, value);
                if (decided === value && clearErrorsWhenRejecting) {
                  setErrors({});
                }
                setValue(decided);
              }}
            />
          ) : (
            <Field.Control data-testid="control" defaultValue={initial} />
          )}
          <Field.Error data-testid="error" />
        </Field.Root>
        <button type="button" onClick={() => setErrors({})}>
          App clears errors
        </button>
        <button type="submit">Send</button>
      </Form>
    );
  }

  await render(<App />);

  const snapshot = (step: string) => {
    const control = screen.getByTestId('control') as HTMLInputElement;
    return {
      step,
      inputShows: control.value,
      errorShown: screen.queryByTestId('error')?.textContent ?? null,
      fieldInvalid: screen.getByTestId('root').hasAttribute('data-invalid'),
      submittedSoFar: submitted.length,
    };
  };

  const rows = [snapshot('mount')];
  steps.forEach((step, index) => {
    if ('type' in step) {
      phase = `step ${index + 1} typing`;
      fireEvent.change(screen.getByTestId('control'), { target: { value: step.type } });
      rows.push(snapshot(`person makes the text ${JSON.stringify(step.type)}`));
    } else if ('send' in step) {
      phase = `step ${index + 1} Send`;
      fireEvent.click(screen.getByText('Send'));
      rows.push(snapshot('person presses Send'));
    } else {
      phase = `step ${index + 1} app clears`;
      fireEvent.click(screen.getByText('App clears errors'));
      rows.push(snapshot('app sets its errors state to {}'));
    }
  });

  return { rows, validateCalls, submitted };
}

const digitsOnly: Decide = (typed, current) => (/^\d*$/.test(typed) ? typed : current);
const atMostThree: Decide = (typed, current) => (typed.length <= 3 ? typed : current);
const trimmed: Decide = (typed) => typed.trim();
const upperCase: Decide = (typed) => typed.toUpperCase();

describe('N3 probe: sending a form again after a server error when the app rejects keystrokes', () => {
  const { render } = createRenderer();

  for (const mode of [undefined, 'onChange'] as const) {
    const modeName = mode ?? 'default';

    it(`N3-1 ${modeName}: Send again without touching the field`, async () => {
      const result = await run(render, { mode, decide: digitsOnly, initial: '12', steps: [{ send: true }] });
      report(`N3-1-send-untouched mode=${modeName}`, result);
    });

    it(`N3-2 ${modeName}: a rejected letter, then Send, then an accepted digit, then Send`, async () => {
      const result = await run(render, {
        mode,
        decide: digitsOnly,
        initial: '12',
        steps: [{ type: '12x' }, { send: true }, { type: '123' }, { send: true }],
      });
      report(`N3-2-rejected-then-accepted mode=${modeName}`, result);
    });

    it(`N3-3 ${modeName}: resend the same value by changing it and changing it back`, async () => {
      const result = await run(render, {
        mode,
        decide: digitsOnly,
        initial: '12',
        steps: [{ type: '12x' }, { type: '123' }, { type: '12' }, { send: true }],
      });
      report(`N3-3-resend-same-value mode=${modeName}`, result);
    });

    it(`N3-4 ${modeName}: a field at its length cap, the app drops a fourth character`, async () => {
      const result = await run(render, {
        mode,
        decide: atMostThree,
        initial: 'abc',
        steps: [{ type: 'abcd' }, { send: true }, { type: 'ab' }, { type: 'abc' }, { send: true }],
      });
      report(`N3-4-length-cap mode=${modeName}`, result);
    });

    it(`N3-5 ${modeName}: the app trims, the person types a trailing space`, async () => {
      const result = await run(render, {
        mode,
        decide: trimmed,
        initial: 'abc',
        steps: [{ type: 'abc ' }, { send: true }],
      });
      report(`N3-5-trim-rewrite mode=${modeName}`, result);
    });

    it(`N3-6 ${modeName}: the app upper-cases, the person types a letter`, async () => {
      const result = await run(render, {
        mode,
        decide: upperCase,
        initial: 'ABC',
        steps: [{ type: 'ABCd' }, { send: true }],
      });
      report(`N3-6-uppercase-rewrite mode=${modeName}`, result);
    });

    it(`N3-7 ${modeName}: the app clears its own errors state, then Send`, async () => {
      const result = await run(render, {
        mode,
        decide: digitsOnly,
        initial: '12',
        steps: [{ type: '12x' }, { appClearsErrors: true }, { send: true }],
      });
      report(`N3-7-app-clears-errors mode=${modeName}`, result);
    });

    it(`N3-8 ${modeName}: the app clears its errors state inside onValueChange when it rejects`, async () => {
      const result = await run(render, {
        mode,
        decide: digitsOnly,
        initial: '12',
        steps: [{ type: '12x' }, { send: true }],
        clearErrorsWhenRejecting: true,
      });
      report(`N3-8-app-clears-on-reject mode=${modeName}`, result);
    });

    it(`N3-9 ${modeName}: reference, the same input without a value prop`, async () => {
      const result = await run(render, {
        mode,
        decide: digitsOnly,
        initial: '12',
        steps: [{ type: '12x' }, { send: true }],
        controlled: false,
      });
      report(`N3-9-uncontrolled-reference mode=${modeName}`, result);
    });
  }

  it('N3-10 sibling controls whose app rejects the change', async () => {
    const errors = { toggle: 'Server error', box: 'Server error', amount: 'Server error' };
    function App() {
      return (
        <Form errors={errors}>
          <Field.Root name="toggle" data-testid="toggle-root">
            <Switch.Root data-testid="toggle" checked={false} onCheckedChange={() => {}} />
            <Field.Error data-testid="toggle-error" />
          </Field.Root>
          <Field.Root name="box" data-testid="box-root">
            <Checkbox.Root data-testid="box" checked={false} onCheckedChange={() => {}} />
            <Field.Error data-testid="box-error" />
          </Field.Root>
          <Field.Root name="amount" data-testid="amount-root">
            <NumberField.Root value={5} onValueChange={() => {}}>
              <NumberField.Input data-testid="amount" />
            </NumberField.Root>
            <Field.Error data-testid="amount-error" />
          </Field.Root>
        </Form>
      );
    }
    await render(<App />);
    const read = () => ({
      switchError: screen.queryByTestId('toggle-error')?.textContent ?? null,
      checkboxError: screen.queryByTestId('box-error')?.textContent ?? null,
      numberFieldError: screen.queryByTestId('amount-error')?.textContent ?? null,
      numberFieldShows: (screen.getByTestId('amount') as HTMLInputElement).value,
    });
    const before = read();
    fireEvent.click(screen.getByTestId('toggle'));
    fireEvent.click(screen.getByTestId('box'));
    const amount = screen.getByTestId('amount');
    fireEvent.focus(amount);
    fireEvent.change(amount, { target: { value: '56' } });
    fireEvent.blur(amount);
    report('N3-10-siblings-reject', { before, afterRejectedInteraction: read() });
  });
});

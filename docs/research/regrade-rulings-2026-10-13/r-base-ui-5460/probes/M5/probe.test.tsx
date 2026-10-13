// Probe M5: a field that already shows a validation error, outside validation on change, whose
// value the app then sets from code while nothing has focus (no blur follows).
// Observations only. Each case prints one PROBE line and asserts nothing.
import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { NumberField } from '@base-ui/react/number-field';

function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(calls: unknown[]) {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>('control');
  return {
    inputValue: control.value,
    validateCalls: [...calls],
    controlAriaInvalid: control.getAttribute('aria-invalid'),
    rootInvalid: root.hasAttribute('data-invalid'),
    rootValid: root.hasAttribute('data-valid'),
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

async function settle(ms = 20) {
  await act(async () => {
    await new Promise((resolve) => {
      setTimeout(resolve, ms);
    });
  });
}

const tooShort = (v: unknown) => (String(v).length >= 3 ? null : 'Too short');

function App(props: {
  calls: unknown[];
  submitted: unknown[];
  validationMode?: 'onBlur' | 'onSubmit' | 'onChange';
  asyncValidator?: boolean;
  controlProps?: Record<string, unknown>;
}) {
  const { calls, submitted, validationMode, asyncValidator, controlProps } = props;
  const [value, setValue] = React.useState('');
  return (
    <Form onFormSubmit={(values) => submitted.push(values)}>
      <Field.Root
        data-testid="root"
        name="code"
        validationMode={validationMode}
        validate={(v) => {
          calls.push(v);
          if (asyncValidator) {
            return new Promise<string | null>((resolve) => {
              setTimeout(() => resolve(tooShort(v)), 5);
            });
          }
          return tooShort(v);
        }}
      >
        <Field.Control data-testid="control" value={value} onValueChange={setValue} {...controlProps} />
        <Field.Error data-testid="error" />
      </Field.Root>
      <button type="button" onClick={() => setValue('x')}>
        code sets x
      </button>
      <button type="button" onClick={() => setValue('abcd')}>
        code sets abcd
      </button>
      <button type="submit">Send</button>
    </Form>
  );
}

async function typeAndBlur(text: string) {
  const control = screen.getByTestId('control');
  fireEvent.focus(control);
  fireEvent.change(control, { target: { value: text } });
  fireEvent.blur(control);
  await settle();
}

describe('M5 probe: a code-set value on a field that shows a validation error', () => {
  const { render } = createRenderer();

  it('M5-1 onBlur mode: "ab" typed and blurred shows "Too short"; then code sets "x" (still too short); then Send', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<App calls={calls} submitted={submitted} validationMode="onBlur" />);
    await typeAndBlur('ab');
    const afterBlur = snapshot(calls);
    fireEvent.click(screen.getByText('code sets x'));
    await settle();
    const afterCodeSet = snapshot(calls);
    fireEvent.click(screen.getByText('Send'));
    await settle();
    report('M5-1-onBlur-code-sets-still-invalid', {
      afterBlur,
      afterCodeSet,
      afterSend: snapshot(calls),
      submitted,
    });
  });

  it('M5-2 onBlur mode: "ab" typed and blurred shows "Too short"; then code sets "abcd" (long enough)', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<App calls={calls} submitted={submitted} validationMode="onBlur" />);
    await typeAndBlur('ab');
    const afterBlur = snapshot(calls);
    fireEvent.click(screen.getByText('code sets abcd'));
    await settle();
    report('M5-2-onBlur-code-sets-valid', { afterBlur, afterCodeSet: snapshot(calls) });
  });

  it('M5-3 onBlur mode, asynchronous validator: error shown after blur; then code sets "x"', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<App calls={calls} submitted={submitted} validationMode="onBlur" asyncValidator />);
    await typeAndBlur('ab');
    const afterBlur = snapshot(calls);
    fireEvent.click(screen.getByText('code sets x'));
    await settle();
    report('M5-3-onBlur-async-code-sets-still-invalid', { afterBlur, afterCodeSet: snapshot(calls) });
  });

  it('M5-4 default mode, no submit yet: Enter in the field shows "Too short"; then code sets "x"; then Send', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    function NoForm() {
      const [value, setValue] = React.useState('');
      return (
        <Field.Root
          data-testid="root"
          name="code"
          validate={(v) => {
            calls.push(v);
            return tooShort(v);
          }}
        >
          <Field.Control data-testid="control" value={value} onValueChange={setValue} />
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue('x')}>
            code sets x
          </button>
        </Field.Root>
      );
    }
    await render(<NoForm />);
    const control = screen.getByTestId('control');
    await act(async () => {
      control.focus();
    });
    fireEvent.change(control, { target: { value: 'ab' } });
    fireEvent.keyDown(control, { key: 'Enter' });
    await act(async () => {
      control.blur();
    });
    await settle();
    const afterEnter = snapshot(calls);
    fireEvent.click(screen.getByText('code sets x'));
    await settle();
    report('M5-4-default-enter-then-code-sets-still-invalid', {
      afterEnter,
      afterCodeSet: snapshot(calls),
    });
  });

  it('M5-5 reference, default mode after a refused Send: code sets "x"', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<App calls={calls} submitted={submitted} />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'ab' } });
    fireEvent.click(screen.getByText('Send'));
    await settle();
    const afterRefusedSend = snapshot(calls);
    fireEvent.click(screen.getByText('code sets x'));
    await settle();
    report('M5-5-ref-default-after-submit-code-sets-still-invalid', {
      afterRefusedSend,
      afterCodeSet: snapshot(calls),
      submitted,
    });
  });

  it('M5-6 reference, validationMode="onChange": code sets "x" on a field showing "Too short"', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<App calls={calls} submitted={submitted} validationMode="onChange" />);
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'ab' } });
    await settle();
    const afterTyping = snapshot(calls);
    fireEvent.click(screen.getByText('code sets x'));
    await settle();
    report('M5-6-ref-onChange-code-sets-still-invalid', { afterTyping, afterCodeSet: snapshot(calls) });
  });

  it('M5-7 reference, onBlur mode: the person types a still-too-short value into the field that shows the error, then leaves it', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<App calls={calls} submitted={submitted} validationMode="onBlur" />);
    await typeAndBlur('ab');
    const afterBlur = snapshot(calls);
    const control = screen.getByTestId('control');
    fireEvent.focus(control);
    fireEvent.change(control, { target: { value: 'x' } });
    await settle();
    const whileTyping = snapshot(calls);
    fireEvent.blur(control);
    await settle();
    report('M5-7-ref-onBlur-person-types-still-invalid', {
      afterBlur,
      whileTyping,
      afterLeaving: snapshot(calls),
    });
  });

  it('M5-8 onBlur mode, a native constraint (type="email"): invalid after blur; then code sets another invalid address', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    function Email() {
      const [value, setValue] = React.useState('');
      return (
        <Field.Root data-testid="root" name="email" validationMode="onBlur">
          <Field.Control data-testid="control" type="email" value={value} onValueChange={setValue} />
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue('still-not-an-address')}>
            code sets invalid address
          </button>
        </Field.Root>
      );
    }
    await render(<Email />);
    await typeAndBlur('not-an-address');
    const afterBlur = snapshot(calls);
    fireEvent.click(screen.getByText('code sets invalid address'));
    await settle();
    const afterCodeSet = snapshot(calls);
    report('M5-8-onBlur-native-email-code-sets-still-invalid', {
      afterBlur,
      afterCodeSet,
      nativeValidityAfterCodeSet: screen.getByTestId<HTMLInputElement>('control').validity.valid,
    });
  });

  it('M5-9 sibling: NumberField in onBlur mode, invalid after blur at 2000; then code sets 3000 (still too big)', async () => {
    const calls: unknown[] = [];
    function Sibling() {
      const [value, setValue] = React.useState<number | null>(2000);
      return (
        <Field.Root
          data-testid="root"
          name="amount"
          validationMode="onBlur"
          validate={(v) => {
            calls.push(v);
            return (v as number) > 1000 ? 'Too big' : null;
          }}
        >
          <NumberField.Root value={value} onValueChange={setValue}>
            <NumberField.Input data-testid="control" />
          </NumberField.Root>
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue(3000)}>
            code sets 3000
          </button>
        </Field.Root>
      );
    }
    const { user } = await render(<Sibling />);
    await user.click(screen.getByTestId('control'));
    await user.tab();
    await settle();
    const afterBlur = snapshot(calls);
    fireEvent.click(screen.getByText('code sets 3000'));
    await settle();
    report('M5-9-sibling-NumberField-onBlur-code-sets-still-invalid', {
      afterBlur,
      afterCodeSet: snapshot(calls),
    });
  });
});

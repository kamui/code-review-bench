import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { NumberField } from '@base-ui/react/number-field';

// Observations only. Each case prints one PROBE line and asserts nothing.
// The thing probed: a controlled Field.Control with type="number" whose app stores a number,
// Number(next). The person changes the text to another spelling of the same number ("05", "5.0"),
// so the input shows new text while the `value` prop stays 5.
const SERVER_ERRORS = { amount: 'Server error' };

function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function describeValue(v: unknown) {
  return { type: typeof v, value: v };
}

interface AppProps {
  type?: 'number' | 'text';
  store?: 'number' | 'text';
  controlled?: boolean;
  mode?: 'onChange' | 'onSubmit';
  errors?: Record<string, string>;
  validate?: (v: unknown) => string | null;
  calls: Record<string, unknown>[];
  submitted: unknown[];
  stored: unknown[];
}

function App(props: AppProps) {
  const { type = 'number', store = 'number', controlled = true } = props;
  const [value, setValue] = React.useState<number | string>(store === 'number' ? 5 : '5');
  props.stored.length = 0;
  props.stored.push(describeValue(value));
  return (
    <Form errors={props.errors} onFormSubmit={(values) => props.submitted.push(values)}>
      <Field.Root
        data-testid="root"
        name="amount"
        validationMode={props.mode}
        validate={(v) => {
          const verdict = props.validate ? props.validate(v) : null;
          props.calls.push({ ...describeValue(v), returned: verdict });
          return verdict;
        }}
      >
        {controlled ? (
          <Field.Control
            data-testid="control"
            type={type}
            value={value}
            onValueChange={(next) => setValue(store === 'number' ? Number(next) : next)}
          />
        ) : (
          <Field.Control data-testid="control" type={type} defaultValue={5} />
        )}
        <Field.Error data-testid="error" />
      </Field.Root>
      <button type="submit">Send</button>
    </Form>
  );
}

function snapshot(props: Pick<AppProps, 'calls' | 'submitted' | 'stored'>) {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>('control');
  return {
    inputText: control.value,
    appHolds: props.stored[0],
    error: screen.queryByTestId('error')?.textContent ?? null,
    ariaInvalid: control.getAttribute('aria-invalid'),
    rootDirty: root.hasAttribute('data-dirty'),
    validateCalls: [...props.calls],
    sent: [...props.submitted],
  };
}

function setText(text: string) {
  fireEvent.change(screen.getByTestId('control'), { target: { value: text } });
}

function send() {
  fireEvent.click(screen.getByText('Send'));
}

function fresh() {
  return { calls: [] as Record<string, unknown>[], submitted: [] as unknown[], stored: [] as unknown[] };
}

describe('C3n probe: a controlled number input respelled while the stored number stays the same', () => {
  const { render } = createRenderer();

  for (const mode of ['onSubmit', 'onChange'] as const) {
    it(`C3n-1 server error, validationMode="${mode}": text 5 -> 05, Send, then 05 -> 056, Send`, async () => {
      const s = fresh();
      await render(<App mode={mode} errors={SERVER_ERRORS} {...s} />);
      const atMount = snapshot(s);
      send();
      const afterSendUntouched = snapshot(s);
      setText('05');
      const afterRespelling = snapshot(s);
      send();
      const afterSend = snapshot(s);
      setText('056');
      const afterAcceptedEdit = snapshot(s);
      send();
      report(`C3n-1-server-error-${mode}`, {
        atMount,
        afterSendUntouched,
        afterTextBecomes05: afterRespelling,
        afterSend,
        afterTextBecomes056: afterAcceptedEdit,
        afterSecondSend: snapshot(s),
      });
    });
  }

  it('C3n-2 server error, other spellings of 5 set as the text one at a time: 5.0, 5.00, 5e0, 005', async () => {
    const results: Record<string, unknown> = {};
    for (const text of ['5.0', '5.00', '5e0', '005']) {
      const s = fresh();
      const view = await render(<App errors={SERVER_ERRORS} {...s} />);
      setText(text);
      const afterRespelling = snapshot(s);
      send();
      results[text] = { afterRespelling, afterSend: snapshot(s) };
      view.unmount();
    }
    report('C3n-2-other-spellings', results);
  });

  it('C3n-3 no server error: dirty mark and what the form sends after 5 -> 05, and after 05 -> 5', async () => {
    const s = fresh();
    await render(<App {...s} />);
    setText('05');
    const afterRespelling = snapshot(s);
    send();
    const afterSend = snapshot(s);
    setText('5');
    report('C3n-3-dirty-and-sent-value', {
      afterTextBecomes05: afterRespelling,
      afterSend,
      afterTextBackTo5: snapshot(s),
    });
  });

  it('C3n-4 the field\'s own validator error ("Too low" under 10) after a first Send, then 5 -> 05', async () => {
    const s = fresh();
    await render(<App validate={(v) => (Number(v) >= 10 ? null : 'Too low')} {...s} />);
    send();
    const afterFirstSend = snapshot(s);
    setText('05');
    const afterRespelling = snapshot(s);
    send();
    report('C3n-4-own-validator-error', {
      afterFirstSend,
      afterTextBecomes05: afterRespelling,
      afterSecondSend: snapshot(s),
    });
  });

  it('C3n-5 a validator that reads the text ("No leading zero"), validationMode onChange: 5 -> 05, Send', async () => {
    const s = fresh();
    await render(
      <App
        mode="onChange"
        validate={(v) => (/^0\d/.test(String(v)) ? 'No leading zero' : null)}
        {...s}
      />,
    );
    setText('05');
    const afterRespelling = snapshot(s);
    send();
    report('C3n-5-validator-that-reads-the-text', {
      afterTextBecomes05: afterRespelling,
      afterSend: snapshot(s),
    });
  });

  it('C3n-6 neighbours with a server error, each given the text 05 and then Send', async () => {
    const results: Record<string, unknown> = {};
    const variants: [string, Partial<AppProps>][] = [
      ['type-number-app-stores-number', {}],
      ['type-text-app-stores-number', { type: 'text' }],
      ['type-number-app-stores-the-text', { store: 'text' }],
      ['type-number-no-value-prop', { controlled: false }],
    ];
    for (const [label, variant] of variants) {
      const s = fresh();
      const view = await render(<App errors={SERVER_ERRORS} {...variant} {...s} />);
      setText('05');
      const afterRespelling = snapshot(s);
      send();
      results[label] = { afterTextBecomes05: afterRespelling, afterSend: snapshot(s) };
      view.unmount();
    }
    report('C3n-6-neighbours', results);
  });

  it('C3n-7 the same field, the app clears its own errors state when onValueChange fires', async () => {
    const submitted: unknown[] = [];
    function Clearing() {
      const [value, setValue] = React.useState(5);
      const [errors, setErrors] = React.useState<Record<string, string>>(SERVER_ERRORS);
      return (
        <Form errors={errors} onFormSubmit={(values) => submitted.push(values)}>
          <Field.Root data-testid="root" name="amount">
            <Field.Control
              data-testid="control"
              type="number"
              value={value}
              onValueChange={(next) => {
                setErrors({});
                setValue(Number(next));
              }}
            />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<Clearing />);
    setText('05');
    const errorAfterRespelling = screen.queryByTestId('error')?.textContent ?? null;
    send();
    report('C3n-7-app-clears-its-own-errors', {
      errorAfterTextBecomes05: errorAfterRespelling,
      errorAfterSend: screen.queryByTestId('error')?.textContent ?? null,
      sent: submitted,
    });
  });

  it('C3n-8 Base UI NumberField, controlled with the number 5, server error: its input text is set to 05, then blur, then Send', async () => {
    const submitted: unknown[] = [];
    const received: unknown[] = [];
    function Sibling() {
      const [value, setValue] = React.useState<number | null>(5);
      return (
        <Form errors={SERVER_ERRORS} onFormSubmit={(values) => submitted.push(values)}>
          <Field.Root data-testid="root" name="amount">
            <NumberField.Root
              value={value}
              onValueChange={(next) => {
                received.push(next);
                setValue(next);
              }}
            >
              <NumberField.Input data-testid="control" />
            </NumberField.Root>
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<Sibling />);
    const control = screen.getByTestId<HTMLInputElement>('control');
    const read = () => ({
      inputText: control.value,
      error: screen.queryByTestId('error')?.textContent ?? null,
      rootDirty: screen.getByTestId('root').hasAttribute('data-dirty'),
      onValueChangeReceived: [...received],
    });
    await act(async () => {
      control.focus();
    });
    fireEvent.change(control, { target: { value: '05' } });
    const afterRespelling = read();
    await act(async () => {
      control.blur();
    });
    const afterBlur = read();
    send();
    report('C3n-8-number-field-sibling', {
      afterTextBecomes05: afterRespelling,
      afterBlur,
      afterSend: read(),
      sent: submitted,
    });
  });
});

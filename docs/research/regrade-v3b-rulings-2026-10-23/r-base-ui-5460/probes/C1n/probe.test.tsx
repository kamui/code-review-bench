// Probe C1: a controlled Field.Control whose starting value the field records later than mount
// (a control disabled at mount) or whose value is set from code after mount.
// Observations only. Each case prints one PROBE line and asserts nothing.
// `baseline` is the field's own recorded initial value, read through Field.Validity.
import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(calls: unknown[]) {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>('control');
  return {
    inputValue: control.value ?? null,
    inputDisabled: control.disabled,
    baseline: screen.getByTestId('baseline').textContent,
    rootDirty: root.hasAttribute('data-dirty'),
    rootFilled: root.hasAttribute('data-filled'),
    rootInvalid: root.hasAttribute('data-invalid'),
    validateCalls: [...calls],
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

// One object for the life of the test, as a server response held in state would be.
const SERVER_ERRORS = { title: 'Server says no' };

type Where = 'root' | 'control' | 'never';

function LoadForm(props: {
  calls: unknown[];
  where: Where;
  validationMode?: 'onChange' | 'onSubmit';
  withValidator?: boolean;
  initial?: string;
  serverError?: boolean;
  rejects?: boolean;
}) {
  const { calls, where, validationMode, withValidator, initial = '', serverError, rejects } = props;
  const [value, setValue] = React.useState(initial);
  const [loading, setLoading] = React.useState(where !== 'never');
  const field = (
    <Field.Root
      data-testid="root"
      name="title"
      disabled={where === 'root' ? loading : undefined}
      validationMode={validationMode}
      validate={
        withValidator
          ? (v) => {
              calls.push(v);
              return rejects ? 'Rejected by the validator' : null;
            }
          : undefined
      }
    >
      <Field.Control
        data-testid="control"
        value={value}
        onValueChange={setValue}
        disabled={where === 'control' ? loading : undefined}
      />
      <Field.Error data-testid="error" />
      <Field.Validity>
        {(validity) => <span data-testid="baseline">{JSON.stringify(validity.initialValue)}</span>}
      </Field.Validity>
      <button
        type="button"
        onClick={() => {
          setValue('loaded');
          setLoading(false);
        }}
      >
        data arrives: enable and set value together
      </button>
      <button type="button" onClick={() => setValue('loaded')}>
        set value only
      </button>
      <button type="button" onClick={() => setLoading(false)}>
        enable only
      </button>
      <button type="button" onClick={() => setValue(initial)}>
        code resets to the starting value
      </button>
    </Field.Root>
  );
  if (serverError) {
    return <Form errors={SERVER_ERRORS}>{field}</Form>;
  }
  return field;
}

describe('C1 probe', () => {
  const { render } = createRenderer();

  // GT-r11's trigger: disabled at mount, enabled in the same render that sets the value.
  for (const where of ['root', 'control'] as const) {
    it(`K1 disabled on ${where}, default mode: enable and set the value in one render`, async () => {
      const calls: unknown[] = [];
      await render(<LoadForm calls={calls} where={where} />);
      const atMount = snapshot(calls);
      fireEvent.click(screen.getByText('data arrives: enable and set value together'));
      const afterLoad = snapshot(calls);
      const control = screen.getByTestId('control');
      fireEvent.change(control, { target: { value: 'loadedX' } });
      fireEvent.change(control, { target: { value: 'loaded' } });
      const afterPersonTypesAndReturns = snapshot(calls);
      report(`K1-${where}-default-enable-and-value-together`, {
        atMount,
        afterLoad,
        afterPersonTypesAndReturns,
      });
    });

    it(`K2 disabled on ${where}, onChange mode, validator accepts: enable and set the value in one render`, async () => {
      const calls: unknown[] = [];
      await render(<LoadForm calls={calls} where={where} validationMode="onChange" withValidator />);
      fireEvent.click(screen.getByText('data arrives: enable and set value together'));
      const afterLoad = snapshot(calls);
      const control = screen.getByTestId('control');
      fireEvent.change(control, { target: { value: 'loadedX' } });
      fireEvent.change(control, { target: { value: 'loaded' } });
      const afterPersonTypesAndReturns = snapshot(calls);
      fireEvent.change(control, { target: { value: '' } });
      const afterPersonEmptiesIt = snapshot(calls);
      report(`K2-${where}-onChange-enable-and-value-together`, {
        afterLoad,
        afterPersonTypesAndReturns,
        afterPersonEmptiesIt,
      });
    });
  }

  // Other routes for a control that is disabled at mount (on the control).
  it('D1 disabled at mount, default mode: the value arrives while disabled, enabled in a later render', async () => {
    const calls: unknown[] = [];
    await render(<LoadForm calls={calls} where="control" />);
    fireEvent.click(screen.getByText('set value only'));
    const afterValueWhileDisabled = snapshot(calls);
    fireEvent.click(screen.getByText('enable only'));
    const afterEnable = snapshot(calls);
    report('D1-control-default-value-while-disabled-then-enable', {
      afterValueWhileDisabled,
      afterEnable,
    });
  });

  it('D2 disabled at mount, default mode: enabled empty first, the value arrives in a later render', async () => {
    const calls: unknown[] = [];
    await render(<LoadForm calls={calls} where="control" />);
    fireEvent.click(screen.getByText('enable only'));
    const afterEnable = snapshot(calls);
    fireEvent.click(screen.getByText('set value only'));
    const afterValue = snapshot(calls);
    report('D2-control-default-enable-then-value', { afterEnable, afterValue });
  });

  it('D3 disabled at mount already holding its value, default mode: enabled in a later render, value unchanged', async () => {
    const calls: unknown[] = [];
    await render(<LoadForm calls={calls} where="control" initial="loaded" />);
    const atMount = snapshot(calls);
    fireEvent.click(screen.getByText('enable only'));
    const afterEnable = snapshot(calls);
    report('D3-control-default-mounted-with-value-then-enable', { atMount, afterEnable });
  });

  // The comment's example: never disabled, value hydrated after mount.
  it('H1 never disabled, default mode: the value is loaded from code after mount', async () => {
    const calls: unknown[] = [];
    await render(<LoadForm calls={calls} where="never" />);
    const atMount = snapshot(calls);
    fireEvent.click(screen.getByText('set value only'));
    const afterLoad = snapshot(calls);
    report('H1-never-disabled-default-load-after-mount', { atMount, afterLoad });
  });

  it('H2 never disabled, onChange mode, validator accepts: the value is loaded from code after mount', async () => {
    const calls: unknown[] = [];
    await render(<LoadForm calls={calls} where="never" validationMode="onChange" withValidator />);
    const atMount = snapshot(calls);
    fireEvent.click(screen.getByText('set value only'));
    const afterLoad = snapshot(calls);
    report('H2-never-disabled-onChange-load-after-mount', { atMount, afterLoad });
  });

  it('H4 never disabled, onChange mode, validator rejects every value: the value is loaded from code after mount', async () => {
    const calls: unknown[] = [];
    await render(
      <LoadForm calls={calls} where="never" validationMode="onChange" withValidator rejects />,
    );
    const atMount = snapshot(calls);
    fireEvent.click(screen.getByText('set value only'));
    const afterLoad = snapshot(calls);
    report('H4-never-disabled-onChange-rejecting-load-after-mount', { atMount, afterLoad });
  });

  it('H3 never disabled, inside a Form showing a server error: the value is loaded from code after mount', async () => {
    const calls: unknown[] = [];
    await render(<LoadForm calls={calls} where="never" serverError />);
    const atMount = snapshot(calls);
    fireEvent.click(screen.getByText('set value only'));
    const afterLoad = snapshot(calls);
    report('H3-never-disabled-server-error-load-after-mount', { atMount, afterLoad });
  });

  // What the change says it is for: a value set from code after the person has typed.
  it('R1 never disabled, default mode, starts "start": the person types, then code resets to "start"', async () => {
    const calls: unknown[] = [];
    await render(<LoadForm calls={calls} where="never" initial="start" />);
    const control = screen.getByTestId('control');
    fireEvent.change(control, { target: { value: 'startX' } });
    const afterPersonTypes = snapshot(calls);
    fireEvent.click(screen.getByText('code resets to the starting value'));
    const afterCodeReset = snapshot(calls);
    report('R1-never-disabled-default-code-reset-after-typing', { afterPersonTypes, afterCodeReset });
  });

  // The documented way to start from a new value: key Field.Root when the data arrives.
  it('W1 Field.Root given a new key when the data arrives (never disabled), default mode', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <div>
          <Field.Root key={value === '' ? 'empty' : 'loaded'} data-testid="root" name="title">
            <Field.Control data-testid="control" value={value} onValueChange={setValue} />
            <Field.Validity>
              {(validity) => (
                <span data-testid="baseline">{JSON.stringify(validity.initialValue)}</span>
              )}
            </Field.Validity>
          </Field.Root>
          <button type="button" onClick={() => setValue('loaded')}>
            set value only
          </button>
        </div>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('set value only'));
    report('W1-keyed-field-root-load-after-mount', { afterLoad: snapshot(calls) });
  });
});

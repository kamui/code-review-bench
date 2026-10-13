// Probe M3: a controlled Field.Control that starts disabled and is enabled in the same commit
// that sets its value (an edit form that keeps its fields disabled while data loads).
// Observations only. Each case prints one PROBE line and asserts nothing.
// `baseline` is the field's own recorded initial value, read through Field.Validity.
import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Checkbox } from '@base-ui/react/checkbox';

function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(calls: unknown[]) {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>('control');
  return {
    inputValue: control.value ?? null,
    inputDisabled: control.disabled ?? control.getAttribute('aria-disabled'),
    baseline: screen.getByTestId('baseline').textContent,
    rootDirty: root.hasAttribute('data-dirty'),
    rootFilled: root.hasAttribute('data-filled'),
    rootInvalid: root.hasAttribute('data-invalid'),
    validateCalls: [...calls],
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

type Where = 'root' | 'control';

function LoadForm(props: {
  calls: unknown[];
  where: Where;
  validationMode?: 'onChange' | 'onSubmit';
  withValidator?: boolean;
}) {
  const { calls, where, validationMode, withValidator } = props;
  const [value, setValue] = React.useState('');
  const [loading, setLoading] = React.useState(true);
  return (
    <Field.Root
      data-testid="root"
      name="title"
      disabled={where === 'root' ? loading : undefined}
      validationMode={validationMode}
      validate={
        withValidator
          ? (v) => {
              calls.push(v);
              return null;
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
      <button type="button" onClick={() => setValue('other')}>
        code sets other
      </button>
      <button type="button" onClick={() => setValue('loaded')}>
        code sets loaded again
      </button>
      <button type="button" onClick={() => setValue('')}>
        code sets empty
      </button>
    </Field.Root>
  );
}

describe('M3 probe: a control enabled in the same commit that sets its value', () => {
  const { render } = createRenderer();

  for (const where of ['root', 'control'] as Where[]) {
    it(`M3-1 disabled on ${where}, default mode: enable and set the value in one commit`, async () => {
      const calls: unknown[] = [];
      await render(<LoadForm calls={calls} where={where} />);
      const atMount = snapshot(calls);
      fireEvent.click(screen.getByText('data arrives: enable and set value together'));
      const afterLoad = snapshot(calls);
      const control = screen.getByTestId('control');
      fireEvent.change(control, { target: { value: 'loadedX' } });
      const afterPersonTypes = snapshot(calls);
      fireEvent.change(control, { target: { value: 'loaded' } });
      const afterPersonReturnsToLoadedValue = snapshot(calls);
      report(`M3-1-${where}-default-enable-and-value-together`, {
        atMount,
        afterLoad,
        afterPersonTypes,
        afterPersonReturnsToLoadedValue,
      });
    });

    it(`M3-2 disabled on ${where}, onChange mode with a validator that accepts: enable and set the value in one commit`, async () => {
      const calls: unknown[] = [];
      await render(<LoadForm calls={calls} where={where} validationMode="onChange" withValidator />);
      const atMount = snapshot(calls);
      fireEvent.click(screen.getByText('data arrives: enable and set value together'));
      const afterLoad = snapshot(calls);
      const control = screen.getByTestId('control');
      fireEvent.change(control, { target: { value: 'loadedX' } });
      const afterPersonTypes = snapshot(calls);
      fireEvent.change(control, { target: { value: 'loaded' } });
      const afterPersonReturnsToLoadedValue = snapshot(calls);
      fireEvent.change(control, { target: { value: '' } });
      const afterPersonEmptiesIt = snapshot(calls);
      report(`M3-2-${where}-onChange-enable-and-value-together`, {
        atMount,
        afterLoad,
        afterPersonTypes,
        afterPersonReturnsToLoadedValue,
        afterPersonEmptiesIt,
      });
    });

    it(`M3-3 disabled on ${where}, default mode: the value arrives while disabled, the control is enabled in a later commit`, async () => {
      const calls: unknown[] = [];
      await render(<LoadForm calls={calls} where={where} />);
      fireEvent.click(screen.getByText('set value only'));
      const afterValueWhileDisabled = snapshot(calls);
      fireEvent.click(screen.getByText('enable only'));
      const afterEnable = snapshot(calls);
      report(`M3-3-${where}-default-value-while-disabled-then-enable`, {
        afterValueWhileDisabled,
        afterEnable,
      });
    });

    it(`M3-7 disabled on ${where}, onChange mode with a validator that accepts: the value arrives while disabled, the control is enabled in a later commit`, async () => {
      const calls: unknown[] = [];
      await render(<LoadForm calls={calls} where={where} validationMode="onChange" withValidator />);
      fireEvent.click(screen.getByText('set value only'));
      const afterValueWhileDisabled = snapshot(calls);
      fireEvent.click(screen.getByText('enable only'));
      const afterEnable = snapshot(calls);
      const control = screen.getByTestId('control');
      fireEvent.change(control, { target: { value: 'loadedX' } });
      fireEvent.change(control, { target: { value: 'loaded' } });
      const afterPersonReturnsToLoadedValue = snapshot(calls);
      report(`M3-7-${where}-onChange-value-while-disabled-then-enable`, {
        afterValueWhileDisabled,
        afterEnable,
        afterPersonReturnsToLoadedValue,
      });
    });

    it(`M3-4 disabled on ${where}, default mode: the control is enabled first, the value arrives in a later commit`, async () => {
      const calls: unknown[] = [];
      await render(<LoadForm calls={calls} where={where} />);
      fireEvent.click(screen.getByText('enable only'));
      const afterEnable = snapshot(calls);
      fireEvent.click(screen.getByText('set value only'));
      const afterValue = snapshot(calls);
      report(`M3-4-${where}-default-enable-then-value`, { afterEnable, afterValue });
    });
  }

  it('M3-5 reference: never disabled, default mode, the value is set from code after mount', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Field.Root data-testid="root" name="title">
          <Field.Control data-testid="control" value={value} onValueChange={setValue} />
          <Field.Validity>
            {(validity) => (
              <span data-testid="baseline">{JSON.stringify(validity.initialValue)}</span>
            )}
          </Field.Validity>
          <button type="button" onClick={() => setValue('loaded')}>
            set value only
          </button>
        </Field.Root>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('set value only'));
    report('M3-5-ref-never-disabled-value-from-code', { afterValue: snapshot(calls) });
  });

  it('M3-8 the way the maintainers name for a new starting value: Field.Root is given a new key when the data arrives', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('');
      const [loading, setLoading] = React.useState(true);
      return (
        <div>
          <Field.Root key={loading ? 'loading' : 'loaded'} data-testid="root" name="title" disabled={loading}>
            <Field.Control data-testid="control" value={value} onValueChange={setValue} />
            <Field.Validity>
              {(validity) => (
                <span data-testid="baseline">{JSON.stringify(validity.initialValue)}</span>
              )}
            </Field.Validity>
          </Field.Root>
          <button
            type="button"
            onClick={() => {
              setValue('loaded');
              setLoading(false);
            }}
          >
            data arrives: enable and set value together
          </button>
        </div>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('data arrives: enable and set value together'));
    const afterLoad = snapshot(calls);
    const control = screen.getByTestId('control');
    fireEvent.change(control, { target: { value: 'loadedX' } });
    const afterPersonTypes = snapshot(calls);
    fireEvent.change(control, { target: { value: 'loaded' } });
    report('M3-8-keyed-field-root-enable-and-value-together', {
      afterLoad,
      afterPersonTypes,
      afterPersonReturnsToLoadedValue: snapshot(calls),
    });
  });

  it('M3-6 sibling: a controlled Checkbox in a Field.Root that is enabled in the commit that ticks it', async () => {
    const calls: unknown[] = [];
    function App() {
      const [checked, setChecked] = React.useState(false);
      const [loading, setLoading] = React.useState(true);
      return (
        <Field.Root data-testid="root" name="agree" disabled={loading}>
          <Checkbox.Root data-testid="control" checked={checked} onCheckedChange={setChecked} />
          <Field.Validity>
            {(validity) => (
              <span data-testid="baseline">{JSON.stringify(validity.initialValue)}</span>
            )}
          </Field.Validity>
          <button
            type="button"
            onClick={() => {
              setChecked(true);
              setLoading(false);
            }}
          >
            data arrives: enable and set value together
          </button>
        </Field.Root>
      );
    }
    await render(<App />);
    const atMount = snapshot(calls);
    fireEvent.click(screen.getByText('data arrives: enable and set value together'));
    report('M3-6-sibling-Checkbox-enable-and-value-together', {
      atMount,
      afterLoad: snapshot(calls),
    });
  });
});

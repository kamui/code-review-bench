import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { expect } from 'vitest';
import { Controller, useForm } from 'react-hook-form';
import { Form } from '@base-ui/react/form';

function snapshot() {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId('control');
  return {
    value: control.value ?? null,
    dirty: root.hasAttribute('data-dirty'),
    filled: root.hasAttribute('data-filled'),
    invalid: root.hasAttribute('data-invalid'),
    touched: root.hasAttribute('data-touched'),
    ariaInvalid: control.getAttribute('aria-invalid'),
    error: screen.queryByTestId('error')?.textContent ?? null,
  };
}
function report(name, data) { console.log('PROBE ' + name + ' ' + JSON.stringify(data)); }
async function click(text) {
  await React.act(async () => { fireEvent.click(screen.getByText(text)); });
}

describe('N1 late load compared with the mount baseline', () => {
  const { render } = createRenderer();
  for (const mode of ['onChange', 'onSubmit']) {
    it(mode + ' custom validator rejects loaded text', async () => {
      const calls = [];
      function App() {
        const [value, setValue] = React.useState('');
        return <Form><Field.Root data-testid="root" name="message" validationMode={mode}
          validate={(v) => { calls.push(v); return v === 'loaded' ? 'Loaded value rejected' : null; }}>
          <Field.Control data-testid="control" value={value} onValueChange={setValue} />
          <Field.Error data-testid="error" />
          <button type="button" onClick={() => setValue('loaded')}>Load</button>
        </Field.Root></Form>;
      }
      await render(<App />);
      const before = snapshot();
      await click('Load');
      report(mode, { before, afterLoad: snapshot(), calls });
    });
  }
  it('remounting Field.Root with loaded data supplies a fresh baseline', async () => {
    const calls = [];
    function App() {
      const [value, setValue] = React.useState('');
      return <><Field.Root key={value} data-testid="root" validationMode="onChange"
        validate={(v) => { calls.push(v); return 'Loaded value rejected'; }}>
        <Field.Control data-testid="control" value={value} onValueChange={setValue} />
        <Field.Error data-testid="error" />
      </Field.Root><button onClick={() => setValue('loaded')}>Load</button></>;
    }
    await render(<App />);
    await click('Load');
    report('fresh-root', { afterLoad: snapshot(), calls });
  });
});


describe('N1 documented external state ownership', () => {
  const { render } = createRenderer();
  it('the handbook integration accepts late-loaded data as pristine', async () => {
    function App() {
      const { control, reset, handleSubmit } = useForm({ defaultValues: { username: '' } });
      return <Form onSubmit={handleSubmit(() => {})}>
        <Controller name="username" control={control}
          rules={{ validate: (value) => value === 'loaded' ? 'Loaded value rejected' : true }}
          render={({ field: { name, ref, value, onBlur, onChange },
            fieldState: { invalid, isTouched, isDirty, error } }) => (
            <Field.Root data-testid="root" name={name} invalid={invalid} touched={isTouched} dirty={isDirty}>
              <Field.Control data-testid="control" ref={ref} value={value}
                onBlur={onBlur} onValueChange={onChange} />
              <Field.Error data-testid="error" match={!!error}>{error?.message}</Field.Error>
            </Field.Root>
          )} />
        <button type="button" onClick={() => reset({ username: 'loaded' })}>Load</button>
        <button type="submit">Submit</button>
      </Form>;
    }
    await render(<App />);
    await click('Load');
    const afterLoad = snapshot();
    expect(afterLoad.value).toBe('loaded');
    expect(afterLoad.dirty).toBe(false);
    expect(afterLoad.touched).toBe(false);
    expect(afterLoad.error).toBe(null);
    await click('Submit');
    const afterSubmit = snapshot();
    expect(afterSubmit.error).toBe('Loaded value rejected');
    report('documented-rhf', { afterLoad, afterSubmit });
  });
});

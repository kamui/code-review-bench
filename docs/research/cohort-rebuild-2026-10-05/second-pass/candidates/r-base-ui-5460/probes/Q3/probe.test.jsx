import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

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

describe('Q3 native required after code prefill and later reset', () => {
  const { render } = createRenderer();
  it('load nonempty text, then return to empty', async () => {
    const calls = [];
    function App() {
      const [value, setValue] = React.useState('');
      return <Field.Root data-testid="root" validationMode="onChange" validate={(v) => { calls.push(v); return null; }}>
        <Field.Control data-testid="control" required value={value} onValueChange={setValue} />
        <Field.Error data-testid="error" />
        <button onClick={() => setValue('loaded')}>Load</button>
        <button onClick={() => setValue('')}>Reset</button>
      </Field.Root>;
    }
    await render(<App />);
    await click('Load');
    const afterLoad = snapshot();
    const callsAfterLoad = [...calls];
    await click('Reset');
    report('load-then-reset', { afterLoad, callsAfterLoad, afterReset: snapshot(), calls });
  });
});

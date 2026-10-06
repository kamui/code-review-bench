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

describe('Q2 changing a controlled value to absent values', () => {
  const { render } = createRenderer();
  for (const next of [null, undefined, '']) {
    it('programmatic transition to ' + String(next), async () => {
      const warnings = [];
      const spy = vi.spyOn(console, 'error').mockImplementation((...args) => { warnings.push(args.map(String).join(' ')); });
      try {
        function App() {
          const [value, setValue] = React.useState('abc');
          return <Field.Root data-testid="root"><Field.Control data-testid="control" value={value} onValueChange={setValue} />
            <button onClick={() => setValue(next)}>Clear</button></Field.Root>;
        }
        await render(<App />);
        const before = snapshot();
        await click('Clear');
        report('prop-to-' + String(next), { before, after: snapshot(), warnings });
      } finally { spy.mockRestore(); }
    });
  }
  it('user clears and consumer stores null', async () => {
    const warnings = [];
    const spy = vi.spyOn(console, 'error').mockImplementation((...args) => { warnings.push(args.map(String).join(' ')); });
    try {
      function App() {
        const [value, setValue] = React.useState('abc');
        return <Field.Root data-testid="root"><Field.Control data-testid="control" value={value} onValueChange={(v) => setValue(v || null)} /></Field.Root>;
      }
      await render(<App />);
      await React.act(async () => { fireEvent.change(screen.getByTestId('control'), { target: { value: '' } }); });
      report('user-clear-stores-null', { after: snapshot(), warnings });
    } finally { spy.mockRestore(); }
  });
});

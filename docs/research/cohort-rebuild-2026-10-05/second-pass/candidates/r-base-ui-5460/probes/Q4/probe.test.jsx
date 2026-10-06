import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
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

describe('Q4 registration changes the submit validator argument', () => {
  const { render } = createRenderer();
  for (const value of [5, ['a', 'b']]) {
    it('submit controlled ' + JSON.stringify(value), async () => {
      const calls = [];
      const baselines = [];
      const submitted = [];
      await render(<Form onFormSubmit={(v) => submitted.push(v)}><Field.Root data-testid="root" name="value"
        validate={(v) => { calls.push({ value: v, type: typeof v, array: Array.isArray(v) }); return typeof v === 'number' || Array.isArray(v) ? null : 'Expected the original value type'; }}>
        <Field.Control data-testid="control" value={value} onValueChange={() => {}} />
        <Field.Validity>{(data) => { baselines.push(data.initialValue); return null; }}</Field.Validity>
        <Field.Error data-testid="error" />
      </Field.Root><button type="submit">Send</button></Form>);
      await click('Send');
      report('submit-' + JSON.stringify(value), { calls, submitted, initialValue: baselines.at(-1), after: snapshot() });
    });
  }
});

import * as React from 'react';
import { expect, vi } from 'vitest';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { Toggle } from '@base-ui/react/toggle';
import { mergeProps } from '@base-ui/react/merge-props';

describe('ruling M1', () => {
  const { render } = createRenderer({ strict: false });
  const cases = ['direct-store', 'render-store', 'direct-reject', 'uncontrolled', 'ordinary-store', 'cancel-uncontrolled', 'disabled-programmatic', 'disabled-invalid', 'disabled-root-programmatic', 'disabled-root-invalid'] as const;
  for (const scenario of cases) {
    it(scenario, async () => {
      const validate = vi.fn((value: unknown) => value === 'bad' ? 'Rejected' : null);
      const serverErrors = { message: 'Server error' };
      const onValueChange = vi.fn();
      const events: object[] = [];
      function App() {
        const [value, setValue] = React.useState('');
        const controlled = scenario !== 'uncontrolled' && scenario !== 'cancel-uncontrolled';
        const onChange: NonNullable<Field.Control.Props['onChange']> = (event) => {
          if (scenario !== 'ordinary-store') event.preventBaseUIHandler();
          if (scenario === 'direct-store' || scenario === 'render-store' || scenario === 'ordinary-store') setValue(event.currentTarget.value);
          events.push({ cancelable: event.nativeEvent.cancelable, nativePrevented: event.nativeEvent.defaultPrevented, baseUIPrevented: event.baseUIHandlerPrevented });
        };
        return <Form errors={serverErrors}>
          <Field.Root name="message" disabled={scenario.startsWith('disabled-root')} validationMode="onChange" validate={validate}>
            <Field.Control data-testid="input" disabled={scenario.startsWith('disabled-') && !scenario.startsWith('disabled-root')}
              {...(controlled ? { value } : {})}
              onValueChange={(next, details) => {
                onValueChange(next);
                if (scenario === 'cancel-uncontrolled') details.cancel();
              }}
              {...(scenario !== 'render-store' && scenario !== 'cancel-uncontrolled' && scenario !== 'disabled-programmatic' ? { onChange } : {})}
              {...(scenario === 'render-store' ? { render: (props: React.ComponentProps<'input'>) => <input {...mergeProps<'input'>(props, { onChange })} /> } : {})}
            />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="button" onClick={() => setValue(scenario.endsWith('invalid') ? 'bad' : 'code')}>Set from code</button>
        </Form>;
      }
      const { user } = await render(<App />);
      const input = screen.getByTestId('input');
      const snap = () => ({ value: input instanceof HTMLInputElement ? input.value : null, dirty: input.hasAttribute('data-dirty'), filled: input.hasAttribute('data-filled'), invalid: input.getAttribute('aria-invalid'), error: screen.queryByTestId('error')?.textContent ?? null, validation: validate.mock.calls.map(call => call[0]), valueCallbacks: onValueChange.mock.calls.length });
      const before = snap();
      if (scenario.startsWith('disabled-')) await user.click(screen.getByText('Set from code'));
      else await user.type(input, 'a');
      console.log('OBSERVATION ' + JSON.stringify({ scenario, phase: 'first', before, after: snap(), events }));
      if (scenario === 'direct-store' || scenario === 'render-store' || scenario === 'direct-reject' || scenario === 'uncontrolled') expect(onValueChange).not.toHaveBeenCalled();
      if (scenario === 'direct-store' || scenario === 'render-store' || scenario === 'direct-reject') {
        validate.mockClear();
        await user.click(screen.getByText('Set from code'));
        console.log('OBSERVATION ' + JSON.stringify({ scenario, phase: 'later-code', after: snap() }));
      }
    });
  }
  it('known-r6-rejecting', async () => {
    const validate = vi.fn((_value: unknown) => 'Too short');
    const { user } = await render(<Field.Root validationMode="onChange" validate={validate}><Field.Control data-testid="input" onValueChange={(_next, details) => details.cancel()} /><Field.Error data-testid="error" /></Field.Root>);
    const input = screen.getByTestId('input');
    await user.type(input, 'ab');
    console.log('OBSERVATION ' + JSON.stringify({ scenario: 'known-r6-rejecting', phase: 'first', after: { value: input instanceof HTMLInputElement ? input.value : null, dirty: input.hasAttribute('data-dirty'), filled: input.hasAttribute('data-filled'), error: screen.queryByTestId('error')?.textContent ?? null, validation: validate.mock.calls.map(call => call[0]) } }));
  });
  for (const rootDisabled of [false, true]) {
    it('known-r8-baseline-' + rootDisabled, async () => {
      const validate = vi.fn((_value: unknown) => 'Always wrong');
      function App() {
        const [value, setValue] = React.useState('foo');
        const [disabled, setDisabled] = React.useState(true);
        return <div><Field.Root data-testid="root" disabled={rootDisabled && disabled} validationMode="onChange" validate={validate}>
          <Field.Control data-testid="input" value={value} disabled={!rootDisabled && disabled} onValueChange={setValue} />
          <Field.Error data-testid="error" />
        </Field.Root><button onClick={() => setValue('bar')}>Change</button><button onClick={() => setDisabled(false)}>Enable</button><button onClick={() => setValue('foo')}>Restore</button></div>;
      }
      const { user } = await render(<App />);
      const root = screen.getByTestId('root');
      const input = screen.getByTestId('input');
      for (const action of ['Change', 'Enable', 'Restore']) {
        await user.click(screen.getByText(action));
        console.log('OBSERVATION ' + JSON.stringify({ scenario: 'known-r8-baseline-' + rootDisabled, phase: action, after: { value: input instanceof HTMLInputElement ? input.value : null, dirty: root.hasAttribute('data-dirty'), invalid: root.hasAttribute('data-invalid'), error: screen.queryByTestId('error')?.textContent ?? null, validation: validate.mock.calls.map(call => call[0]) } }));
      }
    });
  }
  it('documented mergeProps toggle', async () => {
    const changed = vi.fn();
    const { user } = await render(<Toggle aria-label="Favorite" defaultPressed onPressedChange={changed} render={(props) => <button {...mergeProps<'button'>(props, { onClick(event) { event.preventBaseUIHandler(); } })} />} />);
    const button = screen.getByRole('button');
    await user.click(button);
    expect(changed).not.toHaveBeenCalled();
    expect(button.getAttribute('aria-pressed')).toBe('true');
    console.log('OBSERVATION ' + JSON.stringify({ scenario: 'documented-toggle', pressed: button.getAttribute('aria-pressed'), callbacks: changed.mock.calls.length }));
  });
});

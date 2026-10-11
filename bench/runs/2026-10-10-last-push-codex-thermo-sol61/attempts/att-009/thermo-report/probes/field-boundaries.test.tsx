import * as React from 'react';
import { afterEach, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

afterEach(cleanup);

it('preserves native prevention for an echoed controlled value', () => {
  const validate = vi.fn(() => null);
  const changes = vi.fn();
  function App() {
    const [value, setValue] = React.useState('');
    return <Form errors={{ message: 'Server error' }}>
      <Field.Root name="message" validationMode="onChange" validate={validate}>
        <Field.Control value={value} onValueChange={(next) => { changes(next); setValue(next); }} />
        <Field.Error />
      </Field.Root>
    </Form>;
  }
  render(<App />);
  const control = screen.getByRole('textbox');
  control.addEventListener('input', (event) => event.preventDefault(), { capture: true, once: true });
  fireEvent.input(control, { cancelable: true, target: { value: 'a' } });
  expect(changes).toHaveBeenCalledTimes(1);
  expect.soft(validate).not.toHaveBeenCalled();
  expect.soft(screen.queryByText('Server error')).not.toBeNull();
});

it('cancels all uncontrolled internal state updates', () => {
  const validate = vi.fn(() => null);
  render(<Field.Root data-testid="root" validationMode="onChange" validate={validate}>
    <Field.Control onValueChange={(_next, details) => details.cancel()} />
  </Field.Root>);
  fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a' } });
  expect(validate).not.toHaveBeenCalled();
  expect.soft(screen.getByTestId('root').hasAttribute('data-dirty')).toBe(false);
  expect.soft(screen.getByTestId('root').hasAttribute('data-filled')).toBe(false);
});

it('validates the DOM value when a controlled consumer appends a newline', () => {
  const validate = vi.fn((_value, _values) => null);
  function App() {
    const [value, setValue] = React.useState('');
    return <Form><Field.Root name="message" validationMode="onChange" validate={validate}>
      <Field.Control value={value} onValueChange={(next) => setValue(next + '\n')} />
    </Field.Root></Form>;
  }
  render(<App />);
  fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a' } });
  expect((screen.getByRole('textbox') as HTMLInputElement).value).toBe('a');
  expect(validate).toHaveBeenCalledTimes(1);
  expect(validate.mock.lastCall?.[1]).toEqual({ message: 'a' });
  expect(validate.mock.lastCall?.[0]).toBe('a');
});

it('keeps dirty false when a programmatic prop change has identical DOM text', () => {
  function App() {
    const [value, setValue] = React.useState('ab');
    return <Field.Root data-testid="root">
      <Field.Control value={value} onValueChange={setValue} />
      <button onClick={() => setValue('a\nb')}>set</button>
    </Field.Root>;
  }
  render(<App />);
  fireEvent.click(screen.getByRole('button'));
  expect((screen.getByRole('textbox') as HTMLInputElement).value).toBe('ab');
  expect(screen.getByTestId('root').hasAttribute('data-dirty')).toBe(false);
});

it('initializes filled to false when a filled control is replaced by an empty controlled control', () => {
  function App() {
    const [replacement, setReplacement] = React.useState(false);
    return <Field.Root data-testid="root">
      <Field.Control key={String(replacement)} value={replacement ? '' : 'foo'} onValueChange={() => {}} />
      <button onClick={() => setReplacement(true)}>replace</button>
    </Field.Root>;
  }
  render(<App />);
  expect(screen.getByTestId('root').hasAttribute('data-filled')).toBe(true);
  fireEvent.click(screen.getByRole('button'));
  expect((screen.getByRole('textbox') as HTMLInputElement).value).toBe('');
  expect(screen.getByTestId('root').hasAttribute('data-filled')).toBe(false);
});

it('does not validate a controlled value rejected by its owner', () => {
  const validate = vi.fn(() => null);
  render(<Field.Root data-testid="root" validationMode="onChange" validate={validate}>
    <Field.Control value="" onValueChange={() => {}} />
  </Field.Root>);
  fireEvent.change(screen.getByRole('textbox'), { target: { value: 'rejected' } });
  expect(validate).not.toHaveBeenCalled();
  expect(screen.getByTestId('root').hasAttribute('data-filled')).toBe(false);
  expect(screen.getByTestId('root').hasAttribute('data-dirty')).toBe(false);
});

it('validates an accepted rewritten controlled value once', () => {
  const validate = vi.fn(() => null);
  function App() {
    const [value, setValue] = React.useState('');
    return <Field.Root validationMode="onChange" validate={validate}>
      <Field.Control value={value} onValueChange={(next) => setValue(next.toUpperCase())} />
    </Field.Root>;
  }
  render(<App />);
  fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a' } });
  expect(validate).toHaveBeenCalledExactlyOnceWith('A', {});
});

it('invalidates a pending validator after a programmatic controlled change', async () => {
  let resolveOld!: (value: string) => void;
  const validate = vi.fn((value) => value === 'old'
    ? new Promise<string>((resolve) => { resolveOld = resolve; }) : null);
  function App() {
    const [value, setValue] = React.useState('');
    return <Field.Root data-testid="root" validationMode="onChange" validate={validate}>
      <Field.Control value={value} onValueChange={setValue} />
      <Field.Error />
      <button onClick={() => setValue('new')}>set</button>
    </Field.Root>;
  }
  render(<App />);
  fireEvent.change(screen.getByRole('textbox'), { target: { value: 'old' } });
  fireEvent.click(screen.getByRole('button'));
  await act(async () => { resolveOld('Old error'); });
  expect(validate).toHaveBeenCalledTimes(2);
  expect(screen.queryByText('Old error')).toBeNull();
  expect(screen.getByTestId('root').hasAttribute('data-invalid')).toBe(false);
});

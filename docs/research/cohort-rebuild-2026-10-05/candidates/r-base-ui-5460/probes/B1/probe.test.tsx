import * as React from 'react';
import { expect } from 'vitest';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Form re-applies its `errors` prop whenever the object identity changes, so keep it stable
// across re-renders (a real app holds server errors in state).
const SERVER_ERRORS = { message: 'Server error' };

// Observations only. Each case prints one PROBE line; nothing here asserts a "right" answer.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function snapshot(validateCalls: unknown[]) {
  const root = screen.getByTestId('root');
  return {
    validateCalls,
    serverErrorShown: screen.queryByText('Server error') !== null,
    rootDirty: root.hasAttribute('data-dirty'),
    rootFilled: root.hasAttribute('data-filled'),
    rootInvalid: root.hasAttribute('data-invalid'),
    inputValue: screen.getByRole<HTMLInputElement>('textbox').value,
  };
}

describe('B1 probe: prevented or cancelled change, controlled Field.Control', () => {
  const { render } = createRenderer();

  // B1a. The native input event is default-prevented. The consumer still stores the value.
  it('B1a controlled, native input event prevented, consumer stores the value', async () => {
    const calls: unknown[] = [];
    let eventInfo: Record<string, unknown> = {};
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Form errors={SERVER_ERRORS}>
          <Field.Root
            data-testid="root"
            name="message"
            validationMode="onChange"
            validate={(v) => {
              calls.push(v);
              return null;
            }}
          >
            <Field.Control
              value={value}
              onValueChange={(next, details) => {
                eventInfo = {
                  nativeEventType: details.event.type,
                  nativeEventCancelable: details.event.cancelable,
                  nativeEventDefaultPrevented: details.event.defaultPrevented,
                };
                setValue(next);
              }}
            />
            <Field.Error />
          </Field.Root>
        </Form>
      );
    }
    await render(<App />);
    const control = screen.getByRole<HTMLInputElement>('textbox');
    control.addEventListener('input', (event) => event.preventDefault(), {
      capture: true,
      once: true,
    });
    fireEvent.input(control, { cancelable: true, target: { value: 'a' } });
    report('B1a-controlled-prevented', { ...eventInfo, ...snapshot(calls) });
    expect(true).toBe(true);
  });

  // Reference: the same thing uncontrolled (this is the case the repository's own test pins).
  it('B1a-ref uncontrolled, native input event prevented', async () => {
    const calls: unknown[] = [];
    await render(
      <Form errors={SERVER_ERRORS}>
        <Field.Root
          data-testid="root"
          name="message"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return null;
          }}
        >
          <Field.Control />
          <Field.Error />
        </Field.Root>
      </Form>,
    );
    const control = screen.getByRole<HTMLInputElement>('textbox');
    control.addEventListener('input', (event) => event.preventDefault(), {
      capture: true,
      once: true,
    });
    fireEvent.input(control, { cancelable: true, target: { value: 'a' } });
    report('B1a-ref-uncontrolled-prevented', snapshot(calls));
  });

  // Reference: an ordinary (not cancelable) input event, as a browser sends when a person types.
  it('B1a-ref2 controlled, ordinary input event', async () => {
    const calls: unknown[] = [];
    let eventInfo: Record<string, unknown> = {};
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Form errors={SERVER_ERRORS}>
          <Field.Root
            data-testid="root"
            name="message"
            validationMode="onChange"
            validate={(v) => {
              calls.push(v);
              return null;
            }}
          >
            <Field.Control
              value={value}
              onValueChange={(next, details) => {
                eventInfo = {
                  nativeEventType: details.event.type,
                  nativeEventCancelable: details.event.cancelable,
                  nativeEventDefaultPrevented: details.event.defaultPrevented,
                };
                setValue(next);
              }}
            />
            <Field.Error />
          </Field.Root>
        </Form>
      );
    }
    await render(<App />);
    const control = screen.getByRole<HTMLInputElement>('textbox');
    // A listener that tries to prevent an event that is not cancelable has no effect.
    control.addEventListener('input', (event) => event.preventDefault(), {
      capture: true,
      once: true,
    });
    fireEvent.input(control, { target: { value: 'a' } });
    report('B1a-ref2-controlled-ordinary-event', { ...eventInfo, ...snapshot(calls) });
  });

  // B1a variant: the consumer calls preventDefault on React's change event in its own onChange.
  it('B1a-var controlled, consumer onChange calls event.preventDefault()', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Form errors={SERVER_ERRORS}>
          <Field.Root
            data-testid="root"
            name="message"
            validationMode="onChange"
            validate={(v) => {
              calls.push(v);
              return null;
            }}
          >
            <Field.Control
              value={value}
              onValueChange={setValue}
              onChange={(event) => event.preventDefault()}
            />
            <Field.Error />
          </Field.Root>
        </Form>
      );
    }
    await render(<App />);
    const control = screen.getByRole<HTMLInputElement>('textbox');
    fireEvent.input(control, { cancelable: true, target: { value: 'a' } });
    report('B1a-var-consumer-onChange-preventDefault-cancelable-event', snapshot(calls));
  });

  // B1b. The consumer calls details.cancel() and still stores the value.
  it('B1b controlled, details.cancel() and consumer stores the value', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Form errors={SERVER_ERRORS}>
          <Field.Root
            data-testid="root"
            name="message"
            validationMode="onChange"
            validate={(v) => {
              calls.push(v);
              return null;
            }}
          >
            <Field.Control
              value={value}
              onValueChange={(next, details) => {
                details.cancel();
                setValue(next);
              }}
            />
            <Field.Error />
          </Field.Root>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a' } });
    report('B1b-controlled-cancel-and-store', snapshot(calls));
  });

  // B1b reference: the consumer calls details.cancel() and does NOT store the value.
  it('B1b-ref controlled, details.cancel() and consumer rejects the value', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value] = React.useState('');
      return (
        <Form errors={SERVER_ERRORS}>
          <Field.Root
            data-testid="root"
            name="message"
            validationMode="onChange"
            validate={(v) => {
              calls.push(v);
              return null;
            }}
          >
            <Field.Control
              value={value}
              onValueChange={(next, details) => {
                details.cancel();
              }}
            />
            <Field.Error />
          </Field.Root>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a' } });
    report('B1b-ref-controlled-cancel-and-reject', snapshot(calls));
  });
});

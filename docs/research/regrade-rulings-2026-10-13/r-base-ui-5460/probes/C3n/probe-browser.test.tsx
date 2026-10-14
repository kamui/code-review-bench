import * as React from 'react';
import { vi } from 'vitest';
import { createRenderer, screen } from '@mui/internal-test-utils';
import { userEvent } from 'vitest/browser';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Real browser only (Chromium through Playwright). The keys are pressed through the browser's own
// input pipeline, so the input's text and the events are the ones a person's typing produces.
// Observations only. Each case prints one PROBE line and asserts nothing.
const SERVER_ERRORS = { amount: 'Server error' };

function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function pause() {
  return new Promise((resolve) => {
    setTimeout(resolve, 50);
  });
}

describe('C3n browser probe: real key presses in a controlled number input whose app stores Number(next)', () => {
  const { render } = createRenderer();

  async function mount(received: unknown[], submitted: unknown[], stored: unknown[]) {
    function App() {
      const [value, setValue] = React.useState(5);
      stored.length = 0;
      stored.push(value);
      return (
        <Form errors={SERVER_ERRORS} onFormSubmit={(values) => submitted.push(values)}>
          <Field.Root data-testid="root" name="amount">
            <Field.Control
              data-testid="control"
              type="number"
              value={value}
              onValueChange={(next) => {
                received.push(next);
                setValue(Number(next));
              }}
            />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
  }

  function read(received: unknown[], submitted: unknown[], stored: unknown[]) {
    const control = screen.getByTestId<HTMLInputElement>('control');
    return {
      inputText: control.value,
      valueAsNumber: control.valueAsNumber,
      appHolds: stored[0],
      onValueChangeReceived: [...received],
      error: screen.queryByTestId('error')?.textContent ?? null,
      rootDirty: screen.getByTestId('root').hasAttribute('data-dirty'),
      sent: [...submitted],
    };
  }

  it('C3n-B1 a 0 typed in front of the 5, then Send, then a 6 typed at the end, then Send', async () => {
    // Real, un-batched browser events make React print act() warnings, and this repository's
    // test setup fails a test on any console.error. Silence it: it is test-harness noise.
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const received: unknown[] = [];
    const submitted: unknown[] = [];
    const stored: unknown[] = [];
    await mount(received, submitted, stored);
    const control = screen.getByTestId<HTMLInputElement>('control');
    await userEvent.click(control);
    await userEvent.keyboard('{Home}0');
    await pause();
    const afterLeadingZero = read(received, submitted, stored);
    await userEvent.click(screen.getByText('Send'));
    await pause();
    const afterSend = read(received, submitted, stored);
    await userEvent.click(control);
    await userEvent.keyboard('{End}6');
    await pause();
    const afterSix = read(received, submitted, stored);
    await userEvent.click(screen.getByText('Send'));
    await pause();
    report('C3n-B1-real-keys-leading-zero', {
      userAgent: navigator.userAgent,
      afterLeadingZero,
      afterSend,
      afterTypingSixAtTheEnd: afterSix,
      afterSecondSend: read(received, submitted, stored),
    });
  });

  it('C3n-B2 ".0" typed after the 5, one key at a time, then Send', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const received: unknown[] = [];
    const submitted: unknown[] = [];
    const stored: unknown[] = [];
    await mount(received, submitted, stored);
    const control = screen.getByTestId<HTMLInputElement>('control');
    await userEvent.click(control);
    await userEvent.keyboard('{End}.');
    await pause();
    const afterPoint = read(received, submitted, stored);
    await userEvent.keyboard('0');
    await pause();
    const afterPointZero = read(received, submitted, stored);
    await userEvent.click(screen.getByText('Send'));
    await pause();
    report('C3n-B2-real-keys-point-zero', {
      afterPoint,
      afterPointZero,
      afterSend: read(received, submitted, stored),
    });
  });

  it('C3n-B3 the 5 deleted and typed again, then Send', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const received: unknown[] = [];
    const submitted: unknown[] = [];
    const stored: unknown[] = [];
    await mount(received, submitted, stored);
    const control = screen.getByTestId<HTMLInputElement>('control');
    await userEvent.click(control);
    await userEvent.keyboard('{End}{Backspace}');
    await pause();
    const afterDelete = read(received, submitted, stored);
    await userEvent.keyboard('5');
    await pause();
    const afterRetype = read(received, submitted, stored);
    await userEvent.click(screen.getByText('Send'));
    await pause();
    report('C3n-B3-real-keys-delete-and-retype', {
      afterDelete,
      afterRetype,
      afterSend: read(received, submitted, stored),
    });
  });

  it('C3n-B4 the platform alone: a plain <input type="number"> with no library, a 0 typed in front of the 5', async () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const inputEvents: string[] = [];
    await render(
      <input
        data-testid="plain"
        type="number"
        defaultValue="5"
        onInput={(event) => inputEvents.push(event.currentTarget.value)}
      />,
    );
    const plain = screen.getByTestId<HTMLInputElement>('plain');
    await userEvent.click(plain);
    await userEvent.keyboard('{Home}0');
    await pause();
    report('C3n-B4-plain-number-input', {
      inputText: plain.value,
      valueAsNumber: plain.valueAsNumber,
      inputEventsFiredWithText: inputEvents,
      validityValid: plain.validity.valid,
    });
  });
});

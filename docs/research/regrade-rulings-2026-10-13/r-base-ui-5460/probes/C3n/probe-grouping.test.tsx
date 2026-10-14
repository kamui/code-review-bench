import * as React from 'react';
import { vi } from 'vitest';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Observations only. Each case prints one PROBE line and asserts nothing.
// Every failing case of the known problem GT-r5 (routes 1 to 4 of its trigger) and of this group
// (a controlled number input respelled while the stored number stays the same), plus three controls,
// run against the head and against each fix patch under patches/.
// One fix patch does its work in a microtask after the change event, so every step is followed by
// settle(), which lets queued microtasks run inside React's act().
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

async function settle() {
  await act(async () => {});
}

async function click(element: HTMLElement) {
  fireEvent.click(element);
  await settle();
}

async function setText(text: string, testId = 'control') {
  fireEvent.change(screen.getByTestId(testId), { target: { value: text } });
  await settle();
}

function state(calls: unknown[], submitted: unknown[]) {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>('control');
  return {
    inputText: control.value,
    checked: control.type === 'checkbox' || control.type === 'radio' ? control.checked : undefined,
    error: screen.queryByTestId('error')?.textContent ?? null,
    rootInvalid: root.hasAttribute('data-invalid'),
    rootDirty: root.hasAttribute('data-dirty'),
    validateCalls: [...calls],
    sent: [...submitted],
  };
}

describe('C3n grouping probe: GT-r5 cases, respelling cases and controls', () => {
  const { render } = createRenderer();

  beforeEach(() => {
    // A state update made from a microtask can print an act() warning, and this repository's test
    // setup fails a test on any console.error. The lines printed below are what is compared.
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  it('G1 GT-r5 route 1: checkbox value="yes", onChange mode, server error: click', async () => {
    const calls: unknown[] = [];
    await render(
      <Form errors={{ agree: 'Server error' }}>
        <Field.Root
          data-testid="root"
          name="agree"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return null;
          }}
        >
          <Field.Control data-testid="control" type="checkbox" value="yes" />
          <Field.Error data-testid="error" />
        </Field.Root>
      </Form>,
    );
    await click(screen.getByTestId('control'));
    report('G1-gt-r5-checkbox-server-error-click', state(calls, []));
  });

  it('G2 GT-r5 route 2: required checkbox value="yes", default mode: Send unticked, tick, Send', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(
      <Form onFormSubmit={(values) => submitted.push(values)}>
        <Field.Root data-testid="root" name="agree">
          <Field.Control data-testid="control" type="checkbox" value="yes" required />
          <Field.Error data-testid="error" />
        </Field.Root>
        <button type="submit">Send</button>
      </Form>,
    );
    await click(screen.getByText('Send'));
    const afterFirstSend = state(calls, submitted);
    await click(screen.getByTestId('control'));
    const afterTicking = state(calls, submitted);
    await click(screen.getByText('Send'));
    report('G2-gt-r5-required-checkbox', {
      errorAfterFirstSend: afterFirstSend.error,
      errorAfterTicking: afterTicking.error,
      invalidAfterTicking: afterTicking.rootInvalid,
      afterSecondSend: state(calls, submitted),
    });
  });

  it('G3 GT-r5 route 3: ticked checkbox value="yes", server error: untick, tick, Send', async () => {
    const submitted: unknown[] = [];
    await render(
      <Form errors={{ agree: 'Server error' }} onFormSubmit={(values) => submitted.push(values)}>
        <Field.Root data-testid="root" name="agree">
          <Field.Control data-testid="control" type="checkbox" value="yes" defaultChecked />
          <Field.Error data-testid="error" />
        </Field.Root>
        <button type="submit">Send</button>
      </Form>,
    );
    await click(screen.getByTestId('control'));
    await click(screen.getByTestId('control'));
    const afterToggling = state([], submitted);
    await click(screen.getByText('Send'));
    report('G3-gt-r5-checkbox-untick-tick-send', {
      errorAfterToggling: afterToggling.error,
      afterSend: state([], submitted),
    });
  });

  it('G4 GT-r5 route 4: radio value="a", onChange mode, server error: click', async () => {
    const calls: unknown[] = [];
    await render(
      <Form errors={{ plan: 'Server error' }}>
        <Field.Root
          data-testid="root"
          name="plan"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return null;
          }}
        >
          <Field.Control data-testid="control" type="radio" value="a" />
          <Field.Error data-testid="error" />
        </Field.Root>
      </Form>,
    );
    await click(screen.getByTestId('control'));
    report('G4-gt-r5-radio-server-error-click', state(calls, []));
  });

  function NumberApp(props: {
    mode?: 'onChange' | 'onSubmit';
    errors?: Record<string, string>;
    calls: unknown[];
    submitted: unknown[];
  }) {
    const [value, setValue] = React.useState(5);
    return (
      <Form errors={props.errors} onFormSubmit={(values) => props.submitted.push(values)}>
        <Field.Root
          data-testid="root"
          name="amount"
          validationMode={props.mode}
          validate={(v) => {
            props.calls.push(v);
            return null;
          }}
        >
          <Field.Control
            data-testid="control"
            type="number"
            value={value}
            onValueChange={(next) => setValue(Number(next))}
          />
          <Field.Error data-testid="error" />
        </Field.Root>
        <button type="submit">Send</button>
      </Form>
    );
  }

  it('R1 respelling: number input 5, app stores Number(next), server error, default mode: text 05, Send', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<NumberApp errors={{ amount: 'Server error' }} calls={calls} submitted={submitted} />);
    await setText('05');
    const afterRespelling = state(calls, submitted);
    await click(screen.getByText('Send'));
    report('R1-respelling-server-error-05', {
      errorAfterTextBecomes05: afterRespelling.error,
      dirtyAfterTextBecomes05: afterRespelling.rootDirty,
      afterSend: state(calls, submitted),
    });
  });

  it('R2 respelling: the same field, no server error, onChange mode: text 05, then 5.0, then 5', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<NumberApp mode="onChange" calls={calls} submitted={submitted} />);
    await setText('05');
    const after05 = state(calls, submitted);
    await setText('5.0');
    const after50 = state(calls, submitted);
    await setText('5');
    report('R2-respelling-validate-and-dirty', {
      afterTextBecomes05: { rootDirty: after05.rootDirty, validateCalls: after05.validateCalls },
      afterTextBecomes5point0: { rootDirty: after50.rootDirty, validateCalls: after50.validateCalls },
      afterTextBackTo5: state(calls, submitted),
    });
  });

  it('K1 control: a keystroke the app rejects, controlled text field, server error, onChange mode: type, Send', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('12');
      return (
        <Form errors={{ code: 'Server error' }} onFormSubmit={(values) => submitted.push(values)}>
          <Field.Root
            data-testid="root"
            name="code"
            validationMode="onChange"
            validate={(v) => {
              calls.push(v);
              return null;
            }}
          >
            <Field.Control
              data-testid="control"
              value={value}
              onValueChange={(next) => {
                if (/^\d*$/.test(next)) {
                  setValue(next);
                }
              }}
            />
            <Field.Error data-testid="error" />
          </Field.Root>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    await setText('12x');
    const afterRejected = state(calls, submitted);
    await click(screen.getByText('Send'));
    report('K1-control-rejected-keystroke', {
      afterRejectedKeystroke: afterRejected,
      afterSend: state(calls, submitted),
    });
  });

  it('K2 control: a keystroke the app accepts, controlled text field, onChange mode: how often validate runs', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('ab');
      return (
        <Field.Root
          data-testid="root"
          name="code"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return null;
          }}
        >
          <Field.Control data-testid="control" value={value} onValueChange={setValue} />
          <Field.Error data-testid="error" />
        </Field.Root>
      );
    }
    await render(<App />);
    await setText('abc');
    report('K2-control-accepted-keystroke', { validateCalls: calls, state: state(calls, []) });
  });

  it('K3 control: a keystroke the app rewrites to upper case, controlled text field, onChange mode', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('AB');
      return (
        <Field.Root
          data-testid="root"
          name="code"
          validationMode="onChange"
          validate={(v) => {
            calls.push(v);
            return null;
          }}
        >
          <Field.Control
            data-testid="control"
            value={value}
            onValueChange={(next) => setValue(next.toUpperCase())}
          />
          <Field.Error data-testid="error" />
        </Field.Root>
      );
    }
    await render(<App />);
    await setText('ABc');
    report('K3-control-rewritten-keystroke', { validateCalls: calls, state: state(calls, []) });
  });
});

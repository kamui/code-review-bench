import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// This is the probe written for recovery question Q5 (../Q5/probe.test.tsx), copied unchanged apart
// from this note, because N3 is the behaviour that question examined.
// Observations only. Each case prints one PROBE line and asserts nothing.
// A controlled text Field.Control inside a Form that shows a field error.
// The app's onValueChange decides what happens to each typed text.

type Decide = (typed: string, current: string) => string;

const decisions: Record<string, Decide> = {
  'rejects every edit': (_typed, current) => current,
  'rejects non-digits': (typed, current) => (/^\d*$/.test(typed) ? typed : current),
  'accepts the edit': (typed) => typed,
  'rewrites to upper case': (typed) => typed.toUpperCase(),
};

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

interface Options {
  mode?: 'onChange';
  decide: Decide;
  initial: string;
  typed: string;
  serverError?: string;
  validate?: (value: unknown) => string | null;
  submitFirst?: boolean;
}

async function run(render: ReturnType<typeof createRenderer>['render'], options: Options) {
  const { mode, decide, initial, typed, serverError, validate, submitFirst } = options;
  const validateCalls: string[] = [];
  const submitted: string[] = [];
  let phase = 'mount';
  // One object for the whole run, as an app keeps server errors in state. A new object on each
  // render would make the Form show the error again after every accepted keystroke.
  const serverErrors = serverError ? { field: serverError } : undefined;

  function App() {
    const [value, setValue] = React.useState(initial);
    return (
      <Form
        errors={serverErrors}
        onFormSubmit={(values) => submitted.push(`${phase}: ${JSON.stringify(values.field)}`)}
      >
        <Field.Root
          name="field"
          data-testid="root"
          validationMode={mode}
          validate={(candidate) => {
            validateCalls.push(`${phase}: ${JSON.stringify(candidate)}`);
            return validate ? validate(candidate) : null;
          }}
        >
          <Field.Control
            data-testid="control"
            value={value}
            onValueChange={(next) => setValue(decide(next, value))}
          />
          <Field.Error data-testid="error" />
        </Field.Root>
        <button type="submit">Send</button>
      </Form>
    );
  }

  await render(<App />);

  const snapshot = (step: string) => {
    const control = screen.getByTestId('control') as HTMLInputElement;
    return {
      step,
      inputShows: control.value,
      errorShown: screen.queryByTestId('error')?.textContent ?? null,
      ariaInvalid: control.getAttribute('aria-invalid'),
      fieldInvalid: screen.getByTestId('root').hasAttribute('data-invalid'),
      dirty: screen.getByTestId('root').hasAttribute('data-dirty'),
    };
  };

  const rows = [];
  if (submitFirst) {
    phase = 'first Send';
    fireEvent.click(screen.getByText('Send'));
    rows.push(snapshot('after the first Send'));
  } else {
    rows.push(snapshot('mount'));
  }

  phase = 'typing';
  fireEvent.change(screen.getByTestId('control'), { target: { value: typed } });
  rows.push(snapshot(`person types ${JSON.stringify(typed)}`));

  phase = 'Send after typing';
  fireEvent.click(screen.getByText('Send'));
  rows.push(snapshot('after Send'));

  return { rows, validateCalls, submitted };
}

describe('Q5 probe: a typed character the app rejects, on a field that shows an error', () => {
  const { render } = createRenderer();

  for (const mode of [undefined, 'onChange'] as const) {
    const modeName = mode ?? 'default';

    for (const [label, decide] of Object.entries(decisions)) {
      const digits = label === 'rejects non-digits';
      it(`Q5 server error, ${modeName} mode, app ${label}`, async () => {
        const result = await run(render, {
          mode,
          decide,
          initial: digits ? '12' : 'abc',
          typed: digits ? '12x' : 'abcd',
          serverError: 'Server error',
        });
        report(`Q5-server-error mode=${modeName} app=${label.replaceAll(' ', '-')}`, result);
      });
    }

    for (const label of ['rejects every edit', 'accepts the edit']) {
      it(`Q5 validator error after a Send, ${modeName} mode, app ${label}`, async () => {
        const result = await run(render, {
          mode,
          decide: decisions[label],
          initial: 'abc',
          typed: 'abcd',
          validate: (candidate) => (String(candidate).length >= 4 ? null : 'Too short'),
          submitFirst: true,
        });
        report(`Q5-validator-error mode=${modeName} app=${label.replaceAll(' ', '-')}`, result);
      });
    }
  }
});

import * as React from 'react';
import { vi } from 'vitest';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Observations only. Each case prints one PROBE line; nothing asserts a "right" answer.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function describeValue(v: unknown) {
  return { type: Array.isArray(v) ? 'array' : typeof v, value: v };
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => {
      setTimeout(resolve, 20);
    });
  });
}

type AnyValue = any;

interface Shape {
  id: string;
  source: string;
  fn: (value: AnyValue) => string | null;
}

const NUMBER_SHAPES: Shape[] = [
  { id: '1a', source: "value > 3 ? null : 'too small'", fn: (value) => (value > 3 ? null : 'too small') },
  {
    id: '1b',
    source: "Number(value) > 3 ? null : 'too small'",
    fn: (value) => (Number(value) > 3 ? null : 'too small'),
  },
  {
    id: '1c',
    source: "typeof value === 'number' && value > 3 ? null : 'must be a number above 3'",
    fn: (value) => (typeof value === 'number' && value > 3 ? null : 'must be a number above 3'),
  },
  {
    id: '1d',
    source: "Number.isInteger(value) ? null : 'must be a whole number'",
    fn: (value) => (Number.isInteger(value) ? null : 'must be a whole number'),
  },
  { id: '1e', source: "value === 5 ? null : 'must be 5'", fn: (value) => (value === 5 ? null : 'must be 5') },
];

const ARRAY_SHAPES: Shape[] = [
  {
    id: '2a',
    source: "value.length >= 2 ? null : 'pick at least two'",
    fn: (value) => (value.length >= 2 ? null : 'pick at least two'),
  },
  {
    id: '2b',
    source: "value.includes('a') ? null : 'must include a'",
    fn: (value) => (value.includes('a') ? null : 'must include a'),
  },
  {
    id: '2c',
    source: "value.every((x) => x.length === 1) ? null : 'one letter each'",
    fn: (value) => (value.every((x: string) => x.length === 1) ? null : 'one letter each'),
  },
  {
    id: '2d',
    source: "Array.isArray(value) ? null : 'must be a list'",
    fn: (value) => (Array.isArray(value) ? null : 'must be a list'),
  },
];

interface Log {
  validateCalls: Record<string, unknown>[];
  onFormSubmit: Record<string, unknown>[];
  onSubmit: Record<string, unknown>[];
}

function Harness(props: {
  shape: Shape;
  log: Log;
  initial: AnyValue;
  toState: (typed: string) => AnyValue;
  type?: string;
  validationMode?: 'onSubmit' | 'onBlur' | 'onChange';
}) {
  const { shape, log, initial, toState, type, validationMode } = props;
  const [value, setValue] = React.useState<AnyValue>(initial);
  return (
    <Form
      validationMode={validationMode}
      onSubmit={(event) => {
        // The Form's other submit callback: the native submit event.
        const data = new FormData(event.currentTarget);
        log.onSubmit.push({ nativeFormDataField: describeValue(data.get('field')) });
      }}
      onFormSubmit={(values) => {
        log.onFormSubmit.push({ valuesField: describeValue(values.field) });
      }}
    >
      <Field.Root
        name="field"
        validate={(v, formValues) => {
          const entry: Record<string, unknown> = {
            argument: describeValue(v),
            secondArgumentField: describeValue(formValues.field),
          };
          log.validateCalls.push(entry);
          try {
            const result = shape.fn(v);
            entry.returned = result;
            return result;
          } catch (error) {
            entry.threw = String(error);
            throw error;
          }
        }}
      >
        <Field.Control
          data-testid="control"
          type={type}
          value={value}
          onValueChange={(typed) => setValue(toState(typed))}
        />
        <Field.Error data-testid="error" />
      </Field.Root>
      <button type="submit">Send</button>
    </Form>
  );
}

function newLog(): Log {
  return { validateCalls: [], onFormSubmit: [], onSubmit: [] };
}

// Runs one step and returns only what that step added, plus what is on screen afterwards.
async function step(name: string, log: Log, action: () => void) {
  const before = {
    validate: log.validateCalls.length,
    onFormSubmit: log.onFormSubmit.length,
    onSubmit: log.onSubmit.length,
  };
  action();
  await settle();
  const control = screen.queryByTestId<HTMLInputElement>('control');
  return {
    step: name,
    validateCalls: log.validateCalls.slice(before.validate),
    onFormSubmitCalls: log.onFormSubmit.slice(before.onFormSubmit),
    onSubmitCalls: log.onSubmit.slice(before.onSubmit),
    submitWentThrough: log.onFormSubmit.length > before.onFormSubmit,
    errorTextShown: screen.queryByTestId('error')?.textContent ?? null,
    ariaInvalid: control?.getAttribute('aria-invalid') ?? null,
    controlStillRendered: control !== null,
    inputText: control?.value ?? null,
  };
}

describe('B3 shapes probe: plausible validators and what the app receives on submit', () => {
  const { render } = createRenderer();

  let consoleErrors: string[] = [];
  let consoleWarnings: string[] = [];
  let unhandledRejections: string[] = [];
  const onNodeRejection = (reason: unknown) => {
    unhandledRejections.push(String(reason));
  };
  const onBrowserRejection = (event: PromiseRejectionEvent) => {
    unhandledRejections.push(String(event.reason));
    event.preventDefault();
  };

  beforeEach(() => {
    consoleErrors = [];
    consoleWarnings = [];
    unhandledRejections = [];
    vi.spyOn(console, 'error').mockImplementation((...args) => {
      consoleErrors.push(String(args[0]).slice(0, 200));
    });
    vi.spyOn(console, 'warn').mockImplementation((...args) => {
      consoleWarnings.push(String(args[0]).slice(0, 200));
    });
    if (typeof process !== 'undefined' && typeof process.on === 'function') {
      process.on('unhandledRejection', onNodeRejection);
    }
    if (typeof window !== 'undefined') {
      window.addEventListener('unhandledrejection', onBrowserRejection);
    }
  });

  afterEach(() => {
    if (typeof process !== 'undefined' && typeof process.off === 'function') {
      process.off('unhandledRejection', onNodeRejection);
    }
    if (typeof window !== 'undefined') {
      window.removeEventListener('unhandledrejection', onBrowserRejection);
    }
  });

  function diagnostics() {
    return { consoleErrors, consoleWarnings, unhandledRejections };
  }

  // Case 1 and case 4: a number value, default validation mode.
  // Steps: submit; type 6 (valid for every shape but 1e); type 5 again; submit.
  for (const inputType of [undefined, 'number'] as const) {
    for (const shape of NUMBER_SHAPES) {
      const caseId = inputType ? `4-${shape.id}-type-number` : shape.id;
      it(`${caseId} number value, default mode: ${shape.source}`, async () => {
        const log = newLog();
        await render(
          <Harness shape={shape} log={log} initial={5} toState={(typed) => Number(typed)} type={inputType} />,
        );
        const control = screen.getByTestId('control');
        const steps = [
          await step('first submit', log, () => fireEvent.click(screen.getByText('Send'))),
          await step('type 6 after that submit', log, () =>
            fireEvent.change(control, { target: { value: '6' } }),
          ),
          await step('type 5 again', log, () => fireEvent.change(control, { target: { value: '5' } })),
          await step('second submit', log, () => fireEvent.click(screen.getByText('Send'))),
        ];
        report(caseId, { validator: shape.source, steps, ...diagnostics() });
      });
    }
  }

  // Case 5 support: the blur path, validationMode="onBlur", before any submit.
  for (const shape of NUMBER_SHAPES) {
    it(`${shape.id}-blur number value, onBlur mode: ${shape.source}`, async () => {
      const log = newLog();
      await render(
        <Harness
          shape={shape}
          log={log}
          initial={5}
          toState={(typed) => Number(typed)}
          validationMode="onBlur"
        />,
      );
      const control = screen.getByTestId('control');
      const steps = [
        await step('focus then blur without editing', log, () => {
          fireEvent.focus(control);
          fireEvent.blur(control);
        }),
        await step('submit', log, () => fireEvent.click(screen.getByText('Send'))),
      ];
      report(`${shape.id}-blur`, { validator: shape.source, steps, ...diagnostics() });
    });
  }

  // Case 2: an array value, default validation mode.
  // Steps: submit; type "a,b,c" (the app stores ['a','b','c']); submit.
  for (const shape of ARRAY_SHAPES) {
    it(`${shape.id} array value, default mode: ${shape.source}`, async () => {
      const log = newLog();
      await render(
        <Harness shape={shape} log={log} initial={['a', 'b']} toState={(typed) => typed.split(',')} />,
      );
      const control = screen.getByTestId('control');
      const steps = [
        await step('first submit', log, () => fireEvent.click(screen.getByText('Send'))),
        await step('type a,b,c after that submit', log, () =>
          fireEvent.change(control, { target: { value: 'a,b,c' } }),
        ),
        await step('second submit', log, () => fireEvent.click(screen.getByText('Send'))),
      ];
      report(shape.id, { validator: shape.source, steps, ...diagnostics() });
    });
  }

  for (const shape of ARRAY_SHAPES) {
    it(`${shape.id}-blur array value, onBlur mode: ${shape.source}`, async () => {
      const log = newLog();
      await render(
        <Harness
          shape={shape}
          log={log}
          initial={['a', 'b']}
          toState={(typed) => typed.split(',')}
          validationMode="onBlur"
        />,
      );
      const control = screen.getByTestId('control');
      const steps = [
        await step('focus then blur without editing', log, () => {
          fireEvent.focus(control);
          fireEvent.blur(control);
        }),
        await step('submit', log, () => fireEvent.click(screen.getByText('Send'))),
      ];
      report(`${shape.id}-blur`, { validator: shape.source, steps, ...diagnostics() });
    });
  }

  // Case 2 extra: inputs where the list and its text form give different answers for the two
  // validators that do not fail on text.
  it('2a-one-long-item array value ["ab"]: value.length >= 2', async () => {
    const log = newLog();
    await render(
      <Harness shape={ARRAY_SHAPES[0]} log={log} initial={['ab']} toState={(typed) => typed.split(',')} />,
    );
    const steps = [await step('first submit', log, () => fireEvent.click(screen.getByText('Send')))];
    report('2a-one-long-item', { validator: ARRAY_SHAPES[0].source, steps, ...diagnostics() });
  });

  it('2b-substring array value ["banana"]: value.includes("a")', async () => {
    const log = newLog();
    await render(
      <Harness
        shape={ARRAY_SHAPES[1]}
        log={log}
        initial={['banana']}
        toState={(typed) => typed.split(',')}
      />,
    );
    const steps = [await step('first submit', log, () => fireEvent.click(screen.getByText('Send')))];
    report('2b-substring', { validator: ARRAY_SHAPES[1].source, steps, ...diagnostics() });
  });

  // Case 3: what the app receives on a successful submit, with a validator that accepts anything.
  it('3 submitted values: number field and array field, validator accepts anything', async () => {
    const accept: Shape = { id: '3', source: 'null', fn: () => null };
    const numberLog = newLog();
    const first = await render(
      <Harness shape={accept} log={numberLog} initial={5} toState={(typed) => Number(typed)} />,
    );
    const numberStep = await step('submit number field', numberLog, () =>
      fireEvent.click(screen.getByText('Send')),
    );
    first.unmount();

    const arrayLog = newLog();
    await render(
      <Harness shape={accept} log={arrayLog} initial={['a', 'b']} toState={(typed) => typed.split(',')} />,
    );
    const arrayStep = await step('submit array field', arrayLog, () =>
      fireEvent.click(screen.getByText('Send')),
    );
    report('3-submitted-values', { numberField: numberStep, arrayField: arrayStep, ...diagnostics() });
  });
});

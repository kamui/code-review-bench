import * as React from 'react';
import { vi } from 'vitest';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Observations only. Each case prints PROBE lines and asserts nothing.
// The lines are compared between the commit before the change and its head.

type Loose = unknown;

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function describeValue(v: Loose) {
  if (Array.isArray(v)) {
    return `array ${JSON.stringify(v)}`;
  }
  if (v !== null && typeof v === 'object') {
    return `object ${JSON.stringify(v)} (text form ${JSON.stringify(String(v))})`;
  }
  return `${typeof v} ${JSON.stringify(v)}`;
}

let lastValidity: { initialValue: string; value: string } = { initialValue: '', value: '' };

function snapshot(step: string, appValue: Loose) {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId('control') as HTMLInputElement | HTMLSelectElement;
  const selected =
    control.tagName === 'SELECT'
      ? Array.from((control as HTMLSelectElement).selectedOptions).map((option) => option.value)
      : undefined;
  return {
    step,
    appValue: describeValue(appValue),
    dirty: root.hasAttribute('data-dirty'),
    controlDirty: control.hasAttribute('data-dirty'),
    filled: root.hasAttribute('data-filled'),
    controlShows: selected ? `selected options ${JSON.stringify(selected)}` : control.value,
    fieldInitialValue: lastValidity.initialValue,
  };
}

interface HarnessProps {
  initial: Loose;
  steps: Record<string, Loose>;
  parse?: (text: string) => Loose;
  control?: React.ReactElement;
  current: { value: Loose };
}

function Harness(props: HarnessProps) {
  const { initial, steps, parse = (text) => text, control, current } = props;
  const [value, setValue] = React.useState<Loose>(initial);
  current.value = value;
  return (
    <Field.Root data-testid="root">
      <Field.Control
        data-testid="control"
        value={value as string}
        onValueChange={(text) => setValue(parse(text))}
        render={control}
      />
      <Field.Validity>
        {(validity) => {
          lastValidity = {
            initialValue: describeValue(validity.initialValue),
            value: describeValue(validity.value),
          };
          return null;
        }}
      </Field.Validity>
      {Object.entries(steps).map(([label, next]) => (
        <button key={label} type="button" onClick={() => setValue(next)}>
          {label}
        </button>
      ))}
    </Field.Root>
  );
}

describe('N2 probe: dirty state of a controlled Field.Control with an array or object value', () => {
  const { render } = createRenderer();
  let errors: string[] = [];
  let errorSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    errors = [];
    lastValidity = { initialValue: '', value: '' };
    errorSpy = vi.spyOn(console, 'error').mockImplementation((...args: unknown[]) => {
      errors.push(args.map(String).join(' ').slice(0, 300));
    });
  });

  afterEach(() => {
    errorSpy.mockRestore();
  });

  async function runCodeSteps(name: string, initial: Loose, steps: Record<string, Loose>) {
    const current = { value: initial };
    await render(<Harness initial={initial} steps={steps} current={current} />);
    const rows = [snapshot('mount', current.value)];
    for (const label of Object.keys(steps)) {
      fireEvent.click(screen.getByText(label));
      rows.push(snapshot(`code sets: ${label}`, current.value));
    }
    report(name, { rows, consoleErrors: errors });
  }

  it('N2-1 arrays set from code, starting from one element that contains a comma', async () => {
    await runCodeSteps('N2-1-array-code', ['a,b'], {
      'same text, different array': ['a', 'b'],
      'different text': ['c', 'd'],
      'initial text again, new array': ['a,b'],
    });
  });

  it('N2-2 arrays set from code, starting from two elements', async () => {
    await runCodeSteps('N2-2-array-code-reversed', ['a', 'b'], {
      'same text, different array': ['a,b'],
      'different text': ['a', 'b', 'c'],
      'initial text again, new array': ['a', 'b'],
    });
  });

  it('N2-3 plain objects set from code', async () => {
    const first = { id: 1 };
    await runCodeSteps('N2-3-object-code', first, {
      'different object': { id: 2 },
      'another different object': { id: 3, name: 'x' },
      'object with its own toString': {
        toString() {
          return 'custom';
        },
      },
      'the first object again': first,
    });
  });

  it('N2-4 reference: strings set from code', async () => {
    await runCodeSteps('N2-4-string-code', 'a,b', {
      'different text': 'c',
      'initial text again': 'a,b',
    });
  });

  it('N2-5 a person types into an array-valued control; the app stores text.split(",")', async () => {
    const current = { value: undefined as Loose };
    await render(
      <Harness
        initial={['a', 'b']}
        steps={{ 'initial array again': ['a', 'b'] }}
        parse={(text) => text.split(',')}
        current={current}
      />,
    );
    const rows = [snapshot('mount', current.value)];
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a,b,c' } });
    rows.push(snapshot('person types "a,b,c"', current.value));
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a,b' } });
    rows.push(snapshot('person types back to "a,b"', current.value));
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a,b,c' } });
    rows.push(snapshot('person types "a,b,c" again', current.value));
    fireEvent.click(screen.getByText('initial array again'));
    rows.push(snapshot('code sets: initial array again', current.value));
    report('N2-5-array-typing', { rows, consoleErrors: errors });
  });

  it('N2-6 a person types back to the same text, which the app stores as a different array', async () => {
    const current = { value: undefined as Loose };
    await render(
      <Harness
        initial={['a,b']}
        steps={{}}
        parse={(text) => text.split(',')}
        current={current}
      />,
    );
    const rows = [snapshot('mount', current.value)];
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a,bc' } });
    rows.push(snapshot('person types "a,bc"', current.value));
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a,b' } });
    rows.push(snapshot('person types back to "a,b"', current.value));
    report('N2-6-array-typing-same-text', { rows, consoleErrors: errors });
  });

  it('N2-7 a person types into an object-valued control; the app stores { text }', async () => {
    const current = { value: undefined as Loose };
    await render(
      <Harness
        initial={{ text: 'hi' }}
        steps={{}}
        parse={(text) => ({ text })}
        current={current}
      />,
    );
    const rows = [snapshot('mount', current.value)];
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'x' } });
    rows.push(snapshot('person types "x"', current.value));
    report('N2-7-object-typing', { rows, consoleErrors: errors });
  });

  it('N2-8 rendered as <select multiple>, arrays set from code', async () => {
    const current = { value: undefined as Loose };
    const steps: Record<string, Loose> = {
      'same text, different selection': ['a', 'b'],
      'different text': ['a'],
      'initial selection again': ['a,b'],
    };
    await render(
      <Harness
        initial={['a,b']}
        steps={steps}
        control={
          <select multiple>
            <option value="a">a</option>
            <option value="b">b</option>
            <option value="a,b">a,b</option>
          </select>
        }
        current={current}
      />,
    );
    const rows = [snapshot('mount', current.value)];
    for (const label of Object.keys(steps)) {
      fireEvent.click(screen.getByText(label));
      rows.push(snapshot(`code sets: ${label}`, current.value));
    }
    report('N2-8-select-multiple-code', { rows, consoleErrors: errors });
  });

  it('N2-9 what the Form hands the app on submit for two arrays with the same text', async () => {
    const submitted: string[] = [];
    function App() {
      const [value, setValue] = React.useState<string[]>(['a,b']);
      return (
        <Form onFormSubmit={(values) => submitted.push(describeValue(values.tags))}>
          <Field.Root name="tags" data-testid="root">
            <Field.Control
              data-testid="control"
              value={value}
              onValueChange={(text) => setValue(text.split(','))}
            />
          </Field.Root>
          <button type="button" onClick={() => setValue(['a', 'b'])}>
            set two elements
          </button>
          <button type="submit">Send</button>
        </Form>
      );
    }
    await render(<App />);
    fireEvent.click(screen.getByText('Send'));
    const dirtyBefore = screen.getByTestId('root').hasAttribute('data-dirty');
    fireEvent.click(screen.getByText('set two elements'));
    fireEvent.click(screen.getByText('Send'));
    const dirtyAfter = screen.getByTestId('root').hasAttribute('data-dirty');
    report('N2-9-form-submit', {
      submittedWithOneElement: submitted[0],
      submittedWithTwoElements: submitted[1],
      dirtyBefore,
      dirtyAfter,
      consoleErrors: errors,
    });
  });
});

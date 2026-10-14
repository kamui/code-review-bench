// Probe M1: how many times a Field.Control renders for one keystroke.
// Observations only. Each case prints one PROBE line and asserts nothing.
// The counter is the one the pull request's own test uses: a `render` prop on the control that
// adds one each time the control renders. A React.Profiler around Field.Root counts commits of
// the field's subtree as a second measure.
import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { NumberField } from '@base-ui/react/number-field';
import { Checkbox } from '@base-ui/react/checkbox';

function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

interface Counters {
  control: number;
  commits: number;
  validate: unknown[];
}

function counters(): Counters {
  return { control: 0, commits: 0, validate: [] };
}

function type(control: HTMLElement, c: Counters, texts: string[]) {
  return texts.map((text) => {
    const before = { control: c.control, commits: c.commits, validate: c.validate.length };
    fireEvent.change(control, { target: { value: text } });
    return {
      typed: text,
      controlRenders: c.control - before.control,
      fieldCommits: c.commits - before.commits,
      validateCalls: c.validate.length - before.validate,
    };
  });
}

function Counted(props: {
  c: Counters;
  controlled: boolean;
  rootProps?: Record<string, unknown>;
  onBlurTrim?: boolean;
}) {
  const { c, controlled, rootProps } = props;
  const [value, setValue] = React.useState('');
  return (
    <React.Profiler
      id="field"
      onRender={() => {
        c.commits += 1;
      }}
    >
      <Field.Root data-testid="root" name="field" {...rootProps}>
        <Field.Label>Label</Field.Label>
        <Field.Control
          data-testid="control"
          {...(controlled ? { value, onValueChange: setValue } : {})}
          render={(p) => {
            c.control += 1;
            return <input {...p} />;
          }}
        />
        <Field.Error data-testid="error" />
      </Field.Root>
    </React.Profiler>
  );
}

const TEXTS = ['a', 'ab', 'abc'];

describe('M1 probe: renders of Field.Control per keystroke', () => {
  const { render } = createRenderer({ strict: false });

  it('M1-1 controlled, default mode, no validator (the setup of the pull request test)', async () => {
    const c = counters();
    await render(<Counted c={c} controlled />);
    report('M1-1-controlled-default', { keystrokes: type(screen.getByTestId('control'), c, TEXTS) });
  });

  it('M1-2 controlled, validationMode="onChange", validator returns null', async () => {
    const c = counters();
    await render(
      <Counted
        c={c}
        controlled
        rootProps={{
          validationMode: 'onChange',
          validate: (v: unknown) => {
            c.validate.push(v);
            return null;
          },
        }}
      />,
    );
    report('M1-2-controlled-onChange-valid', {
      keystrokes: type(screen.getByTestId('control'), c, TEXTS),
    });
  });

  it('M1-3 controlled, validationMode="onChange", validator returns an error', async () => {
    const c = counters();
    await render(
      <Counted
        c={c}
        controlled
        rootProps={{
          validationMode: 'onChange',
          validate: (v: unknown) => {
            c.validate.push(v);
            return 'Wrong';
          },
        }}
      />,
    );
    report('M1-3-controlled-onChange-invalid', {
      keystrokes: type(screen.getByTestId('control'), c, TEXTS),
    });
  });

  it('M1-4 uncontrolled, validationMode="onChange", validator returns null', async () => {
    const c = counters();
    await render(
      <Counted
        c={c}
        controlled={false}
        rootProps={{
          validationMode: 'onChange',
          validate: (v: unknown) => {
            c.validate.push(v);
            return null;
          },
        }}
      />,
    );
    report('M1-4-uncontrolled-onChange-valid', {
      keystrokes: type(screen.getByTestId('control'), c, TEXTS),
    });
  });

  it('M1-5 uncontrolled, default mode, no validator', async () => {
    const c = counters();
    await render(<Counted c={c} controlled={false} />);
    report('M1-5-uncontrolled-default', {
      keystrokes: type(screen.getByTestId('control'), c, TEXTS),
    });
  });

  it('M1-6 controlled, default mode inside a Form, after a submit attempt', async () => {
    const c = counters();
    await render(
      <Form>
        <Counted
          c={c}
          controlled
          rootProps={{
            validate: (v: unknown) => {
              c.validate.push(v);
              return null;
            },
          }}
        />
        <button type="submit">Send</button>
      </Form>,
    );
    const control = screen.getByTestId('control');
    const beforeSubmit = type(control, c, ['a', 'ab']);
    fireEvent.click(screen.getByText('Send'));
    const afterSubmit = type(control, c, ['abc', 'abcd', 'abcde']);
    report('M1-6-controlled-default-after-submit', { beforeSubmit, afterSubmit });
  });

  it('M1-7 controlled, validationMode="onBlur": a field made invalid by blur, then typing', async () => {
    const c = counters();
    await render(
      <Counted
        c={c}
        controlled
        rootProps={{
          validationMode: 'onBlur',
          validate: (v: unknown) => {
            c.validate.push(v);
            return String(v).length >= 5 ? null : 'Too short';
          },
        }}
      />,
    );
    const control = screen.getByTestId('control');
    const firstTyping = type(control, c, ['a', 'ab']);
    fireEvent.focus(control);
    fireEvent.blur(control);
    const errorAfterBlur = screen.queryByTestId('error')?.textContent ?? null;
    const typingWhileInvalid = type(control, c, ['abc', 'abcd', 'abcde']);
    report('M1-7-controlled-onBlur', {
      firstTyping,
      errorAfterBlur,
      typingWhileInvalid,
      errorAtEnd: screen.queryByTestId('error')?.textContent ?? null,
    });
  });

  it('M1-8 controlled, default mode, Field.Root invalid (the app sets the field invalid)', async () => {
    const c = counters();
    await render(<Counted c={c} controlled rootProps={{ invalid: true }} />);
    report('M1-8-controlled-default-external-invalid', {
      keystrokes: type(screen.getByTestId('control'), c, TEXTS),
    });
  });

  it('M1-9 sibling: controlled NumberField, validationMode="onChange", typing digits', async () => {
    const c = counters();
    function App() {
      const [value, setValue] = React.useState<number | null>(null);
      return (
        <React.Profiler
          id="field"
          onRender={() => {
            c.commits += 1;
          }}
        >
          <Field.Root
            name="field"
            validationMode="onChange"
            validate={(v) => {
              c.validate.push(v);
              return null;
            }}
          >
            <NumberField.Root value={value} onValueChange={setValue}>
              <NumberField.Input
                data-testid="control"
                render={(p) => {
                  c.control += 1;
                  return <input {...p} />;
                }}
              />
            </NumberField.Root>
          </Field.Root>
        </React.Profiler>
      );
    }
    await render(<App />);
    report('M1-9-sibling-NumberField-onChange', {
      keystrokes: type(screen.getByTestId('control'), c, ['1', '12', '123']),
    });
  });

  it('M1-10 sibling: controlled Checkbox, validationMode="onChange", three clicks', async () => {
    const c = counters();
    function App() {
      const [checked, setChecked] = React.useState(false);
      return (
        <React.Profiler
          id="field"
          onRender={() => {
            c.commits += 1;
          }}
        >
          <Field.Root
            name="field"
            validationMode="onChange"
            validate={(v) => {
              c.validate.push(v);
              return null;
            }}
          >
            <Checkbox.Root
              data-testid="control"
              checked={checked}
              onCheckedChange={setChecked}
              render={(p) => {
                c.control += 1;
                return <span {...p} />;
              }}
            />
          </Field.Root>
        </React.Profiler>
      );
    }
    await render(<App />);
    const control = screen.getByTestId('control');
    const clicks = [1, 2, 3].map((n) => {
      const before = { control: c.control, commits: c.commits, validate: c.validate.length };
      fireEvent.click(control);
      return {
        click: n,
        controlRenders: c.control - before.control,
        fieldCommits: c.commits - before.commits,
        validateCalls: c.validate.length - before.validate,
      };
    });
    report('M1-10-sibling-Checkbox-onChange', { clicks });
  });
});

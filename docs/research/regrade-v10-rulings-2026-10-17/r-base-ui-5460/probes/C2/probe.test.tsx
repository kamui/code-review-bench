import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Probe C2: the statements of one comment about what `validate` and `Field.Validity` receive for a
// controlled `value={5}`, checked one by one. Observations only. Each case prints one PROBE line
// and asserts nothing.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function describeValue(v: unknown) {
  return { type: Array.isArray(v) ? 'array' : typeof v, value: v };
}

type Verdict = string | string[] | null;
type Validator = (v: unknown) => Verdict;

// The validator the comment names, `(v) => v > 10`, word for word. It returns a boolean, which the
// prop's declared type does not allow, so it is cast here to reach the code as a JavaScript caller would.
const commentValidator = ((v: any) => v > 10) as unknown as Validator;

// The same loose comparison in the shape the prop documents: null when valid, a message otherwise.
const looseValidator: Validator = (v: any) => (v > 10 ? null : 'Must be more than 10');

// For comparison only, a validator that checks the type: one of the answer key's routes for GT-r4.
const typeCheckValidator: Validator = (v) => (typeof v === 'number' && v > 10 ? null : 'Must be more than 10');

function recorded(calls: Record<string, unknown>[], validator: Validator, phase: () => string) {
  return (v: unknown) => {
    const returned = validator(v);
    calls.push({ phase: phase(), received: describeValue(v), returned: describeValue(returned) });
    return returned;
  };
}

function SubmitCase(props: {
  value: number;
  validator: Validator;
  calls: Record<string, unknown>[];
  submitted: unknown[];
  validity: Record<string, unknown>;
}) {
  const { value: initial, validator, calls, submitted, validity } = props;
  const [value, setValue] = React.useState(initial);
  return (
    <Form onFormSubmit={(values) => submitted.push(values)}>
      <Field.Root data-testid="root" name="amount" validate={recorded(calls, validator, () => 'submit')}>
        <Field.Control
          data-testid="control"
          value={value}
          onValueChange={(next) => setValue(Number(next))}
        />
        <Field.Error data-testid="error" />
        <Field.Validity>
          {(v) => {
            validity.initialValue = describeValue(v.initialValue);
            validity.value = describeValue(v.value);
            return null;
          }}
        </Field.Validity>
      </Field.Root>
      <button type="submit">Send</button>
    </Form>
  );
}

async function submitWith(name: string, value: number, validator: Validator) {
  const calls: Record<string, unknown>[] = [];
  const submitted: unknown[] = [];
  const validity: Record<string, unknown> = {};
  return { calls, submitted, validity, name, value, validator };
}

describe('C2 probe', () => {
  const { render } = createRenderer();

  const cases: [string, number, Validator][] = [
    ['C2-1-comment-validator-value-5', 5, commentValidator],
    ['C2-2-comment-validator-value-15', 15, commentValidator],
    ['C2-3-loose-validator-value-5', 5, looseValidator],
    ['C2-4-loose-validator-value-15', 15, looseValidator],
    ['C2-5-contrast-typecheck-validator-value-15', 15, typeCheckValidator],
  ];

  cases.forEach(([name, value, validator]) => {
    it(`${name}: Form submit with <Field.Control value={${value}} />`, async () => {
      const c = await submitWith(name, value, validator);
      await render(
        <SubmitCase value={value} validator={validator} calls={c.calls} submitted={c.submitted} validity={c.validity} />,
      );
      const fieldValidityAtMount = { ...c.validity };
      fireEvent.click(screen.getByText('Send'));
      report(name, {
        fieldValidityAtMount,
        validateCalls: c.calls,
        rootDataInvalid: screen.getByTestId('root').hasAttribute('data-invalid'),
        errorShown: screen.queryByTestId('error')?.textContent ?? null,
        onFormSubmitCalls: c.submitted,
      });
    });
  });

  it('C2-6 the comment validator and the loose validator through actionsRef.current.validate(), value={5}', async () => {
    const calls: Record<string, unknown>[] = [];
    const commentRef = React.createRef<Field.Root.Actions>();
    const looseRef = React.createRef<Field.Root.Actions>();
    await render(
      <div>
        <Field.Root data-testid="root" name="a" actionsRef={commentRef} validate={recorded(calls, commentValidator, () => 'comment-validator')}>
          <Field.Control value={5} />
        </Field.Root>
        <Field.Root data-testid="root2" name="b" actionsRef={looseRef} validate={recorded(calls, looseValidator, () => 'loose-validator')}>
          <Field.Control value={5} />
          <Field.Error data-testid="error" />
        </Field.Root>
      </div>,
    );
    await act(async () => {
      commentRef.current?.validate();
      looseRef.current?.validate();
    });
    report('C2-6-actionsRef-validate', {
      validateCalls: calls,
      commentFieldDataInvalid: screen.getByTestId('root').hasAttribute('data-invalid'),
      looseFieldDataInvalid: screen.getByTestId('root2').hasAttribute('data-invalid'),
      looseErrorShown: screen.queryByTestId('error')?.textContent ?? null,
    });
  });

  it('C2-7 the change path and the blur path: what validate receives when the person types 6 and leaves', async () => {
    const calls: Record<string, unknown>[] = [];
    let phase = 'typing-6';
    function App(props: { mode: 'onChange' | 'onBlur' }) {
      const [value, setValue] = React.useState(5);
      return (
        <Field.Root name={props.mode} validationMode={props.mode} validate={recorded(calls, looseValidator, () => `${props.mode}:${phase}`)}>
          <Field.Control data-testid={props.mode} value={value} onValueChange={(next) => setValue(Number(next))} />
        </Field.Root>
      );
    }
    await render(
      <div>
        <App mode="onChange" />
        <App mode="onBlur" />
      </div>,
    );
    fireEvent.change(screen.getByTestId('onChange'), { target: { value: '6' } });
    fireEvent.focus(screen.getByTestId('onBlur'));
    fireEvent.change(screen.getByTestId('onBlur'), { target: { value: '6' } });
    phase = 'blur';
    fireEvent.blur(screen.getByTestId('onBlur'));
    report('C2-7-change-and-blur-paths', { validateCalls: calls });
  });
});

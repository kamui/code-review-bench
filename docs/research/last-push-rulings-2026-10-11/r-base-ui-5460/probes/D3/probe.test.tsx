import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

// Observations only. Each case prints one PROBE line and asserts nothing.
// The pull request description as it read at the last push says: "`Field.Control` needs one guard
// the others do not, because it has two paths: the DOM change event and the controlled prop.
// `processedValueRef` records the value the change handler just handled, so the prop echo does not
// validate the same keystroke twice." These cases record how many times, and with what, the
// validator is called for keystrokes on a controlled Field.Control, when the app stores the typed
// text, when it rewrites it, and when it rejects it.

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

type Decide = (typed: string, current: string) => string;

async function run(render: ReturnType<typeof createRenderer>['render'], decide: Decide, typed: string[]) {
  const calls: string[] = [];
  let step = '';
  function App() {
    const [value, setValue] = React.useState('');
    return (
      <Field.Root
        validationMode="onChange"
        validate={(candidate) => {
          calls.push(`${step}: ${JSON.stringify(candidate)}`);
          return null;
        }}
      >
        <Field.Control value={value} onValueChange={(next) => setValue(decide(next, value))} />
      </Field.Root>
    );
  }
  await render(<App />);
  const control = screen.getByRole<HTMLInputElement>('textbox');
  for (const text of typed) {
    step = `typing ${JSON.stringify(text)}`;
    fireEvent.change(control, { target: { value: text } });
  }
  return { inputShows: control.value, validateCalls: calls };
}

describe('D3 probe: validator calls per keystroke on a controlled control', () => {
  const { render } = createRenderer();

  it('D3-1 the app stores the typed text, three keystrokes', async () => {
    report('D3-1-stores-typed-text', await run(render, (typed) => typed, ['a', 'ab', 'abc']));
  });

  it('D3-2 the app rewrites the typed text to upper case, two keystrokes', async () => {
    report('D3-2-rewrites-to-upper-case', await run(render, (typed) => typed.toUpperCase(), ['a', 'Ab']));
  });

  it('D3-3 the app rejects the keystroke', async () => {
    report('D3-3-rejects-the-keystroke', await run(render, (_typed, current) => current, ['a']));
  });
});

import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

// Observations only. Each case prints one PROBE line and asserts nothing.
// The pull request description as it read at the last push says: "`processedValueRef` records
// the value the change handler just handled, so the prop echo does not validate the same
// keystroke twice." This case records how many times, and with what, the validator is called for
// one keystroke on a controlled Field.Control at each commit.

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

describe('B1a description probe: validator calls for one controlled keystroke', () => {
  const { render } = createRenderer();

  it('LP-B1a-1 controlled, validationMode onChange, one ordinary keystroke', async () => {
    const calls: unknown[] = [];
    function App() {
      const [value, setValue] = React.useState('');
      return (
        <Field.Root
          validationMode="onChange"
          validate={(candidate) => {
            calls.push(candidate);
            return null;
          }}
        >
          <Field.Control value={value} onValueChange={setValue} />
        </Field.Root>
      );
    }
    await render(<App />);
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'a' } });
    report('LP-B1a-1-controlled-keystroke-validator-calls', { validateCalls: calls });
  });
});

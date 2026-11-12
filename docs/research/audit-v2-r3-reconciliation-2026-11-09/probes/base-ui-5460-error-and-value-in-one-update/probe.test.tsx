import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';

// Observations only. Each case prints one PROBE line and asserts nothing.
// The review comment's scenario: `onFormSubmit` receives a server response with a normalized
// value and an error and calls `setErrors({ email: 'Already taken' }); setEmail(normalizedEmail)`.
// The comment says the error renders, then the value sync's `clearErrors('email')` removes it
// before paint, so the user never sees it.

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

describe('server error and normalized value set together in onFormSubmit', () => {
  const { render } = createRenderer();

  for (const mode of [undefined, 'onChange', 'onBlur'] as const) {
    const modeName = mode ?? 'default';
    for (const normalize of [true, false]) {
      it(`${modeName} normalize=${normalize}`, async () => {
        function App() {
          const [email, setEmail] = React.useState('Someone@Example.com ');
          const [errors, setErrors] = React.useState<Form.Props['errors']>({});
          return (
            <Form
              errors={errors}
              validationMode={mode}
              onFormSubmit={(values) => {
                setErrors({ email: 'Already taken' });
                if (normalize) {
                  setEmail(String(values.email).trim().toLowerCase());
                }
              }}
            >
              <Field.Root name="email">
                <Field.Control data-testid="control" value={email} onValueChange={setEmail} />
                <Field.Error data-testid="error" />
              </Field.Root>
              <button type="submit" data-testid="submit">
                Submit
              </button>
            </Form>
          );
        }
        await render(<App />);
        await act(async () => {
          fireEvent.click(screen.getByTestId('submit'));
        });
        await act(async () => {});
        report(`submit-error-and-normalized-value mode=${modeName} normalize=${normalize}`, {
          inputShows: (screen.getByTestId('control') as HTMLInputElement).value,
          errorAfter: screen.queryByTestId('error')?.textContent ?? null,
        });
      });
    }
  }
});

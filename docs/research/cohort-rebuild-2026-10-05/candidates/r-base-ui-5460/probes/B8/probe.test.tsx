import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Form } from '@base-ui/react/form';
import { Input } from '@base-ui/react/input';
import { Combobox } from '@base-ui/react/combobox';

// Observations only. Each case prints one PROBE line.
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

interface Country {
  label: string;
  value: string;
}

const COUNTRIES: Country[] = [
  { label: 'France', value: 'fr' },
  { label: 'Germany', value: 'de' },
];

function snapshot(calls: unknown[]) {
  const root = screen.getByTestId('root');
  const input = screen.getByTestId<HTMLInputElement>('input');
  return {
    inputText: input.value,
    validateCalls: [...calls],
    rootInvalid: root.hasAttribute('data-invalid'),
    rootValid: root.hasAttribute('data-valid'),
    errorText: screen.queryByTestId('error')?.textContent ?? null,
  };
}

async function settle() {
  await act(async () => {
    await new Promise((resolve) => {
      setTimeout(resolve, 20);
    });
  });
}

function App(props: {
  calls: unknown[];
  submitted: unknown[];
  fieldAware: boolean;
  validationMode?: 'onChange' | 'onSubmit';
}) {
  const { calls, submitted, fieldAware, validationMode = 'onChange' } = props;
  const [value, setValue] = React.useState<Country | null>(null);
  return (
    <Form onFormSubmit={(values) => submitted.push(values)}>
      <Field.Root
        data-testid="root"
        name="country"
        validationMode={validationMode}
        validate={(v) => {
          calls.push(v);
          // A validator written for the selected item, which is what Combobox validates.
          return v !== null && typeof v === 'object' ? null : 'Pick a country from the list';
        }}
      >
        <Combobox.Root items={COUNTRIES} value={value} onValueChange={setValue}>
          {fieldAware ? (
            <Combobox.Input render={<Input data-testid="input" />} />
          ) : (
            <Combobox.Input data-testid="input" />
          )}
          <Combobox.Portal>
            <Combobox.Positioner>
              <Combobox.Popup>
                <Combobox.List>
                  {(item: Country) => (
                    <Combobox.Item key={item.value} value={item}>
                      {item.label}
                    </Combobox.Item>
                  )}
                </Combobox.List>
              </Combobox.Popup>
            </Combobox.Positioner>
          </Combobox.Portal>
        </Combobox.Root>
        <Field.Error data-testid="error" />
      </Field.Root>
      <button type="button" onClick={() => setValue(COUNTRIES[0])}>
        pick France from code
      </button>
      <button type="submit">Send</button>
    </Form>
  );
}

describe('B8 probe: Combobox.Input rendered as the field-aware Input (which is Field.Control)', () => {
  const { render } = createRenderer();

  it('B8-1 field-aware Input, onChange mode: the app selects France from code, then submit', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<App calls={calls} submitted={submitted} fieldAware />);
    fireEvent.click(screen.getByText('pick France from code'));
    await settle();
    const afterCodeSelect = snapshot(calls);
    fireEvent.click(screen.getByText('Send'));
    await settle();
    report('B8-1-fieldaware-code-select-then-submit', {
      afterCodeSelect,
      afterSubmit: snapshot(calls),
      submitted,
    });
  });

  it('B8-2 reference: plain Combobox.Input, same steps', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<App calls={calls} submitted={submitted} fieldAware={false} />);
    fireEvent.click(screen.getByText('pick France from code'));
    await settle();
    const afterCodeSelect = snapshot(calls);
    fireEvent.click(screen.getByText('Send'));
    await settle();
    report('B8-2-ref-plain-code-select-then-submit', {
      afterCodeSelect,
      afterSubmit: snapshot(calls),
      submitted,
    });
  });

  it('B8-3 field-aware Input, onChange mode: the person picks France with the mouse, then submit', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    const { user } = await render(<App calls={calls} submitted={submitted} fieldAware />);
    await user.click(screen.getByTestId('input'));
    await settle();
    await user.click(screen.getByRole('option', { name: 'France' }));
    await settle();
    const afterUserSelect = snapshot(calls);
    await user.click(screen.getByText('Send'));
    await settle();
    report('B8-3-fieldaware-user-select-then-submit', {
      afterUserSelect,
      afterSubmit: snapshot(calls),
      submitted,
    });
  });

  it('B8-4 field-aware Input, onChange mode: the person types "Fr"', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    await render(<App calls={calls} submitted={submitted} fieldAware />);
    fireEvent.change(screen.getByTestId('input'), { target: { value: 'Fr' } });
    await settle();
    report('B8-4-fieldaware-user-types', { afterTyping: snapshot(calls) });
  });

  it('B8-5 field-aware Input, default (onSubmit) mode: the person picks France, then submit', async () => {
    const calls: unknown[] = [];
    const submitted: unknown[] = [];
    const { user } = await render(
      <App calls={calls} submitted={submitted} fieldAware validationMode="onSubmit" />,
    );
    await user.click(screen.getByTestId('input'));
    await settle();
    await user.click(screen.getByRole('option', { name: 'France' }));
    await settle();
    await user.click(screen.getByText('Send'));
    await settle();
    report('B8-5-fieldaware-default-mode-user-select-then-submit', {
      afterSubmit: snapshot(calls),
      submitted,
    });
  });
});

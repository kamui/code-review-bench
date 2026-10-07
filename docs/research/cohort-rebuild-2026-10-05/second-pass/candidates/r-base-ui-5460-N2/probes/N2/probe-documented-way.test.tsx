import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';
import { Checkbox } from '@base-ui/react/checkbox';
import { CheckboxGroup } from '@base-ui/react/checkbox-group';

// Observations only. The documented ways to do the same thing:
// a list of values through CheckboxGroup, whose `value` prop is documented as `string[]`,
// and a text value through Field.Control with a string.

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

describe('N2 probe: the documented ways', () => {
  const { render } = createRenderer();

  it('DW-1 CheckboxGroup inside Field.Root, arrays set from code', async () => {
    const steps: Record<string, string[]> = {
      'same joined text, different array': ['a', 'b'],
      'different text': ['a'],
      'initial array again, new array': ['a,b'],
    };
    function App() {
      const [value, setValue] = React.useState<string[]>(['a,b']);
      return (
        <Field.Root name="tags" data-testid="root">
          <CheckboxGroup value={value} onValueChange={setValue}>
            <Field.Item>
              <Checkbox.Root value="a" data-testid="box-a" />
            </Field.Item>
            <Field.Item>
              <Checkbox.Root value="b" data-testid="box-b" />
            </Field.Item>
            <Field.Item>
              <Checkbox.Root value="a,b" data-testid="box-ab" />
            </Field.Item>
          </CheckboxGroup>
          {Object.entries(steps).map(([label, next]) => (
            <button key={label} type="button" onClick={() => setValue(next)}>
              {label}
            </button>
          ))}
        </Field.Root>
      );
    }
    await render(<App />);
    const snapshot = (step: string) => ({
      step,
      dirty: screen.getByTestId('root').hasAttribute('data-dirty'),
      ticked: ['box-a', 'box-b', 'box-ab'].filter(
        (id) => screen.getByTestId(id).getAttribute('aria-checked') === 'true',
      ),
    });
    const rows = [snapshot('mount')];
    for (const label of Object.keys(steps)) {
      fireEvent.click(screen.getByText(label));
      rows.push(snapshot(`code sets: ${label}`));
    }
    report('DW-1-checkbox-group-code', { rows });
  });

  it('DW-2 Field.Control with a string value, set from code and typed', async () => {
    function App() {
      const [value, setValue] = React.useState('a,b');
      return (
        <Field.Root name="tags" data-testid="root">
          <Field.Control data-testid="control" value={value} onValueChange={setValue} />
          <button type="button" onClick={() => setValue('c')}>
            set c
          </button>
          <button type="button" onClick={() => setValue('a,b')}>
            set initial
          </button>
        </Field.Root>
      );
    }
    await render(<App />);
    const snapshot = (step: string) => ({
      step,
      dirty: screen.getByTestId('root').hasAttribute('data-dirty'),
      controlShows: (screen.getByTestId('control') as HTMLInputElement).value,
    });
    const rows = [snapshot('mount')];
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a,b,c' } });
    rows.push(snapshot('person types "a,b,c"'));
    fireEvent.change(screen.getByTestId('control'), { target: { value: 'a,b' } });
    rows.push(snapshot('person types back to "a,b"'));
    fireEvent.click(screen.getByText('set c'));
    rows.push(snapshot('code sets "c"'));
    fireEvent.click(screen.getByText('set initial'));
    rows.push(snapshot('code sets "a,b"'));
    report('DW-2-string-control', { rows });
  });
});

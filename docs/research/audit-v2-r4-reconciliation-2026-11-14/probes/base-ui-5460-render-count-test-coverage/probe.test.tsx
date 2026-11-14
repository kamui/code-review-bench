import * as React from 'react';
import { fireEvent, screen } from '@mui/internal-test-utils';
import { createRenderer } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

// Two ways of counting renders in 'avoids rerendering for uncontrolled input changes':
// OLD is the base's wrapper component, NEW is the head's render prop.
// Each case prints one PROBE line with the counts and whether that test's own assertions hold.
describe('probe: render counting, uncontrolled Field.Control', () => {
  const { render: renderNonStrict } = createRenderer({ strict: false });

  it('OLD wrapper count', async () => {
    const renderCountRef = { current: 0 };
    function RenderCountedControl() {
      renderCountRef.current += 1;
      return <Field.Control data-testid="control" />;
    }
    renderNonStrict(
      <Field.Root>
        <RenderCountedControl />
      </Field.Root>,
    );
    const control = screen.getByTestId('control');
    const initial = renderCountRef.current;
    fireEvent.change(control, { target: { value: 'a' } });
    const afterFirst = renderCountRef.current;
    fireEvent.change(control, { target: { value: 'ab' } });
    fireEvent.change(control, { target: { value: 'abc' } });
    const final = renderCountRef.current;
    const passes = final === afterFirst && afterFirst <= initial + 1;
    console.log(`PROBE OLD initial=${initial} afterFirst=${afterFirst} final=${final} assertions=${passes ? 'PASS' : 'FAIL'}`);
  });

  it('NEW render-prop count', async () => {
    const renderCountRef = { current: 0 };
    renderNonStrict(
      <Field.Root>
        <Field.Control
          data-testid="control"
          render={(props) => {
            renderCountRef.current += 1;
            return <input {...props} />;
          }}
        />
      </Field.Root>,
    );
    const control = screen.getByTestId('control');
    const initial = renderCountRef.current;
    fireEvent.change(control, { target: { value: 'a' } });
    const afterFirst = renderCountRef.current;
    fireEvent.change(control, { target: { value: 'ab' } });
    fireEvent.change(control, { target: { value: 'abc' } });
    const final = renderCountRef.current;
    const passes = final === afterFirst && afterFirst <= initial + 1;
    console.log(`PROBE NEW initial=${initial} afterFirst=${afterFirst} final=${final} assertions=${passes ? 'PASS' : 'FAIL'}`);
  });
});

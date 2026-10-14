import * as React from 'react';
import { act, createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

// Observations only. Each case prints one PROBE line and asserts nothing.
// The situation is the comment's own: type="number", a numeric value={5}, and
// onValueChange={next => setValue(Number(next))}; the input's text becomes "05".
function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

function describeValue(v: unknown) {
  return { type: typeof v, value: v };
}

function NumberField(props: {
  type: 'number' | 'text';
  validationMode?: 'onChange' | 'onSubmit' | 'onBlur';
  calls: Record<string, unknown>[];
  stored: unknown[];
  received: unknown[];
}) {
  const [value, setValue] = React.useState(5);
  props.stored.length = 0;
  props.stored.push(describeValue(value));
  return (
    <Field.Root
      data-testid="root"
      name="amount"
      validationMode={props.validationMode}
      validate={(v) => {
        props.calls.push(describeValue(v));
        return null;
      }}
    >
      <Field.Control
        data-testid="control"
        type={props.type}
        value={value}
        onValueChange={(next) => {
          props.received.push(next);
          setValue(Number(next));
        }}
      />
    </Field.Root>
  );
}

function snapshot(calls: Record<string, unknown>[], stored: unknown[]) {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId<HTMLInputElement>('control');
  return {
    nativeInputValue: control.value,
    valueTheAppHolds: stored[0],
    rootDirty: root.hasAttribute('data-dirty'),
    rootFilled: root.hasAttribute('data-filled'),
    validateCalls: [...calls],
  };
}

describe('C3 probe: a controlled number input whose text becomes "05" while the app keeps the number 5', () => {
  const { render } = createRenderer();

  for (const mode of ['onChange', 'onSubmit'] as const) {
    it(`C3-1 type="number", validationMode="${mode}": the text is changed to "05"`, async () => {
      const calls: Record<string, unknown>[] = [];
      const stored: unknown[] = [];
      const received: unknown[] = [];
      await render(
        <NumberField
          type="number"
          validationMode={mode}
          calls={calls}
          stored={stored}
          received={received}
        />,
      );
      const atMount = snapshot(calls, stored);
      fireEvent.change(screen.getByTestId('control'), { target: { value: '05' } });
      const afterEdit = snapshot(calls, stored);
      fireEvent.change(screen.getByTestId('control'), { target: { value: '056' } });
      const afterNextDigit = snapshot(calls, stored);
      report(`C3-1-number-input-05-${mode}`, {
        atMount,
        onValueChangeReceived: received,
        afterTextBecomes05: afterEdit,
        afterTextBecomes056: afterNextDigit,
      });
    });
  }

  it('C3-2 the same field as type="text", validationMode="onChange": the text is changed to "05"', async () => {
    const calls: Record<string, unknown>[] = [];
    const stored: unknown[] = [];
    const received: unknown[] = [];
    await render(
      <NumberField
        type="text"
        validationMode="onChange"
        calls={calls}
        stored={stored}
        received={received}
      />,
    );
    const atMount = snapshot(calls, stored);
    fireEvent.change(screen.getByTestId('control'), { target: { value: '05' } });
    report('C3-2-text-input-05-onChange', {
      atMount,
      onValueChangeReceived: received,
      afterTextBecomes05: snapshot(calls, stored),
    });
  });

  it('C3-3 type="number", validationMode="onBlur": the text is changed to "05", then the field loses focus', async () => {
    const calls: Record<string, unknown>[] = [];
    const stored: unknown[] = [];
    const received: unknown[] = [];
    await render(
      <NumberField
        type="number"
        validationMode="onBlur"
        calls={calls}
        stored={stored}
        received={received}
      />,
    );
    const control = screen.getByTestId('control');
    fireEvent.focus(control);
    fireEvent.change(control, { target: { value: '05' } });
    const afterEdit = snapshot(calls, stored);
    fireEvent.blur(control);
    report('C3-3-number-input-05-onBlur', {
      afterTextBecomes05: afterEdit,
      afterBlur: snapshot(calls, stored),
    });
  });

  it('C3-4 type="number", validationMode="onChange": the text is changed to "05", then Enter is pressed', async () => {
    const calls: Record<string, unknown>[] = [];
    const stored: unknown[] = [];
    const received: unknown[] = [];
    await render(
      <NumberField
        type="number"
        validationMode="onChange"
        calls={calls}
        stored={stored}
        received={received}
      />,
    );
    const control = screen.getByTestId('control');
    await act(async () => {
      control.focus();
    });
    fireEvent.change(control, { target: { value: '05' } });
    const afterEdit = snapshot(calls, stored);
    fireEvent.keyDown(control, { key: 'Enter' });
    report('C3-4-number-input-05-enter', {
      afterTextBecomes05: afterEdit,
      afterEnter: snapshot(calls, stored),
    });
  });
});

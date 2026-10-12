import * as React from 'react';
import { act, createRenderer, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

// Observations only. Each case prints one PROBE line and asserts nothing.
// The pull request description as it read at the last push says: "A control that unmounts or
// becomes disabled while focused no longer latches `data-focused`, since no blur fires in that
// path." These cases record Base UI's focus mark on Field.Root, Field.Label and the control, and
// the browser's own focus state (document.activeElement, :focus, :focus-within), when a focused
// Field.Control is disabled from code, when its Field.Root is disabled, when it unmounts, and,
// as a reference, when focus leaves it the ordinary way.

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

type Change = 'control-disabled' | 'root-disabled' | 'unmount' | 'blur';

async function run(render: ReturnType<typeof createRenderer>['render'], change: Change) {
  let apply: () => void = () => {};
  let blurEvents = 0;

  function App() {
    const [changed, setChanged] = React.useState(false);
    apply = () => setChanged(true);
    const controlDisabled = change === 'control-disabled' && changed;
    const rootDisabled = change === 'root-disabled' && changed;
    const mounted = !(change === 'unmount' && changed);
    return (
      <div>
        <Field.Root data-testid="root" disabled={rootDisabled}>
          <Field.Label data-testid="label">Name</Field.Label>
          {mounted ? (
            <Field.Control
              data-testid="control"
              disabled={controlDisabled}
              onBlur={() => {
                blurEvents += 1;
              }}
            />
          ) : null}
        </Field.Root>
        <button type="button" data-testid="elsewhere">
          elsewhere
        </button>
      </div>
    );
  }

  await render(<App />);
  const root = screen.getByTestId('root');
  const label = screen.getByTestId('label');
  const read = () => {
    const control = screen.queryByTestId('control') as HTMLInputElement | null;
    return {
      baseUiMarkOnRoot: root.hasAttribute('data-focused'),
      baseUiMarkOnLabel: label.hasAttribute('data-focused'),
      baseUiMarkOnControl: control ? control.hasAttribute('data-focused') : null,
      controlMounted: control !== null,
      controlDisabled: control?.disabled ?? null,
      browserActiveElementIsControl: control !== null && document.activeElement === control,
      browserControlMatchesFocus: control ? control.matches(':focus') : null,
      browserRootMatchesFocusWithin: root.matches(':focus-within'),
      blurEvents,
    };
  };
  await act(async () => {
    (screen.getByTestId('control') as HTMLInputElement).focus();
  });
  const afterFocus = read();
  await act(async () => {
    if (change === 'blur') {
      (screen.getByTestId('elsewhere') as HTMLButtonElement).focus();
    } else {
      apply();
    }
  });
  await act(async () => {});
  const afterChange = read();
  return { afterFocus, afterChange };
}

describe('D1 probe: the focus mark when a focused control is disabled or unmounts', () => {
  const { render } = createRenderer();

  it('D1-1 the focused control becomes disabled from code', async () => {
    report('D1-1-control-disabled-while-focused', await run(render, 'control-disabled'));
  });

  it('D1-2 the Field.Root of the focused control becomes disabled from code', async () => {
    report('D1-2-root-disabled-while-focused', await run(render, 'root-disabled'));
  });

  it('D1-3 the focused control unmounts', async () => {
    report('D1-3-unmount-while-focused', await run(render, 'unmount'));
  });

  it('D1-4 reference: focus moves to another element the ordinary way', async () => {
    report('D1-4-reference-ordinary-blur', await run(render, 'blur'));
  });
});

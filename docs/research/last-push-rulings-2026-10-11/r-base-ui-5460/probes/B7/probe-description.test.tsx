import * as React from 'react';
import { act, createRenderer, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

// Observations only. Each case prints one PROBE line and asserts nothing.
// The pull request description as it read at the last push says: "A control that unmounts or
// becomes disabled while focused no longer latches `data-focused`, since no blur fires in that
// path." These cases record the focus mark on Field.Root at each commit when a focused
// Field.Control is disabled from code, when its Field.Root is disabled, and when it unmounts.

function report(name: string, data: unknown) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

type Change = 'control-disabled' | 'root-disabled' | 'unmount';

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
      <Field.Root data-testid="root" disabled={rootDisabled}>
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
    );
  }

  await render(<App />);
  const root = screen.getByTestId('root');
  const read = () => {
    const control = screen.queryByTestId('control') as HTMLInputElement | null;
    return {
      rootFocusedMark: root.hasAttribute('data-focused'),
      controlMounted: control !== null,
      controlDisabled: control?.disabled ?? null,
      controlIsActiveElement: control !== null && document.activeElement === control,
      blurEvents,
    };
  };
  const atMount = read();
  await act(async () => {
    (screen.getByTestId('control') as HTMLInputElement).focus();
  });
  const afterFocus = read();
  await act(async () => {
    apply();
  });
  const afterChange = read();
  return { atMount, afterFocus, afterChange };
}

describe('B7 description probe: the focus mark when a focused control is disabled or unmounts', () => {
  const { render } = createRenderer();

  it('LP-B7-1 the focused control becomes disabled from code', async () => {
    report('LP-B7-1-control-disabled-while-focused', await run(render, 'control-disabled'));
  });

  it('LP-B7-2 the Field.Root of the focused control becomes disabled from code', async () => {
    report('LP-B7-2-root-disabled-while-focused', await run(render, 'root-disabled'));
  });

  it('LP-B7-3 the focused control unmounts', async () => {
    report('LP-B7-3-unmount-while-focused', await run(render, 'unmount'));
  });
});

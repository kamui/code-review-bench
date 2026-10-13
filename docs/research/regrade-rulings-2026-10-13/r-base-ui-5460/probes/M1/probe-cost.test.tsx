// Probe M1, cost: what one controlled keystroke costs in render work, measured two ways.
//  - React.Profiler `actualDuration`, summed over every commit of the field's subtree that the
//    keystroke causes (React's own measure of time spent rendering that subtree);
//  - wall-clock time around the change event, which React flushes synchronously inside it.
// React runs in development mode in this test setup, which is slower than a production build.
// Observations only. Each case prints one PROBE line and asserts nothing.
import * as React from 'react';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

function report(name: string, data: Record<string, unknown>) {
  // eslint-disable-next-line no-console
  console.log(`PROBE ${name} ${JSON.stringify(data)}`);
}

const WARM_UP_ROUNDS = 10;
const ROUNDS = 21;
const KEYSTROKES = 200;

function median(numbers: number[]) {
  const sorted = [...numbers].sort((a, b) => a - b);
  return sorted[Math.floor(sorted.length / 2)];
}

function round3(n: number) {
  return Math.round(n * 1000) / 1000;
}

function App(props: {
  mode: 'onChange' | 'onSubmit';
  extraParts: number;
  stats: { commits: number; duration: number };
}) {
  const { mode, extraParts, stats } = props;
  const [value, setValue] = React.useState('');
  return (
    <React.Profiler
      id="field"
      onRender={(_id, _phase, actualDuration) => {
        stats.commits += 1;
        stats.duration += actualDuration;
      }}
    >
      <Field.Root name="field" validationMode={mode} validate={() => null}>
        <Field.Label>Name</Field.Label>
        <Field.Control data-testid="control" value={value} onValueChange={setValue} />
        <Field.Description>Visible on your profile</Field.Description>
        <Field.Error />
        <Field.Validity>{() => null}</Field.Validity>
        {Array.from({ length: extraParts }, (_, index) => (
          <Field.Validity key={index}>
            {(validity) => <span>{validity.validity.valid ? 'ok' : validity.error}</span>}
          </Field.Validity>
        ))}
      </Field.Root>
    </React.Profiler>
  );
}

async function measure(
  render: ReturnType<typeof createRenderer>['render'],
  mode: 'onChange' | 'onSubmit',
  extraParts = 0,
) {
  const perRound: { commits: number; profilerMs: number; wallMs: number }[] = [];
  for (let round = 0; round < ROUNDS; round += 1) {
    const stats = { commits: 0, duration: 0 };
    // eslint-disable-next-line no-await-in-loop
    const view = await render(<App mode={mode} extraParts={extraParts} stats={stats} />);
    const control = screen.getByTestId('control');
    // The first keystroke also flips dirty and filled; leave it out, as the pull request's test does.
    fireEvent.change(control, { target: { value: 'x' } });
    stats.commits = 0;
    stats.duration = 0;
    let text = 'x';
    const start = performance.now();
    for (let i = 0; i < KEYSTROKES; i += 1) {
      text += 'abcdefghij'[i % 10];
      fireEvent.change(control, { target: { value: text } });
    }
    const wall = performance.now() - start;
    perRound.push({
      commits: stats.commits / KEYSTROKES,
      profilerMs: stats.duration / KEYSTROKES,
      wallMs: wall / KEYSTROKES,
    });
    view.unmount();
  }
  // The first rounds warm the code up (the per-keystroke time falls for several rounds before it
  // settles); report the median of the rest.
  const kept = perRound.slice(WARM_UP_ROUNDS);
  return {
    rounds: kept.length,
    keystrokesPerRound: KEYSTROKES,
    commitsPerKeystroke: median(kept.map((r) => r.commits)),
    profilerMsPerKeystroke_median: round3(median(kept.map((r) => r.profilerMs))),
    profilerMsPerKeystroke_all: kept.map((r) => round3(r.profilerMs)),
    wallMsPerKeystroke_median: round3(median(kept.map((r) => r.wallMs))),
    wallMsPerKeystroke_all: kept.map((r) => round3(r.wallMs)),
  };
}

describe('M1 probe: cost of one controlled keystroke', () => {
  const { render } = createRenderer({ strict: false });

  it('M1-cost-1 controlled, validationMode="onChange", validator returns null', async () => {
    report('M1-cost-1-controlled-onChange', await measure(render, 'onChange'));
  });

  it('M1-cost-2 controlled, default mode (no validation on change before a submit)', async () => {
    report('M1-cost-2-controlled-default', await measure(render, 'onSubmit'));
  });

  it('M1-cost-3 as M1-cost-1, with 50 more parts in the field that read its validity', async () => {
    report('M1-cost-3-controlled-onChange-50-extra-parts', await measure(render, 'onChange', 50));
  });
});

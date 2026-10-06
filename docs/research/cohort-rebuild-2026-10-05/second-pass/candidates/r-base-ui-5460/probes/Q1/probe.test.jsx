import * as React from 'react';
import { renderToString } from 'react-dom/server';
import { hydrateRoot } from 'react-dom/client';
import { createRenderer, fireEvent, screen } from '@mui/internal-test-utils';
import { Field } from '@base-ui/react/field';

function snapshot() {
  const root = screen.getByTestId('root');
  const control = screen.getByTestId('control');
  return {
    value: control.value ?? null,
    dirty: root.hasAttribute('data-dirty'),
    filled: root.hasAttribute('data-filled'),
    invalid: root.hasAttribute('data-invalid'),
    touched: root.hasAttribute('data-touched'),
    ariaInvalid: control.getAttribute('aria-invalid'),
    error: screen.queryByTestId('error')?.textContent ?? null,
  };
}
function report(name, data) { console.log('PROBE ' + name + ' ' + JSON.stringify(data)); }
async function click(text) {
  await React.act(async () => { fireEvent.click(screen.getByText(text)); });
}

describe('Q1 filled at mount and after a DOM-only change', () => {
  const { render } = createRenderer();
  it('family remount to an empty controlled value', async () => {
    function App() {
      const [empty, setEmpty] = React.useState(false);
      return <Field.Root data-testid="root">
        <Field.Control key={String(empty)} data-testid="control" value={empty ? '' : 'abc'} onValueChange={() => {}} />
        <button onClick={() => setEmpty(true)}>Remount empty</button>
      </Field.Root>;
    }
    await render(<App />);
    const before = snapshot();
    await click('Remount empty');
    report('family-remount', { before, after: snapshot() });
  });
  it('family custom render with a nonempty controlled prop', async () => {
    await render(<Field.Root data-testid="root"><Field.Control data-testid="control" value="abc" onValueChange={() => {}} render={<div />} /></Field.Root>);
    report('family-custom-render', snapshot());
  });
  it('uncontrolled DOM-only late fill and rerender', async () => {
    const result = await render(<Field.Root data-testid="root"><Field.Control data-testid="control" /></Field.Root>);
    screen.getByTestId('control').value = 'late';
    await result.rerender(<Field.Root data-testid="root"><Field.Control data-testid="control" /></Field.Root>);
    report('comment-late-dom-fill-no-event', snapshot());
  });
  it('a controlled empty mount replaces a nonempty DOM value', async () => {
    const oldInput = document.createElement('input');
    oldInput.value = 'briefly nonempty';
    const container = document.createElement('div');
    container.append(oldInput);
    document.body.append(container);
    await render(<Field.Root data-testid="root"><Field.Control data-testid="control" value="" onValueChange={() => {}} /></Field.Root>, { container });
    report('client-empty-mount-existing-dom', snapshot());
    container.remove();
  });
  it('hydrates an empty controlled prop over text supplied before hydration', async () => {
    const element = <Field.Root data-testid="root"><Field.Control id="hydration-control" data-testid="control" value="" onValueChange={() => {}} /></Field.Root>;
    const container = document.createElement('div');
    container.innerHTML = renderToString(element);
    document.body.append(container);
    container.querySelector('input').value = 'autofill-like';
    let hydrated;
    try {
      await React.act(async () => { hydrated = hydrateRoot(container, element); });
      report('hydrate-empty-prop-preexisting-text', snapshot());
    } finally {
      await React.act(async () => { hydrated?.unmount(); });
      container.remove();
    }
  });
});

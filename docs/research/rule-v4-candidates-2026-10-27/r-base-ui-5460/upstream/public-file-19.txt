/* eslint-disable testing-library/render-result-naming-convention */
import { createRenderer } from '#test-utils';
import { fireEvent, screen } from '@solidjs/testing-library';
import { createMemo, createRenderEffect, createSignal, flush, onSettled } from 'solid-js';
import type { Accessor } from 'solid-js';
import type { JSX } from '@solidjs/web';
import { expect, describe, it } from 'vitest';
import type { BaseUIEvent } from './types';
import { useRenderElement } from './useRenderElement';

/**
 * Per-key reactivity of `useRenderElement`: every prop tracks only its own sources, so a change to
 * one prop or state value never re-reads, re-applies or re-renders anything that does not use it.
 */
describe('useRenderElement per-key reactivity', () => {
  const { render } = createRenderer();

  it('a render function prop read does not track unrelated props or state', () => {
    const [a, setA] = createSignal('a1');
    const [b, setB] = createSignal('b1');
    const [open, setOpen] = createSignal(false);
    let aReads = 0;

    function Part() {
      const element = useRenderElement(
        'div',
        {
          // The read in its own computation: the compiler groups a template's attributes into
          // one effect, which would count unrelated attribute updates.
          render: (props: Record<string, unknown>) => {
            createRenderEffect(
              () => props['data-a'],
              () => {
                aReads += 1;
              },
            );
            return (
              <span
                data-testid="part"
                data-a={props['data-a'] as string}
                data-b={props['data-b'] as string}
              />
            );
          },
        },
        {
          state: {
            get open() {
              return open();
            },
          },
          props: [
            {
              get 'data-a'() {
                return a();
              },
            },
            {
              get 'data-b'() {
                return b();
              },
            },
          ],
        },
      );
      return <>{element()}</>;
    }

    render(() => <Part />);
    const readsAfterMount = aReads;

    setB('b2');
    flush();
    setOpen(true);
    flush();
    expect(screen.getByTestId('part')).toHaveAttribute('data-b', 'b2');
    expect(aReads).toBe(readsAfterMount);

    setA('a2');
    flush();
    expect(screen.getByTestId('part')).toHaveAttribute('data-a', 'a2');
    expect(aReads).toBe(readsAfterMount + 1);
  });

  it('changing one prop does not re-read the getters of other props', () => {
    const [a, setA] = createSignal('a1');
    const [open, setOpen] = createSignal(false);
    let bReads = 0;

    function Part() {
      const element = useRenderElement(
        'div',
        {},
        {
          state: {
            get open() {
              return open();
            },
          },
          props: [
            {
              'data-testid': 'part',
              get 'data-a'() {
                return a();
              },
              get 'data-b'() {
                bReads += 1;
                return 'b';
              },
            },
          ],
        },
      );
      return <>{element()}</>;
    }

    render(() => <Part />);
    const readsAfterMount = bReads;

    setA('a2');
    flush();
    setOpen(true);
    flush();
    expect(screen.getByTestId('part')).toHaveAttribute('data-a', 'a2');
    expect(screen.getByTestId('part')).toHaveAttribute('data-open', '');
    expect(bReads).toBe(readsAfterMount);
  });

  it('a ref that writes state read by the part props settles in one sync', () => {
    const [registered, setRegistered] = createSignal<Element | null>(null, { ownedWrite: true });
    let refCalls = 0;

    function Part() {
      const element = useRenderElement(
        'div',
        {},
        {
          state: {
            get registered() {
              return registered() != null;
            },
          },
          ref: (el: HTMLElement | null) => {
            refCalls += 1;
            setRegistered(el);
          },
          props: [
            {
              'data-testid': 'part',
              get 'data-has-element'() {
                return registered() ? 'yes' : 'no';
              },
            },
          ],
        },
      );
      return <>{element()}</>;
    }

    render(() => <Part />);
    flush();
    expect(screen.getByTestId('part')).toHaveAttribute('data-has-element', 'yes');
    expect(refCalls).toBe(1);
  });

  it('chains handlers with the later source first and honors preventBaseUIHandler', () => {
    const calls: string[] = [];

    function Part(props: { prevent: boolean }) {
      const element = useRenderElement(
        'button',
        {},
        {
          props: [
            {
              'data-testid': 'part',
              onClick: () => calls.push('part'),
            },
            {
              onClick: (event: BaseUIEvent<MouseEvent>) => {
                calls.push('user');
                if (props.prevent) {
                  event.preventBaseUIHandler();
                }
              },
            },
          ],
        },
      );
      return <>{element()}</>;
    }

    const { unmount } = render(() => <Part prevent={false} />);
    fireEvent.click(screen.getByTestId('part'));
    expect(calls).toEqual(['user', 'part']);
    unmount();

    calls.length = 0;
    render(() => <Part prevent />);
    fireEvent.click(screen.getByTestId('part'));
    expect(calls).toEqual(['user']);
  });

  it('swaps mapped state attributes without leaving the stale one', () => {
    const [open, setOpen] = createSignal(true);

    function Part() {
      const element = useRenderElement(
        'div',
        {},
        {
          state: {
            get open() {
              return open();
            },
          },
          stateAttributesMapping: {
            open: (value: boolean): Record<string, string> =>
              value ? { 'data-open': '' } : { 'data-closed': '' },
          },
          props: [{ 'data-testid': 'part' }],
        },
      );
      return <>{element()}</>;
    }

    render(() => <Part />);
    const part = screen.getByTestId('part');
    expect(part).toHaveAttribute('data-open', '');
    expect(part).not.toHaveAttribute('data-closed');

    setOpen(false);
    flush();
    expect(part).toHaveAttribute('data-closed', '');
    expect(part).not.toHaveAttribute('data-open');
  });

  it('re-evaluates class and style functions only when state changes', () => {
    const [open, setOpen] = createSignal(false);
    const [a, setA] = createSignal('a1');
    let classCalls = 0;
    let styleCalls = 0;

    function Part() {
      const element = useRenderElement(
        'div',
        {
          class: (state: { open: boolean }) => {
            classCalls += 1;
            return state.open ? 'is-open' : 'is-closed';
          },
          style: (state: { open: boolean }) => {
            styleCalls += 1;
            return { color: state.open ? 'red' : 'blue' };
          },
        },
        {
          state: {
            get open() {
              return open();
            },
          },
          props: [
            {
              'data-testid': 'part',
              get 'data-a'() {
                return a();
              },
            },
          ],
        },
      );
      return <>{element()}</>;
    }

    render(() => <Part />);
    const part = screen.getByTestId('part');
    const classAfterMount = classCalls;
    const styleAfterMount = styleCalls;

    setA('a2');
    flush();
    expect(classCalls).toBe(classAfterMount);
    expect(styleCalls).toBe(styleAfterMount);

    setOpen(true);
    flush();
    expect(part).toHaveClass('is-open');
    expect(part.style.color).toBe('red');
    expect(classCalls).toBe(classAfterMount + 1);
    expect(styleCalls).toBe(styleAfterMount + 1);
  });

  it('lets a child of a render function derive the part state from a prop without a cycle', () => {
    // The Field.Control-inside-Combobox.Input shape: the part's state (`filled`) comes from a source
    // the child registers, and that source reads a prop the child receives from the render function.
    const [value, setValue] = createSignal('');
    const [source, setSource] = createSignal<Accessor<boolean> | undefined>(undefined, {
      ownedWrite: true,
    });
    const filled = createMemo(() => source()?.() ?? false);

    function Child(props: { value: string }) {
      onSettled(() => {
        setSource(() => () => props.value !== '');
      });
      return <input data-testid="child" value={props.value} />;
    }

    function Part() {
      const element = useRenderElement(
        'div',
        {
          render: (props: Record<string, unknown>) => <Child value={props.value as string} />,
        },
        {
          state: {
            get filled() {
              return filled();
            },
          },
          props: [
            {
              get value() {
                return value();
              },
            },
          ],
        },
      );
      return <>{element()}</>;
    }

    render(() => <Part />);
    flush();
    expect(filled()).toBe(false);

    setValue('France');
    flush();
    expect(filled()).toBe(true);
    expect(screen.getByTestId('child')).toHaveValue('France');
  });
  it('keeps the children of a render function that spreads its props when state changes', () => {
    const [open, setOpen] = createSignal(false);

    function Part(props: { children?: JSX.Element }) {
      const element = useRenderElement(
        'div',
        {
          get children() {
            return props.children;
          },
          render: (renderProps: Record<string, unknown>) => (
            <div {...renderProps} data-testid="part" />
          ),
        },
        {
          state: {
            get open() {
              return open();
            },
          },
          stateAttributesMapping: {
            open: (value: boolean): Record<string, string> =>
              value ? { 'data-open': '' } : { 'data-closed': '' },
          },
          props: [{ role: 'list' }],
        },
      );
      return <>{element()}</>;
    }

    render(() => (
      <Part>
        <span data-testid="child" />
      </Part>
    ));
    const child = screen.getByTestId('child');

    setOpen(true);
    flush();
    expect(screen.getByTestId('part')).toHaveAttribute('data-open', '');
    expect(screen.getByTestId('child')).toBe(child);
  });
});

import { JSDOM } from 'jsdom';

const dom = new JSDOM('<!doctype html><html><body></body></html>', { url: 'http://localhost/', pretendToBeVisual: true });
const g = globalThis as Record<string, unknown>;
g.window = dom.window;
g.document = dom.window.document;
for (const key of Object.getOwnPropertyNames(dom.window)) {
  if (!(key in g)) {
    try {
      g[key] = (dom.window as unknown as Record<string, unknown>)[key];
    } catch {}
  }
}
g.navigator = dom.window.navigator;
g.IS_REACT_ACT_ENVIRONMENT = true;

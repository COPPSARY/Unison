import "@testing-library/jest-dom/vitest";

// jsdom lacks APIs Radix relies on.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub;
Element.prototype.hasPointerCapture ??= () => false;
Element.prototype.releasePointerCapture ??= () => undefined;
Element.prototype.scrollIntoView ??= () => undefined;

// matchMedia stub; tests flip `mediaMatches` to simulate narrow windows.
export const media = { matches: false };
window.matchMedia ??= (query: string) =>
  ({
    get matches() {
      return media.matches;
    },
    media: query,
    onchange: null,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
    dispatchEvent: () => false,
  }) as MediaQueryList;

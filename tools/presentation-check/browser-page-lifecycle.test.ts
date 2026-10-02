import { Window } from "happy-dom";
import { describe, expect, it, vi } from "vitest";
import { installBrowserPageLifecycle } from "../../web/src/app/browser-page-lifecycle.js";

describe("browser page lifecycle adapter", () => {
  it("handles repeated cache round trips and disposes only on final departure", () => {
    const window = new Window();
    const handlers = { suspend: vi.fn(), restore: vi.fn(), dispose: vi.fn() };
    installBrowserPageLifecycle(window as unknown as globalThis.Window, handlers);
    const dispatch = (type: string, persisted: boolean): void => {
      const event = new window.Event(type);
      Object.defineProperty(event, "persisted", { value: persisted });
      window.dispatchEvent(event);
    };
    dispatch("pageshow", false);
    expect(handlers.restore).not.toHaveBeenCalled();
    for (let roundTrip = 0; roundTrip < 2; roundTrip += 1) {
      dispatch("pagehide", true);
      dispatch("pageshow", true);
    }
    expect(handlers.suspend).toHaveBeenCalledTimes(2);
    expect(handlers.restore).toHaveBeenCalledTimes(2);
    expect(handlers.dispose).not.toHaveBeenCalled();
    dispatch("pagehide", false);
    dispatch("pagehide", false);
    dispatch("pageshow", true);
    expect(handlers.dispose).toHaveBeenCalledTimes(1);
    expect(handlers.restore).toHaveBeenCalledTimes(2);
  });

  it("removes both browser listeners when explicitly detached", () => {
    const window = new Window();
    const handlers = { suspend: vi.fn(), restore: vi.fn(), dispose: vi.fn() };
    const detach = installBrowserPageLifecycle(window as unknown as globalThis.Window, handlers);
    detach();
    window.dispatchEvent(new window.Event("pagehide"));
    window.dispatchEvent(new window.Event("pageshow"));
    expect(handlers.dispose).not.toHaveBeenCalled();
    expect(handlers.restore).not.toHaveBeenCalled();
  });
});

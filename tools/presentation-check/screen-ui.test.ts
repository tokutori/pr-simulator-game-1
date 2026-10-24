import { Window } from "happy-dom";
import { describe, expect, it, vi } from "vitest";
import type { ApplicationInput, GameUiFrame, GameUiView } from "../../web/src/generated/application-contracts.js";
import { bootViewFixture, readyApplicationFixture } from "../game-check/application-fixture.js";

const shared = bootViewFixture();

function adapterView(): GameUiView {
  return { ...shared, interaction_scope: "adapter-inputs", panels: [{
    id: "inputs", title: "描画adapter", anchor: "menu", local_position: [0, 0, 0], size: [2.4, 1.8],
    sections: [{ id: "values", label: "", layout: "column" }], controls: [
      { id: "button", section: "values", label: "操作", enabled: true, content: { kind: "button", presentation: { kind: "action", emphasis: "primary" }, action: { kind: "toggle_technical" } } },
      { id: "toggle", section: "values", label: "表示", enabled: true, content: { kind: "toggle", cue: "telemetry", value: false } },
      { id: "range", section: "values", label: "時刻", enabled: true, content: { kind: "range", minimum: 0, maximum: 10, step: 0.1, value: 1, display_value: "1.0", target: "replay" } },
      { id: "notice", section: "values", label: "", enabled: false, content: { kind: "status", source: "static", value: "発進準備完了" } },
      { id: "fields", section: "values", label: "選択地点", enabled: false, content: { kind: "fields", source: "analysis_cursor",
        fields: [{ id: "speed", label: "対気速度", value: "12.34", unit: "m/s" }] } }
    ]
  }] };
}

function required(root: HTMLElement, selector: string): HTMLElement {
  const element = root.querySelector<HTMLElement>(selector);
  if (element === null) throw new Error(`Missing DOM element ${selector}`);
  return element;
}

async function withAdapter(observe: (root: HTMLElement, adapter: import("../../web/src/presentation/screen-ui.js").ScreenUiAdapter,
  actions: ApplicationInput[], window: Window, measure: (element: Element, width: number, height: number) => void,
  frame: { flush(): void; pending(): number }) => void) {
  const window = new Window();
  const frames = new Map<number, FrameRequestCallback>();
  let nextFrame = 1;
  Object.assign(window, {
    requestAnimationFrame: (callback: FrameRequestCallback) => {
      const id = nextFrame++;
      frames.set(id, callback);
      return id;
    },
    cancelAnimationFrame: (id: number) => { frames.delete(id); }
  });
  let resize: ResizeObserverCallback | undefined;
  Object.assign(window, { ResizeObserver: class {
    constructor(callback: ResizeObserverCallback) { resize = callback; }
    observe = vi.fn();
    unobserve = vi.fn();
    disconnect = vi.fn();
  } });
  Object.assign(globalThis, { window, document: window.document });
  const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
  const documentRef = window.document as unknown as Document;
  const root = documentRef.createElement("main");
  documentRef.body.append(root);
  const actions: ApplicationInput[] = [];
  const adapter = new ScreenUiAdapter(root, input => { actions.push(input); });
  try { observe(root, adapter, actions, window, (element, width, height) => {
    resize?.([{ target: element, contentRect: { width, height } } as ResizeObserverEntry], {} as ResizeObserver);
  }, { pending: () => frames.size, flush: () => {
    const pending = [...frames.values()];
    frames.clear();
    for (const callback of pending) callback(0);
  } }); }
  finally {
    adapter.dispose();
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  }
}

describe("Screen DOM adapter", () => {
  it("renders generated widget payloads and transfers raw button, toggle and range observations", async () => {
    await withAdapter((root, adapter, actions, window) => {
      adapter.render(adapterView());
      expect(required(root, "#notice").textContent).toBe("発進準備完了");
      required(root, "button").click();
      const toggle = required(root, "input[type=checkbox]") as HTMLInputElement;
      toggle.checked = true;
      toggle.dispatchEvent(new window.Event("change", { bubbles: true }) as unknown as Event);
      const range = required(root, "input[type=range]") as HTMLInputElement;
      range.value = "4.2";
      range.dispatchEvent(new window.Event("input", { bubbles: true }) as unknown as Event);
      expect(required(root, "#fields dd").textContent).toBe("12.34m/s");
      expect(actions).toEqual([
        { kind: "control", view_revision: shared.view_revision, input: { type: "activate", controlId: "button" } },
        { kind: "control", view_revision: shared.view_revision, input: { type: "set-toggle", controlId: "toggle", value: true } },
        { kind: "control", view_revision: shared.view_revision, input: { type: "set-range", controlId: "range", value: 4.2 } }
      ]);
    });
  });

  it("updates retained DOM from a frame without mutating its structural View", async () => {
    await withAdapter((root, adapter) => {
      const view = adapterView();
      adapter.render(view);
      const button = required(root, "button");
      const numericValue = required(root, "#fields .screen-ui-field-value");
      const frame: GameUiFrame = { view_revision: view.view_revision, hud: null, chart_cursor: null,
        ranges: [{ id: "range", value: 2.5, display_value: "2.5" }], button_labels: [{ id: "button", label: "一時停止" }], readouts: [{ id: "notice", value: "更新済み" }],
        fields: [{ id: "fields", fields: [{ id: "speed", label: "対気速度", value: "9.87", unit: "m/s" }] }] };
      adapter.renderFrame(frame);
      expect(required(root, "button")).toBe(button);
      expect(button.textContent).toBe("一時停止");
      expect((required(root, "input[type=range]") as HTMLInputElement).value).toBe("2.5");
      expect(required(root, "#notice").textContent).toBe("更新済み");
      expect(required(root, "#fields .screen-ui-field-value")).toBe(numericValue);
      expect(required(root, "#fields dd").textContent).toBe("9.87m/s");
      expect(view.panels[0]?.controls[0]?.label).toBe("操作");
      expect(() => { adapter.renderFrame({ ...frame, view_revision: frame.view_revision + 1 }); }).toThrow("retained structural view");
    });
  });

  it("allocates the shared chart height and transfers measured rectangles and captured pointer lifetimes", async () => {
    const fixture = readyApplicationFixture();
    let view: GameUiView;
    try {
      for (const command of [{ kind: "open_setup" }, { kind: "prepare" }, { kind: "start_countdown" }] as const) {
        fixture.activate({ kind: "session", command });
      }
      for (let now = 0; now <= 3050; now += 10) fixture.observe({ kind: "frame", timestamp_ms: now, gamepads: [],
        display_resource_id: null, spatial: { gamepads: [], sources: [], presented_id: null }, xr: null }, now);
      fixture.activate({ kind: "session", command: { kind: "abort" } }, 3050);
      fixture.activate({ kind: "set_result_tab", tab: "analysis" }, 3050);
      fixture.activate({ kind: "set_analysis_chart", chart: "altitude" }, 3050);
      view = fixture.view;
    } finally { fixture.owner.free(); }
    const control = view.panels.flatMap(panel => panel.controls).find(control => control.content.kind === "chart");
    if (control?.content.kind !== "chart") throw new Error("Missing shared Analysis chart");
    const targetHeight = control.content.plot.presentation.target_height_px;
    await withAdapter((root, adapter, actions, window, measure, frame) => {
      adapter.render(view);
      const viewport = required(root, ".screen-ui-chart-viewport");
      expect(Number.parseFloat(viewport.style.height)).toBeCloseTo(targetHeight, 10);
      const release = vi.fn();
      Object.assign(viewport, { getBoundingClientRect: () => ({ left: 17, top: 29, width: 813, height: 417 }),
        setPointerCapture: vi.fn(), releasePointerCapture: release });
      measure(viewport, 812, 416);
      measure(viewport, 813, 417);
      expect(actions).toEqual([]);
      expect(frame.pending()).toBe(1);
      frame.flush();
      expect(actions.pop()).toEqual({ kind: "chart_measured", view_revision: view.view_revision, control_id: control.id, extent: { width: 813, height: 417 } });
      let reenter = true;
      const notify = vi.spyOn(actions, "push").mockImplementation((...inputs) => {
        const count = Array.prototype.push.apply(actions, inputs);
        if (reenter && inputs.some(input => input.kind === "chart_measured")) {
          reenter = false;
          view = { ...view, view_revision: view.view_revision + 1 };
          adapter.render(view);
          measure(viewport, 913, 517);
        }
        return count;
      });
      measure(viewport, 814, 418);
      frame.flush();
      expect(frame.pending()).toBe(1);
      frame.flush();
      expect(actions.at(-1)).toEqual({ kind: "chart_measured", view_revision: view.view_revision, control_id: control.id, extent: { width: 913, height: 517 } });
      expect(frame.pending()).toBe(0);
      notify.mockRestore();
      actions.length = 0;
      const pointer = (type: string, x: number, buttons: number) => viewport.dispatchEvent(new window.PointerEvent(type, {
        bubbles: true, pointerId: 7, button: 0, buttons, isPrimary: true, clientX: x, clientY: 51
      }) as unknown as PointerEvent);
      pointer("pointerdown", 73, 1);
      pointer("pointermove", 940, 1);
      pointer("pointerup", 940, 0);
      expect(actions.map(input => input.kind === "chart_pointer" ? [input.phase, input.position[0], input.buttons] : input.kind))
        .toEqual([["down", 73, 1], ["move", 940, 1], ["up", 940, 0]]);
      expect(actions[0]).toMatchObject({ kind: "chart_pointer", view_revision: view.view_revision, control_id: control.id,
        pointer_id: 7, is_primary: true, rect: [17, 29, 813, 417] });
      expect(release).toHaveBeenCalledWith(7);
      actions.length = 0;
      pointer("pointerdown", 73, 1);
      adapter.render(view, false);
      expect(actions.at(-1)).toMatchObject({ kind: "chart_pointer", phase: "lost_capture", view_revision: view.view_revision });
      const count = actions.length;
      pointer("pointermove", 940, 1);
      expect(actions).toHaveLength(count);
      adapter.render(view);
      Object.assign(viewport, { setPointerCapture: () => { throw new Error("capture unavailable"); } });
      actions.length = 0;
      pointer("pointerdown", 73, 1);
      expect(actions.map(input => input.kind === "chart_pointer" ? input.phase : input.kind)).toEqual(["down", "lost_capture"]);
      actions.length = 0;
      measure(viewport, 900, 450);
      adapter.render(view, false);
      expect(frame.pending()).toBe(0);
      frame.flush();
      expect(actions).toEqual([]);
      adapter.render(view);
      measure(viewport, 901, 451);
      expect(frame.pending()).toBe(1);
      adapter.dispose();
      expect(frame.pending()).toBe(0);
      frame.flush();
      expect(actions).toEqual([]);
    });
  }, 20000);

  it("retains focus and scroll across a hidden canvas display and resets scroll for a new shared scope", async () => {
    await withAdapter((root, adapter) => {
      const view = adapterView();
      adapter.render(view);
      const button = required(root, "button");
      const shell = required(root, ".screen-ui-shell");
      button.focus();
      shell.scrollTop = 77;
      adapter.render(view, false);
      expect(root.ownerDocument.activeElement).toBe(root.ownerDocument.body);
      const focus = vi.spyOn(button, "focus");
      adapter.render(view, true);
      expect(root.ownerDocument.activeElement).toBe(button);
      expect(shell.scrollTop).toBe(77);
      expect(focus).toHaveBeenCalledExactlyOnceWith({ preventScroll: true });
      adapter.render({ ...view, interaction_scope: "next-view" });
      expect(shell.scrollTop).toBe(0);
    });
  });

  it("releases event listeners and retained DOM independently of later input delivery", async () => {
    await withAdapter((root, adapter, actions) => {
      adapter.render(adapterView());
      const button = required(root, "button");
      adapter.dispose();
      button.click();
      expect(actions).toEqual([]);
      expect(root.childElementCount).toBe(0);
    });
  });
});

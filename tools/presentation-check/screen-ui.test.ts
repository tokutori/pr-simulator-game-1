import { Window } from "happy-dom";
import { describe, expect, it, vi } from "vitest";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { IDENTITY_POSE } from "../../web/src/render/contracts/math.js";
import { normalizedRect } from "../../web/src/render/contracts/ui.js";
import type { UiViewModel } from "../../web/src/render/contracts/ui.js";

describe("screen UI DOM patching", () => {
  it("suppresses hidden events and restores the connected range, focus, scroll, and current values", async () => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    documentRef.body.append(root);
    const actions: unknown[] = [];
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
    const initial = createSceneFixture("FlightSetup");
    adapter.render(initial);
    const mount = required(root, ".screen-ui-mount");
    const shell = required(root, ".screen-ui-shell");
    const range = requiredInput(root, 'input[type="range"]');
    const toggle = requiredInput(root, 'input[type="checkbox"]');
    const button = required(root, "button");
    range.focus();
    shell.scrollTop = 24;
    shell.scrollLeft = 7;
    adapter.render(initial, false);
    expect(mount.hidden).toBe(true);
    expect(mount.hasAttribute("inert")).toBe(true);
    expect(documentRef.activeElement).toBe(documentRef.body);
    const EventConstructor = window.Event as unknown as typeof Event;
    button.dispatchEvent(new EventConstructor("click", { bubbles: true }));
    range.dispatchEvent(new EventConstructor("input", { bubbles: true }));
    toggle.dispatchEvent(new EventConstructor("change", { bubbles: true }));
    expect(actions).toEqual([]);
    shell.scrollTop = 0;
    shell.scrollLeft = 0;
    const updated: UiViewModel = {
      ...initial,
      panels: initial.panels.map((panel) => ({ ...panel, controls: panel.controls.map((control) =>
        control.kind === "range" ? { ...control, value: 0.7 } : control) }))
    };
    adapter.render(updated, false);
    expect(range.value).toBe("0.7");
    expect(required(root, 'input[type="range"]')).toBe(range);
    expect(range.isConnected).toBe(true);
    const focus = vi.spyOn(range, "focus");
    adapter.render(updated, true);
    expect(required(root, ".screen-ui-mount")).toBe(mount);
    expect(required(root, ".screen-ui-shell")).toBe(shell);
    expect(mount.hidden).toBe(false);
    expect(mount.hasAttribute("inert")).toBe(false);
    expect(documentRef.activeElement).toBe(range);
    expect(focus).toHaveBeenCalledExactlyOnceWith({ preventScroll: true });
    expect(shell.scrollTop).toBe(24);
    expect(shell.scrollLeft).toBe(7);
    adapter.render(updated);
    expect(focus).toHaveBeenCalledTimes(1);
    button.dispatchEvent(new EventConstructor("click", { bubbles: true }));
    range.dispatchEvent(new EventConstructor("input", { bubbles: true }));
    toggle.checked = true;
    toggle.dispatchEvent(new EventConstructor("change", { bubbles: true }));
    expect(actions).toEqual([
      { type: "activate", controlId: "flightsetup-action" },
      { type: "set-range", controlId: "flightsetup-range", value: 0.7 },
      { type: "set-toggle", controlId: "flightsetup-toggle", value: true }
    ]);
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  it.each(["external-focus", "disabled", "replaced", "changed-scene"] as const)("does not restore obsolete focus after %s", async (change) => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    const external = documentRef.createElement("button");
    documentRef.body.append(root, external);
    const actions: unknown[] = [];
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
    const initial = createSceneFixture("FlightSetup");
    adapter.render(initial);
    const range = requiredInput(root, 'input[type="range"]');
    range.focus();
    adapter.render(initial, false);
    let updated = initial;
    if (change === "external-focus") external.focus();
    if (change === "changed-scene") updated = createSceneFixture("Result");
    if (change === "disabled") {
      updated = { ...initial, panels: initial.panels.map((panel) => ({
        ...panel, controls: panel.controls.map((control) => ({ ...control, enabled: false }))
      })) };
    }
    if (change === "replaced") adapter.render({ ...initial, panels: [] }, false);
    adapter.render(updated, false);
    adapter.render(updated, true);
    expect(documentRef.activeElement).toBe(change === "external-focus" ? external : documentRef.body);
    if (change === "disabled") {
      const EventConstructor = window.Event as unknown as typeof Event;
      range.dispatchEvent(new EventConstructor("input", { bubbles: true }));
      required(root, 'input[type="checkbox"]').dispatchEvent(new EventConstructor("change", { bubbles: true }));
      required(root, "button").dispatchEvent(new EventConstructor("click", { bubbles: true }));
      expect(actions).toEqual([]);
    }
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  it("preserves range input identity, focus, and scroll across model updates", async () => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    root.style.overflow = "auto";
    window.document.body.append(root as never);
    const actions: unknown[] = [];
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
    const initial = createSceneFixture("FlightSetup");
    const range = initial.panels.flatMap((panel) => panel.controls).find((control) => control.kind === "range");
    if (range?.kind !== "range") throw new Error("FlightSetup fixture has no range control");

    adapter.render(initial);
    const mount = root.querySelector(".screen-ui-mount");
    expect(mount).not.toBeNull();
    const input = root.querySelector<HTMLInputElement>(`input[data-control-id="${range.id}"]`);
    if (input === null) throw new Error(`Rendered range control is missing: ${root.innerHTML}`);
    const shell = root.querySelector<HTMLElement>(".screen-ui-shell");
    if (shell === null) throw new Error("Rendered UI shell is missing");
    input.focus();
    shell.scrollTop = 24;
    input.value = "0.6";
    const EventConstructor = window.Event as unknown as typeof Event;
    input.dispatchEvent(new EventConstructor("input", { bubbles: true }));

    const updated: UiViewModel = {
      ...initial,
      description: "Updated during flight tick",
      panels: initial.panels.map((panel) => ({
        ...panel,
        controls: panel.controls.map((control) => control.kind === "range" && control.id === range.id
          ? { ...control, value: 0.6 }
          : control)
      }))
    };
    adapter.render(updated);

    expect(root.querySelector(".screen-ui-mount")).toBe(mount);
    expect(root.querySelector(`input[data-control-id="${range.id}"]`)).toBe(input);
    expect(window.document.activeElement).toBe(input);
    expect(root.querySelector(".screen-ui-shell")).toBe(shell);
    expect(shell.scrollTop).toBe(24);
    expect(input.value).toBe("0.6");
    expect(actions).toEqual([{ type: "set-range", controlId: range.id, value: 0.6 }]);
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  it.each(["Result", "Replay"] as const)("preserves normalized %s analysis layout and hidden chart updates", async (scene) => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    window.document.body.append(root as never);
    const adapter = new ScreenUiAdapter(root, () => undefined);
    const chart: UiViewModel["panels"][number]["controls"][number] = {
      kind: "chart",
      id: "analysis-chart",
      label: "Altitude",
      xAxisLabel: "Time (s)",
      yAxisLabel: "Altitude (m)",
      xMinimum: 0,
      xMaximum: 1,
      yMinimum: 0,
      yMaximum: 1,
      equalAxisScale: false,
      series: [{ label: "Altitude", color: "#70d6c8", points: [{ x: 0, y: 1 }] }],
      vectors: [],
      markers: [],
      timeMarkers: [],
      referenceLines: [],
      cursorX: null,
      cursorPoints: [],
      enabled: false,
      rect: normalizedRect(0.08, 0.22, 0.84, 0.3)
    };
    const view: UiViewModel = {
      scene,
      title: scene,
      description: "Analysis",
      activeOverlay: null,
      headHud: { kind: "absent" },
      panels: [{
        id: "analysis",
        title: "Analysis",
        anchor: "menu",
        localPose: IDENTITY_POSE,
        size: { width: 2.4, height: 1.8 },
        controls: [
          { kind: "button", id: "map", label: "Map", enabled: true, rect: normalizedRect(0.08, 0.7, 0.25, 0.07) },
          chart
        ]
      }]
    };
    adapter.render(view);

    const shell = root.querySelector<HTMLElement>(".screen-ui-shell");
    const panel = root.querySelector<HTMLElement>(".screen-ui-panel");
    const button = root.querySelector<HTMLElement>('[data-control-id="map"]');
    const figure = root.querySelector<HTMLElement>(".screen-ui-chart");
    const svg = figure?.querySelector("svg");
    expect(shell?.dataset.mode).toBe("analysis");
    expect(panel?.dataset.layout).toBe("normalized");
    expect(svg?.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(svg?.querySelector("path")?.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(svg?.querySelector("text")?.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(button?.style.left).toBe("8%");
    expect(button?.style.top).toBe("70%");
    expect(figure?.style.left).toBe("8%");
    expect(figure?.style.top).toBe("22%");
    expect(figure?.style.height).toBe("30%");
    const updated: UiViewModel = { ...view, panels: view.panels.map((entry) => ({
      ...entry, controls: entry.controls.map((control) => control.kind === "chart" ? { ...control, cursorX: 0.5 } : control)
    })) };
    adapter.render(updated, false);
    expect(required(root, ".screen-ui-mount").hidden).toBe(true);
    expect(root.querySelector("svg")).toBe(svg);
    expect(svg?.isConnected).toBe(true);
    expect(svg?.querySelector('path[stroke-dasharray="7 6"]')?.getAttribute("d")).toBe("M 522 26 V 472");
    adapter.render(updated, true);
    expect(root.querySelector(".screen-ui-chart")).toBe(figure);
    expect(root.querySelector("svg")).toBe(svg);
    expect(required(root, ".screen-ui-mount").hidden).toBe(false);
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  it("patches measured chart pixels once and releases hidden or removed observation targets", async () => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const observers: TestResizeObserver[] = [];
    class TestResizeObserver implements ResizeObserver {
      readonly targets = new Set<Element>();
      constructor(readonly callback: ResizeObserverCallback) { observers.push(this); }
      observe(target: Element): void { this.targets.add(target); }
      unobserve(target: Element): void { this.targets.delete(target); }
      disconnect(): void { this.targets.clear(); }
      emit(target: Element, width: number, height: number): void {
        this.callback([{
          target,
          contentRect: { x: 0, y: 0, width, height, top: 0, left: 0, right: width, bottom: height, toJSON: () => ({ width, height }) },
          borderBoxSize: [], contentBoxSize: [], devicePixelContentBoxSize: []
        }], this);
      }
    }
    Object.defineProperty(window, "ResizeObserver", { value: TestResizeObserver });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    documentRef.body.append(root);
    const actions: unknown[] = [];
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
    const fixture = createSceneFixture("Result");
    const view: UiViewModel = {
      ...fixture,
      panels: fixture.panels.map((panel) => ({ ...panel, controls: [
        ...panel.controls,
        { kind: "status", id: "long-notice", label: "通知", value: "元のfailure原因\n2行目\n3行目\n4行目", enabled: false, rect: normalizedRect(0.04, 0.72, 0.92, 0.085) },
        {
          kind: "chart", id: "measured-chart", label: "Altitude", xAxisLabel: "Time (s)", yAxisLabel: "Altitude (m)",
          xMinimum: 0, xMaximum: 10, yMinimum: 0, yMaximum: 15, equalAxisScale: false,
          series: [{ label: "Altitude", color: "#70d6c8", points: [{ x: 0, y: 15 }, { x: 10, y: 0 }] }],
          vectors: [], markers: [], timeMarkers: [], referenceLines: [], cursorX: 5, cursorPoints: [{ x: 5, y: 7.5 }],
          enabled: false, rect: normalizedRect(0.04, 0.105, 0.92, 0.49)
        }
      ] }))
    };
    adapter.render(view);
    const viewport = required(root, ".screen-ui-chart-viewport");
    const svg = root.querySelector("svg");
    const observer = observers[0];
    if (svg === null || observer === undefined) throw new Error("The measured chart or its observer is missing");
    const render = vi.spyOn(adapter, "render");
    observer.emit(viewport, 298, 160);
    expect(svg.getAttribute("viewBox")).toBe("0 0 298 160");
    expect(svg.style.getPropertyValue("--screen-chart-tick-font")).toBe("14px");
    expect(svg.querySelector('path[stroke-dasharray="7 6"]')?.getAttribute("d")).toBe("M 171 26 V 112");
    expect(svg.querySelector(".screen-ui-chart-axis")?.textContent).toBe("Time (s)");
    const notice = required(root, "#long-notice");
    expect(notice.tabIndex).toBe(0);
    expect(notice.textContent).toBe("通知: 元のfailure原因\n2行目\n3行目\n4行目");
    expect(root.querySelector("svg")).toBe(svg);
    expect(render).toHaveBeenCalledTimes(1);
    observer.emit(viewport, 298, 160);
    expect(render).toHaveBeenCalledTimes(1);
    expect(actions).toEqual([]);
    adapter.render(view, false);
    expect(observer.targets.size).toBe(0);
    observer.emit(viewport, 400, 180);
    expect(svg.getAttribute("viewBox")).toBe("0 0 298 160");
    adapter.render(view, true);
    expect(observer.targets.has(viewport)).toBe(true);
    adapter.render({ ...view, panels: [] });
    expect(observer.targets.size).toBe(0);
    observer.emit(viewport, 400, 180);
    expect(root.querySelector("svg")).toBeNull();
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });
});

function required(root: HTMLElement, selector: string): HTMLElement {
  const element = root.querySelector<HTMLElement>(selector);
  if (element === null) throw new Error(`Missing DOM element: ${selector}`);
  return element;
}

function requiredInput(root: HTMLElement, selector: string): HTMLInputElement {
  const input = root.querySelector<HTMLInputElement>(selector);
  if (input === null) throw new Error(`Missing input: ${selector}`);
  return input;
}

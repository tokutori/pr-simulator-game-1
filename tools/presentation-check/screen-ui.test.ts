import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { IDENTITY_POSE } from "../../web/src/render/contracts/math.js";
import { normalizedRect } from "../../web/src/render/contracts/ui.js";
import type { UiViewModel } from "../../web/src/render/contracts/ui.js";

describe("screen UI DOM patching", () => {
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

  it("applies normalized chart and control positions in the Screen analysis layout", async () => {
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
      scene: "Result",
      title: "Result",
      description: "Analysis",
      activeOverlay: null,
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
    expect(shell?.dataset.mode).toBe("analysis");
    expect(panel?.dataset.layout).toBe("normalized");
    expect(button?.style.left).toBe("8%");
    expect(button?.style.top).toBe("70%");
    expect(figure?.style.left).toBe("8%");
    expect(figure?.style.top).toBe("22%");
    expect(figure?.style.height).toBe("30%");
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });
});

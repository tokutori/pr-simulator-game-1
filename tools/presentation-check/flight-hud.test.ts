import { Window } from "happy-dom";
import { describe, expect, it, vi } from "vitest";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { browserSurfaceContext } from "../../web/src/presentation/browser-canvas.js";
import { paintSurface } from "../../web/src/presentation/surface-canvas.js";
import type { GameUiHud, GameUiHudNode } from "../../web/src/generated/application-contracts.js";
import { flightDrawingFixture } from "../game-check/application-fixture.js";

const flight = flightDrawingFixture();
const hud = flight.drawing.ui.kind === "view" ? flight.drawing.ui.view.hud : flight.drawing.ui.frame.hud;
if (hud == null) throw new Error("Live runtime did not publish its HUD");

function mapNodes(source: GameUiHud, map: (node: GameUiHudNode) => GameUiHudNode): GameUiHud {
  return { ...source, groups: source.groups.map(group => ({ ...group, nodes: group.nodes.map(map) })) };
}

describe("HUD DOM execution", () => {
  it("retains live runtime widgets through hidden frames and directly reflects generated geometry", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const adapter = new FlightHudAdapter(root);
    try {
      adapter.render(hud, flight.view.theme);
      adapter.setVisible(true);
      const nodes = [...root.querySelectorAll("[data-node]")];
      const heading = root.querySelector('[data-instrument="heading_tape"]');
      const ticks = [...heading?.children ?? []];
      adapter.render(null, flight.view.theme);
      expect(root.hidden).toBe(true);
      adapter.render(hud, flight.view.theme);
      expect([...root.querySelectorAll("[data-node]")]).toEqual(nodes);
      expect([...heading?.children ?? []]).toEqual(ticks);
      const changed = mapNodes(hud, node => {
        switch (node.content.kind) {
          case "heading_tape": return { ...node, content: { ...node.content, ticks: [{ segment: [{ x: 110, y: 28 }, { x: 110, y: 40 }], stroke_width: 1.5, label: "CUSTOM", label_anchor: { x: 110, y: 22 } }] } };
          case "direction": return { ...node, content: { ...node.content, arrow: [{ x: 57, y: 36 }, { x: 34, y: 31 }, { x: 38, y: 36 }, { x: 34, y: 41 }] } };
          case "linear_gauge": return { ...node, content: { ...node.content, marker: [{ x: 145, y: 5 }, { x: 139, y: 1 }, { x: 151, y: 1 }], minimum_label: "LOW", maximum_label: "HIGH" } };
          case "attitude": {
            const tick = { segment: [{ x: 30, y: 40 }, { x: 210, y: 60 }] as const, label_anchor: { x: 160, y: 80 }, label_degrees: 20 };
            return { ...node, content: { ...node.content, geometry: { ...node.content.geometry,
              flight_path_center: { x: 120, y: 100 }, horizon: { label_rotation_degrees: 30, pitch_ticks: [tick, tick, tick, tick, tick, tick, tick] }
            } } };
          }
          case "text": return node;
        }
      });
      adapter.render(changed, flight.view.theme);
      expect([...root.querySelectorAll("[data-node]")]).toEqual(nodes);
      expect(root.querySelector('[data-instrument="heading_tape"] text')?.textContent).toBe("CUSTOM");
      expect(root.querySelector('[data-instrument="direction"] polygon')?.getAttribute("points")).toBe("57,36 34,31 38,36 34,41");
      expect(root.querySelector('[data-instrument="linear_gauge"] polygon')?.getAttribute("points")).toBe("145,5 139,1 151,1");
      const attitude = root.querySelector('[data-instrument="attitude"]');
      expect(attitude?.getAttribute("viewBox")).toBe("0 0 240 180");
      expect(attitude?.querySelector("g line")?.getAttribute("x1")).toBe("30");
      expect(attitude?.querySelector("g line")?.getAttribute("x2")).toBe("210");
      expect(attitude?.querySelector("g text")?.getAttribute("transform")).toBe("rotate(30 160 80)");
      expect(attitude?.querySelector("circle")?.getAttribute("cy")).toBe("100");
      const canvas = { beginPath: vi.fn(), roundRect: vi.fn(), stroke: vi.fn(), moveTo: vi.fn(), lineTo: vi.fn(), closePath: vi.fn(), fill: vi.fn() };
      const context = canvas as unknown as CanvasRenderingContext2D;
      paintSurface(browserSurfaceContext(context), [{ kind: "rounded_outline", bounds: { x: 1, y: 2, width: 3, height: 4 },
        radius: 0.5, color: "#fff", widthPx: 2, opacity: 0.7 }], 10, 20);
      expect(canvas.roundRect).toHaveBeenCalledExactlyOnceWith(10, 40, 30, 80, { x: 5, y: 10 });
      expect(canvas.stroke).toHaveBeenCalledOnce();
      expect(context.lineWidth).toBe(2);
      paintSurface(browserSurfaceContext(context), [{ kind: "triangle", points: [{ x: 1, y: 2 }, { x: 3, y: 4 }, { x: 5, y: 6 }], color: "#fff", opacity: 0.7 }], 10, 20);
      expect(canvas.moveTo).toHaveBeenCalledExactlyOnceWith(10, 40);
      expect(canvas.lineTo.mock.calls).toEqual([[30, 80], [50, 120]]);
      expect(canvas.closePath).toHaveBeenCalledOnce();
      expect(canvas.fill).toHaveBeenCalledOnce();
      adapter.setVisible(true);
      expect(root.hidden).toBe(false);
      expect(root.style.width).toBe(`${String(hud.layout.width_px)}px`);
      adapter.render(mapNodes(changed, node => node.content.kind === "direction"
        ? { ...node, content: { ...node.content, arrow: null } } : node), flight.view.theme);
      expect(root.querySelector('[data-instrument="direction"] polygon')).toBeNull();
      expect(root.querySelector('[data-instrument="direction"] rect')).not.toBeNull();
    } finally { await window.happyDOM.abort(); }
  });
});

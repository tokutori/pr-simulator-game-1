import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
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
          case "heading_tape": return { ...node, content: { ...node.content, ticks: [{ position_ratio: 0.5, major: true, label: "CUSTOM" }] } };
          case "direction": return { ...node, content: { ...node.content, rotation_degrees: 90 } };
          case "linear_gauge": return { ...node, content: { ...node.content, position_ratio: 0.75, minimum_label: "LOW", maximum_label: "HIGH" } };
          case "text": case "attitude": return node;
        }
      });
      adapter.render(changed, flight.view.theme);
      expect([...root.querySelectorAll("[data-node]")]).toEqual(nodes);
      expect(root.querySelector('[data-instrument="heading_tape"] text')?.textContent).toBe("CUSTOM");
      expect(root.querySelector('[data-instrument="direction"] g')?.getAttribute("transform")).toBe("rotate(90 36 36)");
      expect(root.querySelector('[data-instrument="linear_gauge"] polygon')?.getAttribute("points")).toBe("145,5 139,1 151,1");
      adapter.setVisible(true);
      expect(root.hidden).toBe(false);
      expect(root.style.width).toBe(`${String(hud.layout.width_px)}px`);
      adapter.render(mapNodes(changed, node => node.content.kind === "direction"
        ? { ...node, content: { ...node.content, rotation_degrees: null } } : node), flight.view.theme);
      expect(root.querySelector('[data-instrument="direction"] g')).toBeNull();
    } finally { await window.happyDOM.abort(); }
  });
});

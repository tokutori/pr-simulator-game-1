import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { currentSharedHudFixture, mapSharedHudNodes, sharedHudNodes } from "../game-check/shared-game-ui-fixture.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";

describe("Flight HUD typed DOM projection", () => {
  it("retains connected keyed widgets in a bottom-anchored rail across hidden frames and value changes", async () => {
    const window = new Window();
    const nativeRoot = window.document.createElement("section");
    window.document.body.append(nativeRoot);
    const root = nativeRoot as unknown as HTMLElement;
    const hud = new FlightHudAdapter(root);
    const model = currentSharedHudFixture({ informationCode: 0 });
    try {
      hud.render(model);
      hud.setVisible(true);
      const nodes = [...root.querySelectorAll("[data-node]")];
      const heading = root.querySelector('[data-instrument="heading_tape"]');
      const ticks = [...heading?.children ?? []];
      hud.render(null);
      expect(root.hidden).toBe(true);
      const next = mapSharedHudNodes(model, (node) => ({ ...node, content: { ...node.content, value: `更新 ${node.content.value}` } }));
      hud.render(next);
      expect(root.hidden).toBe(true);
      expect([...root.querySelectorAll("[data-node]")]).toEqual(nodes);
      expect([...heading?.children ?? []]).toEqual(ticks);
      expect(nodes.every((node) => node.isConnected)).toBe(true);
      expect(root.style.top).toBe("");
      expect(root.style.getPropertyValue("--hud-bottom")).toBe(`${String(model.layout.offset_px[1])}px`);
      expect(root.style.height).toBe(`${String(model.layout.height_px)}px`);
      expect(root.style.maxHeight).toBe("");
      expect([...root.querySelectorAll("output")].map((node) => node.textContent)).toEqual(sharedHudNodes(next).map((node) => node.content.value));
      hud.setVisible(true);
      expect(root.getAttribute("aria-hidden")).toBe("false");
      hud.render({ ...next, layout: { ...next.layout, width_px: 280 }, theme: { ...next.theme, text: "#123456" } });
      expect(root.style.width).toBe("280px");
      expect(root.style.color).toBe("#123456");
    } finally { await window.happyDOM.abort(); }
  });

  it("projects an additional existing-kind node and changed labels without field-specific adapter changes", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const model = currentSharedHudFixture({ informationCode: 0 });
    const first = sharedHudNodes(model)[0];
    if (first === undefined) throw new Error("Missing shared HUD node");
    const addition = { ...first, id: "independent-readout", label: "姿勢 PITCH 参考値", content: { kind: "text" as const, value: "共有値 7", tone: "normal" as const } };
    const groups = model.groups.map((group, index) => ({ ...group, nodes: index === 0 ? [...group.nodes, addition] : group.nodes }));
    const hud = new FlightHudAdapter(root);
    try {
      hud.render({ ...model, groups });
      const ids = [...root.querySelectorAll("[data-node]")].map((node) => node.getAttribute("data-node"));
      expect(ids).toEqual(groups.flatMap((group) => group.nodes.map((node) => node.id)));
      const appended = root.querySelector('[data-node="independent-readout"]');
      expect(appended?.textContent).toBe(`${addition.label} ${addition.content.value}`);
      hud.render({ ...model, groups: groups.map((group) => ({ ...group, nodes: group.nodes.map((node) => ({ ...node, label: `変更 ${node.label}` })) })) });
      expect([...root.querySelectorAll("[data-node]")].map((node) => node.getAttribute("data-node"))).toEqual(ids);
      expect(root.querySelector('[data-node="independent-readout"]')).toBe(appended);
    } finally { await window.happyDOM.abort(); }
  });

  it("projects shared widget geometry, gauge endpoint labels and absent direction without calculating telemetry", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const base = currentSharedHudFixture({ informationCode: 0 });
    const model = mapSharedHudNodes(base, (node) => {
      switch (node.content.kind) {
        case "attitude": return node.content.geometry.kind !== "attitude" ? node : { ...node, content: { ...node.content, geometry: { ...node.content.geometry, horizon: { ...node.content.geometry.horizon, rotation_degrees: -11, pitch_offset_ratio: 0.025 }, flight_path_y_ratio: 0.6 } } };
        case "heading_tape": return { ...node, content: { ...node.content, ticks: [{ position_ratio: 0.5, major: true, label: "CUSTOM" }] } };
        case "direction": return { ...node, content: { ...node.content, rotation_degrees: 90 } };
        case "linear_gauge": return { ...node, content: { ...node.content, position_ratio: 0.75, minimum_label: "LOW", maximum_label: "HIGH" } };
        case "text": return node;
      }
    });
    try {
      const hud = new FlightHudAdapter(root);
      hud.render(model);
      expect(root.querySelector('[data-instrument="attitude"] g > g')?.getAttribute("transform")).toBe("rotate(-11 120 90) translate(0 94.5)");
      expect(root.querySelector('[data-instrument="attitude"] > circle')?.getAttribute("cy")).toBe("108");
      expect(root.querySelector('[data-instrument="heading_tape"] text')?.textContent).toBe("CUSTOM");
      hud.render(mapSharedHudNodes(model, node => node.content.kind === "heading_tape"
        ? { ...node, bounds: { ...node.bounds, width: (110 + model.layout.padding_px[0] * 2) / model.layout.width_px }, instrument_height_em: 2.5 }
        : node));
      const heading = root.querySelector<SVGElement>('[data-instrument="heading_tape"]');
      expect(Number.parseFloat(heading?.style.getPropertyValue("--hud-instrument-font-size") ?? "0")).toBeCloseTo(model.layout.font_size_px * 2, 9);
      expect(root.querySelector('[data-instrument="linear_gauge"] polygon')?.getAttribute("points")).toBe("145,5 139,1 151,1");
      expect(root.querySelector('[data-instrument="linear_gauge"]')?.textContent).toContain("LOWHIGH");
      expect(root.querySelector('[data-instrument="direction"] g')?.getAttribute("transform")).toBe("rotate(90 36 36)");
      hud.render(mapSharedHudNodes(model, (node) => node.content.kind === "direction"
        ? { ...node, content: { ...node.content, rotation_degrees: null } } : node));
      expect(root.querySelector('[data-instrument="direction"] g')).toBeNull();
    } finally { await window.happyDOM.abort(); }
  });

  it("keeps FlightPathOnly geometry visible through its connected ancestors", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    const model = currentSharedHudFixture({ informationCode: 4,
      hudProfile: { telemetry: false, attitude: false, wind: false, flight_path: true, angle_of_attack: false, warnings: false } });
    try {
      const hud = new FlightHudAdapter(root);
      hud.render(model);
      hud.setVisible(true);
      const content = sharedHudNodes(model).find((node) => node.content.kind === "attitude")?.content;
      if (content?.kind !== "attitude" || content.geometry.kind !== "flight_path_only") throw new Error("Missing flight path geometry");
      expect(root.hidden).toBe(false);
      const svg = root.querySelector('[data-instrument="attitude"]');
      expect(svg?.querySelector("g")).toBeNull();
      expect(svg?.querySelector("circle")?.getAttribute("cy")).toBe(String(content.geometry.y_ratio * 180));
    } finally { await window.happyDOM.abort(); }
  });

  it("projects only the shared HUD without appending platform-owned legal panels", async () => {
    const window = new Window();
    const root = window.document.createElement("section") as unknown as HTMLElement;
    try {
      new FlightHudAdapter(root).render(currentSharedHudFixture());
      expect(root.textContent).not.toContain("© OpenStreetMap contributors");
      expect(root.querySelectorAll("[data-node]").length).toBe(sharedHudNodes(currentSharedHudFixture()).length);
      expect(root.querySelector("details")).toBeNull();
      expect(root.textContent).not.toContain("do not incur any liability");
    } finally { await window.happyDOM.abort(); }
  });
});

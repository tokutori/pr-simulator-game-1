import { attributesModule } from "snabbdom/build/modules/attributes.js";
import { h } from "snabbdom/build/h.js";
import { htmlDomApi } from "snabbdom/build/htmldomapi.js";
import { init } from "snabbdom/build/init.js";
import type { VNode } from "snabbdom";
import type { FlightHudModel, FlightHudNode } from "./flight-hud-model.js";

let nextHudId = 0;

export class FlightHudAdapter {
  private readonly patch: ReturnType<typeof init>;
  private tree: VNode | Element;
  private layoutKey = "";
  private readonly clipPrefix = `flight-hud-${String(++nextHudId)}`;

  constructor(private readonly root: HTMLElement) {
    const documentRef = root.ownerDocument;
    this.patch = init([attributesModule], {
      ...htmlDomApi,
      createElement: (name: string, options?: ElementCreationOptions) => documentRef.createElement(name, options),
      createElementNS: (namespace, name, options) => documentRef.createElementNS(namespace, name, options),
      createTextNode: (value) => documentRef.createTextNode(value),
      createComment: (value) => documentRef.createComment(value)
    });
    const mount = documentRef.createElement("div");
    mount.className = "flight-hud-content";
    root.className = "flight-hud";
    root.setAttribute("aria-label", "Flight status");
    root.replaceChildren(mount);
    this.tree = mount;
    this.setVisible(false);
  }

  render(model: FlightHudModel | null): void {
    if (model === null) {
      this.setVisible(false);
      return;
    }
    const layout = model.layout;
    const layoutKey = JSON.stringify([layout, model.theme.text, model.theme.panel_background]);
    if (layoutKey !== this.layoutKey) {
      this.root.dataset.shared = "true";
      Object.assign(this.root.style, {
      width: `${String(layout.width_px)}px`, height: `${String(layout.height_px)}px`,
      fontSize: `${String(layout.font_size_px)}px`, color: model.theme.text
      });
      this.root.style.setProperty("--hud-left", `${String(layout.offset_px[0])}px`);
      this.root.style.setProperty("--hud-bottom", `${String(layout.offset_px[1])}px`);
      this.root.style.setProperty("--hud-card-background", `color-mix(in srgb, ${model.theme.panel_background} ${String(layout.opacity * 100)}%, transparent)`);
      this.root.style.setProperty("--hud-padding-x", `${String(layout.padding_px[0])}px`);
      this.root.style.setProperty("--hud-padding-y", `${String(layout.padding_px[1])}px`);
      this.layoutKey = layoutKey;
    }
    this.tree = this.patch(this.tree, h("div.flight-hud-content", [
      ...model.groups.map((group) => h("section.flight-hud-group", {
        key: group.id,
        attrs: {
          "data-group": group.id, "data-placement": group.placement
        }
      }, group.nodes.map((node) => hudNode(node, model, this.clipPrefix))))
    ]));
  }

  setVisible(visible: boolean): void {
    this.root.hidden = !visible;
    this.root.setAttribute("aria-hidden", String(!visible));
  }
}

function hudNode(node: FlightHudNode, model: FlightHudModel, clipPrefix: string): VNode {
  const content = node.content;
  return h("div.flight-hud-node", {
    key: node.id,
    attrs: { "data-node": node.id, "data-kind": content.kind, "data-tone": content.kind === "text" ? content.tone : "normal",
      style: `left: ${String(node.bounds.x * 100)}%; top: ${String(node.bounds.y * 100)}%; width: ${String(node.bounds.width * 100)}%; height: ${String(node.bounds.height * 100)}%` }
  }, [
    h("div.flight-hud-header", [
      ...(node.label === "" ? [] : [h("span.flight-hud-caption", node.label), " "]),
      h("output.flight-hud-value", { attrs: { "aria-label": node.label || content.value, "aria-live": content.kind === "text" && content.tone === "warning" ? "polite" : "off" } }, content.value)
    ]),
    ...widget(node, model, clipPrefix)
  ]);
}

function widget(node: FlightHudNode, model: FlightHudModel, clipPrefix: string): VNode[] {
  const content = node.content;
  switch (content.kind) {
    case "text": return [];
    case "attitude": {
      const geometry = content.geometry;
      const horizon = geometry.kind === "attitude" ? geometry.horizon : null;
      const flightPath = geometry.kind === "attitude" ? geometry.flight_path_y_ratio : geometry.y_ratio;
      const clipId = `${clipPrefix}-${encodeURIComponent(node.id)}`;
      const shapes: VNode[] = [h("defs", [h("clipPath", { attrs: { id: clipId } }, [shape("rect", { x: 26, y: 18, width: 188, height: 144, rx: 72 })])])];
      if (horizon !== null) shapes.push(h("g", { attrs: { "clip-path": `url(#${clipId})` } }, [
        h("g", { attrs: { transform: `rotate(${String(horizon.rotation_degrees)} 120 90) translate(0 ${String(90 + horizon.pitch_offset_ratio * 180)})` } }, [
          ...horizon.pitch_ticks.flatMap((tick) => {
            const half = tick.half_width_ratio * 240;
            return [shape("path", { d: `M ${String(120 - half)} ${String(tick.offset_ratio * 180)} H ${String(120 + half)}`, stroke: model.theme.text, "stroke-width": 2 }),
              ...(tick.label_degrees === null ? [] : [shape("text", { x: 138 + half, y: tick.offset_ratio * 180 + 4, fill: model.theme.text, "text-anchor": "middle", "font-size": 180 / node.instrument_height_em }, String(tick.label_degrees))])];
          })
        ])
      ]));
      shapes.push(shape("rect", { x: 26, y: 18, width: 188, height: 144, rx: 72, fill: "none", stroke: model.theme.text, "stroke-width": 3 }),
        shape("path", { d: "M 72 91 H 108 L 113 82 H 127 L 132 91 H 168 M 120 86 V 96", fill: "none", stroke: model.theme.focus_outline, "stroke-width": 5 }));
      if (flightPath !== null) shapes.push(shape("circle", { cx: 120, cy: flightPath * 180, r: 5, fill: "none", stroke: "#7df4c5", "stroke-width": 3 }));
      return [gauge(node, "0 0 240 180", shapes)];
    }
    case "heading_tape": return [gauge(node, "0 0 220 50", [
      ...content.ticks.map((tick, index) => h("g", { key: index }, [
        shape("path", { d: `M ${String(tick.position_ratio * 220)} ${tick.major ? "28" : "33"} V 40`, stroke: model.theme.text, "stroke-width": tick.major ? 1.5 : 1 }),
        shape("text", { x: tick.position_ratio * 220, y: 22, "text-anchor": "middle", fill: model.theme.text, "font-size": 50 / node.instrument_height_em }, tick.label)
      ])), shape("path", { d: "M 110 28 L 104 38 H 116 Z", fill: model.theme.focus_outline })
    ])];
    case "direction": return [gauge(node, "0 0 72 72", [
      shape("circle", { cx: 36, cy: 36, r: 27, fill: "none", stroke: model.theme.muted_text, "stroke-width": 1.5 }),
      shape("path", { d: "M 36 9 V 63 M 9 36 H 63", stroke: model.theme.muted_text }),
      shape("text", { x: 36, y: 12, "text-anchor": "middle", fill: model.theme.text, "font-size": 72 / node.instrument_height_em }, "N"),
      ...(content.rotation_degrees === null ? [] : [h("g", { attrs: { transform: `rotate(${String(content.rotation_degrees)} 36 36)` } }, [
        shape("path", { d: "M 36 15 L 31 38 L 36 34 L 41 38 Z", fill: model.theme.focus_outline })
      ])])
    ])];
    case "linear_gauge": return [gauge(node, "0 0 200 42", [
      shape("path", { d: "M 10 16 H 190", stroke: model.theme.muted_text, "stroke-width": 3 }),
      ...Array.from({ length: 7 }, (_, index) => shape("path", { d: `M ${String(10 + index * 30)} 12 V 21`, stroke: model.theme.muted_text })),
      shape("text", { x: 10, y: 39, fill: model.theme.text, "font-size": 42 / node.instrument_height_em }, content.minimum_label),
      shape("text", { x: 190, y: 39, "text-anchor": "end", fill: model.theme.text, "font-size": 42 / node.instrument_height_em }, content.maximum_label),
      ...(content.position_ratio === null ? [] : [shape("polygon", { points: `${String(10 + content.position_ratio * 180)},5 ${String(4 + content.position_ratio * 180)},1 ${String(16 + content.position_ratio * 180)},1`, fill: model.theme.focus_outline })])
    ])];
  }
}

function gauge(node: FlightHudNode, viewBox: string, children: VNode[]): VNode {
  return h("svg", { attrs: { viewBox, role: "img", "aria-label": node.label || node.content.value, "data-instrument": node.content.kind,
    style: `height: ${String(node.instrument_height_em)}em` } }, children);
}

function shape(tag: string, attributes: Record<string, string | number>, text = ""): VNode {
  return h(tag, { attrs: attributes }, text);
}

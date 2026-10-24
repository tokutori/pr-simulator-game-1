import { attributesModule } from "snabbdom/build/modules/attributes.js";
import { h } from "snabbdom/build/h.js";
import { htmlDomApi } from "snabbdom/build/htmldomapi.js";
import { init } from "snabbdom/build/init.js";
import type { VNode } from "snabbdom";
import type { GameUiHud, GameUiHudNode, GameUiTheme } from "../generated/application-contracts.js";

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

  render(hud: GameUiHud | null, theme: GameUiTheme): void {
    if (hud === null) {
      this.setVisible(false);
      return;
    }
    const layout = hud.layout;
    const layoutKey = JSON.stringify([layout, theme.text, theme.panel_background]);
    if (layoutKey !== this.layoutKey) {
      this.root.dataset.shared = "true";
      Object.assign(this.root.style, {
      width: `${String(layout.width_px)}px`, height: `${String(layout.height_px)}px`,
      fontSize: `${String(layout.font_size_px)}px`, color: theme.text
      });
      this.root.style.setProperty("--hud-left", `${String(layout.offset_px[0])}px`);
      this.root.style.setProperty("--hud-bottom", `${String(layout.offset_px[1])}px`);
      this.root.style.setProperty("--hud-card-background", `color-mix(in srgb, ${theme.panel_background} ${String(layout.opacity * 100)}%, transparent)`);
      this.root.style.setProperty("--hud-padding-x", `${String(layout.padding_px[0])}px`);
      this.root.style.setProperty("--hud-padding-y", `${String(layout.padding_px[1])}px`);
      this.layoutKey = layoutKey;
    }
    this.tree = this.patch(this.tree, h("div.flight-hud-content", [
      ...hud.groups.map((group) => h("section.flight-hud-group", {
        key: group.id,
        attrs: {
          "data-group": group.id, "data-placement": group.placement
        }
      }, group.nodes.map((node) => hudNode(node, hud, theme, this.clipPrefix))))
    ]));
  }

  setVisible(visible: boolean): void {
    this.root.hidden = !visible;
    this.root.setAttribute("aria-hidden", String(!visible));
  }
}

function hudNode(node: GameUiHudNode, hud: GameUiHud, theme: GameUiTheme, clipPrefix: string): VNode {
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
    ...widget(node, hud, theme, clipPrefix)
  ]);
}

function widget(node: GameUiHudNode, hud: GameUiHud, theme: GameUiTheme, clipPrefix: string): VNode[] {
  const content = node.content;
  switch (content.kind) {
    case "text": return [];
    case "attitude": {
      const geometry = content.geometry;
      const horizon = geometry.horizon;
      const clipId = `${clipPrefix}-${encodeURIComponent(node.id)}`;
      const clip = geometry.clip;
      const outline = { x: clip.x, y: clip.y, width: clip.width, height: clip.height, rx: geometry.clip_radius };
      const shapes: VNode[] = [h("defs", [h("clipPath", { attrs: { id: clipId } }, [shape("rect", outline)])])];
      if (horizon != null) shapes.push(h("g", { attrs: { "clip-path": `url(#${clipId})` } }, [
        ...horizon.pitch_ticks.flatMap((tick) => [
          ...(tick.segment == null ? [] : [shape("line", { x1: tick.segment[0].x, y1: tick.segment[0].y, x2: tick.segment[1].x, y2: tick.segment[1].y, stroke: theme.text, "stroke-width": 2 })]),
          ...(tick.label_degrees == null || tick.label_anchor == null ? [] : [shape("text", { x: tick.label_anchor.x, y: tick.label_anchor.y, transform: `rotate(${String(horizon.label_rotation_degrees)} ${String(tick.label_anchor.x)} ${String(tick.label_anchor.y)})`, fill: theme.text, "text-anchor": "middle", "font-size": "var(--hud-instrument-font-size)" }, String(tick.label_degrees))])
        ])
      ]));
      const aircraft = geometry.aircraft_segments.map(([start, end], index, segments) => `${index > 0 && segments[index - 1]?.[1].x === start.x && segments[index - 1]?.[1].y === start.y ? "" : `M ${String(start.x)} ${String(start.y)} `}L ${String(end.x)} ${String(end.y)}`).join(" ");
      shapes.push(shape("rect", { ...outline, fill: "none", stroke: theme.text, "stroke-width": 3 }),
        shape("path", { d: aircraft, fill: "none", stroke: theme.focus_outline, "stroke-width": 5 }));
      if (geometry.flight_path_center != null) shapes.push(shape("circle", { cx: geometry.flight_path_center.x, cy: geometry.flight_path_center.y, r: geometry.flight_path_radius, fill: "none", stroke: "#7df4c5", "stroke-width": 3 }));
      return [gauge(node, hud, geometry.canvas_size, shapes)];
    }
    case "heading_tape": return [gauge(node, hud, [220, 50], [
      ...content.ticks.map((tick, index) => h("g", { key: index }, [
        shape("path", { d: `M ${String(tick.position_ratio * 220)} ${tick.major ? "28" : "33"} V 40`, stroke: theme.text, "stroke-width": tick.major ? 1.5 : 1 }),
        shape("text", { x: tick.position_ratio * 220, y: 22, "text-anchor": "middle", fill: theme.text, "font-size": "var(--hud-instrument-font-size)" }, tick.label)
      ])), shape("path", { d: "M 110 28 L 104 38 H 116 Z", fill: theme.focus_outline })
    ])];
    case "direction": return [gauge(node, hud, [72, 72], [
      shape("circle", { cx: 36, cy: 36, r: 27, fill: "none", stroke: theme.muted_text, "stroke-width": 1.5 }),
      shape("path", { d: "M 36 9 V 63 M 9 36 H 63", stroke: theme.muted_text }),
      shape("text", { x: 36, y: 20, "text-anchor": "middle", fill: theme.text, "font-size": "var(--hud-instrument-font-size)" }, "N"),
      ...(content.rotation_degrees === null ? [] : [h("g", { attrs: { transform: `rotate(${String(content.rotation_degrees)} 36 36)` } }, [
        shape("path", { d: "M 36 15 L 31 38 L 36 34 L 41 38 Z", fill: theme.focus_outline })
      ])])
    ])];
    case "linear_gauge": return [gauge(node, hud, [200, 42], [
      shape("path", { d: "M 10 16 H 190", stroke: theme.muted_text, "stroke-width": 3 }),
      ...Array.from({ length: 7 }, (_, index) => shape("path", { d: `M ${String(10 + index * 30)} 12 V 21`, stroke: theme.muted_text })),
      shape("text", { x: 10, y: 39, fill: theme.text, "font-size": "var(--hud-instrument-font-size)" }, content.minimum_label),
      shape("text", { x: 190, y: 39, "text-anchor": "end", fill: theme.text, "font-size": "var(--hud-instrument-font-size)" }, content.maximum_label),
      ...(content.position_ratio == null ? [] : [shape("polygon", { points: `${String(10 + content.position_ratio * 180)},5 ${String(4 + content.position_ratio * 180)},1 ${String(16 + content.position_ratio * 180)},1`, fill: theme.focus_outline })])
    ])];
  }
}

function gauge(node: GameUiHudNode, hud: GameUiHud, [width, height]: readonly [number, number], children: VNode[]): VNode {
  const drawingWidth = node.bounds.width * hud.layout.width_px - hud.layout.padding_px[0] * 2;
  const drawingHeight = node.instrument_height_em * hud.layout.font_size_px;
  const scale = Math.min(drawingWidth / width, drawingHeight / height);
  return h("svg", { attrs: { viewBox: `0 0 ${String(width)} ${String(height)}`, role: "img", "aria-label": node.label || node.content.value, "data-instrument": node.content.kind,
    style: `height: ${String(node.instrument_height_em)}em; --hud-instrument-font-size: ${String(hud.layout.font_size_px / scale)}px` } }, children);
}

function shape(tag: string, attributes: Record<string, string | number>, text = ""): VNode {
  return h(tag, { attrs: attributes }, text);
}

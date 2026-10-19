import { attributesModule } from "snabbdom/build/modules/attributes.js";
import { h } from "snabbdom/build/h.js";
import { htmlDomApi } from "snabbdom/build/htmldomapi.js";
import { init } from "snabbdom/build/init.js";
import type { VNode } from "snabbdom";
import type { FlightHudModel, FlightHudNode } from "./flight-hud-model.js";
import { earcutLicenseUrl, gameUiNumberFormatLicenseUrl, gameUiValidatorLicenseUrl } from "../app/game-ui-codec.js";

let nextHudId = 0;

export class FlightHudAdapter {
  private readonly patch: ReturnType<typeof init>;
  private tree: VNode | Element;
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
    this.root.dataset.shared = "true";
    Object.assign(this.root.style, {
      left: `max(${String(layout.offset_px[0])}px, env(safe-area-inset-left))`,
      top: `max(${String(layout.offset_px[1])}px, env(safe-area-inset-top))`,
      width: `${String(layout.width_px)}px`, maxHeight: `${String(layout.maximum_height_px)}px`,
      padding: `${String(layout.padding_px[1])}px ${String(layout.padding_px[0])}px`,
      fontSize: `${String(layout.font_size_px)}px`, color: model.theme.text,
      background: `color-mix(in srgb, ${model.theme.panel_background} ${String(layout.opacity * 100)}%, transparent)`
    });
    this.tree = this.patch(this.tree, h("div.flight-hud-content", {
      attrs: { style: `gap: ${String(layout.gap_px[1])}px ${String(layout.gap_px[0])}px` }
    }, [
      ...model.groups.map((group) => h("section.flight-hud-group", {
        key: group.id,
        attrs: {
          "data-group": group.id, "data-placement": group.placement, "data-layout": group.layout.kind,
          style: `display: ${group.layout.kind === "row" ? "flex" : "grid"}; grid-template-columns: ${group.layout.kind === "grid" ? `repeat(${String(group.layout.columns)}, minmax(0, 1fr))` : "minmax(0, 1fr)"}`
        }
      }, group.nodes.map((node) => hudNode(node, model, this.clipPrefix)))),
      legalNotices()
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
    attrs: { "data-node": node.id, "data-kind": content.kind, "data-tone": content.kind === "text" ? content.tone : "normal" }
  }, [
    h("span.flight-hud-caption", node.label),
    h("output.flight-hud-value", { attrs: { "aria-label": node.label, "aria-live": content.kind === "text" && content.tone === "warning" ? "polite" : "off" } }, content.value),
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
          shape("rect", { x: -240, y: -240, width: 720, height: 240, fill: "#397d9a" }),
          shape("rect", { x: -240, y: 0, width: 720, height: 240, fill: "#9a7047" }),
          ...[-30, -20, -10, 0, 10, 20, 30].flatMap((pitch) => {
            const half = pitch === 0 ? 94 : Math.abs(pitch) % 20 === 0 ? 27 : 17;
            return [shape("path", { d: `M ${String(120 - half)} ${String(-pitch * 2.2)} H ${String(120 + half)}`, stroke: "white", "stroke-width": 2 }),
              shape("text", { x: 125 + half, y: -pitch * 2.2 + 4, fill: "white", "font-size": 9 }, pitch === 0 ? "" : String(Math.abs(pitch)))];
          })
        ])
      ]));
      shapes.push(shape("rect", { x: 26, y: 18, width: 188, height: 144, rx: 72, fill: "none", stroke: model.theme.text, "stroke-width": 3 }),
        shape("path", { d: "M 72 91 H 108 L 113 82 H 127 L 132 91 H 168 M 120 86 V 96", fill: "none", stroke: model.theme.focus_outline, "stroke-width": 5 }));
      if (flightPath !== null) shapes.push(shape("circle", { cx: 120, cy: flightPath * 180, r: 5, fill: "none", stroke: "#7df4c5", "stroke-width": 3 }));
      return [gauge(node, "0 0 240 180", shapes, { width: `${String(model.layout.attitude_size_px[0])}px`, height: `${String(model.layout.attitude_size_px[1])}px` })];
    }
    case "heading_tape": return [gauge(node, "0 0 220 50", [
      ...content.ticks.map((tick, index) => h("g", { key: index }, [
        shape("path", { d: `M ${String(tick.position_ratio * 220)} ${tick.major ? "28" : "33"} V 40`, stroke: model.theme.text, "stroke-width": tick.major ? 1.5 : 1 }),
        shape("text", { x: tick.position_ratio * 220, y: 22, "text-anchor": "middle", fill: model.theme.text, "font-size": 8 }, tick.label)
      ])), shape("path", { d: "M 110 28 L 104 38 H 116 Z", fill: model.theme.focus_outline })
    ])];
    case "direction": return [gauge(node, "0 0 72 72", [
      shape("circle", { cx: 36, cy: 36, r: 27, fill: "none", stroke: model.theme.muted_text, "stroke-width": 1.5 }),
      shape("path", { d: "M 36 9 V 63 M 9 36 H 63", stroke: model.theme.muted_text }),
      shape("text", { x: 36, y: 8, "text-anchor": "middle", fill: model.theme.text, "font-size": 7 }, "N"),
      ...(content.rotation_degrees === null ? [] : [h("g", { attrs: { transform: `rotate(${String(content.rotation_degrees)} 36 36)` } }, [
        shape("path", { d: "M 36 15 L 31 38 L 36 34 L 41 38 Z", fill: model.theme.focus_outline })
      ])])
    ])];
    case "linear_gauge": return [gauge(node, "0 0 200 42", [
      shape("path", { d: "M 10 16 H 190", stroke: model.theme.muted_text, "stroke-width": 3 }),
      ...Array.from({ length: 7 }, (_, index) => shape("path", { d: `M ${String(10 + index * 30)} 12 V 21`, stroke: model.theme.muted_text })),
      shape("text", { x: 10, y: 39, fill: model.theme.text, "font-size": 9 }, content.minimum_label),
      shape("text", { x: 190, y: 39, "text-anchor": "end", fill: model.theme.text, "font-size": 9 }, content.maximum_label),
      ...(content.position_ratio === null ? [] : [shape("polygon", { points: `${String(10 + content.position_ratio * 180)},5 ${String(4 + content.position_ratio * 180)},1 ${String(16 + content.position_ratio * 180)},1`, fill: model.theme.focus_outline })])
    ])];
  }
}

function gauge(node: FlightHudNode, viewBox: string, children: VNode[], style: Record<string, string> = {}): VNode {
  return h("svg", { attrs: { viewBox, role: "img", "aria-label": node.label, "data-instrument": node.content.kind,
    style: Object.entries(style).map(([name, value]) => `${name}: ${value}`).join("; ") } }, children);
}

function shape(tag: string, attributes: Record<string, string | number>, text = ""): VNode {
  return h(tag, { attrs: attributes }, text);
}

function legalNotices(): VNode {
  return h("footer.flight-hud-attributions", { key: "legal-notices" }, [
    legalLink("https://www.openstreetmap.org/copyright", "© OpenStreetMap contributors · ODbL"),
    legalLink("https://earth.jaxa.jp/en/data/policy/", "地形 AW3D30 (JAXA)"),
    legalLink("https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM",
      "© DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved."),
    h("details.flight-hud-attribution-notice", { attrs: { open: true } }, [
      h("summary", "Copernicus GLO-30 利用条件"),
      h("p", "produced using Copernicus WorldDEM-30."),
      h("p", "The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30.")
    ]),
    legalLink(gameUiValidatorLicenseUrl, "Game UI validator 利用条件"),
    legalLink(gameUiNumberFormatLicenseUrl, "ryu-js 利用条件"),
    legalLink(earcutLicenseUrl, "earcut 利用条件")
  ]);
}

function legalLink(href: string, label: string): VNode {
  return h("a.flight-hud-attribution-link", { attrs: { href, target: "_blank", rel: "noopener noreferrer" } }, label);
}

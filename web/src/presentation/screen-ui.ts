import { actionForControl } from "./panel-interaction.js";
import { chartScaleBarDistance, fitPlotRectToEqualScale, formatChartTick } from "../render/contracts/ui.js";
import type { UiActionDispatcher, UiControl, UiPanel, UiViewModel } from "../render/contracts/ui.js";
import {
  attributesModule,
  classModule,
  datasetModule,
  h,
  init,
  propsModule,
  vnode
} from "snabbdom";
import type { VNode } from "snabbdom";

const patch = init([attributesModule, classModule, datasetModule, propsModule]);

export class ScreenUiAdapter {
  private readonly mount: HTMLElement;
  private currentVNode: VNode;
  private controls = new Map<string, UiControl>();

  constructor(private readonly root: HTMLElement, private readonly dispatch: UiActionDispatcher) {
    this.mount = root.ownerDocument.createElement("div");
    this.mount.className = "screen-ui-mount";
    this.root.append(this.mount);
    this.currentVNode = keyDomTree(this.mount, true);
    this.root.addEventListener("click", this.onClick);
    this.root.addEventListener("change", this.onChange);
    this.root.addEventListener("input", this.onInput);
  }

  render(viewModel: UiViewModel): void {
    const documentRef = this.root.ownerDocument;
    this.controls = new Map(viewModel.panels.flatMap((panel) => panel.controls.map((control) => [control.id, control])));
    const shell = documentRef.createElement("section");
    shell.className = "screen-ui-shell";
    shell.dataset.vnodeKey = "screen-shell";
    shell.dataset.scene = viewModel.scene;
    if (viewModel.panels.some((panel) => panel.controls.some((control) => control.kind === "chart"))) {
      shell.dataset.mode = "analysis";
    }
    if (viewModel.activeOverlay !== null) shell.dataset.overlay = viewModel.activeOverlay;
    const heading = documentRef.createElement("h1");
    heading.textContent = viewModel.title;
    const description = documentRef.createElement("p");
    description.className = "screen-ui-description";
    description.textContent = viewModel.description;
    shell.append(heading, description);
    for (const panel of viewModel.panels) shell.append(this.createPanel(documentRef, panel));
    const nextVNode = h("div.screen-ui-mount", [keyDomTree(shell)]);
    this.currentVNode = patch(this.currentVNode, nextVNode);
  }

  clear(): void {
    this.controls.clear();
    this.currentVNode = patch(this.currentVNode, h("div.screen-ui-mount", []));
  }

  private readonly onClick = (event: Event): void => {
    const target = eventElement(event, this.root);
    const button = target?.closest("button[data-control-id]");
    const controlId = button?.getAttribute("data-control-id");
    if (controlId === null || controlId === undefined) return;
    const control = this.controls.get(controlId);
    if (control?.kind !== "button") return;
    const action = actionForControl(control);
    if (action !== null) this.dispatch(action);
  };

  private readonly onChange = (event: Event): void => {
    const input = eventElement(event, this.root);
    if (input?.tagName !== "INPUT" || input.getAttribute("type") !== "checkbox") return;
    const controlId = input.getAttribute("data-control-id");
    if (controlId === null) return;
    const control = this.controls.get(controlId);
    if (control?.kind !== "toggle") return;
    const checked = (input as HTMLInputElement).checked;
    this.dispatch({ type: "set-toggle", controlId: control.id, value: checked });
  };

  private readonly onInput = (event: Event): void => {
    const input = eventElement(event, this.root);
    if (input?.tagName !== "INPUT" || input.getAttribute("type") !== "range") return;
    const controlId = input.getAttribute("data-control-id");
    if (controlId === null) return;
    const control = this.controls.get(controlId);
    if (control?.kind !== "range") return;
    const value = Number((input as HTMLInputElement).value);
    if (Number.isFinite(value)) this.dispatch({ type: "set-range", controlId: control.id, value });
  };

  private createPanel(documentRef: Document, panel: UiPanel): HTMLElement {
    const section = documentRef.createElement("section");
    section.className = "screen-ui-panel";
    section.dataset.vnodeKey = panel.id;
    section.dataset.anchor = panel.anchor;
    const usesNormalizedLayout = panel.controls.some((control) => control.kind === "chart");
    if (usesNormalizedLayout) section.dataset.layout = "normalized";
    section.setAttribute("aria-label", panel.title);
    const title = documentRef.createElement("h2");
    title.textContent = panel.title;
    const controls = documentRef.createElement("div");
    controls.className = "screen-ui-controls";
    for (const control of panel.controls) {
      const element = this.createControl(documentRef, control);
      if (usesNormalizedLayout) applyNormalizedRect(element, control.rect);
      controls.append(element);
    }
    section.append(title, controls);
    return section;
  }

  private createControl(documentRef: Document, control: UiControl): HTMLElement {
    if (control.kind === "button") {
      const button = documentRef.createElement("button");
      button.type = "button";
      button.dataset.vnodeKey = control.id;
      button.dataset.controlId = control.id;
      if (control.id === "game-result-configuration") button.className = "screen-ui-multiline";
      button.disabled = !control.enabled;
      button.textContent = control.label;
      return button;
    }
    if (control.kind === "toggle") {
      const label = documentRef.createElement("label");
      label.className = "screen-ui-toggle";
      label.dataset.vnodeKey = control.id;
      const input = documentRef.createElement("input");
      input.type = "checkbox";
      input.dataset.vnodeKey = `${control.id}-input`;
      input.dataset.controlId = control.id;
      input.checked = control.value;
      input.disabled = !control.enabled;
      const text = documentRef.createElement("span");
      text.textContent = control.label;
      label.append(input, text);
      return label;
    }
    if (control.kind === "range") {
      const label = documentRef.createElement("label");
      label.className = "screen-ui-range";
      label.dataset.vnodeKey = control.id;
      const text = documentRef.createElement("span");
      text.textContent = `${control.label}: ${control.value.toFixed(1)}`;
      const input = documentRef.createElement("input");
      input.type = "range";
      input.dataset.vnodeKey = `${control.id}-input`;
      input.dataset.controlId = control.id;
      input.min = String(control.minimum);
      input.max = String(control.maximum);
      input.step = String(control.step);
      input.value = String(control.value);
      input.disabled = !control.enabled;
      label.append(text, input);
      return label;
    }
    if (control.kind === "chart") return this.createChart(documentRef, control);
    const output = documentRef.createElement("output");
    output.className = "screen-ui-status";
    output.dataset.vnodeKey = control.id;
    output.setAttribute("aria-live", "polite");
    output.textContent = `${control.label}: ${control.value}`;
    return output;
  }

  private createChart(documentRef: Document, control: Extract<UiControl, { readonly kind: "chart" }>): HTMLElement {
    const figure = documentRef.createElement("figure");
    figure.className = "screen-ui-chart";
    figure.dataset.vnodeKey = control.id;
    const caption = documentRef.createElement("figcaption");
    caption.textContent = control.label;
    const svg = documentRef.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 1000 520");
    svg.setAttribute("role", "img");
    svg.setAttribute("aria-label", `${control.label}; ${control.xAxisLabel}; ${control.yAxisLabel}`);
    svg.setAttribute("preserveAspectRatio", "xMidYMid meet");
    const plot = fitPlotRectToEqualScale(78, 34, 970, 420, control.equalAxisScale);
    const plotTop = plot.top;
    const plotBottom = plot.bottom;
    const plotWidth = plot.right - plot.left;
    const plotLeft = plot.left;
    const plotRight = plot.right;
    const plotX = (value: number): number => plotLeft + (value - control.xMinimum) / (control.xMaximum - control.xMinimum) * plotWidth;
    const plotY = (value: number): number => plotBottom - (value - control.yMinimum) / (control.yMaximum - control.yMinimum) * (plotBottom - plotTop);
    svg.append(createSvgElement(documentRef, "path", {
      d: `M ${String(plotLeft)} ${String(plotTop)} V ${String(plotBottom)} H ${String(plotRight)}`,
      fill: "none",
      stroke: "#9db8b8",
      "stroke-width": "3"
    }));
    for (let index = 1; index <= 3; index += 1) {
      const y = plotTop + ((plotBottom - plotTop) * index) / 4;
      svg.append(createSvgElement(documentRef, "path", {
        d: `M ${String(plotLeft)} ${String(y)} H ${String(plotRight)}`,
        fill: "none",
        stroke: "#456168",
        "stroke-width": "1"
      }));
    }
    for (let index = 0; index <= 4; index += 1) {
      const ratio = index / 4;
      const x = plotLeft + ratio * plotWidth;
      const y = plotBottom - ratio * (plotBottom - plotTop);
      const xTick = createSvgElement(documentRef, "text", {
        x: String(x), y: String(plotBottom + 23), class: "screen-ui-chart-tick"
      });
      xTick.textContent = formatChartTick(control.xMinimum + ratio * (control.xMaximum - control.xMinimum));
      const yTick = createSvgElement(documentRef, "text", {
        x: String(plotLeft - 10), y: String(y + 5), class: "screen-ui-chart-tick screen-ui-chart-y-tick"
      });
      yTick.textContent = formatChartTick(control.yMinimum + ratio * (control.yMaximum - control.yMinimum));
      svg.append(xTick, yTick);
    }
    control.referenceLines.forEach((line) => {
      const y = plotY(line.value);
      svg.append(createSvgElement(documentRef, "path", {
        d: `M ${String(plotLeft)} ${String(y)} H ${String(plotRight)}`,
        fill: "none",
        stroke: line.color,
        "stroke-width": "2",
        "stroke-dasharray": "8 6"
      }));
      const text = createSvgElement(documentRef, "text", { x: String(plotLeft + 8), y: String(y - 7), class: "screen-ui-chart-reference" });
      text.textContent = line.label;
      svg.append(text);
    });
    control.series.forEach((series) => {
      if (series.segmentColors !== undefined && series.points.length > 1) {
        series.segmentColors.forEach((color, index) => {
          const first = series.points[index];
          const second = series.points[index + 1];
          if (first === undefined || second === undefined) return;
          svg.append(createSvgElement(documentRef, "path", {
            d: `M ${String(plotX(first.x))} ${String(plotY(first.y))} L ${String(plotX(second.x))} ${String(plotY(second.y))}`,
            fill: "none",
            stroke: color,
            "stroke-width": "5",
            "stroke-linecap": "round"
          }));
        });
        return;
      }
      const points = series.points.map((point, index) => {
        const x = plotX(point.x);
        const y = plotY(point.y);
        return `${index === 0 ? "M" : "L"} ${String(x)} ${String(y)}`;
      }).join(" ");
      svg.append(createSvgElement(documentRef, "path", {
        d: points,
        fill: "none",
        stroke: series.color,
        "stroke-width": "4",
        "stroke-linejoin": "round",
        "stroke-linecap": "round"
      }));
    });
    if (control.cursorX !== null) {
      const x = plotX(control.cursorX);
      svg.append(createSvgElement(documentRef, "path", {
        d: `M ${String(x)} ${String(plotTop)} V ${String(plotBottom)}`,
        fill: "none",
        stroke: "#f3f4e8",
        "stroke-width": "2",
        "stroke-dasharray": "7 6"
      }));
    }
    control.cursorPoints.forEach((point, index) => {
      const x = plotX(point.x);
      const y = plotY(point.y);
      svg.append(createSvgElement(documentRef, "circle", {
        cx: String(x), cy: String(y), r: "8", fill: control.series[index]?.color ?? "#f3f4e8"
      }));
    });
    control.markers.forEach((marker) => {
      const x = plotX(marker.point.x);
      const y = plotY(marker.point.y);
      svg.append(createSvgElement(documentRef, "circle", {
        cx: String(x), cy: String(y), r: "7", fill: marker.color, stroke: "#10242b", "stroke-width": "2"
      }));
      const text = createSvgElement(documentRef, "text", { x: String(x + 10), y: String(y - 8), class: "screen-ui-chart-marker" });
      text.textContent = marker.label;
      svg.append(text);
    });
    control.timeMarkers.forEach((marker) => {
      const x = plotX(marker.point.x);
      const y = plotY(marker.point.y);
      svg.append(createSvgElement(documentRef, "circle", {
        cx: String(x), cy: String(y), r: "4", fill: marker.color, stroke: "#10242b", "stroke-width": "2"
      }));
      const text = createSvgElement(documentRef, "text", { x: String(x + 7), y: String(y + 18), class: "screen-ui-chart-time-marker" });
      text.textContent = marker.label;
      svg.append(text);
    });
    control.vectors.forEach((vector) => {
      if (vector.start === null || vector.end === null) return;
      const startX = plotX(vector.start.x);
      const startY = plotY(vector.start.y);
      const endX = plotX(vector.end.x);
      const endY = plotY(vector.end.y);
      const angle = Math.atan2(endY - startY, endX - startX);
      const arrowLength = 12;
      svg.append(createSvgElement(documentRef, "path", {
        d: `M ${String(startX)} ${String(startY)} L ${String(endX)} ${String(endY)} M ${String(endX)} ${String(endY)} L ${String(endX - arrowLength * Math.cos(angle - Math.PI / 6))} ${String(endY - arrowLength * Math.sin(angle - Math.PI / 6))} M ${String(endX)} ${String(endY)} L ${String(endX - arrowLength * Math.cos(angle + Math.PI / 6))} ${String(endY - arrowLength * Math.sin(angle + Math.PI / 6))}`,
        fill: "none",
        stroke: vector.color,
        "stroke-width": "4",
        "stroke-linecap": "round"
      }));
    });
    if (control.equalAxisScale) {
      const scaleDistance = chartScaleBarDistance(control.xMaximum - control.xMinimum);
      const scaleWidth = scaleDistance / (control.xMaximum - control.xMinimum) * plotWidth;
      const scaleX = plotRight - scaleWidth - 14;
      const scaleY = plotBottom - 18;
      svg.append(createSvgElement(documentRef, "path", {
        d: `M ${String(scaleX)} ${String(scaleY)} H ${String(scaleX + scaleWidth)} M ${String(scaleX)} ${String(scaleY - 6)} V ${String(scaleY + 3)} M ${String(scaleX + scaleWidth)} ${String(scaleY - 6)} V ${String(scaleY + 3)}`,
        fill: "none", stroke: "#f3f4e8", "stroke-width": "3"
      }));
      const scaleLabel = createSvgElement(documentRef, "text", { x: String(scaleX + scaleWidth / 2), y: String(scaleY - 8), class: "screen-ui-chart-map-label" });
      scaleLabel.textContent = `${formatChartTick(scaleDistance)} m`;
      svg.append(scaleLabel);
      const northX = plotRight - 22;
      const northTop = plotTop + 18;
      const northBottom = northTop + 34;
      svg.append(createSvgElement(documentRef, "path", {
        d: `M ${String(northX)} ${String(northBottom)} V ${String(northTop)} M ${String(northX)} ${String(northTop)} L ${String(northX - 6)} ${String(northTop + 10)} M ${String(northX)} ${String(northTop)} L ${String(northX + 6)} ${String(northTop + 10)}`,
        fill: "none", stroke: "#f3f4e8", "stroke-width": "3"
      }));
      const northLabel = createSvgElement(documentRef, "text", { x: String(northX), y: String(northTop - 6), class: "screen-ui-chart-map-label" });
      northLabel.textContent = "N";
      svg.append(northLabel);
    }
    const xLabel = createSvgElement(documentRef, "text", { x: String((plotLeft + plotRight) / 2), y: "492", class: "screen-ui-chart-axis" });
    xLabel.textContent = control.xAxisLabel;
    const yLabel = createSvgElement(documentRef, "text", { x: "16", y: "230", class: "screen-ui-chart-axis screen-ui-chart-y-axis" });
    yLabel.textContent = control.yAxisLabel;
    svg.append(xLabel, yLabel);
    const legend = documentRef.createElement("div");
    legend.className = "screen-ui-chart-legend";
    control.series.forEach((series) => {
      const item = documentRef.createElement("span");
      item.textContent = series.label;
      item.style.setProperty("--chart-series-color", series.color);
      if (series.segmentColors !== undefined && series.segmentColors.length > 0) {
        item.dataset.progressScale = "true";
        item.style.setProperty("--chart-series-start", series.segmentColors[0] ?? series.color);
        item.style.setProperty("--chart-series-end", series.segmentColors[series.segmentColors.length - 1] ?? series.color);
      }
      legend.append(item);
    });
    control.markers.forEach((marker) => {
      const item = documentRef.createElement("span");
      item.textContent = marker.label;
      item.style.setProperty("--chart-series-color", marker.color);
      legend.append(item);
    });
    control.vectors.forEach((vector) => {
      if (vector.label.length === 0) return;
      const item = documentRef.createElement("span");
      item.textContent = vector.label;
      item.style.setProperty("--chart-series-color", vector.color);
      legend.append(item);
    });
    figure.append(caption, svg, legend);
    return figure;
  }
}

function applyNormalizedRect(element: HTMLElement, rect: UiControl["rect"]): void {
  element.style.position = "absolute";
  element.style.left = `${rect.x * 100}%`;
  element.style.top = `${rect.y * 100}%`;
  element.style.width = `${rect.width * 100}%`;
  element.style.height = `${rect.height * 100}%`;
}

function keyDomTree(element: HTMLElement, attachExisting = false): VNode {
  const data: NonNullable<VNode["data"]> = {};
  const attributes: Record<string, string> = {};
  const dataset: Record<string, string> = {};
  for (const attribute of Array.from(element.attributes)) {
    if (attribute.name.length === 0) continue;
    if (attribute.name === "class") continue;
    if (attribute.name.startsWith("data-")) {
      const key = attribute.name.slice(5).replace(/-([a-z])/g, (_match, letter: string) => letter.toUpperCase());
      dataset[key] = attribute.value;
    } else {
      attributes[attribute.name] = attribute.value;
    }
  }
  if (Object.keys(attributes).length > 0) data.attrs = attributes;
  if (Object.keys(dataset).length > 0) data.dataset = dataset;
  const classes = Array.from(element.classList);
  const key = element.dataset.vnodeKey;
  if (key !== undefined) data.key = key;
  if (element.tagName === "INPUT") {
    const input = element as HTMLInputElement;
    data.props = { ...data.props, checked: input.checked, value: input.value };
  }
  const children = Array.from(element.childNodes).map((child): VNode => {
    if (child.nodeType === 1) return keyDomTree(child as HTMLElement, attachExisting);
    if (child.nodeType === 3) return vnode(undefined, undefined, undefined, child.textContent ?? "", undefined);
    return vnode("!", {}, [], child.textContent ?? "", undefined);
  });
  return vnode(`${element.tagName.toLowerCase()}${classes.map((name) => `.${name}`).join("")}`, data, children, undefined, attachExisting ? element : undefined);
}

function eventElement(event: Event, root: HTMLElement): Element | null {
  const ElementConstructor = root.ownerDocument.defaultView?.Element;
  if (ElementConstructor === undefined || !(event.target instanceof ElementConstructor)) return null;
  return event.target;
}

function createSvgElement(
  documentRef: Document,
  name: string,
  attributes: Readonly<Record<string, string>>
): SVGElement {
  const element = documentRef.createElementNS("http://www.w3.org/2000/svg", name);
  for (const [attribute, value] of Object.entries(attributes)) element.setAttribute(attribute, value);
  return element;
}

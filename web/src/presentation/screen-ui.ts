import { APP_BUILD_LABEL } from "./build-info.js";
import type { GameUiChartExtent } from "../generated/game-ui-input.js";
import type { UiActionDispatcher, UiControl, UiPanel, UiViewModel } from "../render/contracts/ui.js";
import {
  attributesModule,
  classModule,
  datasetModule,
  h,
  init,
  propsModule,
  styleModule
} from "snabbdom";
import type { VNode, VNodeData } from "snabbdom";

const patch = init([attributesModule, classModule, datasetModule, propsModule, styleModule]);

interface RetainedScreenDom {
  readonly reason: "hidden" | "disabled";
  readonly focus: HTMLElement | null;
  readonly shell: HTMLElement;
  readonly scope: string | undefined;
  readonly scrollTop: number;
  readonly scrollLeft: number;
}

export class ScreenUiAdapter {
  private readonly mount: HTMLElement;
  private currentVNode: VNode;
  private controls = new Map<string, UiControl>();
  private visible = true;
  private retainedDom: RetainedScreenDom | null = null;
  private currentViewModel: UiViewModel | null = null;
  private readonly chartViewports = new Map<string, GameUiChartExtent>();
  private readonly observedCharts = new Set<Element>();
  private readonly chartObserver: ResizeObserver | null;

  constructor(private readonly root: HTMLElement, private readonly dispatch: UiActionDispatcher,
    private readonly chartExtentChanged: () => void = () => undefined) {
    this.mount = root.ownerDocument.createElement("div");
    this.mount.className = "screen-ui-mount";
    this.root.append(this.mount);
    this.currentVNode = patch(this.mount, h("div.screen-ui-mount", []));
    this.root.addEventListener("click", this.onClick);
    this.root.addEventListener("change", this.onChange);
    this.root.addEventListener("input", this.onInput);
    const ResizeObserverConstructor = root.ownerDocument.defaultView?.ResizeObserver;
    this.chartObserver = ResizeObserverConstructor === undefined ? null : new ResizeObserverConstructor(this.onChartResize);
  }

  chartExtent(): GameUiChartExtent | null {
    if (!this.visible) return null;
    for (const [id, extent] of this.chartViewports) {
      if (this.controls.get(id)?.kind === "chart") return extent;
    }
    return null;
  }

  render(viewModel: UiViewModel, visible = true): void {
    const scopeChanged = this.currentViewModel !== null && (this.currentViewModel.interactionScope !== viewModel.interactionScope);
    this.currentViewModel = viewModel;
    const wasVisible = this.visible;
    if (wasVisible && !visible) {
      this.retainDom("hidden");
      this.retainedDom?.focus?.blur();
    }
    this.visible = visible;
    this.controls = new Map(viewModel.panels.flatMap((panel) => panel.controls.map((control) => [control.id, control])));
    const active = this.root.ownerDocument.activeElement;
    const controlId = active?.getAttribute("data-control-id");
    if (wasVisible && visible && this.retainedDom === null && active !== null && this.mount.contains(active)
        && controlId !== null && controlId !== undefined && this.controls.get(controlId)?.enabled === false) {
      this.retainDom("disabled");
    }
    const data = viewModel.sharedPresentation === undefined ? {} : sharedPresentationData(viewModel.sharedPresentation);
    const dataset: NonNullable<VNodeData["dataset"]> = { ...data.dataset, scene: viewModel.scene };
    if (viewModel.sharedPresentation === undefined) {
      if (viewModel.panels.some((panel) => panel.controls.some((control) => control.kind === "chart"))) dataset.mode = "analysis";
      else if (viewModel.presentationStyle === "cinematic") dataset.mode = "cinematic";
    }
    dataset.scope = viewModel.interactionScope;
    if (viewModel.activeOverlay !== null) dataset.overlay = viewModel.activeOverlay;
    const shell = h("section.screen-ui-shell", { ...data, key: "screen-shell", dataset }, [
      h("h1", viewModel.title),
      h("p.screen-ui-description", viewModel.description),
      ...viewModel.panels.map((panel) => this.createPanel(panel)),
      h("small.build-version", APP_BUILD_LABEL)
    ]);
    const nextVNode = h("div.screen-ui-mount", {
      attrs: { hidden: !visible, inert: !visible, "aria-hidden": String(!visible) }
    }, [shell]);
    this.currentVNode = patch(this.currentVNode, nextVNode);
    if (scopeChanged) {
      const element = this.mount.querySelector<HTMLElement>(".screen-ui-shell");
      if (element !== null) {
        element.scrollTop = 0;
        element.scrollLeft = 0;
      }
    }
    this.restoreDom();
    this.observeCharts();
  }

  private observeCharts(): void {
    if (this.chartObserver === null) return;
    const targets = this.visible ? new Set(this.mount.querySelectorAll(".screen-ui-chart-viewport")) : new Set<Element>();
    for (const target of this.observedCharts) {
      if (!targets.has(target)) {
        this.chartObserver.unobserve(target);
        this.observedCharts.delete(target);
      }
    }
    for (const target of targets) {
      if (!this.observedCharts.has(target)) {
        this.observedCharts.add(target);
        this.chartObserver.observe(target);
      }
    }
    for (const id of this.chartViewports.keys()) {
      if (this.controls.get(id)?.kind !== "chart") this.chartViewports.delete(id);
    }
  }

  private readonly onChartResize: ResizeObserverCallback = (entries): void => {
    if (!this.visible || this.currentViewModel === null) return;
    let changed = false;
    for (const entry of entries) {
      if (!this.observedCharts.has(entry.target) || !this.mount.contains(entry.target)) continue;
      const id = entry.target.getAttribute("data-chart-control-id");
      const { width, height } = entry.contentRect;
      const control = id === null ? undefined : this.controls.get(id);
      if (id === null || control?.kind !== "chart" || !Number.isFinite(width) || !Number.isFinite(height)
          || width < control.presentation.measurement_minimum.width || height < control.presentation.measurement_minimum.height) continue;
      const previous = this.chartViewports.get(id);
      if (previous !== undefined && Math.abs(previous.width - width) < 0.5 && Math.abs(previous.height - height) < 0.5) continue;
      this.chartViewports.set(id, Object.freeze({ width, height }));
      changed = true;
    }
    if (changed) this.chartExtentChanged();
  };

  private retainDom(reason: RetainedScreenDom["reason"]): void {
    this.discardRetainedDom();
    const shell = this.mount.querySelector<HTMLElement>(".screen-ui-shell");
    if (shell === null) return;
    const active = this.root.ownerDocument.activeElement;
    const ElementConstructor = this.root.ownerDocument.defaultView?.HTMLElement;
    const focus = ElementConstructor !== undefined && active instanceof ElementConstructor && this.mount.contains(active)
      ? active
      : null;
    this.retainedDom = {
      reason, focus, shell, scope: shell.dataset.scope,
      scrollTop: shell.scrollTop, scrollLeft: shell.scrollLeft
    };
    this.root.ownerDocument.addEventListener("focusin", this.discardRetainedDom);
    this.root.ownerDocument.addEventListener("pointerdown", this.discardRetainedDom);
    this.root.ownerDocument.addEventListener("keydown", this.discardRetainedDom);
    this.root.ownerDocument.defaultView?.addEventListener("blur", this.discardRetainedDom);
  }

  private restoreDom(): void {
    const retained = this.retainedDom;
    if (retained === null) return;
    const active = this.root.ownerDocument.activeElement;
    const focus = retained.focus;
    if (!retained.shell.isConnected || !this.mount.contains(retained.shell)
        || retained.shell.dataset.scope !== retained.scope
        || (focus !== null && (!focus.isConnected || !this.mount.contains(focus)))) {
      this.discardRetainedDom();
      return;
    }
    if (!this.visible) return;
    if ((active !== null && active !== this.root.ownerDocument.body && active !== focus)
        || (focus !== null && focus.closest("[hidden], [inert]") !== null)) {
      this.discardRetainedDom();
      return;
    }
    if (retained.reason === "disabled" && focus?.matches(":disabled")) return;
    this.discardRetainedDom();
    if (retained.reason === "hidden") {
      retained.shell.scrollTop = retained.scrollTop;
      retained.shell.scrollLeft = retained.scrollLeft;
    }
    if (focus !== null && !focus.matches(":disabled")) {
      focus.focus({ preventScroll: true });
    }
  }

  private readonly discardRetainedDom = (): void => {
    this.retainedDom = null;
    this.root.ownerDocument.removeEventListener("focusin", this.discardRetainedDom);
    this.root.ownerDocument.removeEventListener("pointerdown", this.discardRetainedDom);
    this.root.ownerDocument.removeEventListener("keydown", this.discardRetainedDom);
    this.root.ownerDocument.defaultView?.removeEventListener("blur", this.discardRetainedDom);
  };

  private readonly onClick = (event: Event): void => {
    if (!this.visible) return;
    const target = eventElement(event, this.root);
    const button = target?.closest("button[data-control-id]");
    const controlId = button?.getAttribute("data-control-id");
    if (controlId === null || controlId === undefined) return;
    const control = this.controls.get(controlId);
    if (control?.kind !== "button" || !control.enabled) return;
    this.dispatch({ type: "activate", controlId: control.id });
  };

  private readonly onChange = (event: Event): void => {
    if (!this.visible) return;
    const input = eventElement(event, this.root);
    if (input?.tagName !== "INPUT" || input.getAttribute("type") !== "checkbox") return;
    const controlId = input.getAttribute("data-control-id");
    if (controlId === null) return;
    const control = this.controls.get(controlId);
    if (control?.kind !== "toggle" || !control.enabled) return;
    const checked = (input as HTMLInputElement).checked;
    this.dispatch({ type: "set-toggle", controlId: control.id, value: checked });
  };

  private readonly onInput = (event: Event): void => {
    if (!this.visible) return;
    const input = eventElement(event, this.root);
    if (input?.tagName !== "INPUT" || input.getAttribute("type") !== "range") return;
    const controlId = input.getAttribute("data-control-id");
    if (controlId === null) return;
    const control = this.controls.get(controlId);
    if (control?.kind !== "range" || !control.enabled) return;
    const value = Number((input as HTMLInputElement).value);
    if (Number.isFinite(value)) this.dispatch({ type: "set-range", controlId: control.id, value });
  };

  private createPanel(panel: UiPanel): VNode {
    const data: VNodeData = { key: panel.id, dataset: { anchor: panel.anchor }, attrs: { "aria-label": panel.title } };
    const children: VNode[] = [];
    if (panel.sections !== undefined) {
      for (const section of panel.sections) {
        const content = h("div.screen-ui-section-controls", panel.controls.filter((control) => control.section === section.id).map((control) => this.createControl(control)));
        children.push(h(section.label.length > 0 ? "fieldset.screen-ui-section" : "div.screen-ui-section", {
          key: `${panel.id}-section-${section.id}`, dataset: { sectionId: section.id, layout: section.layout }
        }, section.label.length > 0 ? [h("legend", section.label), content] : [content]));
      }
    } else {
      let choices: VNode[] | null = null;
      let previousChoiceGroup = "";
      for (const control of panel.controls) {
        const node = this.createControl(control);
        if (control.kind === "button" && control.presentation?.kind === "choice") {
          if (choices === null || previousChoiceGroup !== control.presentation.group) {
            choices = [];
            children.push(h("fieldset.screen-ui-choice-group", { key: `${panel.id}-choices-${control.presentation.group}` }, [
              h("legend", control.presentation.groupLabel), h("div.screen-ui-choice-options", choices)
            ]));
            previousChoiceGroup = control.presentation.group;
          }
          choices.push(node);
        } else {
          choices = null;
          previousChoiceGroup = "";
          children.push(node);
        }
      }
    }
    return h("section.screen-ui-panel", data, [h("h2", panel.title), h("div.screen-ui-controls", children)]);
  }

  private createControl(control: UiControl): VNode {
    if (control.kind === "button") {
      const data: VNodeData = { key: control.id, attrs: { type: "button" }, dataset: { controlId: control.id }, props: { disabled: !control.enabled } };
      const presentation = control.presentation;
      let selector = "button";
      let children: string | VNode[] = control.label;
      if (presentation?.kind === "action") data.dataset = { ...data.dataset, emphasis: presentation.emphasis };
      if (presentation?.kind === "choice") {
        selector = "button.screen-ui-choice";
        data.attrs = { ...data.attrs, "aria-pressed": String(presentation.selected) };
        children = [
          h("span.screen-ui-choice-label", `${presentation.selected ? "● " : ""}${control.label}`),
          ...(presentation.description.length > 0 ? [h("small", presentation.description)] : [])
        ];
      }
      if (presentation?.kind === "disclosure") {
        data.dataset = { ...data.dataset, emphasis: "secondary" };
        data.attrs = { ...data.attrs, "aria-expanded": String(presentation.expanded), "aria-controls": `${control.id}-content` };
        children = `${presentation.expanded ? "▾" : "▸"} ${control.label}`;
      }
      return h(selector, data, children);
    }
    if (control.kind === "toggle") {
      return h("label.screen-ui-toggle", { key: control.id }, [
        h("input", { key: `${control.id}-input`, attrs: { type: "checkbox" }, dataset: { controlId: control.id }, props: { checked: control.value, disabled: !control.enabled } }),
        h("span", control.label)
      ]);
    }
    if (control.kind === "range") {
      return h("label.screen-ui-range", { key: control.id }, [
        h("span", `${control.label}: ${control.displayValue}`),
        h("input", { key: `${control.id}-input`, attrs: { type: "range", min: String(control.minimum), max: String(control.maximum), step: String(control.step) }, dataset: { controlId: control.id }, props: { value: String(control.value), disabled: !control.enabled } })
      ]);
    }
    if (control.kind === "chart") return this.createChart(control);
    return h("output.screen-ui-status", { key: control.id, attrs: { id: control.id, "aria-live": "polite" } }, control.label.length === 0 ? control.value : `${control.label}: ${control.value}`);
  }

  private createChart(control: Extract<UiControl, { readonly kind: "chart" }>): VNode {
    const caption = h("figcaption", control.label);
    const svg: VNode[] = [];
    const svgAttributes: Record<string, string> = {};
    const svgStyle: NonNullable<VNodeData["style"]> = {};
    const layout = control.presentation;
    svgAttributes["viewBox"] = `0 0 ${String(layout.viewport.width)} ${String(layout.viewport.height)}`;
    svgStyle["--screen-chart-tick-font"] = `${String(layout.tick_font_size)}px`;
    svgStyle["--screen-chart-axis-font"] = `${String(layout.axis_font_size)}px`;
    svgAttributes["role"] = "img";
    svgAttributes["aria-label"] = `${control.label}; ${control.x_axis_label}; ${control.y_axis_label}`;
    svgAttributes["preserveAspectRatio"] = "xMidYMid meet";
    const [plotLeft, plotTop, plotRight, plotBottom] = layout.plot;
    svg.push(svgElement("path", {
      d: `M ${String(plotLeft)} ${String(plotTop)} V ${String(plotBottom)} H ${String(plotRight)}`,
      fill: "none",
      stroke: layout.axes_color,
      "stroke-width": "1.5"
    }));
    for (const [start, end] of layout.grid_lines) {
      svg.push(svgElement("path", {
        d: `M ${String(start.x)} ${String(start.y)} L ${String(end.x)} ${String(end.y)}`,
        fill: "none",
        stroke: layout.grid_color,
        "stroke-opacity": String(layout.grid_opacity),
        "stroke-width": "1"
      }));
    }
    for (const tick of layout.x_ticks) svg.push(chartText(tick.text, "screen-ui-chart-tick"));
    for (const tick of layout.y_ticks) svg.push(chartText(tick.text, "screen-ui-chart-tick screen-ui-chart-y-tick"));
    control.reference_lines.forEach((line, index) => {
      const y = line.position_y_px;
      svg.push(svgElement("path", {
        d: `M ${String(plotLeft)} ${String(y)} H ${String(plotRight)}`,
        fill: "none",
        stroke: line.color,
        "stroke-width": "2",
        "stroke-dasharray": "8 6"
      }));
      const label = layout.reference_labels[index];
      if (label !== undefined) svg.push(chartText(label, "screen-ui-chart-reference"));
    });
    control.series.forEach((series) => {
      if (series.segment_colors.length > 0 && series.points.length > 1) {
        series.segment_colors.forEach((color, index) => {
          const first = series.points[index];
          const second = series.points[index + 1];
          if (first === undefined || second === undefined) return;
          svg.push(svgElement("path", {
            d: `M ${String(first.x)} ${String(first.y)} L ${String(second.x)} ${String(second.y)}`,
            fill: "none",
            stroke: color,
            "stroke-width": "2.5",
            "stroke-linecap": "round"
          }));
        });
        return;
      }
      const points = series.points.map((point, index) => {
        const x = point.x;
        const y = point.y;
        return `${index === 0 ? "M" : "L"} ${String(x)} ${String(y)}`;
      }).join(" ");
      svg.push(svgElement("path", {
        d: points,
        fill: "none",
        stroke: series.color,
        "stroke-width": "2",
        "stroke-linejoin": "round",
        "stroke-linecap": "round"
      }));
    });
    if (control.cursor_x !== null) {
      const x = control.cursor_x;
      svg.push(svgElement("path", {
        d: `M ${String(x)} ${String(plotTop)} V ${String(plotBottom)}`,
        fill: "none",
        stroke: "#f3f4e8",
        "stroke-width": "2",
        "stroke-dasharray": "7 6"
      }));
    }
    control.cursor_points.forEach((cursor) => {
      const coordinateX = cursor.point.x;
      const coordinateY = cursor.point.y;
      svg.push(svgElement("circle", {
        cx: String(coordinateX), cy: String(coordinateY), r: "4", fill: cursor.color
      }));
    });
    control.markers.forEach((marker, index) => {
      const x = marker.point.x;
      const y = marker.point.y;
      svg.push(svgElement("circle", {
        cx: String(x), cy: String(y), r: "4", fill: marker.color, stroke: "#10242b", "stroke-width": "1"
      }));
      const label = layout.marker_labels[index];
      if (label !== undefined) svg.push(chartText(label, "screen-ui-chart-marker"));
    });
    control.time_markers.forEach((marker, index) => {
      const x = marker.point.x;
      const y = marker.point.y;
      svg.push(svgElement("circle", {
        cx: String(x), cy: String(y), r: "2.5", fill: marker.color, stroke: "#10242b", "stroke-width": "1"
      }));
      const label = layout.time_marker_labels[index];
      if (label !== undefined) svg.push(chartText(label, "screen-ui-chart-time-marker"));
    });
    control.vectors.forEach((vector) => {
      if (vector.segments.length === 0) return;
      svg.push(svgElement("path", {
        d: vector.segments.map(([from, to]) =>
          `M ${String(from.x)} ${String(from.y)} L ${String(to.x)} ${String(to.y)}`).join(" "),
        fill: "none",
        stroke: vector.color,
        "stroke-width": "2",
        "stroke-linecap": "round"
      }));
    });
    if (layout.map_guides !== null) {
      const guides = layout.map_guides;
      svg.push(svgElement("path", {
        d: [...guides.scale_segments, ...guides.north_segments].map(([start, end]) =>
          `M ${String(start.x)} ${String(start.y)} L ${String(end.x)} ${String(end.y)}`).join(" "),
        fill: "none", stroke: "#f3f4e8", "stroke-width": "1.5"
      }));
      svg.push(chartText(guides.scale_label, "screen-ui-chart-map-label"), chartText(guides.north_label, "screen-ui-chart-map-label"));
    }
    svg.push(chartText(layout.x_axis, "screen-ui-chart-axis"), chartText(layout.y_axis, "screen-ui-chart-axis screen-ui-chart-y-axis"));
    const legend: VNode[] = [];
    layout.legend.forEach((entry) => {
      const item = h("span", { style: {}, dataset: {} }, "");
      item.text = entry.label;
      setNodeStyle(item, "--chart-series-color", entry.color);
      if (entry.segment_gradient !== null) {
        setNodeDataset(item, "progressScale", "true");
        setNodeStyle(item, "--chart-series-start", entry.segment_gradient[0]);
        setNodeStyle(item, "--chart-series-end", entry.segment_gradient[1]);
      }
      legend.push(item);
    });
    return h("figure.screen-ui-chart", { key: control.id, style: {
      overflow: layout.scroll_overflow ? "auto" : "hidden",
      "--chart-minimum-width": `${String(layout.viewport_minimum_rem[0])}rem`,
      "--chart-minimum-height": `${String(layout.viewport_minimum_rem[1])}rem`,
      "--chart-legend-font": `${String(layout.legend_font_rem)}rem`,
      "--chart-legend-gap": `${String(layout.legend_gap_rem)}rem`,
      "--chart-legend-swatch-width": `${String(layout.legend_swatch_rem[0])}rem`,
      "--chart-legend-swatch-height": `${String(layout.legend_swatch_rem[1])}rem`,
      "--chart-legend-swatch-gap": `${String(layout.legend_swatch_gap_rem)}rem`,
      "--chart-legend-gradient-width": `${String(layout.legend_gradient_rem[0])}rem`,
      "--chart-legend-gradient-height": `${String(layout.legend_gradient_rem[1])}rem`,
      "--chart-legend-gradient-radius": `${String(layout.legend_gradient_radius_rem)}rem`
    } }, [
      caption,
      h("div.screen-ui-chart-viewport", { dataset: { chartControlId: control.id } }, [
        h("svg", { attrs: svgAttributes, style: svgStyle }, svg)
      ]),
      h("div.screen-ui-chart-legend", legend)
    ]);
  }
}

function chartText(label: Extract<UiControl, { readonly kind: "chart" }>["presentation"]["x_axis"], className: string): VNode {
  const { x: coordinateX, y: coordinateY } = label.position;
  const node = svgElement("text", { x: String(coordinateX), y: String(coordinateY), class: className,
    "text-anchor": label.anchor, fill: label.color,
    transform: `rotate(${String(label.rotation_degrees)} ${String(coordinateX)} ${String(coordinateY)})`,
    style: `font-size:${String(label.font_size)}px;font-weight:${String(label.font_weight)};text-anchor:${label.anchor};fill:${label.color}` });
  node.text = label.label;
  return node;
}

function sharedPresentationData(presentation: NonNullable<UiViewModel["sharedPresentation"]>): VNodeData {
  const { layout, theme } = presentation;
  return {
    dataset: {
      shared: "true", shell: layout.shell, compact: String(layout.compact), passthrough: String(layout.passthrough), scrollY: String(layout.scroll_y),
      ...(layout.shell === "analysis" || layout.shell === "cinematic" ? { mode: layout.shell } : {})
    },
    style: {
      width: `${String(layout.width_px)}px`,
      maxHeight: `${String(layout.maximum_height_px)}px`,
      padding: `${String(layout.padding_px)}px`,
      overflowX: layout.clip_x ? "hidden" : "auto",
      overflowY: layout.scroll_y ? "auto" : "hidden",
      background: layout.passthrough ? "transparent" : `color-mix(in srgb, ${theme.panel_background} ${String(theme.panel_opacity * 100)}%, transparent)`,
      ...(layout.shell === "analysis" ? { height: `${String(layout.maximum_height_px)}px` } : {}),
      "--game-ui-gap": `${String(layout.gap_px)}px`,
      "--game-ui-columns": String(layout.choice_columns),
      "--game-ui-text": theme.text,
      "--game-ui-muted-text": theme.muted_text,
      "--game-ui-panel": theme.panel_background,
      "--game-ui-opacity": String(theme.panel_opacity),
      "--game-ui-hud-opacity": String(theme.hud_opacity),
      "--game-ui-normal": theme.normal_background,
      "--game-ui-primary": theme.primary_background,
      "--game-ui-choice": theme.choice_background,
      "--game-ui-focus": theme.focus_outline,
      "--game-ui-button-height": `${String(theme.button_min_height_px)}px`,
      "--game-ui-primary-height": `${String(theme.primary_min_height_px)}px`
    }
  };
}

function eventElement(event: Event, root: HTMLElement): Element | null {
  const ElementConstructor = root.ownerDocument.defaultView?.Element;
  if (ElementConstructor === undefined || !(event.target instanceof ElementConstructor)) return null;
  return event.target;
}

function svgElement(name: string, attributes: Readonly<Record<string, string>>): VNode {
  return h(name, { ns: "http://www.w3.org/2000/svg", attrs: { ...attributes } }, "");
}

function setNodeStyle(node: VNode, name: string, value: string): void {
  const data = node.data ?? {};
  const style = data.style ?? {};
  style[name] = value;
  node.data = { ...data, style };
}

function setNodeDataset(node: VNode, name: string, value: string): void {
  node.data = { ...node.data, dataset: { ...node.data?.dataset, [name]: value } };
}

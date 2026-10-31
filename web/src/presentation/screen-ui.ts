import type { ApplicationInput, GameUiChart, GameUiControl, GameUiControlInput, GameUiFrame, GameUiPanel, GameUiView } from "../generated/application-contracts.js";
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

declare const __APP_BUILD_LABEL__: string;

const APP_BUILD_LABEL = typeof __APP_BUILD_LABEL__ === "string"
  ? __APP_BUILD_LABEL__
  : "v0.1.0 · test";

const patch = init([attributesModule, classModule, datasetModule, propsModule, styleModule]);
type ControlPointer = Extract<ApplicationInput, { kind: "control_pointer" }>;
type ChartMeasurement = Extract<ApplicationInput, { kind: "chart_measured" }>;

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
  private controls = new Map<string, GameUiControl>();
  private visible = true;
  private retainedDom: RetainedScreenDom | null = null;
  private currentViewModel: GameUiView | null = null;
  private frame: GameUiFrame | null = null;
  private readonly observedCharts = new Map<Element, number>();
  private readonly chartMeasurements = new Map<Element, ChartMeasurement>();
  private chartFrame: number | null = null;
  private readonly capturedPointers = new Map<number, { element: Element; last: ControlPointer }>();
  private readonly chartObserver: ResizeObserver | null;

  constructor(private readonly root: HTMLElement, private readonly dispatch: (input: ApplicationInput) => void) {
    this.mount = root.ownerDocument.createElement("div");
    this.mount.className = "screen-ui-mount";
    this.root.append(this.mount);
    this.currentVNode = patch(this.mount, h("div.screen-ui-mount", []));
    this.root.addEventListener("click", this.onClick);
    this.root.addEventListener("change", this.onChange);
    this.root.addEventListener("input", this.onInput);
    this.root.addEventListener("pointerdown", this.onPointerDown);
    this.root.addEventListener("pointermove", this.onPointerMove);
    this.root.addEventListener("pointerup", this.onPointerUp);
    this.root.addEventListener("pointercancel", this.onPointerCancel);
    this.root.addEventListener("lostpointercapture", this.onPointerLost);
    const ResizeObserverConstructor = root.ownerDocument.defaultView?.ResizeObserver;
    this.chartObserver = ResizeObserverConstructor === undefined ? null : new ResizeObserverConstructor(this.onChartResize);
  }

  dispose(): void {
    const released = this.releasePointers();
    this.chartObserver?.disconnect();
    this.chartMeasurements.clear();
    this.cancelChartFrame();
    this.discardRetainedDom();
    this.root.removeEventListener("click", this.onClick);
    this.root.removeEventListener("change", this.onChange);
    this.root.removeEventListener("input", this.onInput);
    this.root.removeEventListener("pointerdown", this.onPointerDown);
    this.root.removeEventListener("pointermove", this.onPointerMove);
    this.root.removeEventListener("pointerup", this.onPointerUp);
    this.root.removeEventListener("pointercancel", this.onPointerCancel);
    this.root.removeEventListener("lostpointercapture", this.onPointerLost);
    this.observedCharts.clear();
    this.controls.clear();
    this.currentViewModel = null;
    this.frame = null;
    this.mount.remove();
    for (const input of released) this.dispatch(input);
  }

  render(viewModel: GameUiView, visible = true): void {
    this.frame = null;
    this.renderDom(viewModel, visible);
  }

  renderFrame(frame: GameUiFrame, visible = true): void {
    const view = this.currentViewModel;
    if (view === null || view.view_revision !== frame.view_revision) throw new Error("The UI frame has no retained structural view");
    this.frame = frame;
    this.renderDom(view, visible);
  }

  private renderDom(viewModel: GameUiView, visible: boolean): void {
    const scopeChanged = this.currentViewModel !== null && (this.currentViewModel.interaction_scope !== viewModel.interaction_scope);
    const nextControls = new Map(viewModel.panels.flatMap(panel => panel.controls.map(control => [control.id, control] as const)));
    const released = !visible || scopeChanged ? this.releasePointers() : this.currentViewModel?.view_revision !== viewModel.view_revision
      ? this.releasePointers(last => {
        if (last.control_id == null) return false;
        const before = this.controls.get(last.control_id)?.content;
        const after = nextControls.get(last.control_id);
        return last.pointer_kind === "touch" && before?.kind === "flight_touch" && after?.enabled === true
          && after.content.kind === "flight_touch" && JSON.stringify(before.bounds) === JSON.stringify(after.content.bounds);
      }) : [];
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
    const data = sharedPresentationData(viewModel);
    const dataset: NonNullable<VNodeData["dataset"]> = { ...data.dataset, scene: viewModel.scene };
    dataset.scope = viewModel.interaction_scope;
    if (viewModel.active_overlay != null) dataset.overlay = viewModel.active_overlay;
    const shell = h("section.screen-ui-shell", { ...data, key: "screen-shell", dataset }, [
      h("h1", viewModel.title),
      h("p.screen-ui-description", viewModel.description),
      ...viewModel.panels.map((panel) => this.createPanel(panel)),
      h("small.build-version", APP_BUILD_LABEL)
    ]);
    const nextVNode = h("div.screen-ui-mount", {
      attrs: { hidden: !visible, inert: !visible, "aria-hidden": String(!visible) }
    }, [shell, ...viewModel.panels.flatMap(panel => panel.controls.filter(control => control.content.kind === "flight_touch").map(control => this.createControl(control)))]);
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
    for (const input of released) this.dispatch(input);
  }

  private observeCharts(): void {
    if (this.chartObserver === null) return;
    const targets = this.visible ? new Set(this.mount.querySelectorAll(".screen-ui-chart-viewport")) : new Set<Element>();
    const revision = this.currentViewModel?.view_revision;
    if (revision === undefined) return;
    for (const [target, observedRevision] of this.observedCharts) {
      if (!targets.has(target) || observedRevision !== revision) {
        this.chartObserver.unobserve(target);
        this.observedCharts.delete(target);
        this.chartMeasurements.delete(target);
      }
    }
    for (const target of targets) {
      if (!this.observedCharts.has(target)) {
        this.observedCharts.set(target, revision);
        this.chartObserver.observe(target);
      }
    }
    if (this.chartMeasurements.size === 0) this.cancelChartFrame();
  }

  private readonly onChartResize: ResizeObserverCallback = (entries): void => {
    if (!this.visible || this.currentViewModel === null) return;
    for (const entry of entries) {
      const revision = this.observedCharts.get(entry.target);
      if (revision === undefined || !this.mount.contains(entry.target)) continue;
      const id = entry.target.getAttribute("data-chart-control-id");
      const { width, height } = entry.contentRect;
      if (id === null || this.controls.get(id)?.content.kind !== "chart") continue;
      this.chartMeasurements.set(entry.target, { kind: "chart_measured", view_revision: revision, control_id: id, extent: { width, height } });
    }
    if (this.chartFrame === null && this.chartMeasurements.size > 0) {
      this.chartFrame = this.root.ownerDocument.defaultView?.requestAnimationFrame(this.deliverChartMeasurements) ?? null;
    }
  };

  private readonly deliverChartMeasurements = (): void => {
    this.chartFrame = null;
    const measurements = [...this.chartMeasurements];
    this.chartMeasurements.clear();
    for (const [element, input] of measurements) {
      if (this.observedCharts.get(element) === input.view_revision && this.mount.contains(element)) this.dispatch(input);
    }
  };

  private cancelChartFrame(): void {
    if (this.chartFrame !== null) this.root.ownerDocument.defaultView?.cancelAnimationFrame(this.chartFrame);
    this.chartFrame = null;
  }

  private controlInput(input: GameUiControlInput): void {
    if (this.currentViewModel !== null) this.dispatch({ kind: "control", view_revision: this.currentViewModel.view_revision, input });
  }

  private controlPointer(event: PointerEvent, phase: ControlPointer["phase"], element: Element, controlId: string | null, revision: number): ControlPointer {
    const rect = element.getBoundingClientRect();
    return { kind: "control_pointer", view_revision: revision, control_id: controlId, pointer_id: { low: event.pointerId >>> 0, high: 0 }, pointer_kind: ["mouse", "touch", "pen"].includes(event.pointerType) ? event.pointerType as "mouse" | "touch" | "pen" : "unknown", phase,
      button: event.button, buttons: event.buttons, is_primary: event.isPrimary, position: [event.clientX, event.clientY],
      rect: [rect.left, rect.top, rect.width, rect.height] };
  }

  private readonly onPointerDown = (event: PointerEvent): void => {
    if (!this.visible || this.currentViewModel === null) return;
    const element = eventElement(event, this.root)?.closest("[data-pointer-control-id], .screen-ui-chart-viewport");
    const controlId = element?.getAttribute("data-pointer-control-id") ?? element?.getAttribute("data-chart-control-id");
    if (element == null || controlId == null) return;
    const handle = { element, last: this.controlPointer(event, "down", element, controlId, this.currentViewModel.view_revision) };
    this.capturedPointers.set(event.pointerId, handle);
    let failure = false;
    try { element.setPointerCapture(event.pointerId); } catch { failure = true; }
    this.dispatch(handle.last);
    if (failure && this.capturedPointers.get(event.pointerId) === handle) {
      this.capturedPointers.delete(event.pointerId);
      this.dispatch({ ...handle.last, phase: "lost_capture" });
    }
  };

  private pointerEvent(event: PointerEvent, phase: ControlPointer["phase"], release: boolean): void {
    const handle = this.capturedPointers.get(event.pointerId);
    if (handle === undefined) return;
    handle.last = this.controlPointer(event, phase, handle.element, handle.last.control_id ?? null, handle.last.view_revision);
    if (release) {
      this.capturedPointers.delete(event.pointerId);
      if (phase !== "lost_capture") {
        try { handle.element.releasePointerCapture(event.pointerId); } catch {
          this.dispatch(handle.last);
          this.dispatch({ ...handle.last, phase: "lost_capture" });
          return;
        }
      }
    }
    this.dispatch(handle.last);
  }

  private readonly onPointerMove = (event: PointerEvent): void => { this.pointerEvent(event, "move", false); };
  private readonly onPointerUp = (event: PointerEvent): void => { this.pointerEvent(event, "up", true); };
  private readonly onPointerCancel = (event: PointerEvent): void => { this.pointerEvent(event, "cancel", true); };
  private readonly onPointerLost = (event: PointerEvent): void => { this.pointerEvent(event, "lost_capture", true); };

  private releasePointers(keep: (input: ControlPointer) => boolean = () => false): ControlPointer[] {
    const released = [...this.capturedPointers.entries()].filter(([, handle]) => !keep(handle.last));
    for (const [pointerId] of released) this.capturedPointers.delete(pointerId);
    return released.map(([pointerId, handle]) => {
      const input: ControlPointer = { ...handle.last, phase: "lost_capture" };
      try { handle.element.releasePointerCapture(pointerId); } catch { return input; }
      return input;
    });
  }

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
    if (control?.content.kind !== "button" || !control.enabled) return;
    this.controlInput({ type: "activate", controlId: control.id });
  };

  private readonly onChange = (event: Event): void => {
    if (!this.visible) return;
    const input = eventElement(event, this.root);
    if (input?.tagName !== "INPUT" || input.getAttribute("type") !== "checkbox") return;
    const controlId = input.getAttribute("data-control-id");
    if (controlId === null) return;
    const control = this.controls.get(controlId);
    if (control?.content.kind !== "toggle" || !control.enabled) return;
    const checked = (input as HTMLInputElement).checked;
    this.controlInput({ type: "set-toggle", controlId: control.id, value: checked });
  };

  private readonly onInput = (event: Event): void => {
    if (!this.visible) return;
    const input = eventElement(event, this.root);
    if (input?.tagName !== "INPUT" || input.getAttribute("type") !== "range") return;
    const controlId = input.getAttribute("data-control-id");
    if (controlId === null) return;
    const control = this.controls.get(controlId);
    if (control?.content.kind !== "range" || !control.enabled) return;
    const value = Number((input as HTMLInputElement).value);
    if (Number.isFinite(value)) this.controlInput({ type: "set-range", controlId: control.id, value });
  };

  private createPanel(panel: GameUiPanel): VNode {
    const data: VNodeData = { key: panel.id, dataset: { anchor: panel.anchor }, attrs: { "aria-label": panel.title } };
    const children: VNode[] = [];
    for (const section of panel.sections) {
      const controls = panel.controls.filter(control => control.section === section.id && control.content.kind !== "flight_touch");
      if (controls.length === 0) continue;
      const content = h("div.screen-ui-section-controls", controls.map(control => this.createControl(control)));
      children.push(h(section.label.length > 0 ? "fieldset.screen-ui-section" : "div.screen-ui-section", {
        key: `${panel.id}-section-${section.id}`, dataset: { sectionId: section.id, layout: section.layout }
      }, section.label.length > 0 ? [h("legend", section.label), content] : [content]));
    }
    return h("section.screen-ui-panel", data, [h("h2", panel.title), h("div.screen-ui-controls", children)]);
  }

  private createControl(control: GameUiControl): VNode {
    const content = control.content;
    const label = this.frame?.button_labels.find(button => button.id === control.id)?.label ?? control.label;
    if (content.kind === "flight_touch") {
      const theme = this.currentViewModel?.theme;
      if (theme === undefined) throw new Error("Touch control has no retained UI view");
      const value = this.frame?.flight_touch.find(value => value.id === control.id) ?? content;
      const point = value.position;
      const origin = value.origin;
      return h("div.screen-ui-flight-touch", {
        key: control.id, dataset: { pointerControlId: control.id },
        attrs: { "aria-label": control.label },
        style: { position: "fixed", left: `${String(content.bounds.x)}px`, top: `${String(content.bounds.y)}px`, width: `${String(content.bounds.width)}px`, height: `${String(content.bounds.height)}px`, pointerEvents: "auto", touchAction: "none", userSelect: "none" }
      }, [h("svg", { attrs: { width: "100%", height: "100%", "aria-hidden": "true" }, style: { overflow: "visible", pointerEvents: "none" } }, [
        h("rect", { attrs: { x: 0, y: 0, width: "100%", height: "100%", rx: String(content.radius), fill: theme.panel_background, "fill-opacity": String(theme.panel_opacity), stroke: theme.text } }),
        ...(origin == null ? [] : [h("circle", { attrs: { cx: String(origin[0]), cy: String(origin[1]), r: String(content.radius), fill: "none", stroke: theme.text } })]),
        h("circle", { attrs: { cx: String(point[0]), cy: String(point[1]), r: String(content.radius), fill: theme.text, stroke: theme.panel_background } })
      ]), h("span", { style: { position: "absolute", top: "0", left: "0", color: theme.text, pointerEvents: "none" } }, control.label)]);
    }
    if (content.kind === "button") {
      const data: VNodeData = { key: control.id, attrs: { type: "button" }, dataset: { controlId: control.id }, props: { disabled: !control.enabled } };
      const presentation = content.presentation;
      let selector = "button";
      let children: string | VNode[] = label;
      if (presentation.kind === "action") data.dataset = { ...data.dataset, emphasis: presentation.emphasis };
      if (presentation.kind === "choice") {
        selector = "button.screen-ui-choice";
        data.attrs = { ...data.attrs, "aria-pressed": String(presentation.selected) };
        children = [
          h("span.screen-ui-choice-label", `${presentation.selected ? "● " : ""}${label}`),
          ...(presentation.description.length > 0 ? [h("small", presentation.description)] : [])
        ];
      }
      if (presentation.kind === "disclosure") {
        data.dataset = { ...data.dataset, emphasis: "secondary" };
        data.attrs = { ...data.attrs, "aria-expanded": String(presentation.expanded), "aria-controls": `${control.id}-content` };
        children = `${presentation.expanded ? "▾" : "▸"} ${label}`;
      }
      return h(selector, data, children);
    }
    if (content.kind === "toggle") {
      return h("label.screen-ui-toggle", { key: control.id }, [
        h("input", { key: `${control.id}-input`, attrs: { type: "checkbox" }, dataset: { controlId: control.id }, props: { checked: content.value, disabled: !control.enabled } }),
        h("span", control.label)
      ]);
    }
    if (content.kind === "range") {
      const range = this.frame?.ranges.find(value => value.id === control.id) ?? content;
      return h("label.screen-ui-range", { key: control.id }, [
        h("span", `${control.label}: ${range.display_value}`),
        h("input", { key: `${control.id}-input`, attrs: { type: "range", min: String(content.minimum), max: String(content.maximum), step: String(content.step) }, dataset: { controlId: control.id }, props: { value: String(range.value), disabled: !control.enabled } })
      ]);
    }
    if (content.kind === "chart") return this.createChart(control, content.plot);
    if (content.kind === "fields") {
      const fields = this.frame?.fields.find(group => group.id === control.id)?.fields ?? content.fields;
      return h("section.screen-ui-fields", { key: control.id, attrs: { id: control.id, "aria-label": control.label } }, [
        ...(control.label.length === 0 ? [] : [h("h3", control.label)]),
        h("dl.screen-ui-fields-grid", fields.map(field => h("div.screen-ui-field", { key: field.id, dataset: { fieldId: field.id } }, [
          h("dt", field.label), h("dd", [h("span.screen-ui-field-value", field.value), h("span.screen-ui-field-unit", field.unit)])
        ])))
      ]);
    }
    const value = this.frame?.readouts.find(readout => readout.id === control.id)?.value ?? content.value;
    return h("output.screen-ui-status", { key: control.id, attrs: { id: control.id, "aria-live": "polite" } }, control.label.length === 0 ? value : `${control.label}: ${value}`);
  }

  private createChart(control: GameUiControl, plot: GameUiChart): VNode {
    const caption = h("figcaption", control.label);
    const svg: VNode[] = [];
    const svgAttributes: Record<string, string> = {};
    const svgStyle: NonNullable<VNodeData["style"]> = {};
    const layout = plot.presentation;
    const cursor = this.frame?.chart_cursor;
    const selectedCursor = cursor?.id === control.id ? cursor : null;
    svgAttributes["viewBox"] = `0 0 ${String(layout.viewport.width)} ${String(layout.viewport.height)}`;
    svgStyle["--screen-chart-tick-font"] = `${String(layout.tick_font_size)}px`;
    svgStyle["--screen-chart-axis-font"] = `${String(layout.axis_font_size)}px`;
    svgAttributes["role"] = "img";
    svgAttributes["aria-label"] = `${control.label}; ${plot.x_axis_label}; ${plot.y_axis_label}`;
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
    plot.reference_lines.forEach((line, index) => {
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
    plot.series.forEach((series) => {
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
    const cursorX = selectedCursor === null ? plot.cursor_x : selectedCursor.cursor_x;
    if (cursorX != null) {
      const x = cursorX;
      svg.push(svgElement("path", {
        d: `M ${String(x)} ${String(plotTop)} V ${String(plotBottom)}`,
        fill: "none",
        stroke: "#f3f4e8",
        "stroke-width": "2",
        "stroke-dasharray": "7 6"
      }));
    }
    (selectedCursor?.cursor_points ?? plot.cursor_points).forEach((cursor) => {
      const coordinateX = cursor.point.x;
      const coordinateY = cursor.point.y;
      svg.push(svgElement("circle", {
        cx: String(coordinateX), cy: String(coordinateY), r: "4", fill: cursor.color
      }));
    });
    plot.markers.forEach((marker, index) => {
      const x = marker.point.x;
      const y = marker.point.y;
      svg.push(svgElement("circle", {
        cx: String(x), cy: String(y), r: "4", fill: marker.color, stroke: "#10242b", "stroke-width": "1"
      }));
      const label = layout.marker_labels[index];
      if (label !== undefined) svg.push(chartText(label, "screen-ui-chart-marker"));
    });
    plot.time_markers.forEach((marker, index) => {
      const x = marker.point.x;
      const y = marker.point.y;
      svg.push(svgElement("circle", {
        cx: String(x), cy: String(y), r: "2.5", fill: marker.color, stroke: "#10242b", "stroke-width": "1"
      }));
      const label = layout.time_marker_labels[index];
      if (label !== undefined) svg.push(chartText(label, "screen-ui-chart-time-marker"));
    });
    plot.vectors.forEach((retained, index) => {
      const vector = index === 0 ? selectedCursor?.selected_wind_vector ?? retained : retained;
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
    if (layout.map_guides != null) {
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
      item.text = entry.source === "analysis_cursor" ? selectedCursor?.selected_wind_vector?.label ?? entry.label : entry.label;
      setNodeStyle(item, "--chart-series-color", entry.color);
      if (entry.segment_gradient != null) {
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
      h("div.screen-ui-chart-viewport", { dataset: { chartControlId: control.id }, style: { height: `${String(layout.target_height_px)}px`, flex: "0 0 auto" } }, [
        h("svg", { attrs: svgAttributes, style: svgStyle }, svg)
      ]),
      h("div.screen-ui-chart-legend", legend)
    ]);
  }
}

function chartText(label: GameUiChart["presentation"]["x_axis"], className: string): VNode {
  const { x: coordinateX, y: coordinateY } = label.position;
  const node = svgElement("text", { x: String(coordinateX), y: String(coordinateY), class: className,
    "text-anchor": label.anchor, fill: label.color,
    transform: `rotate(${String(label.rotation_degrees)} ${String(coordinateX)} ${String(coordinateY)})`,
    style: `font-size:${String(label.font_size)}px;font-weight:${String(label.font_weight)};text-anchor:${label.anchor};fill:${label.color}` });
  node.text = label.label;
  return node;
}

function sharedPresentationData(presentation: GameUiView): VNodeData {
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

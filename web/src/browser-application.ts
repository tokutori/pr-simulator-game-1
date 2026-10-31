import { ApplicationRuntimeBridge } from "../pkg/birdman_game_wasm.js";
import type { ApplicationDrawingOutput, ApplicationInput, ApplicationOutput, ApplicationPlatformObservation, ApplicationRendererObservation, ApplicationRequest, ApplicationTextProbe, GameUiView } from "./generated/application-contracts.js";
import { BrowserFlightLogDownload } from "./app/browser-flight-log-download.js";
import { BrowserFlightRecordStore } from "./game/flight-record-store.js";
import { browserSurfaceContext } from "./presentation/browser-canvas.js";
import { FlightHudAdapter } from "./presentation/flight-hud.js";
import { ScreenUiAdapter } from "./presentation/screen-ui.js";
import { ThreeApplicationRenderer } from "./render/engines/three/application-renderer.js";

export class BrowserApplication {
  private owner: ApplicationRuntimeBridge | null;
  private output: ApplicationOutput | null = null;
  private notificationDepth = 0;
  private readonly stage: HTMLDivElement;
  private readonly screen: ScreenUiAdapter;
  private readonly hud: FlightHudAdapter;
  private readonly frameRate: HTMLOutputElement;
  private readonly download: BrowserFlightLogDownload;
  private readonly records: BrowserFlightRecordStore;
  private readonly timers = new Map<number, ReturnType<typeof setTimeout>>();
  private readonly renderers = new Map<number, ThreeApplicationRenderer>();
  private readonly phoneSensors = new Map<number, () => void>();
  private readonly measurement: CanvasRenderingContext2D;
  private readonly documentStyle: MutationObserver;
  private fontGeneration = 0;
  private renderer: ThreeApplicationRenderer | null = null;
  private view: GameUiView | null = null;

  constructor(private readonly mount: HTMLElement, private readonly capabilities: ApplicationPlatformObservation["capabilities"]) {
    const documentRef = mount.ownerDocument;
    this.stage = documentRef.createElement("div");
    this.stage.className = "presentation-shell";
    const uiRoot = documentRef.createElement("div");
    uiRoot.className = "screen-ui-root";
    const hudRoot = documentRef.createElement("section");
    this.frameRate = documentRef.createElement("output");
    this.frameRate.className = "render-frame-rate";
    this.frameRate.setAttribute("aria-label", "描画フレームレート");
    this.stage.append(uiRoot, hudRoot, this.frameRate);
    mount.replaceChildren(this.stage);
    const measurement = documentRef.createElement("canvas").getContext("2d");
    if (measurement === null) throw new Error("Canvas text measurement is unavailable");
    this.measurement = measurement;
    this.screen = new ScreenUiAdapter(uiRoot, input => { this.observe(input); });
    this.hud = new FlightHudAdapter(hudRoot);
    this.download = new BrowserFlightLogDownload(documentRef);
    this.records = new BrowserFlightRecordStore(window.indexedDB, input => { this.observe(input); });
    this.documentStyle = new MutationObserver(this.onPlatform);
    this.documentStyle.observe(documentRef.documentElement, { attributes: true, attributeFilter: ["style", "class", "lang"] });
    const platform = this.platform();
    this.owner = new ApplicationRuntimeBridge(JSON.stringify(platform));
    this.mount.addEventListener("pointerdown", this.onPointerObserved, true);
    window.addEventListener("resize", this.onPlatform);
    window.addEventListener("keydown", this.onKeyDown);
    window.addEventListener("keyup", this.onKeyUp);
    window.addEventListener("focus", this.onFocus);
    window.addEventListener("blur", this.onBlur);
    window.addEventListener("pagehide", this.onPageHide);
    window.addEventListener("pageshow", this.onPageShow);
    documentRef.addEventListener("visibilitychange", this.onVisibility);
    documentRef.fonts.addEventListener("loadingdone", this.onFontLoaded);
    this.observe({ kind: "platform", observation: platform });
  }

  private platform(): ApplicationPlatformObservation {
    const style = getComputedStyle(this.mount.ownerDocument.documentElement);
    return {
      gpu_timing_requested: false,
      renderer: this.renderer?.observation() ?? null,
      viewport: { width: window.innerWidth, height: window.innerHeight, root_font_size: Number.parseFloat(style.fontSize) },
      capabilities: this.capabilities,
      surface: { font: { family: style.fontFamily, weight: Number.parseFloat(style.fontWeight),
        style: style.fontStyle === "italic" ? "italic" : "normal", generation: this.fontGeneration }, locale: this.mount.ownerDocument.documentElement.lang }
    };
  }

  private readonly measure = (json: string): Float64Array => {
    const request = JSON.parse(json) as ApplicationTextProbe;
    const context = browserSurfaceContext(this.measurement);
    context.setLocale(request.locale);
    context.setTextBaseline("alphabetic");
    context.setTextAlign("left");
    context.setFont(`${request.font.style} ${String(request.font.weight)} ${String(request.font_size_px)}px ${request.font.family}`);
    const metrics = context.measureText(request.text);
    return new Float64Array([metrics.width, metrics.left, metrics.right, metrics.ascent, metrics.descent]);
  };

  private observe(input: ApplicationInput): ApplicationOutput | null {
    const owner = this.owner;
    if (owner === null) return null;
    this.notificationDepth++;
    try {
      const output = JSON.parse(owner.update(JSON.stringify({ observed_ms: performance.now(), input }), this.measure)) as ApplicationOutput;
      this.output = output;
      const drawing = output.drawing;
      const renderingFailures = new Map<number, unknown[]>();
      const failed = (id: number | null | undefined, cause: unknown): void => {
        if (id == null) { console.error(cause); return; }
        const previous = renderingFailures.get(id) ?? [];
        previous.push(cause);
        renderingFailures.set(id, previous);
      };
      if (drawing != null) {
        for (const [id, renderer] of this.renderers) {
          try { renderer.apply(drawing); } catch (cause) { failed(id, cause); }
        }
        try {
          if (drawing.ui.kind === "view") {
            this.view = drawing.ui.view;
            this.screen.render(drawing.ui.view, drawing.presentation.screen_ui_visible);
          } else this.screen.renderFrame(drawing.ui.frame, drawing.presentation.screen_ui_visible);
        } catch (cause) { failed(drawing.graphics.renderer_resource_id, cause); }
        const hud = drawing.ui.kind === "view" ? drawing.ui.view.hud : drawing.ui.frame.hud;
        try {
          if (this.view !== null) this.hud.render(hud ?? null, this.view.theme);
          this.hud.setVisible(drawing.presentation.screen_hud_visible && hud != null);
          this.frameRate.hidden = !drawing.presentation.screen_ui_visible;
          if (this.frameRate.textContent !== drawing.presentation.frame_rate_label) {
            this.frameRate.textContent = drawing.presentation.frame_rate_label;
          }
        } catch (cause) { failed(drawing.graphics.renderer_resource_id, cause); }
      }
      if (output.diagnostic != null) console.error(output.diagnostic);
      const failures: unknown[] = [];
      for (const request of output.requests) {
        try { this.execute(request, drawing); } catch (cause) { failures.push(cause); }
      }
      for (const [id, causes] of renderingFailures) this.observe({ kind: "renderer_failed", renderer_resource_id: id,
        cause: causes.map(errorMessage).join("; ") });
      if (failures.length > 0) throw new AggregateError(failures, "Browser I/O execution failed");
      return output;
    } finally {
      this.notificationDepth--;
      if (this.notificationDepth === 0 && this.output?.lifecycle === "drained") this.free();
    }
  }

  private execute(request: ApplicationRequest, drawing: ApplicationDrawingOutput | null | undefined): void {
    switch (request.kind) {
      case "create_renderer": {
        let renderer: ThreeApplicationRenderer | null = null;
        let observation: ApplicationRendererObservation;
        const canvas = this.mount.ownerDocument.createElement("canvas");
        canvas.className = "presentation-canvas";
        canvas.setAttribute("aria-hidden", "true");
        this.stage.prepend(canvas);
        try {
          renderer = new ThreeApplicationRenderer(canvas, request.request_id,
            id => {
              if (this.owner === null) throw new Error("Drawing buffer owner is released");
              return this.owner.asset_bytes(id);
            }, input => { this.observe(input); }, this.onPlatform);
          observation = renderer.observation();
        } catch (cause) {
          const failures = [cause];
          for (const release of [() => { renderer?.dispose(); }, () => { canvas.remove(); }]) {
            try { release(); } catch (cleanup) { failures.push(cleanup); }
          }
          this.observe({ kind: "renderer_created", request_id: request.request_id, result: { kind: "failed",
            cause: errorMessage(failures.length === 1 ? cause : new AggregateError(failures, "Renderer acquisition cleanup failed")) } });
          break;
        }
        this.renderers.set(request.request_id, renderer);
        this.renderer = renderer;
        this.observe({ kind: "renderer_created", request_id: request.request_id, result: { kind: "ready", renderer: observation } });
        break;
      }
      case "release_renderer": {
        const renderer = this.renderers.get(request.renderer_resource_id);
        this.renderers.delete(request.renderer_resource_id);
        if (this.renderer === renderer) this.renderer = null;
        let failure: string | null = renderer === undefined ? "Requested graphics context is unavailable" : null;
        try { renderer?.dispose(); } catch (cause) { failure = errorMessage(cause); }
        this.observe({ kind: "renderer_released", renderer_resource_id: request.renderer_resource_id, failure });
        break;
      }
      case "prepare_resources": {
        const renderer = this.renderers.get(request.graphics.renderer_resource_id);
        if (renderer === undefined) this.observe({ kind: "resources", request_id: request.request_id,
          observation: { kind: "failed", cause: "Requested graphics context is unavailable" } });
        else renderer.prepare(request);
        break;
      }
      case "commit_resources":
      case "cancel_resources":
      case "release_resources": {
        const renderer = [...this.renderers.values()].find(candidate => candidate.owns(request));
        if (renderer !== undefined) renderer.execute(request);
        else if (request.kind === "release_resources") this.observe({ kind: "resources_released", resource_id: request.resource_id,
          failure: "Requested GPU resource handle is unavailable" });
        else this.observe({ kind: "resources", request_id: request.request_id, observation: request.kind === "cancel_resources"
          ? { kind: "cancelled" } : { kind: "binding_lost", resource_id: request.resource_id, cause: "Requested GPU resource binding is unavailable" } });
        break;
      }
      case "display": {
        const operation = request.request;
        const id = operation.kind === "release" || operation.kind === "stop" ? operation.resource_id
          : operation.kind === "start" ? operation.permission_resource_id : null;
        const rendererId = drawing?.graphics.renderer_resource_id;
        const renderer = id == null ? rendererId == null ? this.renderer : this.renderers.get(rendererId)
          : [...this.renderers.values()].find(candidate => candidate.ownsDisplay(id));
        if (renderer != null) renderer.display(operation);
        else if (operation.kind === "release") this.observe({ kind: "display_released", resource_id: operation.resource_id,
          failure: "Requested display resource handle is unavailable" });
        else this.observe({ kind: "display_completed", request_id: operation.request_id,
          completion: { kind: "failed", cause: "Requested display resource handle is unavailable" } });
        break;
      }
      case "start_phone_sensors": {
        this.startPhoneSensors(request.resource_id);
        break;
      }
      case "stop_phone_sensors": {
        const detach = this.phoneSensors.get(request.resource_id);
        let failure: string | null = detach === undefined ? "Requested phone sensor subscription is unavailable" : null;
        try { detach?.(); this.phoneSensors.delete(request.resource_id); } catch (cause) { failure = errorMessage(cause); }
        this.observe({ kind: "phone_sensors_stopped", resource_id: request.resource_id, failure });
        break;
      }
      case "read_record":
      case "begin_record_preview":
      case "record_preview":
      case "begin_record_save":
      case "record_write":
        this.records.execute(request);
        break;
      case "cancel_record_io":
        this.records.cancel(request.request_id);
        break;
      case "schedule_wake": {
        const timer = setTimeout(() => {
          this.timers.delete(request.request_id);
          this.observe({ kind: "wake", request_id: request.request_id });
        }, Math.max(0, request.at_ms - performance.now()));
        this.timers.set(request.request_id, timer);
        break;
      }
      case "cancel_wake":
        clearTimeout(this.timers.get(request.request_id));
        this.timers.delete(request.request_id);
        break;
      case "download_file": {
        let failure: string | null = null;
        try { this.download.download(request); } catch (cause) { failure = errorMessage(cause); }
        this.observe({ kind: "download_completed", request_id: request.request_id, failure });
        break;
      }
      case "reload_application": window.location.reload(); break;
    }
  }

  private startPhoneSensors(id: number): void {
    if (this.phoneSensors.has(id)) {
      this.observe({ kind: "phone_sensors_started", resource_id: id, failure: "Phone sensor subscription already exists" });
      return;
    }
    const orientation = (event: DeviceOrientationEvent): void => {
      this.observe({ kind: "phone_orientation", resource_id: id, observation: { alpha: event.alpha, beta: event.beta, gamma: event.gamma,
        absolute: event.absolute, source: event.type === "deviceorientationabsolute" ? "absolute_orientation" : "orientation",
        timestamp_ms: event.timeStamp, screen_angle_degrees: screen.orientation.angle, sample_id: null } });
    };
    const motion = (event: DeviceMotionEvent): void => {
      const acceleration = event.acceleration;
      const gravity = event.accelerationIncludingGravity;
      this.observe({ kind: "phone_motion", resource_id: id, observation: {
        acceleration: [acceleration?.x ?? null, acceleration?.y ?? null, acceleration?.z ?? null],
        acceleration_including_gravity: [gravity?.x ?? null, gravity?.y ?? null, gravity?.z ?? null], timestamp_ms: event.timeStamp, sample_id: null } });
    };
    const rotation = (): void => { this.observe({ kind: "phone_screen_rotation", resource_id: id, angle_degrees: screen.orientation.angle }); };
    const detach = (): void => {
      const failures: unknown[] = [];
      for (const remove of [() => { window.removeEventListener("deviceorientation", orientation); },
        () => { window.removeEventListener("deviceorientationabsolute", orientation); },
        () => { window.removeEventListener("devicemotion", motion); }, () => { screen.orientation.removeEventListener("change", rotation); }]) {
        try { remove(); } catch (cause) { failures.push(cause); }
      }
      if (failures.length > 0) throw new AggregateError(failures, "Phone sensor subscription cleanup failed");
    };
    let failure: string | null = null;
    try {
      this.phoneSensors.set(id, detach);
      window.addEventListener("deviceorientation", orientation);
      window.addEventListener("deviceorientationabsolute", orientation);
      if (typeof window.DeviceMotionEvent === "function") window.addEventListener("devicemotion", motion);
      screen.orientation.addEventListener("change", rotation);
    } catch (cause) {
      let result = cause;
      try { detach(); this.phoneSensors.delete(id); } catch (cleanup) { result = new AggregateError([cause, cleanup], "Phone sensor subscription failed"); }
      failure = errorMessage(result);
    }
    this.observe({ kind: "phone_sensors_started", resource_id: id, failure });
  }

  private readonly onPointerObserved = (event: PointerEvent): void => {
    if (event.pointerType === "touch") {
      const observation = this.platform();
      this.observe({ kind: "platform", observation: { ...observation,
        capabilities: { ...observation.capabilities, touch_available: true } } });
    }
  };

  private readonly onPlatform = (): void => { this.observe({ kind: "platform", observation: this.platform() }); };
  private readonly onFontLoaded = (): void => {
    if (this.fontGeneration >= 0xffff_ffff) throw new Error("Font resource generation exhausted");
    this.fontGeneration++;
    this.onPlatform();
  };
  private readonly onFocus = (): void => { this.observe({ kind: "document_focus", focused: true }); };
  private readonly onBlur = (): void => { this.observe({ kind: "document_focus", focused: false }); };
  private readonly onVisibility = (): void => { this.observe({ kind: "page_visibility", visible: !this.mount.ownerDocument.hidden }); };
  private readonly onPageHide = (event: PageTransitionEvent): void => { this.observe({ kind: "page_hide", persisted: event.persisted }); };
  private readonly onPageShow = (): void => { this.observe({ kind: "page_visibility", visible: !this.mount.ownerDocument.hidden }); };
  private readonly onKeyDown = (event: KeyboardEvent): void => {
    const target = event.target;
    if (target instanceof HTMLElement && (target.isContentEditable || target.matches("input, textarea, select"))) return;
    if (this.observe({ kind: "keyboard", code: event.code, pressed: true, repeat: event.repeat })?.keyboard_consumed === true) event.preventDefault();
  };
  private readonly onKeyUp = (event: KeyboardEvent): void => {
    if (this.observe({ kind: "keyboard", code: event.code, pressed: false, repeat: false })?.keyboard_consumed === true) event.preventDefault();
  };

  private free(): void {
    const owner = this.owner;
    this.owner = null;
    this.mount.removeEventListener("pointerdown", this.onPointerObserved, true);
    window.removeEventListener("resize", this.onPlatform);
    window.removeEventListener("keydown", this.onKeyDown);
    window.removeEventListener("keyup", this.onKeyUp);
    window.removeEventListener("focus", this.onFocus);
    window.removeEventListener("blur", this.onBlur);
    window.removeEventListener("pagehide", this.onPageHide);
    window.removeEventListener("pageshow", this.onPageShow);
    this.mount.ownerDocument.removeEventListener("visibilitychange", this.onVisibility);
    this.mount.ownerDocument.fonts.removeEventListener("loadingdone", this.onFontLoaded);
    this.documentStyle.disconnect();
    const failures: unknown[] = [];
    const sensors = [...this.phoneSensors.values()];
    this.phoneSensors.clear();
    for (const release of [...sensors, () => { this.screen.dispose(); }, () => { this.records.dispose(); }, () => { this.download.dispose(); }, () => { owner?.free(); }]) {
      try { release(); } catch (cause) { failures.push(cause); }
    }
    if (failures.length > 0) console.error(new AggregateError(failures, "Browser resource cleanup failed"));
  }
}

function errorMessage(cause: unknown): string {
  if (cause instanceof AggregateError) return `${cause.name}: ${cause.message} (${cause.errors.map(errorMessage).join("; ")})`;
  return cause instanceof Error ? `${cause.name}: ${cause.message}${cause.cause === undefined ? "" : `: ${errorMessage(cause.cause)}`}` : String(cause);
}

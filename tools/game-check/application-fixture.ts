import { ApplicationRuntimeBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { ApplicationDrawingOutput, ApplicationInput, ApplicationOutput, ApplicationPlatformObservation, ApplicationRequest,
  ApplicationTextProbe, GameUiAction, GameUiView } from "../../web/src/generated/application-contracts.js";
import { initializeWasmFixture } from "./wasm-fixture.js";

initializeWasmFixture();

export function applicationPlatformFixture(): ApplicationPlatformObservation {
  return {
    renderer: null,
    surface: { font: { family: "Arial", generation: 0, style: "normal", weight: 400 }, locale: "ja" },
    viewport: { width: 1280, height: 720, root_font_size: 16 },
    chart_extent: null,
    capabilities: {
      analysis_available: true, flight_log_available: true, hmd_available: true,
      phone_vr_available: true, replay_available: true, stored_records_available: false,
      supported_replay_cameras: ["auto", "pilot", "chase", "orbit", "platform", "shore", "overhead", "side", "front", "telephoto"]
    }
  };
}

export function bootViewFixture(): GameUiView {
  const platform = applicationPlatformFixture();
  const owner = new ApplicationRuntimeBridge(JSON.stringify(platform));
  try {
    const output = JSON.parse(owner.update(JSON.stringify({ observed_ms: 0, input: { kind: "platform", observation: platform } }),
      () => new Float64Array([12, 0, 12, 9, 3]))) as ApplicationOutput;
    if (output.drawing?.ui.kind !== "view") throw new Error("Runtime did not publish its initial structural view");
    return output.drawing.ui.view;
  } finally { owner.free(); }
}

export function readyApplicationFixture() {
  const platform: ApplicationPlatformObservation = { ...applicationPlatformFixture(), renderer: {
    resource_id: 100, initial_quality: { scale: "100", shadow: "full", water_quality: "high" },
    extent_px: [1280, 720], screen_extent_px: [1280, 720], xr_extent_px: null, gpu_timing: { kind: "available" }
  } };
  const owner = new ApplicationRuntimeBridge(JSON.stringify(platform));
  const prepared = new Map<number, Extract<ApplicationRequest, { kind: "prepare_resources" }>>();
  const requests: ApplicationRequest[] = [];
  let view: GameUiView | null = null;
  let drawing: ApplicationDrawingOutput | null = null;
  const measure = (json: string): Float64Array => {
    const request = JSON.parse(json) as ApplicationTextProbe;
    const width = Array.from(request.text).length * request.font_size_px * 0.7;
    return new Float64Array([width, 0, width, request.font_size_px * 0.8, request.font_size_px * 0.2]);
  };
  const observe = (input: ApplicationInput, now = 0): ApplicationOutput => {
    let output = JSON.parse(owner.update(JSON.stringify({ observed_ms: now, input }), measure)) as ApplicationOutput;
    if (output.drawing != null) {
      drawing = output.drawing;
      if (drawing.ui.kind === "view") view = drawing.ui.view;
    }
    for (const request of output.requests) {
      requests.push(request);
      if (request.kind === "prepare_resources") {
        prepared.set(request.request_id, request);
        output = observe({ kind: "resources", request_id: request.request_id,
          observation: { kind: "prepared", resource_id: request.request_id } }, now);
      } else if (request.kind === "commit_resources") {
        const candidate = prepared.get(request.resource_id);
        if (candidate === undefined) throw new Error("Missing mock GPU candidate");
        output = observe({ kind: "resources", request_id: request.request_id, observation: { kind: "committed", resource_id: request.resource_id,
          actual: { applied: candidate.graphics.quality, extent_px: candidate.graphics.extent_px } } }, now);
      } else if (request.kind === "release_resources") {
        prepared.delete(request.resource_id);
        output = observe({ kind: "resources_released", resource_id: request.resource_id, failure: null }, now);
      }
    }
    return output;
  };
  observe({ kind: "platform", observation: platform });
  return {
    owner, platform, requests, observe,
    get view(): GameUiView {
      if (view === null) throw new Error("Runtime has not published a View");
      return view;
    },
    get drawing(): ApplicationDrawingOutput {
      if (drawing === null) throw new Error("Runtime has not published a drawing");
      return drawing;
    },
    activate(action: GameUiAction, now = 0): ApplicationOutput {
      const control = view?.panels.flatMap(panel => panel.controls)
        .find(control => control.content.kind === "button" && JSON.stringify(control.content.action) === JSON.stringify(action));
      if (control === undefined || view === null) throw new Error(`Missing current shared control ${JSON.stringify(action)}`);
      return observe({ kind: "control", view_revision: view.view_revision, input: { type: "activate", controlId: control.id } }, now);
    }
  };
}

export function flightDrawingFixture(): { view: GameUiView; drawing: ApplicationDrawingOutput } {
  const fixture = readyApplicationFixture();
  try {
    for (const command of [{ kind: "open_setup" }, { kind: "prepare" }, { kind: "start_countdown" }] as const) {
      fixture.activate({ kind: "session", command });
    }
    for (let now = 0; now <= 3010; now += 10) fixture.observe({ kind: "frame", timestamp_ms: now, gamepads: [],
      display_resource_id: null, spatial: { gamepads: [], pointers: [], presented_id: null }, xr: null }, now);
    return { view: fixture.view, drawing: fixture.drawing };
  } finally { fixture.owner.free(); }
}

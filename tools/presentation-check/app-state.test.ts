import { describe, expect, it } from "vitest";
import { createBootViewModel } from "../../web/src/app/boot-view.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppModel } from "../../web/src/app/app-state.js";

describe("Boot application state", () => {
  it("initializes once and accepts only the matching completion", () => {
    const initial = createInitialAppModel();
    const initialized = updateApp(initial, { type: "initialize" });
    expect(initialized.model.presentation).toEqual({ type: "initializing", requestId: 1 });
    expect(initialized.effects).toEqual([{ type: "initialize-presentation", requestId: 1 }]);
    expect(updateApp(initialized.model, { type: "initialize" })).toEqual({ model: initialized.model, effects: [] });

    const stale = updateApp(initialized.model, {
      type: "presentation-initialized",
      requestId: 2,
      activeMode: "screen",
      webXrAvailable: true,
      phoneVrAvailable: false,
      status: "ready"
    });
    expect(stale.model).toBe(initialized.model);

    const completed = updateApp(initialized.model, {
      type: "presentation-initialized",
      requestId: 1,
      activeMode: "screen",
      webXrAvailable: true,
      phoneVrAvailable: false,
      status: "Screen active; WebXR supported"
    });
    expect(completed.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(createBootViewModel(completed.model).panels[0]?.controls.find((control) => control.id === "boot-enter-webxr")?.enabled).toBe(true);
  });

  it("serializes permission requests and backend changes", () => {
    const ready = readyModel();
    const requested = updateApp(ready, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
    expect(requested.model.presentation).toEqual({ type: "transitioning", requestId: 2, from: "screen", to: "webxr", phase: "requesting" });
    expect(requested.effects).toEqual([{ type: "request-permission", mode: "webxr", requestId: 2 }]);
    expect(createBootViewModel(requested.model).panels[0]?.controls.filter((control) => control.kind === "button" && control.id.startsWith("boot-enter-")).every((control) => !control.enabled)).toBe(true);

    const competing = updateApp(requested.model, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
    expect(competing.model).toBe(requested.model);
    expect(competing.effects).toHaveLength(0);

    const permitted = updateApp(requested.model, {
      type: "permission-completed", requestId: 2, mode: "webxr", ok: true, message: "granted"
    });
    expect(permitted.model.presentation).toEqual({ type: "transitioning", requestId: 2, from: "screen", to: "webxr", phase: "starting" });
    expect(permitted.effects).toEqual([{ type: "switch-backend", mode: "webxr", requestId: 2 }]);

    const active = updateApp(permitted.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: "webxr",
      ok: true, message: "", successStatus: "WebXR active"
    });
    expect(active.model.presentation).toEqual({ type: "ready", mode: "webxr" });
    expect(active.model.status).toBe("WebXR active");
  });

  it("cancels a stale successful permission result", () => {
    const ready = readyModel();
    const transition = updateApp(ready, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
    const stale = updateApp(transition.model, {
      type: "permission-completed", requestId: 1, mode: "webxr", ok: true, message: "granted"
    });
    expect(stale.model).toBe(transition.model);
    expect(stale.effects).toEqual([{ type: "cancel-pending-request", mode: "webxr" }]);
  });

  it("reports the backend actually active after a failed switch", () => {
    const ready = readyModel();
    const requested = updateApp(ready, { type: "ui-action", action: { type: "activate", controlId: "boot-enter-webxr" } });
    const starting = updateApp(requested.model, {
      type: "permission-completed", requestId: 2, mode: "webxr", ok: true, message: "granted"
    });
    const fallback = updateApp(starting.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: "screen",
      ok: false, message: "Phone startup failed", successStatus: "WebXR active"
    });
    expect(fallback.model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(fallback.model.status).toBe("Phone startup failed; Screen is active");

    const noBackend = updateApp(starting.model, {
      type: "backend-transition-completed", requestId: 2, requestedMode: "webxr", activeMode: null,
      ok: false, message: "Screen recovery failed", successStatus: "WebXR active"
    });
    expect(noBackend.model.presentation).toEqual({ type: "failed", message: "Screen recovery failed" });
  });

  it("restores Screen after an unexpected active backend end and ignores stale ends", () => {
    const active = { ...readyModel(), presentation: Object.freeze({ type: "ready" as const, mode: "webxr" as const }) };
    const recovery = updateApp(active, { type: "backend-ended", mode: "webxr", message: "Session ended" });
    expect(recovery.model.presentation).toEqual({ type: "transitioning", requestId: 2, from: "webxr", to: "screen", phase: "stopping" });
    expect(recovery.effects).toEqual([{ type: "switch-backend", mode: "screen", requestId: 2 }]);
    const screen = readyModel();
    const stale = updateApp(screen, { type: "backend-ended", mode: "webxr", message: "Late end" });
    expect(stale.model).toBe(screen);
  });

  it("cancels pending permissions and disposes presentation on page hide", () => {
    const pending = updateApp(readyModel(), { type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
    const hidden = updateApp(pending.model, { type: "page-hidden" });
    expect(hidden.model.presentation).toEqual({ type: "hidden" });
    expect(hidden.effects).toEqual([
      { type: "cancel-pending-request", mode: "webxr" },
      { type: "cancel-pending-request", mode: "phone-vr" },
      { type: "dispose-presentation" }
    ]);
    const latePermission = updateApp(hidden.model, {
      type: "permission-completed", requestId: 2, mode: "phone-vr", ok: true, message: "granted"
    });
    expect(latePermission.model).toBe(hidden.model);
    expect(latePermission.effects).toEqual([{ type: "cancel-pending-request", mode: "phone-vr" }]);
  });
});

function readyModel(): AppModel {
  const initialized = updateApp(createInitialAppModel(), { type: "initialize" });
  const ready = updateApp(initialized.model, {
    type: "presentation-initialized", requestId: 1, activeMode: "screen",
    webXrAvailable: true, phoneVrAvailable: true, status: "Screen ready"
  });
  return ready.model;
}

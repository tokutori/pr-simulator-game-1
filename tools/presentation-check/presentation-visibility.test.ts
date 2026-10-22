import { Window } from "happy-dom";
import { afterEach, describe, expect, it } from "vitest";
import { createInitialAppModel, gameSessionPhase, gameSessionSnapshot, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, AppModel, PresentationUiState } from "../../web/src/app/app-state.js";
import { createBootViewModel } from "../../web/src/app/boot-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { screenUiVisible } from "../../web/src/app/presentation-visibility.js";
import { currentFlightSnapshotFixture } from "../game-check/current-session-fixture.js";
import type { GamePhase } from "../../web/src/app/session-snapshot.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import { createFlightUiHudModel } from "../../web/src/app/flight-frame-view.js";
import { createSharedGameUiSession, currentGameSessionProjectionFixture, currentSharedGameUiFixture, sharedGameUiPlatform } from "../game-check/shared-game-ui-fixture.js";
import type { UiAction } from "../../web/src/render/contracts/ui.js";

describe("Model-derived Screen visibility", () => {
  const cases: readonly (readonly [PresentationUiState, boolean])[] = [
    [{ type: "uninitialized" }, true],
    [{ type: "initializing", requestId: 1 }, true],
    [{ type: "failed", message: "No renderer" }, true],
    [{ type: "ready", mode: "screen" }, true],
    [{ type: "ready", mode: "phone-vr" }, false],
    [{ type: "ready", mode: "webxr" }, false],
    [{ type: "transitioning", requestId: 2, origin: "user-request", from: "screen", to: "phone-vr", phase: "requesting" }, true],
    [{ type: "transitioning", requestId: 2, origin: "user-request", from: "screen", to: "webxr", phase: "starting" }, false],
    [{ type: "transitioning", requestId: 2, origin: "user-request", from: "webxr", to: "phone-vr", phase: "requesting" }, false],
    [{ type: "transitioning", requestId: 2, origin: "user-request", from: "phone-vr", to: "screen", phase: "stopping" }, false],
    [{ type: "transitioning", requestId: 2, origin: "page-restoration", from: null, to: "screen", phase: "stopping" }, false],
    [{ type: "transitioning", requestId: 2, origin: "user-request", from: null, to: "phone-vr", phase: "requesting" }, false],
    [{ type: "cached", retained: { type: "ready", mode: "screen" } }, false],
    [{ type: "cached", retained: { type: "ready", mode: "phone-vr" } }, false],
    [{ type: "hidden" }, false]
  ];

  it.each(cases)("projects %j to visible=%s", (state, visible) => {
    const before = JSON.stringify(state);
    expect(screenUiVisible(state)).toBe(visible);
    expect(JSON.stringify(state)).toBe(before);
  });
});

describe("App update to Screen DOM and HUD lifecycle", () => {
  let window: Window | null = null;
  let sharedSession: ReturnType<typeof createSharedGameUiSession> | null = null;

  afterEach(async () => {
    sharedSession?.session.dispose();
    sharedSession = null;
    await window?.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  async function harness(phase: GamePhase) {
    window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    const hudRoot = documentRef.createElement("section");
    documentRef.body.append(root, hudRoot);
    const renderedTicks: number[] = [];
    const hud = new FlightHudAdapter(hudRoot);
    const actions: UiAction[] = [];
    sharedSession = phase !== "boot" ? createSharedGameUiSession(phase) : null;
    let model = readyModel(phase, sharedSession?.session.readSharedUi(sharedSession.platform) ?? null);
    const adapter = new ScreenUiAdapter(root, (action) => {
      actions.push(action);
      send({ type: "ui-action", action });
    });
    function render(): void {
      const phase = gameSessionPhase(model.gameSession);
      if (phase !== "boot") {
        if (sharedSession === null || sharedSession.session.readLifecycle().phase !== phase) throw new Error("Expected the retained Rust scene");
        const view = sharedSession.session.readSharedUi(sharedGameUiPlatform({ notification: model.status }));
        model = updateApp(model, { type: "shared-game-ui-observed", view: { ...view,
          hud: model.sharedGameUi?.hud ?? view.hud } }).model;
      }
      const snapshot = gameSessionSnapshot(model.gameSession);
      const visible = screenUiVisible(model.presentation);
      const hudModel = createFlightUiHudModel(model);
      hud.render(hudModel);
      hud.setVisible(visible && hudModel !== null);
      if (snapshot !== null) {
        renderedTicks.push(("frame" in snapshot && snapshot.frame.kind !== "menu" ? snapshot.frame.state.tick : "tick_index" in snapshot ? snapshot.tick_index : 0));
      }
      const view = model.sharedGameUi;
      adapter.render(model.gameSession.phase === "boot" || view === null ? createBootViewModel(model) : createGameViewModel(view), visible);
    }
    function send(message: AppMessage): void {
      const transition = updateApp(model, message);
      model = transition.model;
      for (const effect of transition.effects) {
        if (effect.type !== "update-shared-game-ui") continue;
        if (sharedSession === null) throw new Error("Expected the retained Rust session");
        const shared = sharedSession.session.updateSharedUi(effect.action, sharedGameUiPlatform({ notification: model.status }));
        for (const effect of shared.effects) model = updateApp(model, { type: "shared-game-ui-effect", effect }).model;
      }
      render();
    }
    render();
    return { root, hudRoot, renderedTicks, actions, send, model: () => model };
  }

  it.each(["boot", "title", "flight_setup", "briefing_preparing", "briefing_ready", "countdown", "flight_running", "flight_paused", "result", "briefing_failed", "replay", "attract"] as const)("preserves connected DOM and HUD during stereo and cache transitions in phase %s", async (phase) => {
    const app = await harness(phase);
    const mount = required(app.root, ".screen-ui-mount");
    const shell = required(app.root, ".screen-ui-shell");
    const hudNodes = Array.from(app.hudRoot.childNodes);
    const expectsHud = (app.model().sharedGameUi?.hud ?? null) !== null;
    const assertVisibility = (visible: boolean): void => {
      expect(mount.hidden).toBe(!visible);
      expect(mount.hasAttribute("inert")).toBe(!visible);
      expect(mount.getAttribute("aria-hidden")).toBe(String(!visible));
      expect(required(app.root, ".screen-ui-mount")).toBe(mount);
      expect(required(app.root, ".screen-ui-shell")).toBe(shell);
      expect(mount.isConnected && shell.isConnected).toBe(true);
      expect(app.hudRoot.hidden).toBe(!(visible && expectsHud));
      expect(Array.from(app.hudRoot.childNodes)).toEqual(hudNodes);
      expect(hudNodes.every((node) => node.isConnected)).toBe(true);
    };

    for (const mode of ["phone-vr", "webxr"] as const) {
      app.send(phase === "boot" || phase === "title"
        ? { type: "ui-action", action: { type: "activate", controlId: `boot-enter-${mode === "webxr" ? "hmd" : mode}` } }
        : { type: "shared-game-ui-effect", effect: { kind: "platform", command: {
          kind: "set_presentation", mode: mode === "phone-vr" ? "phone_vr" : "hmd"
        } } });
      const request = app.model().presentation;
      if (request.type !== "transitioning") throw new Error("Expected permission request");
      assertVisibility(true);
      app.send({ type: "permission-completed", requestId: request.requestId, mode, ok: true, message: "Granted" });
      assertVisibility(false);
      app.send({
        type: "backend-transition-completed", requestId: request.requestId, requestedMode: mode,
        activeMode: mode, ok: true, message: "", successStatus: "Stereo"
      });
      assertVisibility(false);
      const renderCount = app.renderedTicks.length;
      app.send({ type: "game-session-status", message: "Stereo update" });
      if (expectsHud) expect(app.renderedTicks.length).toBeGreaterThan(renderCount);
      app.send({ type: "backend-ended", mode, message: "Tracking unavailable" });
      assertVisibility(false);
      completeScreenRecovery(app);
      assertVisibility(true);
    }

    app.send({ type: "page-suspended" });
    assertVisibility(false);
    app.send({ type: "page-restored" });
    expect(app.model().presentation).toMatchObject({ type: "transitioning", from: null, phase: "stopping" });
    assertVisibility(false);
    completeScreenRecovery(app);
    assertVisibility(true);
    app.send({ type: "page-hidden" });
    assertVisibility(false);
  });

  it("retains Screen during rejection and restores after startup failure", async () => {
    const app = await harness("title");
    const mount = required(app.root, ".screen-ui-mount");
    app.send({ type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
    const denied = app.model().presentation;
    if (denied.type !== "transitioning") throw new Error("Expected request");
    expect(mount.hidden).toBe(false);
    app.send({ type: "permission-completed", requestId: denied.requestId, mode: "phone-vr", ok: false, message: "Denied" });
    expect(mount.hidden).toBe(false);
    for (const activeMode of ["screen", null] as const) {
      app.send({ type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
      const request = app.model().presentation;
      if (request.type !== "transitioning") throw new Error("Expected request");
      app.send({ type: "permission-completed", requestId: request.requestId, mode: "phone-vr", ok: true, message: "Granted" });
      expect(mount.hidden).toBe(true);
      app.send({
        type: "backend-transition-completed", requestId: request.requestId, requestedMode: "phone-vr",
        activeMode, ok: false, message: "Startup failed", successStatus: ""
      });
      expect(mount.hidden).toBe(false);
      expect(app.root.textContent).toContain("Startup failed");
    }
  });

  it.each(["settings", "help"] as const)("keeps Pause %s connected through cache recovery", async (overlay) => {
    const app = await harness("flight_paused");
    required(app.root, `[data-control-id="game-pause-open-${overlay}"]`).click();
    const back = required(app.root, `[data-control-id="game-pause-${overlay}-back"]`);
    app.send({ type: "page-suspended" });
    back.click();
    expect(app.actions).toHaveLength(1);
    app.send({ type: "page-restored" });
    completeScreenRecovery(app);
    expect(required(app.root, `[data-control-id="game-pause-${overlay}-back"]`)).toBe(back);
    expect(back.isConnected).toBe(true);
    back.click();
    expect(app.actions).toHaveLength(2);
    expect(app.model().gameSession.phase).toBe("flight_paused");
    expect(app.model().sharedGameUi?.selections.pause_overlay).toBe("menu");
  });

  it("updates the connected HUD while its monocular DOM is hidden", async () => {
    const app = await harness("flight_running");
    const nodes = Array.from(app.hudRoot.childNodes);
    app.send({ type: "page-suspended" });
    const base = currentFlightSnapshotFixture();
    const snapshot = { ...base, frame: { ...base.frame, state: { ...base.frame.state, tick: 20, flight_time_s: 0.2 } } };
    app.send({ type: "game-session-synced", ...currentGameSessionProjectionFixture("flight_running"),
      phase: "flight_running", display: { kind: "available", value: snapshot } });
    const observation = currentSharedGameUiFixture("flight_running");
    if (observation.hud === null) throw new Error("Missing shared HUD");
    app.send({ type: "shared-game-ui-observed", view: { ...observation, hud: { ...observation.hud,
      groups: observation.hud.groups.map((group) => ({ ...group, nodes: group.nodes.map((node) => node.id === "hud-primary-readouts"
        ? { ...node, content: { ...node.content, value: "ALT 7.0 m" } } : node) })) } } });
    expect(app.hudRoot.hidden).toBe(true);
    expect(app.renderedTicks.at(-1)).toBe(20);
    expect(app.hudRoot.querySelector('[data-node="hud-primary-readouts"] output')?.textContent).toContain("ALT 7.0 m");
    app.send({ type: "page-restored" });
    completeScreenRecovery(app);
    expect(app.hudRoot.hidden).toBe(false);
    expect(Array.from(app.hudRoot.childNodes)).toEqual(nodes);
    expect(nodes.every((node) => node.isConnected)).toBe(true);
  });
});

function readyModel(phase: GamePhase, sharedGameUi: AppModel["sharedGameUi"]): AppModel {
  const initial = updateApp(createInitialAppModel(), { type: "initialize" }).model;
  const ready = updateApp(initial, {
    type: "presentation-initialized", requestId: 1, activeMode: "screen",
    webXrAvailable: true, phoneVrAvailable: true, status: "Ready"
  }).model;
  const gameSession = phase === "boot" ? { phase: "boot" as const } : currentGameSessionProjectionFixture(phase);
  return { ...ready, gameSession, sharedGameUi };
}

function completeScreenRecovery(app: { model(): AppModel; send(message: AppMessage): void }): void {
  const pending = app.model().presentation;
  if (pending.type !== "transitioning") throw new Error("Expected Screen recovery");
  app.send({ type: "backend-transition-completed", requestId: pending.requestId, requestedMode: "screen", activeMode: "screen", ok: true, message: "", successStatus: "Screen" });
}


function required(root: HTMLElement, selector: string): HTMLElement {
  const element = root.querySelector<HTMLElement>(selector);
  if (element === null) throw new Error(`Missing DOM element: ${selector}`);
  return element;
}

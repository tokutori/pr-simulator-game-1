import { Window } from "happy-dom";
import { afterEach, describe, expect, it } from "vitest";
import { createInitialAppModel, gameSessionSnapshot, gameSessionState, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage, AppModel, PresentationUiState } from "../../web/src/app/app-state.js";
import { createBootViewModel } from "../../web/src/app/boot-view.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { screenUiVisible } from "../../web/src/app/presentation-visibility.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { FlightHudAdapter } from "../../web/src/presentation/flight-hud.js";
import type { UiAction } from "../../web/src/render/contracts/ui.js";

describe("Model-derived Screen visibility", () => {
  const cases: readonly (readonly [PresentationUiState, boolean])[] = [
    [{ type: "uninitialized" }, true],
    [{ type: "initializing", requestId: 1 }, true],
    [{ type: "failed", message: "No renderer" }, true],
    [{ type: "ready", mode: "screen" }, true],
    [{ type: "ready", mode: "phone-vr" }, false],
    [{ type: "ready", mode: "webxr" }, false],
    [{ type: "transitioning", requestId: 2, from: "screen", to: "phone-vr", phase: "requesting" }, true],
    [{ type: "transitioning", requestId: 2, from: "screen", to: "webxr", phase: "starting" }, false],
    [{ type: "transitioning", requestId: 2, from: "webxr", to: "phone-vr", phase: "requesting" }, false],
    [{ type: "transitioning", requestId: 2, from: "phone-vr", to: "screen", phase: "stopping" }, false],
    [{ type: "transitioning", requestId: 2, from: null, to: "screen", phase: "stopping" }, false],
    [{ type: "transitioning", requestId: 2, from: null, to: "phone-vr", phase: "requesting" }, false],
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

  afterEach(async () => {
    await window?.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  async function harness(phaseCode: number) {
    window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    const hudRoot = documentRef.createElement("section");
    documentRef.body.append(root, hudRoot);
    const renderedTicks: number[] = [];
    const hud = new FlightHudAdapter(hudRoot, (snapshot) => renderedTicks.push(snapshot.tick));
    const actions: UiAction[] = [];
    let model = readyModel(phaseCode);
    const adapter = new ScreenUiAdapter(root, (action) => {
      actions.push(action);
      send({ type: "ui-action", action });
    });
    function render(): void {
      const snapshot = gameSessionSnapshot(model.gameSession);
      const visible = screenUiVisible(model.presentation);
      hud.setVisible(visible && (model.gameSession.kind === "flight" || model.gameSession.kind === "paused-flight"));
      if (snapshot !== null) hud.render(snapshot);
      adapter.render(model.gameSession.kind === "boot" ? createBootViewModel(model) : createGameViewModel(model, snapshot), visible);
    }
    function send(message: AppMessage): void {
      model = updateApp(model, message).model;
      render();
    }
    render();
    return { root, hudRoot, renderedTicks, actions, send, model: () => model };
  }

  it.each([-1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10])("preserves connected DOM and HUD during stereo and cache transitions in phase %s", async (phaseCode) => {
    const app = await harness(phaseCode);
    const mount = required(app.root, ".screen-ui-mount");
    const shell = required(app.root, ".screen-ui-shell");
    const hudNodes = Array.from(app.hudRoot.childNodes);
    const expectsHud = phaseCode === 5 || phaseCode === 6;
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
      app.send({ type: "ui-action", action: { type: "activate", controlId: `boot-enter-${mode}` } });
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
    const app = await harness(0);
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
    const app = await harness(6);
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
    expect(app.model().gameSession).toMatchObject({ kind: "paused-flight", overlay: { kind: "menu" } });
  });

  it("updates the connected HUD while its monocular DOM is hidden", async () => {
    const app = await harness(5);
    const nodes = Array.from(app.hudRoot.childNodes);
    app.send({ type: "page-suspended" });
    const values = snapshotValues();
    values[0] = 20;
    values[20] = 7;
    values[31] = 1;
    app.send({
      type: "game-session-synced", phaseCode: 5, controlModeCode: 0,
      difficulty: app.model().difficulty, configurationMetadata: null,
      countdownRemaining: 0, snapshot: parseFlightSnapshot(values), canResume: false
    });
    expect(app.hudRoot.hidden).toBe(true);
    expect(app.renderedTicks.at(-1)).toBe(20);
    expect(app.hudRoot.querySelector(".flight-hud-readouts")?.textContent).toContain("ALT 7.0 m");
    app.send({ type: "page-restored" });
    completeScreenRecovery(app);
    expect(app.hudRoot.hidden).toBe(false);
    expect(Array.from(app.hudRoot.childNodes)).toEqual(nodes);
    expect(nodes.every((node) => node.isConnected)).toBe(true);
  });
});

function readyModel(phaseCode: number): AppModel {
  const initial = updateApp(createInitialAppModel(), { type: "initialize" }).model;
  const ready = updateApp(initial, {
    type: "presentation-initialized", requestId: 1, activeMode: "screen",
    webXrAvailable: true, phoneVrAvailable: true, status: "Ready"
  }).model;
  const gameSession = gameSessionState(phaseCode, 3, phaseCode === 5 || phaseCode === 6 ? parseFlightSnapshot(snapshotValues()) : null, true);
  if (gameSession === null) throw new Error("Invalid game phase fixture");
  return { ...ready, gameSession };
}

function completeScreenRecovery(app: { model(): AppModel; send(message: AppMessage): void }): void {
  const pending = app.model().presentation;
  if (pending.type !== "transitioning") throw new Error("Expected Screen recovery");
  app.send({ type: "backend-transition-completed", requestId: pending.requestId, requestedMode: "screen", activeMode: "screen", ok: true, message: "", successStatus: "Screen" });
}

function snapshotValues(): number[] {
  const values = new Array<number>(33).fill(0);
  values[7] = 1;
  values[19] = -1;
  return values;
}

function required(root: HTMLElement, selector: string): HTMLElement {
  const element = root.querySelector<HTMLElement>(selector);
  if (element === null) throw new Error(`Missing DOM element: ${selector}`);
  return element;
}

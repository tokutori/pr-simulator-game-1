import { readFileSync } from "node:fs";
import { Window } from "happy-dom";
import { describe, expect, it, vi } from "vitest";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { createGameViewModel } from "../../web/src/app/game-view.js";
import { currentSharedGameUiFixture, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import { projectSharedFlightHudModel } from "../../web/src/presentation/flight-hud-model.js";
import { IDENTITY_POSE } from "../../web/src/render/contracts/math.js";
import { normalizedRect } from "../../web/src/render/contracts/ui.js";
import type { UiViewModel } from "../../web/src/render/contracts/ui.js";
import type { GameUiPlatformInput } from "../../web/src/generated/game-ui-input.js";

describe("screen UI DOM patching", () => {
  it.each(["", "通知"])("renders a status separator only for the nonempty label %s", async label => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    try {
      const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
      const documentRef = window.document as unknown as Document;
      const root = documentRef.createElement("main");
      documentRef.body.append(root);
      const adapter = new ScreenUiAdapter(root, () => undefined);
      const fixture = createSceneFixture("Title");
      adapter.render({ ...fixture, panels: fixture.panels.map(panel => ({ ...panel,
        sections: [{ id: "notice", label: "", layout: "column" }], controls: [{
          kind: "status", id: "notice", section: "notice", label, value: "発進準備完了", enabled: false,
          rect: normalizedRect(0.04, 0.1, 0.92, 0.1)
        }]
      })) });
      expect(required(root, "output#notice").textContent).toBe(label.length === 0 ? "発進準備完了" : "通知: 発進準備完了");
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it("retains temporarily disabled focus until the same enabled node returns without resetting user scroll", async () => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    documentRef.body.append(root);
    const adapter = new ScreenUiAdapter(root, () => undefined);
    const initial = createSceneFixture("FlightSetup");
    try {
      adapter.render(initial);
      const button = required(root, "button");
      const shell = required(root, ".screen-ui-shell");
      const pending: UiViewModel = { ...initial, panels: initial.panels.map((panel) => ({ ...panel,
        controls: panel.controls.map((control) => ({ ...control, enabled: false })) })) };
      button.focus();
      shell.scrollTop = 24;
      adapter.render(pending);
      button.removeAttribute("disabled");
      button.blur();
      button.setAttribute("disabled", "");
      const focus = vi.spyOn(button, "focus");
      adapter.render(pending);
      expect(documentRef.activeElement).toBe(documentRef.body);
      expect(focus).not.toHaveBeenCalled();
      shell.scrollTop = 77;
      adapter.render(initial);
      expect(required(root, "button")).toBe(button);
      expect(documentRef.activeElement).toBe(button);
      expect(focus).toHaveBeenCalledExactlyOnceWith({ preventScroll: true });
      expect(shell.scrollTop).toBe(77);
      adapter.render(initial);
      expect(focus).toHaveBeenCalledTimes(1);
    } finally {
      root.remove();
      adapter.render(initial);
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it.each(["external-focus", "external-focus-then-blur", "pointerdown", "keydown", "window-blur",
    "changed-scene", "changed-overlay", "replaced", "detached", "hidden"] as const)(
    "discards temporarily disabled focus after %s, including a later return to the original view", async (change) => {
      const window = new Window();
      Object.assign(globalThis, { window, document: window.document });
      const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
      const documentRef = window.document as unknown as Document;
      const root = documentRef.createElement("main");
      const external = documentRef.createElement("button");
      documentRef.body.append(root, external);
      const adapter = new ScreenUiAdapter(root, () => undefined);
      const initial = createSceneFixture("FlightSetup");
      try {
        adapter.render(initial);
        const button = required(root, "button");
        const pending: UiViewModel = { ...initial, panels: initial.panels.map((panel) => ({ ...panel,
          controls: panel.controls.map((control) => ({ ...control, enabled: false })) })) };
        button.focus();
        adapter.render(pending);
        button.removeAttribute("disabled");
        button.blur();
        button.setAttribute("disabled", "");
        const focus = vi.spyOn(button, "focus");
        const EventConstructor = window.Event as unknown as typeof Event;
        if (change === "external-focus" || change === "external-focus-then-blur") external.focus();
        if (change === "external-focus-then-blur") external.blur();
        if (change === "pointerdown") external.dispatchEvent(new EventConstructor("pointerdown", { bubbles: true }));
        if (change === "keydown") documentRef.dispatchEvent(new EventConstructor("keydown", { bubbles: true }));
        if (change === "window-blur") window.dispatchEvent(new window.Event("blur"));
        if (change === "changed-scene") adapter.render({ ...pending, scene: "Briefing" });
        if (change === "changed-overlay") adapter.render({ ...pending, activeOverlay: "settings" });
        if (change === "replaced") adapter.render({ ...pending, panels: [] });
        if (change === "detached") root.remove();
        if (change === "hidden") adapter.render(pending, false);
        adapter.render(initial);
        expect(focus).not.toHaveBeenCalled();
        expect(documentRef.activeElement).toBe(change === "external-focus" ? external : documentRef.body);
      } finally {
        root.remove();
        adapter.render(initial);
        await window.happyDOM.abort();
        Reflect.deleteProperty(globalThis, "window");
        Reflect.deleteProperty(globalThis, "document");
      }
    });

  it("suppresses hidden events and restores the connected range, focus, scroll, and current values", async () => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    documentRef.body.append(root);
    const actions: unknown[] = [];
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
    const initial = createSceneFixture("FlightSetup");
    adapter.render(initial);
    const mount = required(root, ".screen-ui-mount");
    const shell = required(root, ".screen-ui-shell");
    const range = requiredInput(root, 'input[type="range"]');
    const toggle = requiredInput(root, 'input[type="checkbox"]');
    const button = required(root, "button");
    range.focus();
    shell.scrollTop = 24;
    shell.scrollLeft = 7;
    adapter.render(initial, false);
    expect(mount.hidden).toBe(true);
    expect(mount.hasAttribute("inert")).toBe(true);
    expect(documentRef.activeElement).toBe(documentRef.body);
    const EventConstructor = window.Event as unknown as typeof Event;
    button.dispatchEvent(new EventConstructor("click", { bubbles: true }));
    range.dispatchEvent(new EventConstructor("input", { bubbles: true }));
    toggle.dispatchEvent(new EventConstructor("change", { bubbles: true }));
    expect(actions).toEqual([]);
    shell.scrollTop = 0;
    shell.scrollLeft = 0;
    const updated: UiViewModel = {
      ...initial,
      panels: initial.panels.map((panel) => ({ ...panel, controls: panel.controls.map((control) =>
        control.kind === "range" ? { ...control, value: 0.7, displayValue: "0.7" } : control) }))
    };
    adapter.render(updated, false);
    expect(range.value).toBe("0.7");
    expect(required(root, 'input[type="range"]')).toBe(range);
    expect(range.isConnected).toBe(true);
    const focus = vi.spyOn(range, "focus");
    adapter.render(updated, true);
    expect(required(root, ".screen-ui-mount")).toBe(mount);
    expect(required(root, ".screen-ui-shell")).toBe(shell);
    expect(mount.hidden).toBe(false);
    expect(mount.hasAttribute("inert")).toBe(false);
    expect(documentRef.activeElement).toBe(range);
    expect(focus).toHaveBeenCalledExactlyOnceWith({ preventScroll: true });
    expect(shell.scrollTop).toBe(24);
    expect(shell.scrollLeft).toBe(7);
    adapter.render(updated);
    expect(focus).toHaveBeenCalledTimes(1);
    button.dispatchEvent(new EventConstructor("click", { bubbles: true }));
    range.dispatchEvent(new EventConstructor("input", { bubbles: true }));
    toggle.checked = true;
    toggle.dispatchEvent(new EventConstructor("change", { bubbles: true }));
    expect(actions).toEqual([
      { type: "activate", controlId: "flightsetup-action" },
      { type: "set-range", controlId: "flightsetup-range", value: 0.7 },
      { type: "set-toggle", controlId: "flightsetup-toggle", value: true }
    ]);
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  it.each(["external-focus", "disabled", "replaced", "changed-scene", "changed-scene-roundtrip", "changed-overlay-roundtrip"] as const)("does not restore obsolete focus after %s", async (change) => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    const external = documentRef.createElement("button");
    documentRef.body.append(root, external);
    const actions: unknown[] = [];
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
    const initial = createSceneFixture("FlightSetup");
    adapter.render(initial);
    const range = requiredInput(root, 'input[type="range"]');
    range.focus();
    adapter.render(initial, false);
    let updated = initial;
    if (change === "external-focus") external.focus();
    if (change === "changed-scene") updated = createSceneFixture("Result");
    if (change === "changed-scene-roundtrip") adapter.render({ ...initial, scene: "Result" }, false);
    if (change === "changed-overlay-roundtrip") adapter.render({ ...initial, activeOverlay: "settings" }, false);
    if (change === "disabled") {
      updated = { ...initial, panels: initial.panels.map((panel) => ({
        ...panel, controls: panel.controls.map((control) => ({ ...control, enabled: false }))
      })) };
    }
    if (change === "replaced") adapter.render({ ...initial, panels: [] }, false);
    adapter.render(updated, false);
    adapter.render(updated, true);
    expect(documentRef.activeElement).toBe(change === "external-focus" ? external : documentRef.body);
    if (change === "disabled") {
      const EventConstructor = window.Event as unknown as typeof Event;
      range.dispatchEvent(new EventConstructor("input", { bubbles: true }));
      required(root, 'input[type="checkbox"]').dispatchEvent(new EventConstructor("change", { bubbles: true }));
      required(root, "button").dispatchEvent(new EventConstructor("click", { bubbles: true }));
      expect(actions).toEqual([]);
    }
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  it("preserves range input identity, focus, and scroll across model updates", async () => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    root.style.overflow = "auto";
    window.document.body.append(root as never);
    const actions: unknown[] = [];
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
    const initial = createSceneFixture("FlightSetup");
    const range = initial.panels.flatMap((panel) => panel.controls).find((control) => control.kind === "range");
    if (range?.kind !== "range") throw new Error("FlightSetup fixture has no range control");

    adapter.render(initial);
    const mount = root.querySelector(".screen-ui-mount");
    expect(mount).not.toBeNull();
    const input = root.querySelector<HTMLInputElement>(`input[data-control-id="${range.id}"]`);
    if (input === null) throw new Error(`Rendered range control is missing: ${root.innerHTML}`);
    const shell = root.querySelector<HTMLElement>(".screen-ui-shell");
    if (shell === null) throw new Error("Rendered UI shell is missing");
    input.focus();
    shell.scrollTop = 24;
    input.value = "0.6";
    const EventConstructor = window.Event as unknown as typeof Event;
    input.dispatchEvent(new EventConstructor("input", { bubbles: true }));

    const updated: UiViewModel = {
      ...initial,
      description: "Updated during flight tick",
      panels: initial.panels.map((panel) => ({
        ...panel,
        controls: panel.controls.map((control) => control.kind === "range" && control.id === range.id
          ? { ...control, value: 0.6, displayValue: "0.6" }
          : control)
      }))
    };
    adapter.render(updated);

    expect(root.querySelector(".screen-ui-mount")).toBe(mount);
    expect(root.querySelector(`input[data-control-id="${range.id}"]`)).toBe(input);
    expect(window.document.activeElement).toBe(input);
    expect(root.querySelector(".screen-ui-shell")).toBe(shell);
    expect(shell.scrollTop).toBe(24);
    expect(input.value).toBe("0.6");
    expect(actions).toEqual([{ type: "set-range", controlId: range.id, value: 0.6 }]);
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  it.each([7, 9])("projects shared initial and frame range text without rounding the seek in phase %s", async (phase) => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    documentRef.body.append(root);
    const adapter = new ScreenUiAdapter(root, () => undefined);
    const seekKind = phase === 7 ? "seek_analysis" : "seek_replay";
    const id = phase === 7 ? "game-analysis-cursor" : "game-replay-cursor";
    try {
      withSharedGameUiSession(phase, { ticks: 30, actions: [
        phase === 7 ? { kind: "set_result_tab", tab: "analysis" } : { kind: "set_replay_view", view: "telemetry" },
        { kind: seekKind, time_seconds: 0.25 }
      ] }, (session, platform) => {
        const initial = session.readSharedUi(platform);
        const revision = initial.view_revision;
        const record = session.exportRecordJson();
        const control = initial.panels.flatMap((panel) => panel.controls).find((control) => control.id === id);
        if (control?.content.kind !== "range") throw new Error("Missing shared range");
        adapter.render(createGameViewModel(initial));
        const input = requiredInput(root, `input[data-control-id="${id}"]`);
        expect(input.value).toBe("0.25");
        expect(input.step).toBe("0.01");
        expect(input.parentElement?.querySelector("span")?.textContent).toBe(`${control.label}: 0.3`);
        for (const [time_seconds, display] of [[0.24999999999999997, "0.2"], [0.25, "0.3"], [0.25000000000000006, "0.3"]] as const) {
          const action = { kind: seekKind, time_seconds } as const;
          session.updateSharedUi(action, platform);
          session.executeSharedRecordAction(action);
          const observed = session.readSharedUi(platform);
          expect(observed.view_revision).toBe(revision);
          adapter.render(createGameViewModel(observed));
          expect(requiredInput(root, `input[data-control-id="${id}"]`)).toBe(input);
          expect(Number(input.value)).toBe(time_seconds);
          expect(input.parentElement?.querySelector("span")?.textContent).toBe(`${control.label}: ${display}`);
          expect(session.exportRecordJson()).toBe(record);
        }
      });
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it.each(["Result", "Replay"] as const)("preserves ordered %s analysis layout and hidden chart updates", async (scene) => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    window.document.body.append(root as never);
    const adapter = new ScreenUiAdapter(root, () => undefined);
    const chart: UiViewModel["panels"][number]["controls"][number] = {
      ...analysisChart(),
      id: "analysis-chart",
      section: "analysis",
      cursor_x: null,
      cursor_points: [],
      enabled: false,
      rect: normalizedRect(0.08, 0.22, 0.84, 0.3)
    };
    const view: UiViewModel = {
      scene,
      title: scene,
      description: "Analysis",
      activeOverlay: null,
      headHud: { kind: "absent" },
      panels: [{
        id: "analysis",
        title: "Analysis",
        anchor: "menu",
        localPose: IDENTITY_POSE,
        size: { width: 2.4, height: 1.8 },
        sections: [{ id: "analysis", label: "", layout: "column" }],
        controls: [
          { kind: "button", id: "map", section: "analysis", label: "Map", enabled: true, rect: normalizedRect(0.08, 0.7, 0.25, 0.07) },
          chart
        ]
      }]
    };
    adapter.render(view);

    const shell = root.querySelector<HTMLElement>(".screen-ui-shell");
    const panel = root.querySelector<HTMLElement>(".screen-ui-panel");
    const button = root.querySelector<HTMLElement>('[data-control-id="map"]');
    const figure = root.querySelector<HTMLElement>(".screen-ui-chart");
    const svg = figure?.querySelector("svg");
    expect(shell?.dataset.mode).toBe("analysis");
    expect(panel?.querySelector('[data-layout="column"]')).not.toBeNull();
    expect(svg?.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(svg?.querySelector("path")?.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(svg?.querySelector("text")?.namespaceURI).toBe("http://www.w3.org/2000/svg");
    expect(button?.style.position).toBe("");
    expect(figure?.style.position).toBe("");
    expect((button?.compareDocumentPosition(figure as Node) ?? 0) & 4).toBe(4);
    const updated: UiViewModel = { ...view, panels: view.panels.map((entry) => ({
      ...entry, controls: entry.controls.map((control) => control.kind === "chart" ? { ...control, cursor_x: 0.1 } : control)
    })) };
    adapter.render(updated, false);
    expect(required(root, ".screen-ui-mount").hidden).toBe(true);
    expect(root.querySelector("svg")).toBe(svg);
    expect(svg?.isConnected).toBe(true);
    expect(svg?.querySelector('path[stroke-dasharray="7 6"]')?.getAttribute("d")).toBe("M 522 26 V 472");
    adapter.render(updated, true);
    expect(root.querySelector(".screen-ui-chart")).toBe(figure);
    expect(root.querySelector("svg")).toBe(svg);
    expect(required(root, ".screen-ui-mount").hidden).toBe(false);
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });

  it.each([
    { scene: "Result", phase: 7, width: 1280, height: 720 },
    { scene: "Result", phase: 7, width: 360, height: 300 },
    { scene: "Replay", phase: 9, width: 1280, height: 720 },
    { scene: "Replay", phase: 9, width: 360, height: 300 }
  ] as const)("projects shared $scene analysis rows and focusable controls at $width x $height", async ({ scene, phase, width, height }) => {
    const window = new Window({ width, height });
    Object.assign(globalThis, { window, document: window.document });
    try {
      const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
      const documentRef = window.document as unknown as Document;
      const stylesheet = documentRef.createElement("style");
      stylesheet.textContent = readFileSync(new URL("../../web/src/styles.css", import.meta.url), "utf8");
      documentRef.head.append(stylesheet);
      const root = documentRef.createElement("main");
      documentRef.body.append(root);
      const shared = currentSharedGameUiFixture(phase, {
        ticks: 20,
        platform: { viewport: { width, height, root_font_size: 16 } },
        actions: scene === "Result" ? [{ kind: "set_result_tab", tab: "analysis" }] : [{ kind: "set_replay_view", view: "analysis" }]
      });
      const view = createGameViewModel(shared);
      const actions: unknown[] = [];
      const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
      adapter.render(view);
      const shell = required(root, ".screen-ui-shell");
      const panel = required(root, ".screen-ui-panel");
      const sharedSections = shared.panels.flatMap((entry) => entry.sections);
      const sections = Array.from(panel.querySelectorAll<HTMLElement>(".screen-ui-section"));
      expect(shared.layout.scroll_y).toBe(true);
      expect(shared.layout.passthrough).toBe(false);
      expect(shell.style.width).toBe(`${String(width)}px`);
      expect(shell.style.maxHeight).toBe(`${String(height)}px`);
      expect(shell.style.overflowY).toBe("auto");
      expect(window.getComputedStyle(shell as never).pointerEvents).not.toBe("none");
      expect(panel.dataset.layout).toBeUndefined();
      expect(sections.map((section) => section.dataset.sectionId)).toEqual(sharedSections.map((section) => section.id));
      expect(sections.map((section) => section.dataset.layout)).toEqual(sharedSections.map((section) => section.layout));
      expect(sections[0]?.dataset.sectionId).toBe("analysis-tabs");
      expect(sections[1]?.dataset.sectionId).toBe("analysis-chart");
      expect(sections[1]?.querySelector(".screen-ui-chart")).not.toBeNull();
      expect(required(root, ".screen-ui-chart").style.position).toBe("");
      expect(required(root, ".screen-ui-range").style.height).toBe("");
      for (const section of sections) {
        const content = required(section, ".screen-ui-section-controls");
        const computed = window.getComputedStyle(content as never);
        expect(computed.display).toBe(section.dataset.layout === "row" ? "flex" : "grid");
        if (section.dataset.layout === "row") expect(computed.flexWrap).toBe("wrap");
      }
      const controls = view.panels.flatMap((entry) => entry.controls);
      const buttons = Array.from(root.querySelectorAll<HTMLButtonElement>("button[data-control-id]"));
      expect(buttons.map((button) => button.dataset.controlId)).toEqual(controls.filter((control) => control.kind === "button").map((control) => control.id));
      for (const control of controls) {
        if (control.kind !== "button") continue;
        const button = required(root, `button[data-control-id="${control.id}"]`);
        const minimumHeight = control.presentation?.kind === "action" && control.presentation.emphasis === "primary"
          ? shared.theme.primary_min_height_px : shared.theme.button_min_height_px;
        expect(window.getComputedStyle(button as never).minHeight).toBe(`${String(minimumHeight)}px`);
        expect(button.style.position).toBe("");
        if (control.enabled) {
          button.focus();
          expect(documentRef.activeElement).toBe(button);
          button.click();
        }
      }
      expect(actions).toEqual(controls.filter((control) => control.kind === "button" && control.enabled)
        .map((control) => ({ type: "activate", controlId: control.id })));
      expect(window.getComputedStyle(required(root, ".build-version") as never).position).toBe("static");
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it.each(["cinematic", "telemetry"] as const)("projects shared Replay %s as flow sections with reachable controls", async (replayView) => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    try {
      const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
      const { FlightHudAdapter } = await import("../../web/src/presentation/flight-hud.js");
      const documentRef = window.document as unknown as Document;
      const stylesheet = documentRef.createElement("style");
      stylesheet.textContent = readFileSync(new URL("../../web/src/styles.css", import.meta.url), "utf8");
      documentRef.head.append(stylesheet);
      const root = documentRef.createElement("main");
      const hudRoot = documentRef.createElement("section");
      hudRoot.className = "flight-hud";
      documentRef.body.append(root, hudRoot);
      const actions: unknown[] = [];
      const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
      const hudAdapter = new FlightHudAdapter(hudRoot);
      const EventConstructor = window.Event as unknown as typeof Event;
      for (const [width, height] of [[360, 300], [800, 600], [1280, 720], [360, 300]] as const) {
        window.happyDOM.setViewport({ width, height });
        const shared = currentSharedGameUiFixture(9, {
          ticks: 20,
          platform: { viewport: { width, height, root_font_size: 16 } },
          actions: [{ kind: "set_replay_view", view: replayView }]
        });
        const view = createGameViewModel(shared);
        adapter.render(view);
        hudAdapter.render(shared.hud === null ? null : projectSharedFlightHudModel(shared.hud, shared.theme));
        hudAdapter.setVisible(shared.hud !== null);
        const shell = required(root, ".screen-ui-shell");
        const panel = required(root, ".screen-ui-panel");
        expect(panel.dataset.layout).toBeUndefined();
        expect(shell.style.maxHeight).toBe(`${String(shared.layout.maximum_height_px)}px`);
        expect(window.getComputedStyle(shell as never).overflowY).toBe("auto");
        expect(panel.querySelector('[data-layout="normalized"]')).toBeNull();
        const sections = Array.from(panel.querySelectorAll<HTMLElement>(".screen-ui-section"));
        expect(sections.map((section) => section.dataset.sectionId)).toEqual(shared.panels[0]?.sections.map((section) => section.id));
        const navigation = required(panel, '[data-section-id="replay-actions"] > .screen-ui-section-controls');
        expect(window.getComputedStyle(navigation as never).display).toBe("flex");
        expect(window.getComputedStyle(navigation as never).flexWrap).toBe("wrap");
        if (replayView === "telemetry") {
          if (shared.hud === null) throw new Error("Replay Telemetry must retain its shared HUD");
          expect(hudRoot.hidden).toBe(false);
          expect(hudRoot.style.maxHeight).toBe(`${String(shared.hud.layout.maximum_height_px)}px`);
          if (width === 360) {
            expect(shell.dataset.shell).toBe("flight_bar");
            expect(window.getComputedStyle(shell as never).position).toBe("fixed");
            expect(window.getComputedStyle(shell as never).bottom).toBe("0px");
            expect(window.getComputedStyle(shell as never).right).toBe("0px");
            expect(shared.hud.layout.offset_px[1] + shared.hud.layout.maximum_height_px)
              .toBeLessThan(height - shared.layout.maximum_height_px);
          } else {
            expect(shell.dataset.shell).toBe("cinematic");
            expect(shared.hud.layout.offset_px[0] + shared.hud.layout.width_px)
              .toBeLessThan(width - 12 - shared.layout.width_px);
          }
        } else {
          expect(shell.dataset.shell).toBe("cinematic");
          expect(hudRoot.hidden).toBe(true);
        }
        actions.length = 0;
        const buttons = Array.from(panel.querySelectorAll<HTMLButtonElement>("button[data-control-id]"));
        for (const button of buttons) {
          expect(button.style.position).toBe("");
          expect(window.getComputedStyle(button as never).minHeight).toBe(`${String(shared.theme.button_min_height_px)}px`);
          button.focus();
          expect(documentRef.activeElement).toBe(button);
          button.click();
        }
        expect(actions).toEqual(buttons.map((button) => ({ type: "activate", controlId: button.dataset.controlId })));
        if (replayView === "telemetry") {
          const cursor = requiredInput(panel, 'input[data-control-id="game-replay-cursor"]');
          expect(cursor.style.position).toBe("");
          const position = (Number(cursor.min) + Number(cursor.max)) / 2;
          cursor.value = String(position);
          cursor.dispatchEvent(new EventConstructor("input", { bubbles: true }));
          expect(actions.at(-1)).toEqual({ type: "set-range", controlId: "game-replay-cursor", value: position });
        } else expect(panel.querySelector('[data-control-id="game-replay-cursor"]')).toBeNull();
      }
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it.each([false, true])("uses Result Summary flow controls with technical disclosure %s", async (technicalOpen) => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    try {
      const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
      const documentRef = window.document as unknown as Document;
      const stylesheet = documentRef.createElement("style");
      stylesheet.textContent = readFileSync(new URL("../../web/src/styles.css", import.meta.url), "utf8");
      documentRef.head.append(stylesheet);
      const root = documentRef.createElement("main");
      documentRef.body.append(root);
      const actions: unknown[] = [];
      const adapter = new ScreenUiAdapter(root, action => actions.push(action));
      for (const [width, height] of [[1280, 720], [360, 300], [1280, 720]] as const) {
        window.happyDOM.setViewport({ width, height });
        const shared = currentSharedGameUiFixture(7, {
          ticks: 20,
          platform: { viewport: { width, height, root_font_size: 16 } },
          actions: technicalOpen ? [{ kind: "toggle_technical" }] : []
        });
        adapter.render(createGameViewModel(shared));
        const panel = required(root, ".screen-ui-panel");
        expect(panel.dataset.layout).toBeUndefined();
        expect(panel.querySelector('[data-layout="normalized"]')).toBeNull();
        expect(Array.from(panel.querySelectorAll<HTMLElement>(".screen-ui-section")).map(section => section.dataset.sectionId))
          .toEqual(shared.panels[0]?.sections.map(section => section.id));
        actions.length = 0;
        const buttons = Array.from(panel.querySelectorAll<HTMLButtonElement>("button[data-control-id]:enabled"));
        for (const button of buttons) {
          expect(button.style.position).toBe("");
          expect(window.getComputedStyle(button as never).minHeight).toBe(`${String(shared.theme.button_min_height_px)}px`);
          button.focus();
          expect(documentRef.activeElement).toBe(button);
          button.click();
        }
        expect(actions).toEqual(buttons.map(button => ({ type: "activate", controlId: button.dataset.controlId })));
      }
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it("projects Screen Attract flow information and retains the reachable Return after resize", async () => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    try {
      const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
      const documentRef = window.document as unknown as Document;
      const stylesheet = documentRef.createElement("style");
      stylesheet.textContent = readFileSync(new URL("../../web/src/styles.css", import.meta.url), "utf8");
      documentRef.head.append(stylesheet);
      const root = documentRef.createElement("main");
      documentRef.body.append(root);
      const actions: unknown[] = [];
      const adapter = new ScreenUiAdapter(root, action => actions.push(action));
      let returnButton: HTMLButtonElement | null = null;
      withSharedGameUiSession(10, {}, (session, platform) => {
        const clock = session.readPlaybackClock();
        for (const [width, height] of [[1280, 720], [360, 300], [1280, 720]] as const) {
          window.happyDOM.setViewport({ width, height });
          const shared = session.readSharedUi({ ...platform, viewport: { width, height, root_font_size: 16 } });
          adapter.render(createGameViewModel(shared));
          const shell = required(root, ".screen-ui-shell");
          const panel = required(root, ".screen-ui-panel");
          expect(shell.dataset.scene).toBe("Title");
          expect(shell.style.overflowY).toBe("auto");
          expect(shell.style.maxHeight).toBe(`${String(shared.layout.maximum_height_px)}px`);
          expect(panel.dataset.layout).toBeUndefined();
          expect(panel.querySelector('[data-layout="normalized"]')).toBeNull();
          expect(Array.from(panel.querySelectorAll<HTMLElement>(".screen-ui-section")).map(section => section.dataset.sectionId))
            .toEqual(shared.panels[0]?.sections.map(section => section.id));
          for (const control of shared.panels[0]?.controls ?? []) {
            if (control.content.kind !== "status") continue;
            const status = required(root, `output#${control.id}`);
            expect(status.style.position).toBe("");
            expect(status.style.height).toBe("");
            expect(status.textContent).toBe(control.label.length === 0 ? control.content.value : `${control.label}: ${control.content.value}`);
          }
          const button = required(root, 'button[data-control-id="game-attract-return"]') as HTMLButtonElement;
          expect(button.disabled).toBe(false);
          expect(button.style.position).toBe("");
          expect(window.getComputedStyle(button as never).minHeight).toBe(`${String(shared.theme.button_min_height_px)}px`);
          if (returnButton !== null) {
            expect(button).toBe(returnButton);
            expect(documentRef.activeElement).toBe(button);
          }
          returnButton = button;
          button.focus();
          actions.length = 0;
          button.click();
          expect(actions).toEqual([{ type: "activate", controlId: "game-attract-return" }]);
          expect(session.readPlaybackClock()).toEqual(clock);
        }
      });
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it.each(["menu", "settings", "help"] as const)("projects bounded Pause %s rows and the same HUD reservation after resize", async (overlay) => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    try {
      const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
      const { FlightHudAdapter } = await import("../../web/src/presentation/flight-hud.js");
      const documentRef = window.document as unknown as Document;
      const stylesheet = documentRef.createElement("style");
      stylesheet.textContent = readFileSync(new URL("../../web/src/styles.css", import.meta.url), "utf8");
      documentRef.head.append(stylesheet);
      const root = documentRef.createElement("main");
      const hudRoot = documentRef.createElement("section");
      hudRoot.className = "flight-hud";
      documentRef.body.append(root, hudRoot);
      const actions: unknown[] = [];
      const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
      const hudAdapter = new FlightHudAdapter(hudRoot);
      const hudNodes = Array.from(hudRoot.childNodes);
      for (const informationCode of [0, 2] as const) {
        for (const [width, height] of [[360, 300], [1280, 720], [360, 300], [800, 600]] as const) {
          window.happyDOM.setViewport({ width, height });
          const shared = currentSharedGameUiFixture(6, {
            informationCode,
            platform: { viewport: { width, height, root_font_size: 16 } },
            ...(overlay === "menu" ? {} : { actions: [{ kind: "set_pause_overlay", overlay }] })
          });
          if (shared.hud === null) throw new Error("Paused Screen must retain its shared HUD");
          adapter.render(createGameViewModel(shared));
          hudAdapter.render(projectSharedFlightHudModel(shared.hud, shared.theme));
          hudAdapter.setVisible(true);
          for (const control of shared.panels.flatMap(panel => panel.controls)) {
            if (control.content.kind !== "status") continue;
            expect(required(root, `output#${control.id}`).textContent)
              .toBe(control.label.length === 0 ? control.content.value : `${control.label}: ${control.content.value}`);
          }
          const shell = required(root, ".screen-ui-shell");
          const panel = required(root, ".screen-ui-panel");
          expect(panel.dataset.layout).toBeUndefined();
          expect(shell.dataset.passthrough).toBe("false");
          expect(shell.style.maxHeight).toBe(`${String(shared.layout.maximum_height_px)}px`);
          expect(shell.style.overflowY).toBe("auto");
          expect(window.getComputedStyle(shell as never).pointerEvents).toBe("auto");
          expect(window.getComputedStyle(hudRoot as never).pointerEvents).toBe("auto");
          expect(hudRoot.style.maxHeight).toBe(`${String(shared.hud.layout.maximum_height_px)}px`);
          expect(hudRoot.querySelector('[data-node="hud-primary-readouts"] .flight-hud-caption')?.textContent).toBe("一時停止");
          expect(Array.from(hudRoot.childNodes)).toEqual(hudNodes);
          const sections = Array.from(panel.querySelectorAll<HTMLElement>(".screen-ui-section"));
          expect(sections.map((section) => section.dataset.sectionId)).toEqual(shared.panels[0]?.sections.map((section) => section.id));
          const navigation = required(panel, '[data-section-id="flight-actions"] > .screen-ui-section-controls');
          expect(window.getComputedStyle(navigation as never).display).toBe("flex");
          expect(window.getComputedStyle(navigation as never).flexWrap).toBe("wrap");
          const computed = window.getComputedStyle(shell as never);
          if (shared.layout.shell === "flight_bar") {
            expect(computed.bottom).toBe("0px");
            expect(computed.right).toBe("0px");
            expect(shared.hud.layout.offset_px[1] + shared.hud.layout.maximum_height_px)
              .toBeLessThan(height - shared.layout.maximum_height_px);
          } else {
            expect(shared.layout.shell).toBe("pause_settings");
            expect(shared.hud.layout.offset_px[0] + shared.hud.layout.width_px)
              .toBeLessThan(width - 12 - shared.layout.width_px);
          }
          actions.length = 0;
          const buttons = Array.from(panel.querySelectorAll<HTMLButtonElement>("button[data-control-id]"));
          for (const button of buttons) {
            expect(button.style.position).toBe("");
            expect(window.getComputedStyle(button as never).minHeight).toBe(`${String(shared.theme.button_min_height_px)}px`);
            button.focus();
            expect(documentRef.activeElement).toBe(button);
            button.click();
          }
          expect(actions).toEqual(buttons.map((button) => ({ type: "activate", controlId: button.dataset.controlId })));
        }
      }
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window");
      Reflect.deleteProperty(globalThis, "document");
    }
  });

  it("patches measured chart pixels once and releases hidden or removed observation targets", async () => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    const observers: TestResizeObserver[] = [];
    class TestResizeObserver implements ResizeObserver {
      readonly targets = new Set<Element>();
      constructor(readonly callback: ResizeObserverCallback) { observers.push(this); }
      observe(target: Element): void { this.targets.add(target); }
      unobserve(target: Element): void { this.targets.delete(target); }
      disconnect(): void { this.targets.clear(); }
      emit(target: Element, width: number, height: number): void {
        this.callback([{
          target,
          contentRect: { x: 0, y: 0, width, height, top: 0, left: 0, right: width, bottom: height, toJSON: () => ({ width, height }) },
          borderBoxSize: [], contentBoxSize: [], devicePixelContentBoxSize: []
        }], this);
      }
    }
    Object.defineProperty(window, "ResizeObserver", { value: TestResizeObserver });
    const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
    const documentRef = window.document as unknown as Document;
    const root = documentRef.createElement("main");
    documentRef.body.append(root);
    const actions: unknown[] = [];
    let view: UiViewModel;
    const adapter = new ScreenUiAdapter(root, (action) => actions.push(action), () => {
      view = { ...view, panels: view.panels.map((panel) => ({ ...panel, controls: panel.controls.map((control) =>
        control.kind === "chart" ? { ...analysisChart(adapter.chartExtent()), id: control.id,
          ...(control.section === undefined ? {} : { section: control.section }), rect: control.rect, cursor_x: 0.1 } : control) })) };
      adapter.render(view);
    });
    const fixture = createSceneFixture("Result");
    view = {
      ...fixture,
      panels: fixture.panels.map((panel) => ({ ...panel, sections: [{ id: "measured", label: "", layout: "column" }], controls: [
        ...panel.controls.map((control) => ({ ...control, section: "measured" })),
        { kind: "status", id: "long-notice", section: "measured", label: "通知", value: "元のfailure原因\n2行目\n3行目\n4行目", enabled: false, rect: normalizedRect(0.04, 0.72, 0.92, 0.085) },
        {
          ...analysisChart(), id: "measured-chart", section: "measured", cursor_x: 0.1,
          enabled: false, rect: normalizedRect(0.04, 0.105, 0.92, 0.49)
        }
      ] }))
    };
    adapter.render(view);
    const viewport = required(root, ".screen-ui-chart-viewport");
    const svg = root.querySelector("svg");
    const observer = observers[0];
    if (svg === null || observer === undefined) throw new Error("The measured chart or its observer is missing");
    const render = vi.spyOn(adapter, "render");
    observer.emit(viewport, 139, 160);
    observer.emit(viewport, 298, 99);
    observer.emit(viewport, Number.NaN, 160);
    observer.emit(documentRef.createElement("div"), 298, 160);
    expect(adapter.chartExtent()).toBeNull();
    expect(render).not.toHaveBeenCalled();
    observer.emit(viewport, 298, 160);
    expect(adapter.chartExtent()).toEqual({ width: 298, height: 160 });
    expect(svg.getAttribute("viewBox")).toBe("0 0 298 160");
    expect(svg.style.getPropertyValue("--screen-chart-tick-font")).toBe("14px");
    expect(svg.querySelector('path[stroke-dasharray="7 6"]')?.getAttribute("d")).toBe("M 171 26 V 112");
    expect(svg.querySelector(".screen-ui-chart-axis")?.textContent).toBe("Time (s)");
    const notice = required(root, "#long-notice");
    expect(notice.style.position).toBe("");
    expect(notice.style.maxHeight).toBe("");
    expect(notice.textContent).toBe("通知: 元のfailure原因\n2行目\n3行目\n4行目");
    expect(root.querySelector("svg")).toBe(svg);
    expect(required(root, ".screen-ui-chart").style.overflow).toBe("auto");
    const sharedChart = view.panels.flatMap((panel) => panel.controls).find((control) => control.kind === "chart");
    if (sharedChart?.kind !== "chart") throw new Error("Missing shared cursor colours");
    for (const cursor of sharedChart.cursor_points) expect(svg.querySelector(`circle[fill="${cursor.color}"]`)).not.toBeNull();
    expect(render).toHaveBeenCalledTimes(1);
    observer.emit(viewport, 298, 160);
    expect(render).toHaveBeenCalledTimes(1);
    expect(actions).toEqual([]);
    adapter.render(view, false);
    expect(adapter.chartExtent()).toBeNull();
    expect(observer.targets.size).toBe(0);
    observer.emit(viewport, 400, 180);
    expect(svg.getAttribute("viewBox")).toBe("0 0 298 160");
    adapter.render(view, true);
    expect(observer.targets.has(viewport)).toBe(true);
    adapter.render({ ...view, panels: [] });
    expect(adapter.chartExtent()).toBeNull();
    expect(observer.targets.size).toBe(0);
    observer.emit(viewport, 400, 180);
    expect(root.querySelector("svg")).toBeNull();
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  });
});

function analysisChart(chart_extent: GameUiPlatformInput["chart_extent"] = null): Extract<UiViewModel["panels"][number]["controls"][number], { kind: "chart" }> {
  const view = createGameViewModel(currentSharedGameUiFixture(7, { ticks: 20, platform: { chart_extent }, actions: [
    { kind: "set_result_tab", tab: "analysis" }, { kind: "set_analysis_chart", chart: "altitude" }
  ] }));
  const chart = view.panels.flatMap((panel) => panel.controls).find((control) => control.kind === "chart");
  if (chart?.kind !== "chart") throw new Error("Missing current shared chart fixture");
  return chart;
}

function required(root: HTMLElement, selector: string): HTMLElement {
  const element = root.querySelector<HTMLElement>(selector);
  if (element === null) throw new Error(`Missing DOM element: ${selector}`);
  return element;
}

function requiredInput(root: HTMLElement, selector: string): HTMLInputElement {
  const input = root.querySelector<HTMLInputElement>(selector);
  if (input === null) throw new Error(`Missing input: ${selector}`);
  return input;
}

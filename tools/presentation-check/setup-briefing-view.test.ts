import { Window } from "happy-dom";
import { describe, expect, it } from "vitest";
import { currentSharedGameUiFixture, withSharedGameUiSession } from "../game-check/shared-game-ui-fixture.js";
import { createGameViewModel, sharedGameUiAction } from "../../web/src/app/game-view.js";
import { requestMenuLayout, layoutMeasuredMenu, menuViewport, menuRectInViewport } from "../../web/src/presentation/menu-layout.js";
import type { MenuTextMeasurement, MenuTextRequest } from "../../web/src/render/contracts/menu-layout.js";
import { uiButtonLabel, uiControlBackground, validateUiViewModel } from "../../web/src/render/contracts/ui.js";
import type { UiViewModel } from "../../web/src/render/contracts/ui.js";

function statusValue(view: UiViewModel, id: string): string {
  const control = view.panels.flatMap((panel) => panel.controls).find((entry) => entry.id === id);
  if (control?.kind !== "status") throw new Error(`Missing status ${id}`);
  return control.value;
}

function measurementFor(request: MenuTextRequest): MenuTextMeasurement {
  const characterWidth = request.fontMeters * 0.55;
  const columns = Math.max(1, Math.floor(request.widthMeters / characterWidth));
  const lines = request.value.split("\n").flatMap((paragraph, paragraphIndex) => {
    const characters = Array.from(paragraph);
    const chunks = characters.length === 0 ? [""] : Array.from({ length: Math.ceil(characters.length / columns) },
      (_slot, index) => characters.slice(index * columns, (index + 1) * columns).join(""));
    return chunks.map((value) => ({ value, paragraphIndex, advanceMeters: Array.from(value).length * characterWidth,
      leftMeters: 0, rightMeters: Array.from(value).length * characterWidth,
      ascentMeters: value.trim() === "" ? 0 : request.fontMeters * 0.65,
      descentMeters: value.trim() === "" ? 0 : request.fontMeters * 0.15 }));
  });
  return { identity: request.identity, lines };
}

describe("Shared Flight Setup and Briefing presentation", () => {
  it.each(["screen", "phone_vr", "webxr"] as const)("opens and closes the same Credits without moving the session in %s", (presentation_mode) => {
    for (const phase of [0, 6]) {
      withSharedGameUiSession(phase, { platform: { presentation_mode } }, (session, platform) => {
        const before = session.readGameSessionProjection();
        const normal = session.readSharedUi(platform);
        const openAction = sharedGameUiAction(normal, { type: "activate", controlId: "game-credits-open" });
        expect(openAction).toEqual({ kind: "set_credits", open: true });
        expect(session.updateSharedUi({ kind: "set_credits", open: true }, platform).effects).toEqual([]);
        const credits = session.readSharedUi(platform);
        const view = createGameViewModel(credits);
        validateUiViewModel(view);
        expect(credits.hud).toBeNull();
        for (const source of ["osm", "jaxa", "copernicus", "jma", "font", "earcut", "ryu", "validators", "time"]) {
          expect(statusValue(view, `game-credits-${source}`).length).toBeGreaterThan(0);
        }
        expect(sharedGameUiAction(credits, { type: "activate", controlId: "game-credits-close" })).toEqual({ kind: "set_credits", open: false });
        expect(session.updateSharedUi({ kind: "set_credits", open: false }, platform).effects).toEqual([]);
        expect(session.readGameSessionProjection()).toEqual(before);
        expect(session.readSharedUi(platform).selections.credits_open).toBe(false);
      });
    }
  });

  it.each(["screen", "phone_vr", "webxr"] as const)("projects the same direct candidates, groups and Weather in %s", (presentation_mode) => {
    const shared = currentSharedGameUiFixture(1, { platform: { presentation_mode } });
    const view = createGameViewModel(shared);
    validateUiViewModel(view);
    const controls = view.panels.flatMap((panel) => panel.controls);
    const choices = controls.filter((control) => control.kind === "button" && control.presentation?.kind === "choice");
    expect(choices.filter((control) => control.id.startsWith("game-setup-select-"))).toHaveLength(18);
    expect(choices.filter((control) => control.id.startsWith("presentation-water-quality-")).map((control) => control.label)).toEqual(["Low", "Medium", "High"]);
    const scales = choices.filter((control) => control.id.startsWith("presentation-render-scale-"));
    expect(scales.map((control) => ({ id: control.id, label: control.label, enabled: control.enabled,
      selected: control.kind === "button" && control.presentation?.kind === "choice" && control.presentation.selected }))).toEqual(presentation_mode === "screen" ? [
      { id: "presentation-render-scale-auto", label: "Auto", enabled: true, selected: false },
      { id: "presentation-render-scale-100", label: "100%", enabled: true, selected: true },
      { id: "presentation-render-scale-75", label: "75%", enabled: true, selected: false },
      { id: "presentation-render-scale-50", label: "50%", enabled: true, selected: false }
    ] : []);
    expect(controls.some((control) => control.id === "presentation-render-scale-status")).toBe(presentation_mode === "screen");
    expect(controls.map((control) => control.id)).not.toContain("game-state");
    for (const control of choices) {
      if (control.kind !== "button" || control.presentation?.kind !== "choice") continue;
      expect(Array.from(control.presentation.description).length).toBeLessThanOrEqual(30);
      if (!control.presentation.selected) expect(control.presentation.description).toBe("");
      expect(sharedGameUiAction(shared, { type: "activate", controlId: control.id })).not.toBeNull();
    }
    expect(controls.find((control) => control.id === "game-setup-start")).toMatchObject({
      label: "飛行準備へ進む", presentation: { kind: "action", emphasis: "primary" }
    });
    expect(controls.find((control) => control.id === "game-setup-back")).toMatchObject({ presentation: { emphasis: "secondary" } });
    const source = shared.panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-weather-conditions");
    if (source?.content.kind !== "status") throw new Error("Missing Rust Weather");
    expect(statusValue(view, source.id)).toBe(source.content.value);
    expect(source.content.value).toContain("m/s");
    expect(source.content.value).toContain("水面から");
    expect(source.content.value).not.toMatch(/N\/E\/D|未定義|Seed:/);
  });

  it("projects the Rust-owned closed and expanded technical disclosure", () => {
    const closed = currentSharedGameUiFixture(3);
    const open = currentSharedGameUiFixture(3, { actions: [{ kind: "toggle_technical" }] });
    const closedView = createGameViewModel(closed);
    const openView = createGameViewModel(open);
    expect(JSON.stringify(closedView)).not.toContain("Seed:");
    expect(statusValue(openView, "game-briefing-technical-content")).toContain("Seed:");
    expect(sharedGameUiAction(closed, { type: "activate", controlId: "game-briefing-technical" })).toEqual({ kind: "toggle_technical" });
    expect(openView.panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-briefing-start"))
      .toMatchObject({ label: "発進カウントダウンを開始", presentation: { emphasis: "primary" } });
    expect(closed.selections.technical_open).toBe(false);
    expect(open.selections.technical_open).toBe(true);
  });

  it.each([1, 2, 3, 4, 8])("preserves preparation phase %i and explicit operation admission", (phase) => {
    const shared = currentSharedGameUiFixture(phase);
    const view = createGameViewModel(shared);
    expect(view.panels.flatMap((panel) => panel.controls.map((control) => control.id))).not.toContain("game-state");
    const pending = currentSharedGameUiFixture(phase, { platform: { operation_pending: true, notification: "準備条件を確認する。" } });
    expect(createGameViewModel(pending).panels.flatMap((panel) => panel.controls).filter((control) => control.kind !== "status").every((control) => !control.enabled)).toBe(true);
    expect(shared.scene).toBe(phase === 1 ? "FlightSetup" : phase === 4 ? "Countdown" : "Briefing");
  });

  it.each([1, 3])("keeps primary, back and disclosure reachable in a short portrait VR menu in phase %i", (phase) => {
    const shared = currentSharedGameUiFixture(phase, { platform: { presentation_mode: "phone_vr" },
      actions: phase === 3 ? [{ kind: "toggle_technical" }] : [] });
    const view = createGameViewModel(shared);
    const panel = view.panels[0];
    if (panel === undefined) throw new Error("Missing panel");
    const request = requestMenuLayout(panel, { width: 0.5, height: 0.6 }, {
      theme: shared.theme, fontMeters: 0.04, font: { family: "system-ui, sans-serif", weight: 600, style: "normal", generation: 0 },
      locale: "ja", previousLabel: "前頁", nextLabel: "次頁", caption: view.description
    });
    const result = layoutMeasuredMenu(request, request.texts.map(measurementFor));
    if (result.kind !== "ready") throw new Error(result.reason);
    const initial = menuViewport(result.document, 0);
    expect(initial.maximumOffsetMeters).toBeGreaterThan(0);
    const ids = phase === 1 ? ["game-setup-start", "game-setup-back"]
      : ["game-briefing-start", "game-briefing-cancel", "game-briefing-technical"];
    for (const id of ids) {
      const control = result.document.controls.find((control) => control.control.id === id);
      if (control === undefined) throw new Error(`Missing control ${id}`);
      const progress = Math.min(1, Math.max(0, control.bounds.y / initial.maximumOffsetMeters));
      expect(menuRectInViewport(menuViewport(result.document, progress), control.bounds).kind, id).toBe("visible");
    }
  });

  it("applies shared layout, selection and disclosure while preserving keyed DOM, focus and scroll", async () => {
    const window = new Window({ width: 320, height: 240 });
    Object.assign(globalThis, { window, document: window.document });
    try {
      const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
      const documentRef = window.document as unknown as Document;
      const root = documentRef.createElement("main");
      documentRef.body.append(root);
      const actions: unknown[] = [];
      const adapter = new ScreenUiAdapter(root, (action) => actions.push(action));
      const platform = { viewport: { width: 320, height: 240, root_font_size: 16 } };
      const initial = currentSharedGameUiFixture(1, { platform });
      adapter.render(createGameViewModel(initial));
      for (const group of ["preset", "information", "assistance", "weather", "water-quality", "render-scale"]) {
        expect(root.querySelector(`[data-section-id="${group}"] button`)?.getAttribute("disabled")).toBeNull();
        expect(root.querySelector(`[data-section-id="${group}"] legend`)?.textContent.length).toBeGreaterThan(0);
      }
      expect([...root.querySelectorAll('[data-section-id="render-scale"] button')].map((button) => ({
        id: button.getAttribute("data-control-id"), label: button.querySelector(".screen-ui-choice-label")?.textContent,
        selected: button.getAttribute("aria-pressed")
      }))).toEqual([
        { id: "presentation-render-scale-auto", label: "Auto", selected: "false" },
        { id: "presentation-render-scale-100", label: "● 100%", selected: "true" },
        { id: "presentation-render-scale-75", label: "75%", selected: "false" },
        { id: "presentation-render-scale-50", label: "50%", selected: "false" }
      ]);
      const selected = root.querySelector<HTMLButtonElement>('[data-control-id="game-setup-select-information-0"]');
      const shell = root.querySelector<HTMLElement>(".screen-ui-shell");
      if (selected === null || shell === null) throw new Error("Missing Setup DOM");
      selected.focus(); shell.scrollTop = 40;
      const updated = currentSharedGameUiFixture(1, { platform, informationCode: 2 });
      adapter.render(createGameViewModel(updated));
      expect(root.querySelector('[data-control-id="game-setup-select-information-0"]')).toBe(selected);
      expect(documentRef.activeElement).toBe(selected);
      expect(shell.scrollTop).toBe(40);
      expect(shell.style.width).toBe(`${String(updated.layout.width_px)}px`);
      expect(shell.style.maxHeight).toBe(`${String(updated.layout.maximum_height_px)}px`);
      expect(shell.style.overflowY).toBe(updated.layout.scroll_y ? "auto" : "hidden");
      expect(shell.style.getPropertyValue("--game-ui-columns")).toBe(String(updated.layout.choice_columns));
      expect(selected.getAttribute("aria-pressed")).toBe("false");
      expect(root.querySelector('[data-control-id="game-setup-select-information-2"]')?.getAttribute("aria-pressed")).toBe("true");
      selected.click();
      expect(actions).toEqual([{ type: "activate", controlId: selected.dataset.controlId }]);
      expect(root.querySelector('[data-control-id="game-setup-start"]')?.getAttribute("data-emphasis")).toBe("primary");
      expect(root.querySelector('[data-control-id="game-setup-back"]')?.getAttribute("data-emphasis")).toBe("secondary");
      adapter.render(createGameViewModel(currentSharedGameUiFixture(3)));
      const disclosure = root.querySelector<HTMLButtonElement>('[data-control-id="game-briefing-technical"]');
      if (disclosure === null) throw new Error("Missing disclosure");
      disclosure.focus();
      adapter.render(createGameViewModel(currentSharedGameUiFixture(3, { actions: [{ kind: "toggle_technical" }] })));
      expect(root.querySelector('[data-control-id="game-briefing-technical"]')).toBe(disclosure);
      expect(documentRef.activeElement).toBe(disclosure);
      expect(disclosure.getAttribute("aria-expanded")).toBe("true");
      expect(root.querySelector("#game-briefing-technical-content")?.textContent).toContain("Seed:");
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window"); Reflect.deleteProperty(globalThis, "document");
    }
  });

  it.each([
    { phase: 3, disclosureId: "game-briefing-technical" },
    { phase: 7, disclosureId: "game-result-technical" }
  ])("links $disclosureId to its rendered content and preserves focus across disclosure updates", async ({ phase, disclosureId }) => {
    const window = new Window();
    Object.assign(globalThis, { window, document: window.document });
    try {
      const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
      const documentRef = window.document as unknown as Document;
      const root = documentRef.createElement("main");
      documentRef.body.append(root);
      const adapter = new ScreenUiAdapter(root, () => undefined);
      const contentId = `${disclosureId}-content`;
      adapter.render(createGameViewModel(currentSharedGameUiFixture(phase)));
      const disclosure = root.querySelector<HTMLButtonElement>(`[data-control-id="${disclosureId}"]`);
      if (disclosure === null) throw new Error(`Missing disclosure ${disclosureId}`);
      expect(disclosure.getAttribute("aria-controls")).toBe(contentId);
      expect(disclosure.getAttribute("aria-expanded")).toBe("false");
      expect(documentRef.getElementById(contentId)).toBeNull();
      disclosure.focus();
      for (const expanded of [true, false]) {
        adapter.render(createGameViewModel(currentSharedGameUiFixture(phase, {
          actions: expanded ? [{ kind: "toggle_technical" }] : []
        })));
        expect(root.querySelector(`[data-control-id="${disclosureId}"]`)).toBe(disclosure);
        expect(documentRef.activeElement).toBe(disclosure);
        expect(disclosure.getAttribute("aria-controls")).toBe(contentId);
        expect(disclosure.getAttribute("aria-expanded")).toBe(String(expanded));
        const content = documentRef.getElementById(contentId);
        if (expanded) expect(content?.textContent).toContain("Seed:");
        else expect(content).toBeNull();
      }
    } finally {
      await window.happyDOM.abort();
      Reflect.deleteProperty(globalThis, "window"); Reflect.deleteProperty(globalThis, "document");
    }
  });

  it("uses common selected markers and primary/secondary emphasis for VR painting", () => {
    const shared = currentSharedGameUiFixture(1);
    const controls = createGameViewModel(shared).panels.flatMap((panel) => panel.controls);
    const choice = controls.find((control) => control.id === "game-setup-select-information-0");
    const primary = controls.find((control) => control.id === "game-setup-start");
    const secondary = controls.find((control) => control.id === "game-setup-back");
    if (choice?.kind !== "button" || primary === undefined || secondary === undefined) throw new Error("Missing controls");
    expect(uiButtonLabel(choice)).toContain(`● ${choice.label}`);
    expect(uiControlBackground(primary, shared.theme)).not.toBe(uiControlBackground(secondary, shared.theme));
  });
});

import { Window } from "happy-dom";
import { describe, expect, it, vi } from "vitest";
import type { GameUiControlInput, GameUiFrame, GameUiView } from "../../web/src/generated/application-contracts.js";
import { bootViewFixture } from "../game-check/application-fixture.js";

const shared = bootViewFixture();

function adapterView(): GameUiView {
  return { ...shared, interaction_scope: "adapter-inputs", panels: [{
    id: "inputs", title: "描画adapter", anchor: "menu", local_position: [0, 0, 0], size: [2.4, 1.8],
    sections: [{ id: "values", label: "", layout: "column" }], controls: [
      { id: "button", section: "values", label: "操作", enabled: true, content: { kind: "button", presentation: { kind: "action", emphasis: "primary" }, action: { kind: "toggle_technical" } } },
      { id: "toggle", section: "values", label: "表示", enabled: true, content: { kind: "toggle", cue: "telemetry", value: false } },
      { id: "range", section: "values", label: "時刻", enabled: true, content: { kind: "range", minimum: 0, maximum: 10, step: 0.1, value: 1, display_value: "1.0", target: "replay" } },
      { id: "notice", section: "values", label: "", enabled: false, content: { kind: "status", source: "static", value: "発進準備完了" } }
    ]
  }] };
}

function required(root: HTMLElement, selector: string): HTMLElement {
  const element = root.querySelector<HTMLElement>(selector);
  if (element === null) throw new Error(`Missing DOM element ${selector}`);
  return element;
}

async function withAdapter(observe: (root: HTMLElement, adapter: import("../../web/src/presentation/screen-ui.js").ScreenUiAdapter,
  actions: GameUiControlInput[], window: Window) => void) {
  const window = new Window();
  Object.assign(globalThis, { window, document: window.document });
  const { ScreenUiAdapter } = await import("../../web/src/presentation/screen-ui.js");
  const documentRef = window.document as unknown as Document;
  const root = documentRef.createElement("main");
  documentRef.body.append(root);
  const actions: GameUiControlInput[] = [];
  const adapter = new ScreenUiAdapter(root, input => { actions.push(input); });
  try { observe(root, adapter, actions, window); }
  finally {
    adapter.dispose();
    await window.happyDOM.abort();
    Reflect.deleteProperty(globalThis, "window");
    Reflect.deleteProperty(globalThis, "document");
  }
}

describe("Screen DOM adapter", () => {
  it("renders generated widget payloads and transfers raw button, toggle and range observations", async () => {
    await withAdapter((root, adapter, actions, window) => {
      adapter.render(adapterView());
      expect(required(root, "#notice").textContent).toBe("発進準備完了");
      required(root, "button").click();
      const toggle = required(root, "input[type=checkbox]") as HTMLInputElement;
      toggle.checked = true;
      toggle.dispatchEvent(new window.Event("change", { bubbles: true }) as unknown as Event);
      const range = required(root, "input[type=range]") as HTMLInputElement;
      range.value = "4.2";
      range.dispatchEvent(new window.Event("input", { bubbles: true }) as unknown as Event);
      expect(actions).toEqual([{ type: "activate", controlId: "button" }, { type: "set-toggle", controlId: "toggle", value: true }, { type: "set-range", controlId: "range", value: 4.2 }]);
    });
  });

  it("updates retained DOM from a frame without mutating its structural View", async () => {
    await withAdapter((root, adapter) => {
      const view = adapterView();
      adapter.render(view);
      const button = required(root, "button");
      const frame: GameUiFrame = { view_revision: view.view_revision, selections: view.selections, hud: null, chart_cursor: null,
        ranges: [{ id: "range", value: 2.5, display_value: "2.5" }], button_labels: [{ id: "button", label: "一時停止" }], readouts: [{ id: "notice", value: "更新済み" }] };
      adapter.renderFrame(frame);
      expect(required(root, "button")).toBe(button);
      expect(button.textContent).toBe("一時停止");
      expect((required(root, "input[type=range]") as HTMLInputElement).value).toBe("2.5");
      expect(required(root, "#notice").textContent).toBe("更新済み");
      expect(view.panels[0]?.controls[0]?.label).toBe("操作");
      expect(() => { adapter.renderFrame({ ...frame, view_revision: frame.view_revision + 1 }); }).toThrow("retained structural view");
    });
  });

  it("retains focus and scroll across a hidden canvas display and resets scroll for a new shared scope", async () => {
    await withAdapter((root, adapter) => {
      const view = adapterView();
      adapter.render(view);
      const button = required(root, "button");
      const shell = required(root, ".screen-ui-shell");
      button.focus();
      shell.scrollTop = 77;
      adapter.render(view, false);
      expect(root.ownerDocument.activeElement).toBe(root.ownerDocument.body);
      const focus = vi.spyOn(button, "focus");
      adapter.render(view, true);
      expect(root.ownerDocument.activeElement).toBe(button);
      expect(shell.scrollTop).toBe(77);
      expect(focus).toHaveBeenCalledExactlyOnceWith({ preventScroll: true });
      adapter.render({ ...view, interaction_scope: "next-view" });
      expect(shell.scrollTop).toBe(0);
    });
  });

  it("releases event listeners and retained DOM independently of later input delivery", async () => {
    await withAdapter((root, adapter, actions) => {
      adapter.render(adapterView());
      const button = required(root, "button");
      adapter.dispose();
      button.click();
      expect(actions).toEqual([]);
      expect(root.childElementCount).toBe(0);
    });
  });
});

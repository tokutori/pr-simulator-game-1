import { h } from "snabbdom/build/h.js";
import { htmlDomApi } from "snabbdom/build/htmldomapi.js";
import { init } from "snabbdom/build/init.js";
import type { VNode } from "snabbdom";
import type { GameUiCountdown, GameUiTheme } from "../generated/application-contracts.js";

/**
 * Game-side countdown display: a full-screen overlay with no panel, matching
 * the shared derivation in the retained view and per-second frame values.
 */
export class CountdownAdapter {
  private readonly patch: ReturnType<typeof init>;
  private tree: VNode | Element;
  private renderedKey = "";

  constructor(private readonly root: HTMLElement) {
    const documentRef = root.ownerDocument;
    this.patch = init([], {
      ...htmlDomApi,
      createElement: (name: string, options?: ElementCreationOptions) => documentRef.createElement(name, options),
      createElementNS: (namespace, name, options) => documentRef.createElementNS(namespace, name, options),
      createTextNode: (value) => documentRef.createTextNode(value),
      createComment: (value) => documentRef.createComment(value)
    });
    const mount = documentRef.createElement("div");
    mount.className = "game-countdown-content";
    root.className = "game-countdown";
    root.setAttribute("aria-label", "発進カウントダウン");
    root.replaceChildren(mount);
    this.tree = mount;
    this.setVisible(false);
  }

  render(countdown: GameUiCountdown | null, theme: GameUiTheme): void {
    if (countdown === null) {
      this.renderedKey = "";
      this.setVisible(false);
      return;
    }
    const key = JSON.stringify([countdown.digits, countdown.unit, countdown.font_size_px, theme.text]);
    if (key !== this.renderedKey) {
      this.renderedKey = key;
      this.root.style.color = theme.text;
      this.tree = this.patch(this.tree, h("div.game-countdown-content", [
        h("span.game-countdown-digits", {
          style: { fontSize: `${String(countdown.font_size_px)}px` }
        }, countdown.digits),
        h("span.game-countdown-unit", {
          style: { fontSize: `${String(countdown.font_size_px * 0.35)}px` }
        }, countdown.unit)
      ]));
    }
    this.setVisible(true);
  }

  setVisible(visible: boolean): void {
    this.root.hidden = !visible;
  }
}

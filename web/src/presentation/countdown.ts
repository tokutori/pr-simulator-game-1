import type { GameUiCountdown, GameUiTheme } from "../generated/application-contracts.js";

/** Projects the shared head-centred countdown to ordinary screen text. */
export class CountdownAdapter {
  private readonly content: HTMLElement;
  private readonly digits: HTMLElement;
  private readonly unit: HTMLElement;

  constructor(private readonly root: HTMLElement) {
    const documentRef = root.ownerDocument;
    this.content = documentRef.createElement("div");
    this.digits = documentRef.createElement("span");
    this.unit = documentRef.createElement("span");
    this.content.className = "game-countdown-content";
    root.className = "game-countdown";
    root.setAttribute("aria-label", "発進カウントダウン");
    this.content.append(this.digits, this.unit);
    root.replaceChildren(this.content);
    this.setVisible(false);
  }

  render(countdown: GameUiCountdown | null, theme: GameUiTheme): void {
    this.setVisible(countdown !== null);
    if (countdown === null) return;
    this.root.dataset.anchor = countdown.anchor;
    Object.assign(this.root.style, {
      color: theme.text,
      textShadow: `${String(theme.text_shadow_offset_px[0])}px ${String(theme.text_shadow_offset_px[1])}px 0 ${theme.text_shadow_color}`
    });
    this.content.style.gap = `${String(countdown.gap_px)}px`;
    for (const [node, value, style] of [
      [this.digits, countdown.digits, countdown.digit_text],
      [this.unit, countdown.unit, countdown.unit_text]
    ] as const) {
      node.textContent = value;
      Object.assign(node.style, {
        fontSize: `${String(style.font_size_px)}px`,
        fontWeight: String(style.font_weight),
        lineHeight: `${String(style.line_height_px)}px`
      });
    }
  }

  setVisible(visible: boolean): void { this.root.hidden = !visible; }
}

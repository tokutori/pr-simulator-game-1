import { actionForControl } from "./panel-interaction.js";
import type { UiActionDispatcher, UiControl, UiPanel, UiViewModel } from "../render/contracts/ui.js";

export class ScreenUiAdapter {
  constructor(private readonly root: HTMLElement, private readonly dispatch: UiActionDispatcher) {}

  render(viewModel: UiViewModel): void {
    const documentRef = this.root.ownerDocument;
    const shell = documentRef.createElement("section");
    shell.className = "screen-ui-shell";
    shell.dataset.scene = viewModel.scene;
    if (viewModel.activeOverlay !== null) shell.dataset.overlay = viewModel.activeOverlay;
    const heading = documentRef.createElement("h1");
    heading.textContent = viewModel.title;
    const description = documentRef.createElement("p");
    description.className = "screen-ui-description";
    description.textContent = viewModel.description;
    shell.append(heading, description);
    for (const panel of viewModel.panels) shell.append(this.createPanel(documentRef, panel));
    this.root.replaceChildren(shell);
  }

  clear(): void {
    this.root.replaceChildren();
  }

  private createPanel(documentRef: Document, panel: UiPanel): HTMLElement {
    const section = documentRef.createElement("section");
    section.className = "screen-ui-panel";
    section.dataset.anchor = panel.anchor;
    section.setAttribute("aria-label", panel.title);
    const title = documentRef.createElement("h2");
    title.textContent = panel.title;
    const controls = documentRef.createElement("div");
    controls.className = "screen-ui-controls";
    for (const control of panel.controls) controls.append(this.createControl(documentRef, control));
    section.append(title, controls);
    return section;
  }

  private createControl(documentRef: Document, control: UiControl): HTMLElement {
    if (control.kind === "button") {
      const button = documentRef.createElement("button");
      button.type = "button";
      button.disabled = !control.enabled;
      button.textContent = control.label;
      button.addEventListener("click", () => {
        const action = actionForControl(control);
        if (action !== null) this.dispatch(action);
      });
      return button;
    }
    if (control.kind === "toggle") {
      const label = documentRef.createElement("label");
      label.className = "screen-ui-toggle";
      const input = documentRef.createElement("input");
      input.type = "checkbox";
      input.checked = control.value;
      input.disabled = !control.enabled;
      input.addEventListener("change", () => {
        this.dispatch({ type: "set-toggle", controlId: control.id, value: input.checked });
      });
      const text = documentRef.createElement("span");
      text.textContent = control.label;
      label.append(input, text);
      return label;
    }
    if (control.kind === "range") {
      const label = documentRef.createElement("label");
      label.className = "screen-ui-range";
      const text = documentRef.createElement("span");
      text.textContent = `${control.label}: ${control.value.toFixed(1)}`;
      const input = documentRef.createElement("input");
      input.type = "range";
      input.min = String(control.minimum);
      input.max = String(control.maximum);
      input.step = String(control.step);
      input.value = String(control.value);
      input.disabled = !control.enabled;
      input.addEventListener("input", () => {
        const value = Number(input.value);
        if (Number.isFinite(value)) this.dispatch({ type: "set-range", controlId: control.id, value });
      });
      label.append(text, input);
      return label;
    }
    const output = documentRef.createElement("output");
    output.className = "screen-ui-status";
    output.setAttribute("aria-live", "polite");
    output.textContent = `${control.label}: ${control.value}`;
    return output;
  }
}

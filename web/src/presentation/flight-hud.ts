import type { FlightHudPort } from "../game/flight-controller.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";
import { createFlightHudModel } from "./flight-hud-model.js";
import type { InformationLevelCode } from "./flight-hud-model.js";

export class FlightHudAdapter implements FlightHudPort {
  private readonly status: HTMLOutputElement;
  private readonly telemetry: HTMLOutputElement;
  private readonly adi: SVGSVGElement;
  private readonly horizon: SVGGElement;
  private readonly readouts: HTMLOutputElement;
  private readonly headingInstrument: HTMLDivElement;
  private readonly headingReadout: HTMLOutputElement;
  private readonly pilotPositionInstrument: HTMLDivElement;
  private readonly pilotPositionReadout: HTMLOutputElement;
  private readonly windInstrument: HTMLDivElement;
  private readonly windReadout: HTMLOutputElement;
  private readonly angleInstrument: HTMLDivElement;
  private readonly angleReadout: HTMLOutputElement;
  private informationCode: InformationLevelCode = 0;

  constructor(
    private readonly root: HTMLElement,
    private readonly onRender: (snapshot: FlightSnapshot) => void = () => undefined
  ) {
    const documentRef = root.ownerDocument;
    const heading = documentRef.createElement("h2");
    heading.textContent = "FLIGHT";
    this.status = documentRef.createElement("output");
    this.status.setAttribute("aria-live", "polite");
    this.telemetry = documentRef.createElement("output");
    this.telemetry.className = "flight-hud-telemetry";
    this.adi = documentRef.createElementNS("http://www.w3.org/2000/svg", "svg");
    this.adi.setAttribute("viewBox", "0 0 240 180");
    this.adi.setAttribute("role", "img");
    this.adi.setAttribute("aria-label", "姿勢指示器");
    this.adi.classList.add("flight-hud-adi");
    const defs = documentRef.createElementNS("http://www.w3.org/2000/svg", "defs");
    const clip = documentRef.createElementNS("http://www.w3.org/2000/svg", "clipPath");
    clip.id = "flight-adi-clip";
    const clipRect = documentRef.createElementNS("http://www.w3.org/2000/svg", "rect");
    clipRect.setAttribute("x", "26"); clipRect.setAttribute("y", "18");
    clipRect.setAttribute("width", "188"); clipRect.setAttribute("height", "144");
    clipRect.setAttribute("rx", "72");
    clip.append(clipRect); defs.append(clip);
    this.horizon = documentRef.createElementNS("http://www.w3.org/2000/svg", "g");
    this.horizon.setAttribute("clip-path", "url(#flight-adi-clip)");
    this.horizon.setAttribute("transform", "translate(0 90)");
    const sky = documentRef.createElementNS("http://www.w3.org/2000/svg", "rect");
    sky.setAttribute("x", "-240"); sky.setAttribute("y", "-240"); sky.setAttribute("width", "720"); sky.setAttribute("height", "420");
    sky.setAttribute("fill", "#397d9a");
    const ground = documentRef.createElementNS("http://www.w3.org/2000/svg", "rect");
    ground.setAttribute("x", "-240"); ground.setAttribute("y", "0"); ground.setAttribute("width", "720"); ground.setAttribute("height", "240");
    ground.setAttribute("fill", "#9a7047");
    this.horizon.append(sky, ground);
    for (const pitch of [-30, -20, -10, 10, 20, 30]) {
      const y = -pitch * 2.2;
      const line = documentRef.createElementNS("http://www.w3.org/2000/svg", "path");
      const half = Math.abs(pitch) % 20 === 0 ? 27 : 17;
      line.setAttribute("d", `M ${String(120 - half)} ${String(y)} H ${String(120 + half)}`);
      line.setAttribute("stroke", "white"); line.setAttribute("stroke-width", "2");
      this.horizon.append(line);
      const label = documentRef.createElementNS("http://www.w3.org/2000/svg", "text");
      label.setAttribute("x", String(120 + half + 5)); label.setAttribute("y", String(y + 4));
      label.setAttribute("fill", "white"); label.setAttribute("font-size", "9"); label.textContent = String(Math.abs(pitch));
      this.horizon.append(label);
    }
    const frame = documentRef.createElementNS("http://www.w3.org/2000/svg", "rect");
    frame.setAttribute("x", "26"); frame.setAttribute("y", "18"); frame.setAttribute("width", "188"); frame.setAttribute("height", "144");
    frame.setAttribute("rx", "72"); frame.setAttribute("fill", "none"); frame.setAttribute("stroke", "#e8eee5"); frame.setAttribute("stroke-width", "3");
    const aircraft = documentRef.createElementNS("http://www.w3.org/2000/svg", "path");
    aircraft.setAttribute("d", "M 72 91 H 108 L 113 82 H 127 L 132 91 H 168 M 120 86 V 96");
    aircraft.setAttribute("fill", "none"); aircraft.setAttribute("stroke", "#ffd45c"); aircraft.setAttribute("stroke-width", "5"); aircraft.setAttribute("stroke-linecap", "round"); aircraft.setAttribute("stroke-linejoin", "round");
    this.adi.append(defs, this.horizon, frame, aircraft);
    const instruments = documentRef.createElement("div");
    instruments.className = "flight-hud-instruments";
    const headingInstrument = instrumentOutput(documentRef, instruments, "HDG", "heading");
    this.headingInstrument = headingInstrument.cell;
    this.headingReadout = headingInstrument.output;
    const pilotPositionInstrument = instrumentOutput(documentRef, instruments, "PILOT CG", "pilot-position");
    this.pilotPositionInstrument = pilotPositionInstrument.cell;
    this.pilotPositionReadout = pilotPositionInstrument.output;
    const windInstrument = instrumentOutput(documentRef, instruments, "WIND N / E / D", "wind");
    this.windInstrument = windInstrument.cell;
    this.windReadout = windInstrument.output;
    const angleInstrument = instrumentOutput(documentRef, instruments, "ANGLE OF ATTACK", "angle");
    this.angleInstrument = angleInstrument.cell;
    this.angleReadout = angleInstrument.output;
    this.readouts = documentRef.createElement("output");
    this.readouts.className = "flight-hud-readouts";
    const controls = documentRef.createElement("p");
    controls.className = "flight-hud-controls";
    controls.textContent = "A/D roll · ↑/↓ pitch · ←/→ yaw · J/L CG · Gamepad sticks";
    root.className = "flight-hud";
    root.setAttribute("aria-label", "Flight status");
    root.replaceChildren(heading, this.status, this.adi, this.readouts, instruments, this.telemetry, controls);
    this.setVisible(false);
  }

  setInformationCode(code: number): void {
    if (!Number.isInteger(code) || code < 0 || code > 3) {
      throw new RangeError("Information code must lie in [0, 3]");
    }
    this.informationCode = code as InformationLevelCode;
  }

  render(snapshot: FlightSnapshot): void {
    const model = createFlightHudModel(snapshot, this.informationCode);
    this.status.textContent = model.status;
    if (model.attitude === null) {
      this.adi.classList.add("is-hidden");
    } else {
      this.adi.classList.remove("is-hidden");
    }
    this.readouts.textContent = model.readouts;
    this.headingInstrument.hidden = model.heading === null;
    this.headingReadout.textContent = model.heading ?? "";
    this.pilotPositionInstrument.hidden = model.pilotPosition === null;
    this.pilotPositionReadout.textContent = model.pilotPosition ?? "";
    this.windInstrument.hidden = model.wind === null;
    this.windReadout.textContent = model.wind ?? "";
    this.angleInstrument.hidden = model.angleOfAttack === null;
    this.angleReadout.textContent = model.angleOfAttack ?? "";
    this.telemetry.textContent = model.telemetry;
    if (model.attitude !== null) {
      const pitchShift = Math.max(-55, Math.min(55, model.attitude.pitchDegrees * 2.2));
      this.horizon.setAttribute("transform", `rotate(${String(-model.attitude.rollDegrees)} 120 90) translate(0 ${String(90 + pitchShift)})`);
    }
    this.onRender(snapshot);
  }

  fail(message: string): void {
    this.status.textContent = `飛行処理を停止した: ${message}`;
  }

  setVisible(visible: boolean): void {
    this.root.hidden = !visible;
    this.root.setAttribute("aria-hidden", String(!visible));
  }
}

function instrumentOutput(documentRef: Document, root: HTMLElement, label: string, name: string): { readonly cell: HTMLDivElement; readonly output: HTMLOutputElement } {
  const cell = documentRef.createElement("div");
  cell.className = `flight-hud-instrument flight-hud-instrument-${name}`;
  const caption = documentRef.createElement("span");
  caption.textContent = label;
  const output = documentRef.createElement("output");
  output.setAttribute("aria-label", label);
  cell.append(caption, output);
  root.append(cell);
  return Object.freeze({ cell, output });
}

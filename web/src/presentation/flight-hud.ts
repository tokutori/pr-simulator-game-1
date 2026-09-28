import type { FlightHudPort } from "../game/flight-controller.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";

export class FlightHudAdapter implements FlightHudPort {
  private readonly status: HTMLOutputElement;
  private readonly telemetry: HTMLOutputElement;
  private readonly adi: SVGSVGElement;
  private readonly horizon: SVGGElement;
  private readonly readouts: HTMLOutputElement;
  private readonly headingReadout: HTMLOutputElement;
  private readonly pilotPositionReadout: HTMLOutputElement;
  private readonly windReadout: HTMLOutputElement;
  private readonly angleReadout: HTMLOutputElement;
  private informationCode = 0;

  constructor(private readonly root: HTMLElement) {
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
    this.headingReadout = instrumentOutput(documentRef, instruments, "HDG", "heading");
    this.pilotPositionReadout = instrumentOutput(documentRef, instruments, "PILOT CG", "pilot-position");
    this.windReadout = instrumentOutput(documentRef, instruments, "WIND N / E / D", "wind");
    this.angleReadout = instrumentOutput(documentRef, instruments, "ANGLE OF ATTACK", "angle");
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
    this.informationCode = code;
  }

  render(snapshot: FlightSnapshot): void {
    this.status.textContent = terminalLabel(snapshot.terminal);
    const telemetry = snapshot.telemetry;
    if (telemetry === null) {
      this.adi.classList.add("is-hidden");
      this.readouts.textContent = "";
      this.headingReadout.textContent = "—";
      this.pilotPositionReadout.textContent = `${snapshot.pilotPositionMeters.toFixed(2)} m`;
      this.windReadout.textContent = "unavailable";
      this.angleReadout.textContent = "unavailable";
      this.telemetry.textContent = `Tick ${String(snapshot.tick)} · telemetry unavailable · distance ${snapshot.scoreCourseMeters.toFixed(1)} m`;
      return;
    }
    this.adi.classList.remove("is-hidden");
    const rollDegrees = telemetry.rollRadians * 180 / Math.PI;
    const pitchDegrees = telemetry.pitchRadians * 180 / Math.PI;
    const pitchShift = Math.max(-55, Math.min(55, pitchDegrees * 2.2));
    this.horizon.setAttribute("transform", `rotate(${String(-rollDegrees)} 120 90) translate(0 ${String(90 + pitchShift)})`);
    const headingDegrees = ((telemetry.headingRadians * 180 / Math.PI) % 360 + 360) % 360;
    this.readouts.textContent = `IAS ${telemetry.airspeedMetersPerSecond.toFixed(1)} m/s   ALT ${telemetry.altitudeMeters.toFixed(1)} m\nPITCH ${pitchDegrees.toFixed(0)}°   ROLL ${rollDegrees.toFixed(0)}°`;
    this.headingReadout.textContent = `${headingDegrees.toFixed(0)}°`;
    this.pilotPositionReadout.textContent = `${snapshot.pilotPositionMeters >= 0 ? "+" : ""}${snapshot.pilotPositionMeters.toFixed(2)} m`;
    const distance = `distance ${snapshot.scoreCourseMeters.toFixed(1)} m`;
    const time = `time ${snapshot.flightTimeSeconds.toFixed(1)} s`;
    const groundspeed = `groundspeed ${telemetry.groundspeedMetersPerSecond.toFixed(1)} m/s`;
    const wind = telemetry.windVelocityNedMetersPerSecond;
    this.windReadout.textContent = `N ${wind.north.toFixed(1)} · E ${wind.east.toFixed(1)} · D ${wind.down.toFixed(1)} m/s`;
    this.angleReadout.textContent = telemetry.angleOfAttackRadians === null
      ? "—"
      : `${(telemetry.angleOfAttackRadians * 180 / Math.PI).toFixed(1)}°`;
    const modeLabel = this.informationCode === 3 ? "synthetic instruments" : "";
    const fields = this.informationCode === 2
      ? [distance, time]
      : this.informationCode === 1 || this.informationCode === 3
        ? [groundspeed, distance, time]
      : [distance, time];
    this.telemetry.textContent = [modeLabel, ...fields].filter(Boolean).join(" · ");
  }

  fail(message: string): void {
    this.status.textContent = `飛行処理を停止した: ${message}`;
  }

  setVisible(visible: boolean): void {
    this.root.hidden = !visible;
    this.root.setAttribute("aria-hidden", String(!visible));
  }
}

function instrumentOutput(documentRef: Document, root: HTMLElement, label: string, name: string): HTMLOutputElement {
  const cell = documentRef.createElement("div");
  cell.className = `flight-hud-instrument flight-hud-instrument-${name}`;
  const caption = documentRef.createElement("span");
  caption.textContent = label;
  const output = documentRef.createElement("output");
  output.setAttribute("aria-label", label);
  cell.append(caption, output);
  root.append(cell);
  return output;
}

function terminalLabel(terminal: FlightSnapshot["terminal"]): string {
  switch (terminal) {
    case "airborne": return "滑空中";
    case "water-contact": return "着水";
    case "time-limit": return "時間制限";
    case "out-of-valid-envelope": return "空力モデルの適用範囲外";
    case "manual-abort": return "手動終了";
    case "fatal-simulation-error": return "シミュレーションエラー";
  }
}

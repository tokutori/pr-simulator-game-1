import type { FlightHudPort } from "../game/flight-controller.js";
import type { FlightSnapshot } from "../game/flight-snapshot.js";

export class FlightHudAdapter implements FlightHudPort {
  private readonly status: HTMLOutputElement;
  private readonly telemetry: HTMLOutputElement;

  constructor(private readonly root: HTMLElement) {
    const documentRef = root.ownerDocument;
    const heading = documentRef.createElement("h2");
    heading.textContent = "合成飛行";
    this.status = documentRef.createElement("output");
    this.status.setAttribute("aria-live", "polite");
    this.telemetry = documentRef.createElement("output");
    const controls = documentRef.createElement("p");
    controls.textContent = "A/D: roll · ↑/↓: pitch · ←/→: yaw · J/L: pilot position · Gamepad: left/right stick";
    root.className = "flight-hud";
    root.setAttribute("aria-label", "Flight status");
    root.replaceChildren(heading, this.status, this.telemetry, controls);
  }

  render(snapshot: FlightSnapshot): void {
    this.status.textContent = terminalLabel(snapshot.terminal);
    this.telemetry.textContent = `Tick ${String(snapshot.tick)} · altitude ${(-snapshot.positionNed.down).toFixed(1)} m · distance ${snapshot.scoreCourseMeters.toFixed(1)} m`;
  }

  fail(message: string): void {
    this.status.textContent = `飛行処理を停止した: ${message}`;
  }

  clear(): void {
    this.root.replaceChildren();
  }
}

function terminalLabel(terminal: FlightSnapshot["terminal"]): string {
  switch (terminal) {
    case "airborne": return "滑空中";
    case "water-contact": return "着水";
    case "time-limit": return "時間制限";
  }
}

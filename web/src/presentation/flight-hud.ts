import type { TailFlightHudPort } from "../game/tail-flight-controller.js";
import type { FlightDisplaySnapshot } from "../game/flight-display-snapshot.js";
import { createFlightDisplayHudModel } from "./flight-hud-model.js";
import type { FlightHudModel, InformationLevelCode } from "./flight-hud-model.js";
import type { HudProfileUiState } from "../app/app-state.js";

export class FlightHudAdapter implements TailFlightHudPort {
  private readonly status: HTMLOutputElement;
  private readonly warning: HTMLOutputElement;
  private readonly telemetry: HTMLOutputElement;
  private readonly location: HTMLOutputElement;
  private readonly mapAttribution: HTMLAnchorElement;
  private readonly terrainAttribution: HTMLAnchorElement;
  private readonly copernicusAttribution: HTMLAnchorElement;
  private readonly copernicusLicenseNotice: HTMLDetailsElement;
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
  private readonly headingScale: SVGGElement;
  private readonly windNeedle: SVGGElement;
  private readonly angleIndicator: SVGPolygonElement;
  private readonly flightPathIndicator: SVGCircleElement;
  private readonly controls: HTMLParagraphElement;
  private informationCode: InformationLevelCode = 0;
  private informationProfile: HudProfileUiState = fullProfile;

  constructor(
    private readonly root: HTMLElement,
    private readonly onRender: (snapshot: FlightDisplaySnapshot) => void = () => undefined
  ) {
    const documentRef = root.ownerDocument;
    const heading = documentRef.createElement("h2");
    heading.textContent = "FLIGHT";
    this.status = documentRef.createElement("output");
    this.status.setAttribute("aria-live", "polite");
    this.warning = documentRef.createElement("output");
    this.warning.className = "flight-hud-warning";
    this.telemetry = documentRef.createElement("output");
    this.telemetry.className = "flight-hud-telemetry";
    this.location = documentRef.createElement("output");
    this.location.className = "flight-hud-location";
    this.mapAttribution = documentRef.createElement("a");
    this.mapAttribution.className = "flight-hud-attribution";
    this.mapAttribution.href = "https://www.openstreetmap.org/copyright";
    this.mapAttribution.target = "_blank";
    this.mapAttribution.rel = "noopener noreferrer";
    this.terrainAttribution = documentRef.createElement("a");
    this.terrainAttribution.className = "flight-hud-attribution-link";
    this.terrainAttribution.href = "https://earth.jaxa.jp/en/data/policy/";
    this.terrainAttribution.target = "_blank";
    this.terrainAttribution.rel = "noopener noreferrer";
    this.terrainAttribution.textContent = "地形 AW3D30 (JAXA)";
    this.copernicusAttribution = documentRef.createElement("a");
    this.copernicusAttribution.className = "flight-hud-attribution-link";
    this.copernicusAttribution.href = "https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM";
    this.copernicusAttribution.target = "_blank";
    this.copernicusAttribution.rel = "noopener noreferrer";
    this.copernicusAttribution.textContent = "© DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved.";
    this.copernicusLicenseNotice = documentRef.createElement("details");
    this.copernicusLicenseNotice.className = "flight-hud-attribution-notice";
    this.copernicusLicenseNotice.open = true;
    const licenseSummary = documentRef.createElement("summary");
    licenseSummary.textContent = "Copernicus GLO-30 利用条件";
    const modifiedNotice = documentRef.createElement("p");
    modifiedNotice.textContent = "produced using Copernicus WorldDEM-30.";
    const liabilityNotice = documentRef.createElement("p");
    liabilityNotice.textContent = "The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30.";
    this.copernicusLicenseNotice.append(licenseSummary, modifiedNotice, liabilityNotice);
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
    this.flightPathIndicator = documentRef.createElementNS("http://www.w3.org/2000/svg", "circle");
    this.flightPathIndicator.setAttribute("r", "5");
    this.flightPathIndicator.setAttribute("fill", "none");
    this.flightPathIndicator.setAttribute("stroke", "#7df4c5");
    this.flightPathIndicator.setAttribute("stroke-width", "3");
    this.adi.append(defs, this.horizon, frame, aircraft, this.flightPathIndicator);
    const instruments = documentRef.createElement("div");
    instruments.className = "flight-hud-instruments";
    const headingInstrument = instrumentOutput(documentRef, instruments, "HDG", "heading");
    this.headingInstrument = headingInstrument.cell;
    this.headingReadout = headingInstrument.output;
    const headingGauge = svgGauge(documentRef, headingInstrument.cell, "heading", "0 0 220 50", "方位目盛");
    this.headingScale = documentRef.createElementNS("http://www.w3.org/2000/svg", "g");
    headingGauge.append(this.headingScale, svgElement(documentRef, "path", {
      d: "M 110 28 L 104 38 H 116 Z", fill: "#ffd45c"
    }));
    const pilotPositionInstrument = instrumentOutput(documentRef, instruments, "PILOT CG", "pilot-position");
    this.pilotPositionInstrument = pilotPositionInstrument.cell;
    this.pilotPositionReadout = pilotPositionInstrument.output;
    const windInstrument = instrumentOutput(documentRef, instruments, "WIND N / E / D", "wind");
    this.windInstrument = windInstrument.cell;
    this.windReadout = windInstrument.output;
    const windGauge = svgGauge(documentRef, windInstrument.cell, "wind", "0 0 72 72", "風向計。矢印は風の流れる方向を示す");
    windGauge.append(
      svgElement(documentRef, "circle", { cx: "36", cy: "36", r: "27", fill: "none", stroke: "#b9c9c2", "stroke-width": "1.5" }),
      svgElement(documentRef, "path", { d: "M 36 9 V 63 M 9 36 H 63", stroke: "#526c70", "stroke-width": "1" })
    );
    const northLabel = svgElement(documentRef, "text", { x: "36", y: "8", "text-anchor": "middle", fill: "#f3f4e8", "font-size": "7" });
    northLabel.textContent = "N";
    windGauge.append(northLabel);
    this.windNeedle = documentRef.createElementNS("http://www.w3.org/2000/svg", "g");
    this.windNeedle.append(svgElement(documentRef, "path", { d: "M 36 15 L 31 38 L 36 34 L 41 38 Z", fill: "#ffd45c" }));
    windGauge.append(this.windNeedle);
    const angleInstrument = instrumentOutput(documentRef, instruments, "ANGLE OF ATTACK", "angle");
    this.angleInstrument = angleInstrument.cell;
    this.angleReadout = angleInstrument.output;
    const angleGauge = svgGauge(documentRef, angleInstrument.cell, "angle-of-attack", "0 0 200 34", "迎角目盛");
    angleGauge.append(svgElement(documentRef, "path", { d: "M 10 16 H 190", stroke: "#b9c9c2", "stroke-width": "3" }));
    for (let index = 0; index <= 6; index += 1) {
      const x = 10 + index * 30;
      angleGauge.append(svgElement(documentRef, "path", { d: `M ${String(x)} 12 V 21`, stroke: "#b9c9c2", "stroke-width": "1" }));
    }
    this.angleIndicator = svgElement(documentRef, "polygon", { points: "70,5 64,1 76,1", fill: "#ffd45c" });
    angleGauge.append(this.angleIndicator);
    this.readouts = documentRef.createElement("output");
    this.readouts.className = "flight-hud-readouts";
    this.controls = documentRef.createElement("p");
    this.controls.className = "flight-hud-controls";
    const attributions = documentRef.createElement("div");
    attributions.className = "flight-hud-attributions";
    attributions.append(this.mapAttribution, this.terrainAttribution, this.copernicusAttribution, this.copernicusLicenseNotice);
    root.className = "flight-hud";
    root.setAttribute("aria-label", "Flight status");
    root.replaceChildren(heading, this.status, this.warning, this.adi, this.readouts, instruments, this.telemetry, this.location, attributions, this.controls);
    this.setVisible(false);
  }

  setInformationProfile(code: number, profile: HudProfileUiState): void {
    if (!Number.isInteger(code) || code < 0 || code > 4) {
      throw new RangeError("Information code must lie in [0, 4]");
    }
    this.informationCode = code as InformationLevelCode;
    this.informationProfile = profile;
  }

  render(snapshot: FlightDisplaySnapshot, model: FlightHudModel = createFlightDisplayHudModel(snapshot, this.informationCode, this.informationProfile)): void {
    this.applyModel(model);
    this.onRender(snapshot);
  }

  renderDisplaySnapshot(snapshot: FlightDisplaySnapshot, model: FlightHudModel = createFlightDisplayHudModel(snapshot, this.informationCode, this.informationProfile)): void {
    this.applyModel(model);
  }

  private applyModel(model: FlightHudModel): void {
    this.status.textContent = model.status;
    this.warning.textContent = model.warning ?? "";
    this.warning.hidden = model.warning === null;
    if (model.attitude === null) {
      this.adi.classList.add("is-hidden");
    } else {
      this.adi.classList.remove("is-hidden");
    }
    this.readouts.textContent = [model.readouts, ...model.supplementaryReadouts].filter((line) => line !== "").join("\n");
    this.controls.textContent = model.controlsDescription;
    this.flightPathIndicator.setAttribute("visibility", model.flightPathAngleDegrees === null ? "hidden" : "visible");
    if (model.flightPathAngleDegrees !== null) {
      const y = 90 - Math.max(-30, Math.min(30, model.flightPathAngleDegrees)) * 2.2;
      this.flightPathIndicator.setAttribute("cx", "120");
      this.flightPathIndicator.setAttribute("cy", String(y));
    }
    this.headingInstrument.hidden = model.heading === null;
    this.headingReadout.textContent = model.heading ?? "";
    if (model.headingDegrees !== null) renderHeadingScale(this.headingScale, model.headingDegrees);
    this.pilotPositionInstrument.hidden = model.pilotPosition === null;
    this.pilotPositionReadout.textContent = model.pilotPosition ?? "";
    this.windInstrument.hidden = model.wind === null;
    this.windReadout.textContent = model.wind ?? "";
    this.windNeedle.setAttribute("visibility", model.windDirectionDegrees === null ? "hidden" : "visible");
    if (model.windDirectionDegrees !== null) this.windNeedle.setAttribute("transform", `rotate(${String(model.windDirectionDegrees)} 36 36)`);
    this.angleInstrument.hidden = model.angleOfAttack === null;
    this.angleReadout.textContent = model.angleOfAttack ?? "";
    this.angleIndicator.setAttribute("visibility", model.angleOfAttackDegrees === null ? "hidden" : "visible");
    if (model.angleOfAttackDegrees !== null) {
      const ratio = (Math.max(-10, Math.min(20, model.angleOfAttackDegrees)) + 10) / 30;
      const x = 10 + ratio * 180;
      this.angleIndicator.setAttribute("points", `${String(x)},5 ${String(x - 6)},1 ${String(x + 6)},1`);
    }
    this.telemetry.textContent = model.telemetry;
    this.location.textContent = model.location;
    this.mapAttribution.textContent = model.mapAttribution;
    if (model.attitude !== null) {
      const pitchShift = Math.max(-55, Math.min(55, model.attitude.pitchDegrees * 2.2));
      this.horizon.setAttribute("transform", `rotate(${String(-model.attitude.rollDegrees)} 120 90) translate(0 ${String(90 + pitchShift)})`);
    }
  }

  fail(message: string): void {
    this.status.textContent = `飛行処理を停止した: ${message}`;
  }

  setVisible(visible: boolean): void {
    this.root.hidden = !visible;
    this.root.setAttribute("aria-hidden", String(!visible));
  }
}

const fullProfile: HudProfileUiState = Object.freeze({
  telemetry: true,
  attitude: true,
  wind: true,
  flightPath: true,
  angleOfAttack: true,
  warnings: true
});

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

function svgGauge(documentRef: Document, cell: HTMLElement, name: string, viewBox: string, label: string): SVGSVGElement {
  const svg = documentRef.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", viewBox);
  svg.setAttribute("role", "img");
  svg.setAttribute("aria-label", label);
  svg.setAttribute("data-instrument", name);
  cell.append(svg);
  return svg;
}

function renderHeadingScale(group: SVGGElement, headingDegrees: number): void {
  const documentRef = group.ownerDocument;
  group.replaceChildren();
  for (let offset = -60; offset <= 60; offset += 10) {
    const x = 110 + offset * 1.2;
    const major = offset % 30 === 0;
    group.append(svgElement(documentRef, "path", {
      d: `M ${String(x)} ${major ? "28" : "33"} V 40`,
      stroke: major ? "#f3f4e8" : "#b9c9c2",
      "stroke-width": major ? "1.5" : "1"
    }));
    if (major) {
      const bearing = normalizeDegrees(Math.round(headingDegrees + offset));
      const label = svgElement(documentRef, "text", {
        x: String(x), y: "22", "text-anchor": "middle", fill: "#f3f4e8", "font-size": "8"
      });
      label.textContent = compassLabel(bearing);
      group.append(label);
    }
  }
}

function compassLabel(degrees: number): string {
  if (degrees === 0) return "N";
  if (degrees === 90) return "E";
  if (degrees === 180) return "S";
  if (degrees === 270) return "W";
  return String(degrees).padStart(3, "0");
}

function normalizeDegrees(degrees: number): number {
  return ((degrees % 360) + 360) % 360;
}

function svgElement<K extends keyof SVGElementTagNameMap>(
  documentRef: Document,
  name: K,
  attributes: Readonly<Record<string, string>>
): SVGElementTagNameMap[K] {
  const element = documentRef.createElementNS("http://www.w3.org/2000/svg", name);
  for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, value);
  return element;
}

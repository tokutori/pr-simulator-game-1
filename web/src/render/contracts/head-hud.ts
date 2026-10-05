import type { Pose } from "./math.js";

export interface HeadHudRect {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

interface HeadHudElementBase {
  readonly id: string;
  readonly label: string;
  readonly bounds: HeadHudRect;
}

export type HeadHudElement = HeadHudElementBase & (
  | { readonly kind: "text"; readonly value: string; readonly tone: "normal" | "warning" }
  | { readonly kind: "attitude"; readonly rollDegrees: number; readonly pitchDegrees: number; readonly flightPathAngleDegrees: number | null }
  | { readonly kind: "heading"; readonly degrees: number }
  | { readonly kind: "pilot-position"; readonly ratio: number; readonly value: string }
  | { readonly kind: "wind"; readonly degrees: number; readonly value: string }
  | { readonly kind: "angle-of-attack"; readonly degrees: number; readonly value: string }
  | { readonly kind: "flight-path"; readonly degrees: number; readonly value: string }
);

export interface VisibleHeadHud {
  readonly kind: "visible";
  readonly locale: string;
  readonly anchor: "head";
  readonly localPose: Pose;
  readonly size: Readonly<{ width: number; height: number }>;
  readonly clearRegion: HeadHudRect;
  readonly backgroundAlpha: number;
  readonly foregroundAlpha: number;
  readonly elements: readonly HeadHudElement[];
}

export type HeadHudLayer = { readonly kind: "absent" } | VisibleHeadHud;

export type HeadHudFrame =
  | { readonly kind: "absent" }
  | { readonly kind: "visible"; readonly trackingFromHead: Pose; readonly view: VisibleHeadHud };

export const NO_HEAD_HUD = Object.freeze({ kind: "absent" } as const);

export function resolveHeadHudFrame(layer: HeadHudLayer, trackingFromHead: Pose | null): HeadHudFrame {
  return layer.kind === "absent" || trackingFromHead === null
    ? NO_HEAD_HUD
    : Object.freeze({ kind: "visible", trackingFromHead, view: layer });
}

export function validateHeadHudLayer(layer: HeadHudLayer): void {
  if (layer.kind === "absent") return;
  if (layer.locale.trim().length === 0) throw new RangeError("Head HUD locale must identify the displayed language");
  if (![layer.size.width, layer.size.height].every((value) => Number.isFinite(value) && value > 0)) {
    throw new RangeError("Head HUD dimensions must be positive and finite");
  }
  if (![layer.backgroundAlpha, layer.foregroundAlpha].every((value) => Number.isFinite(value) && value >= 0 && value <= 1)) {
    throw new RangeError("Head HUD alpha must lie in [0, 1]");
  }
  const position = layer.localPose.position;
  const orientation = layer.localPose.orientation;
  if (![position.x, position.y, position.z, orientation.w, orientation.x, orientation.y, orientation.z].every(Number.isFinite) ||
      Math.abs(Math.hypot(orientation.w, orientation.x, orientation.y, orientation.z) - 1) > 1.0e-9) {
    throw new RangeError("Head HUD pose must be finite with unit orientation");
  }
  validateRect(layer.clearRegion);
  const ids = new Set<string>();
  for (const element of layer.elements) {
    if (ids.has(element.id)) throw new Error(`Duplicate Head HUD element: ${element.id}`);
    ids.add(element.id);
    validateRect(element.bounds);
    if (intersects(element.bounds, layer.clearRegion)) throw new RangeError(`Head HUD element covers the clear region: ${element.id}`);
    switch (element.kind) {
      case "text":
        break;
      case "attitude":
        if (![element.rollDegrees, element.pitchDegrees].every(Number.isFinite) ||
            (element.flightPathAngleDegrees !== null && !Number.isFinite(element.flightPathAngleDegrees))) {
          throw new RangeError(`Head HUD attitude is non-finite: ${element.id}`);
        }
        break;
      case "pilot-position":
        if (!Number.isFinite(element.ratio) || element.ratio < -1 || element.ratio > 1) {
          throw new RangeError(`Head HUD pilot position must lie in [-1, 1]: ${element.id}`);
        }
        break;
      case "heading":
      case "wind":
      case "angle-of-attack":
      case "flight-path":
        if (!Number.isFinite(element.degrees)) throw new RangeError(`Head HUD angle is non-finite: ${element.id}`);
        break;
    }
  }
}

function validateRect(bounds: HeadHudRect): void {
  if (![bounds.left, bounds.top, bounds.width, bounds.height].every(Number.isFinite) ||
      bounds.left < 0 || bounds.top < 0 || bounds.width <= 0 || bounds.height <= 0 ||
      bounds.left + bounds.width > 1 || bounds.top + bounds.height > 1) {
    throw new RangeError("Head HUD rectangles must lie inside the normalized layer");
  }
}

function intersects(first: HeadHudRect, second: HeadHudRect): boolean {
  return first.left < second.left + second.width && second.left < first.left + first.width &&
    first.top < second.top + second.height && second.top < first.top + first.height;
}

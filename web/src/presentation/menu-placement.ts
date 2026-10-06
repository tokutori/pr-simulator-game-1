import { composePose } from "../render/contracts/math.js";
import type { Pose } from "../render/contracts/math.js";
import type { UiPanel, UiViewModel } from "../render/contracts/ui.js";
import type { BinocularGeometry, ViewerFrame } from "../render/contracts/viewer-frame.js";
import { fitMenuView } from "./menu-view-fit.js";
import type { MenuViewFit } from "./menu-view-fit.js";

interface MenuOpening {
  readonly scene: UiViewModel["scene"];
  readonly overlay: UiViewModel["activeOverlay"];
  readonly panelId: string;
  readonly size: UiPanel["size"];
  readonly localPose: Pose;
  readonly eyes: BinocularGeometry;
  readonly headFromMenu: Pose;
}

export type MenuPlacementModel =
  | { readonly kind: "closed" }
  | { readonly kind: "placed"; readonly opening: MenuOpening; readonly referenceFromMenu: Pose };

export type MenuPlacementResult =
  | { readonly kind: "ready"; readonly referenceFromMenu: Pose }
  | Extract<MenuViewFit, { readonly kind: "unavailable" }>;

export const CLOSED_MENU_PLACEMENT: MenuPlacementModel = Object.freeze({ kind: "closed" });

export function openMenuPlacement(
  previous: MenuPlacementModel,
  view: Pick<UiViewModel, "scene" | "activeOverlay">,
  panel: UiPanel,
  referenceFromHead: Pose,
  viewer: ViewerFrame
): Readonly<{ model: MenuPlacementModel; result: MenuPlacementResult }> {
  if (viewer.source === "unavailable") return Object.freeze({ model: previous,
    result: Object.freeze({ kind: "unavailable", reason: viewer.reason }) });
  if (previous.kind === "placed" && matchesOpening(previous.opening, view, panel, viewer.eyes)) {
    return Object.freeze({ model: previous, result: Object.freeze({ kind: "ready", referenceFromMenu: previous.referenceFromMenu }) });
  }
  const fit = fitMenuView(viewer, panel.size, panel.localPose);
  if (fit.kind === "unavailable") return Object.freeze({ model: previous, result: fit });
  try {
    const referenceFromMenu = composePose(referenceFromHead, fit.headFromMenu);
    const opening: MenuOpening = Object.freeze({ scene: view.scene, overlay: view.activeOverlay,
      panelId: panel.id, size: panel.size, localPose: panel.localPose, eyes: viewer.eyes, headFromMenu: fit.headFromMenu });
    const model: MenuPlacementModel = Object.freeze({ kind: "placed", opening, referenceFromMenu });
    return Object.freeze({ model, result: Object.freeze({ kind: "ready", referenceFromMenu }) });
  } catch (error) {
    if (error instanceof RangeError) return Object.freeze({ model: previous,
      result: Object.freeze({ kind: "unavailable", reason: "invalid-view-geometry" }) });
    throw error;
  }
}

export function recenterMenuPlacement(model: MenuPlacementModel, referenceFromHead: Pose): MenuPlacementModel {
  return model.kind === "closed" ? model : Object.freeze({ ...model,
    referenceFromMenu: composePose(referenceFromHead, model.opening.headFromMenu) });
}

export function transformMenuPlacement(model: MenuPlacementModel, newReferenceFromPrevious: Pose): MenuPlacementModel {
  return model.kind === "closed" ? model : Object.freeze({ ...model,
    referenceFromMenu: composePose(newReferenceFromPrevious, model.referenceFromMenu) });
}

function matchesOpening(opening: MenuOpening, view: Pick<UiViewModel, "scene" | "activeOverlay">, panel: UiPanel, eyes: BinocularGeometry): boolean {
  return opening.scene === view.scene && opening.overlay === view.activeOverlay && opening.panelId === panel.id &&
    opening.size.width === panel.size.width && opening.size.height === panel.size.height &&
    equalPose(opening.localPose, panel.localPose, 0) && opening.eyes.every((eye, index) => {
      const current = eyes[index];
      return current !== undefined && eye.eye === current.eye && equalPose(eye.headFromEye, current.headFromEye, 1e-8) &&
        eye.projection.every((value, coordinate) => closeNumber(value, current.projection[coordinate] ?? Number.NaN, 1e-8));
    });
}

function equalPose(left: Pose, right: Pose, tolerance: number): boolean {
  const positionValues = [left.position.x, left.position.y, left.position.z];
  const otherPositionValues = [right.position.x, right.position.y, right.position.z];
  const rotationValues = [left.orientation.w, left.orientation.x, left.orientation.y, left.orientation.z];
  const otherRotationValues = [right.orientation.w, right.orientation.x, right.orientation.y, right.orientation.z];
  return positionValues.every((value, index) => closeNumber(value, otherPositionValues[index] ?? Number.NaN, tolerance)) &&
    [-1, 1].some((sign) => rotationValues.every((value, index) => closeNumber(value, sign * (otherRotationValues[index] ?? Number.NaN), tolerance)));
}

function closeNumber(left: number, right: number, tolerance: number): boolean {
  return Number.isFinite(left) && Number.isFinite(right) && Math.abs(left - right) <= tolerance * Math.max(1, Math.abs(left), Math.abs(right));
}

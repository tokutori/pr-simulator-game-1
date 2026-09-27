import { inversePose, rotateVec3, transformPoint, vec3 } from "../render/contracts/math.js";
import type { Pose, Vec3 } from "../render/contracts/math.js";
import type { UiAction, UiControl, UiPanel } from "../render/contracts/ui.js";

export interface Ray {
  readonly origin: Vec3;
  readonly direction: Vec3;
}

export interface PanelPoint {
  readonly x: number;
  readonly y: number;
}

export function intersectPanel(ray: Ray, worldFromPanel: Pose): PanelPoint | null {
  const panelFromWorld = inversePose(worldFromPanel);
  const localOrigin = transformPoint(panelFromWorld, ray.origin);
  const localDirection = rotateVec3(panelFromWorld.orientation, ray.direction);
  if (localDirection.z >= -Number.EPSILON) return null;
  const distance = -localOrigin.z / localDirection.z;
  if (!Number.isFinite(distance) || distance < 0) return null;
  const hit = vec3(
    localOrigin.x + localDirection.x * distance,
    localOrigin.y + localDirection.y * distance,
    0
  );
  return Object.freeze({ x: hit.x, y: hit.y });
}

export function panelCoordinates(point: PanelPoint, panel: UiPanel): PanelPoint | null {
  const x = point.x / panel.size.width + 0.5;
  const y = 0.5 - point.y / panel.size.height;
  if (x < 0 || x > 1 || y < 0 || y > 1) return null;
  return Object.freeze({ x, y });
}

export function hitTestControl(panel: UiPanel, point: PanelPoint): UiControl | null {
  const normalized = panelCoordinates(point, panel);
  if (normalized === null) return null;
  return panel.controls.find((control) => control.enabled &&
    normalized.x >= control.rect.x && normalized.x <= control.rect.x + control.rect.width &&
    normalized.y >= control.rect.y && normalized.y <= control.rect.y + control.rect.height) ?? null;
}

export function actionForControl(control: UiControl): UiAction | null {
  if (!control.enabled) return null;
  switch (control.kind) {
    case "button":
      return { type: "activate", controlId: control.id };
    case "toggle":
      return { type: "set-toggle", controlId: control.id, value: !control.value };
    case "range":
    case "status":
      return null;
  }
}

export function rangeAction(control: UiControl, panel: UiPanel, point: PanelPoint): UiAction | null {
  if (!control.enabled || control.kind !== "range") return null;
  const normalized = panelCoordinates(point, panel);
  if (normalized === null) return null;
  const ratio = clamp((normalized.x - control.rect.x) / control.rect.width, 0, 1);
  const raw = control.minimum + ratio * (control.maximum - control.minimum);
  const steps = Math.round((raw - control.minimum) / control.step);
  const value = clamp(control.minimum + steps * control.step, control.minimum, control.maximum);
  return { type: "set-range", controlId: control.id, value };
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(maximum, Math.max(minimum, value));
}

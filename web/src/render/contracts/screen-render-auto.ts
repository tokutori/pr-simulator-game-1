import type { ScreenRenderAutoSnapshot } from "../../generated/screen-render-auto.js";
import { validateScreenRenderAutoSnapshot } from "../../generated/game-ui-validators.js";

export function parseScreenRenderAutoSnapshot(json: string): ScreenRenderAutoSnapshot {
  const snapshot: unknown = JSON.parse(json);
  if (!validateScreenRenderAutoSnapshot(snapshot)) throw new Error("Invalid shared Screen Auto snapshot");
  return snapshot;
}

export type ScreenRenderInvalidation = "resize" | "backend_changed" | "context_restored" | "disjoint" | "interrupted" | "quality_changed";

export interface ScreenRenderAutoPolicy {
  snapshot_json(): string;
  epoch(): number;
  measurement_enabled(): boolean;
  requested_scale_percent(): number;
  requested_shadow_map_size(): number;
  requested_epoch(): number;
  select_manual(percent: number, now: number): void;
  select_auto(enabled: boolean, now: number): void;
  observe(epoch: number, frame: number, source: number, gpuWork: number, now: number): void;
  pending(now: number): void;
  invalidate(reason: ScreenRenderInvalidation, now: number): void;
  unsupported(cause: string, now: number): void;
  applied(epoch: number, percent: number, shadowMapSize: number, now: number, cleanupFailure?: string): void;
  failed(epoch: number, percent: number, shadowMapSize: number, cause: string, now: number): void;
  free(): void;
}

export interface ScreenRenderAutoBinding {
  readonly policy: ScreenRenderAutoPolicy;
  readonly onSnapshot: (snapshot: ScreenRenderAutoSnapshot) => void;
}

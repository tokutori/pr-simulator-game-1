import type { ScreenRenderAutoSnapshot } from "../../generated/screen-render-auto.js";
import type { LakeWaterQuality } from "./lake-water.js";
import type { GameScene } from "./ui.js";

export type ScreenRenderInvalidation = "resize" | "backend_changed" | "context_restored" | "disjoint" | "interrupted" | "quality_changed" | "scene_preparation";

export interface ScreenRenderAutoPolicy {
  epoch(): number;
  measurement_enabled(): boolean;
  set_scene(scene: GameScene, now: number): void;
  requested_scale_percent(): number;
  requested_shadow_map_size(): number;
  requested_water_quality(): string | undefined;
  requested_epoch(): number;
  select_manual(percent: number, now: number): void;
  select_auto(enabled: boolean, now: number): void;
  select_water_quality(quality: LakeWaterQuality, now: number): void;
  observe(epoch: number, frame: number, source: number, gpuWork: number, now: number): void;
  pending(now: number): void;
  invalidate(reason: ScreenRenderInvalidation, now: number): void;
  unsupported(cause: string, now: number): void;
  applied(epoch: number, percent: number, shadowMapSize: number, quality: LakeWaterQuality, now: number, cleanupFailure?: string): void;
  failed(epoch: number, percent: number, shadowMapSize: number, quality: LakeWaterQuality, cause: string, now: number): void;
  free(): void;
}

export interface ScreenRenderAutoBinding {
  readonly policy: ScreenRenderAutoPolicy;
  readonly snapshot: () => ScreenRenderAutoSnapshot;
  readonly scene: () => GameScene;
  readonly onSnapshot: (snapshot: ScreenRenderAutoSnapshot) => void;
}

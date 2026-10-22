import {
  ClampToEdgeWrapping,
  DataTexture,
  EquirectangularReflectionMapping,
  LinearFilter,
  LinearMipmapLinearFilter,
  RepeatWrapping,
  RGBAFormat,
  SRGBColorSpace,
  UnsignedByteType
} from "three";
import { LakeSkyImageBridge, lake_scene_haze, lake_scene_sunlight } from "../../../../pkg/birdman_game_wasm.js";
import { createLakeSkyCondition } from "../../contracts/lake-sky.js";
import type { LakeSkyCondition } from "../../contracts/lake-sky.js";

export function lakeSkyHaze(visibility: number | undefined): readonly [number, number, number] {
  const [color, near, far] = lake_scene_haze(visibility);
  if (color === undefined || near === undefined || far === undefined) throw new Error("Incomplete shared scene haze");
  return [color, near, far];
}

export function lakeSkySunlight(condition: LakeSkyCondition | null): readonly [number, number, number, number] {
  const [east, up, back, daylight] = lake_scene_sunlight(condition?.sunAzimuthDegrees, condition?.sunElevationDegrees);
  if (east === undefined || up === undefined || back === undefined || daylight === undefined) throw new Error("Incomplete shared sunlight");
  return [east, up, back, daylight];
}

export function createLakeSkyTexture(condition: LakeSkyCondition | null = null): DataTexture {
  const sky = condition === null ? null : createLakeSkyCondition(condition);
  const image = sky === null ? LakeSkyImageBridge.neutral() : new LakeSkyImageBridge(
    sky.sunAzimuthDegrees, sky.sunElevationDegrees, sky.cloudFraction, sky.visibilityMeters);
  try {
    const texture = new DataTexture(image.rgba(), image.width, image.height, RGBAFormat, UnsignedByteType);
    texture.mapping = EquirectangularReflectionMapping;
    texture.colorSpace = SRGBColorSpace;
    texture.wrapS = RepeatWrapping;
    texture.wrapT = ClampToEdgeWrapping;
    texture.magFilter = LinearFilter;
    texture.minFilter = LinearMipmapLinearFilter;
    texture.generateMipmaps = true;
    texture.needsUpdate = true;
    return texture;
  } finally {
    image.free();
  }
}

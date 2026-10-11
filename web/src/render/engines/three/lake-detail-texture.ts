import { DataTexture, LinearFilter, LinearMipmapLinearFilter, RepeatWrapping } from "three";
import { LakeDetailImageBridge } from "../../../../pkg/birdman_game_wasm.js";
import type { LakeVisualCondition } from "../../contracts/lake-water.js";

export interface LakeDetailLayer {
  readonly texture: DataTexture;
  readonly extentMeters: number;
}

interface BroaderWavelets {
  readonly count: number;
  readonly featureScaleMeters: number;
  readonly heightScale: number;
}

export function createLakeDetailLayer(
  extentMeters: number,
  waveletCount: number,
  seed: number,
  directionX: number,
  directionZ: number,
  broaderWavelets?: BroaderWavelets
): LakeDetailLayer {
  const image = new LakeDetailImageBridge(extentMeters, waveletCount, seed >>> 0,
    new Float64Array([directionX, directionZ]), new Float64Array(broaderWavelets === undefined ? []
      : [broaderWavelets.count, broaderWavelets.featureScaleMeters, broaderWavelets.heightScale]));
  return uploadLakeDetailImage(image);
}

export function createProductionLakeDetailLayer(condition: LakeVisualCondition, far: boolean): LakeDetailLayer {
  const image = LakeDetailImageBridge.for_visual_condition(condition.windNorthMetersPerSecond,
    condition.windEastMetersPerSecond, condition.fetchMeters, condition.detailAmplitudeScale, condition.patternSeed, far);
  return uploadLakeDetailImage(image);
}

function uploadLakeDetailImage(image: LakeDetailImageBridge): LakeDetailLayer {
  try {
    const texture = new DataTexture(image.rgba(), image.size, image.size);
    texture.wrapS = RepeatWrapping;
    texture.wrapT = RepeatWrapping;
    texture.magFilter = LinearFilter;
    texture.minFilter = LinearMipmapLinearFilter;
    texture.generateMipmaps = true;
    texture.anisotropy = 8;
    texture.needsUpdate = true;
    return { texture, extentMeters: image.extent_meters };
  } finally {
    image.free();
  }
}

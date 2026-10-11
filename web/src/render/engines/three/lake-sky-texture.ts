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
import { LakeSkyImageBridge } from "../../../../pkg/birdman_game_wasm.js";
import { createLakeSkyCondition } from "../../contracts/lake-sky.js";
import type { LakeSkyCondition } from "../../contracts/lake-sky.js";

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

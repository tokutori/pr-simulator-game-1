import { ShaderChunk, Vector4 } from "three";
import type { DisplayAdjustment } from "../../../generated/application-contracts.js";

/**
 * Shared display adjustment uniform referenced by every tone-mapped scene
 * program: (exposure multiplier, contrast, saturation, contrast pivot).
 * Menu, head HUD and cursor materials are exempt through `toneMapped = false`.
 */
export const displayAdjustmentUniform: { value: Vector4 } = {
  value: new Vector4(1, 1, 1, 0.18)
};

const CUSTOM_TONE_MAPPING_STUB = "vec3 CustomToneMapping( vec3 color ) { return color; }";
const CUSTOM_TONE_MAPPING_GLSL = [
  "uniform vec4 displayAdjustment;",
  "vec3 CustomToneMapping( vec3 color ) {",
  "  color *= toneMappingExposure * displayAdjustment.x;",
  "  color = ( color - displayAdjustment.w ) * displayAdjustment.y + displayAdjustment.w;",
  "  float displayLuminance = dot( color, vec3( 0.2126, 0.7152, 0.0722 ) );",
  "  color = mix( vec3( displayLuminance ), color, displayAdjustment.z );",
  "  return color;",
  "}"
].join("\n");

let installed = false;

/**
 * Installs the shared adjustment formula as three.js CustomToneMapping.
 * The renderer owns the only WebGLRenderer, so installing once at adapter
 * construction keeps every scene program on the same definition.
 */
export function installDisplayAdjustment(): void {
  if (installed) return;
  const chunk = ShaderChunk.tonemapping_pars_fragment;
  if (!chunk.includes(CUSTOM_TONE_MAPPING_STUB)) {
    throw new Error("three.js custom tone mapping definition is unavailable");
  }
  ShaderChunk.tonemapping_pars_fragment = chunk.replace(
    CUSTOM_TONE_MAPPING_STUB,
    CUSTOM_TONE_MAPPING_GLSL
  );
  installed = true;
}

const retained = new Vector4(1, 1, 1, 0.18);

/** Applies a newly derived adjustment; returns whether the uniform changed. */
export function applyDisplayAdjustment(adjustment: DisplayAdjustment): boolean {
  const next = new Vector4(
    adjustment.exposure_multiplier,
    adjustment.contrast,
    adjustment.saturation,
    adjustment.contrast_pivot
  );
  if (retained.equals(next)) return false;
  retained.copy(next);
  displayAdjustmentUniform.value.copy(next);
  return true;
}

import { ShaderChunk } from "three";
import { afterAll, describe, expect, it } from "vitest";
import type { DisplayAdjustment } from "../../../generated/application-contracts.js";
import {
  applyDisplayAdjustment,
  displayAdjustmentUniform,
  installDisplayAdjustment
} from "./display-adjustment.js";

const originalChunk = ShaderChunk.tonemapping_pars_fragment;

afterAll(() => {
  ShaderChunk.tonemapping_pars_fragment = originalChunk;
});

const identity: DisplayAdjustment = {
  exposure_multiplier: 1,
  exposure_ev_shift: 0,
  contrast: 1,
  contrast_pivot: 0.18,
  saturation: 1
};

describe("display adjustment tone mapping", () => {
  it("installs the shared formula as the custom tone mapping definition", () => {
    installDisplayAdjustment();
    const chunk = ShaderChunk.tonemapping_pars_fragment;
    expect(chunk).not.toContain("vec3 CustomToneMapping( vec3 color ) { return color; }");
    expect(chunk).toContain("uniform vec4 displayAdjustment;");
    expect(chunk).toContain("color *= toneMappingExposure * displayAdjustment.x;");
    expect(chunk).toContain(
      "color = ( color - displayAdjustment.w ) * displayAdjustment.y + displayAdjustment.w;"
    );
    expect(chunk).toContain("dot( color, vec3( 0.2126, 0.7152, 0.0722 ) )");
    expect(chunk).toContain("mix( vec3( displayLuminance ), color, displayAdjustment.z )");
    installDisplayAdjustment();
    expect(ShaderChunk.tonemapping_pars_fragment).toBe(chunk);
  });

  it("publishes changed adjustments to the shared uniform only on change", () => {
    expect(applyDisplayAdjustment(identity)).toBe(false);
    expect(displayAdjustmentUniform.value.toArray()).toEqual([1, 1, 1, 0.18]);
    const adjusted: DisplayAdjustment = {
      exposure_multiplier: 1.25,
      exposure_ev_shift: -0.32192809488736235,
      contrast: 0.75,
      contrast_pivot: 0.18,
      saturation: 1.5
    };
    expect(applyDisplayAdjustment(adjusted)).toBe(true);
    expect(displayAdjustmentUniform.value.toArray()).toEqual([1.25, 0.75, 1.5, 0.18]);
    expect(applyDisplayAdjustment(adjusted)).toBe(false);
    expect(applyDisplayAdjustment(identity)).toBe(true);
    expect(displayAdjustmentUniform.value.toArray()).toEqual([1, 1, 1, 0.18]);
  });
});

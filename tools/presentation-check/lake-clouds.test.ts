import "./lake-model-fixture.js";
import { describe, expect, it, vi } from "vitest";
import { Color, DataTexture, NoColorSpace, RGFormat, SRGBColorSpace, Vector4 } from "three";
import { LakeCloudsBridge, lake_scene_ambient_color_srgb,
  lake_scene_sunlight_color_srgb } from "../../web/pkg/birdman_game_wasm.js";
import { createLakeClouds, disposeLakeClouds } from "../../web/src/render/engines/three/lake-clouds.js";

const condition = { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25,
  cloudBaseMeters: 1500, visibilityMeters: 25_000 };

describe("finite shared cloud projection", () => {
  it("projects the actual packet into one world mesh shared by main and reflection views", () => {
    const packet = new LakeCloudsBridge(135, 55, 0.25, 25_000, 1500);
    const mesh = createLakeClouds(condition);
    try {
      const positions = mesh.geometry.getAttribute("position");
      const offsets = mesh.geometry.getAttribute("cloudOffset");
      const uvs = mesh.geometry.getAttribute("uv");
      const shared = packet.vertices();
      expect(positions.count).toBe(shared.length / 7);
      expect(positions.count).toBeGreaterThan(0);
      expect(positions.count).toBeLessThanOrEqual(96);
      for (let index = 0; index < positions.count; index++) {
        expect([positions.getX(index), positions.getY(index), positions.getZ(index)])
          .toEqual([shared[index * 7 + 1], -(shared[index * 7 + 2] ?? NaN), -(shared[index * 7] ?? NaN)]);
        expect([offsets.getX(index), offsets.getY(index)]).toEqual(Array.from(shared.slice(index * 7 + 3, index * 7 + 5)));
        expect([uvs.getX(index), uvs.getY(index)]).toEqual(Array.from(shared.slice(index * 7 + 5, index * 7 + 7)));
      }
      expect(Array.from(mesh.geometry.index?.array ?? [])).toEqual(Array.from(packet.indices()));
      const atlas: unknown = mesh.material.uniforms.cloudAtlas?.value;
      if (!(atlas instanceof DataTexture)) throw new Error("Missing cloud atlas");
      if (!(atlas.image.data instanceof Uint8Array)) throw new Error("Missing atlas byte upload");
      expect(Buffer.from(atlas.image.data).equals(Buffer.from(packet.rgba()))).toBe(true);
      expect(atlas.colorSpace).toBe(SRGBColorSpace);
      const normals: unknown = mesh.material.uniforms.normalAtlas?.value;
      if (!(normals instanceof DataTexture) || !(normals.image.data instanceof Uint8Array)) throw new Error("Missing normal atlas");
      expect(Buffer.from(normals.image.data).equals(Buffer.from(packet.normal_rg()))).toBe(true);
      expect(normals.colorSpace).toBe(NoColorSpace);
      expect(normals.format).toBe(RGFormat);
      const sun: unknown = mesh.material.uniforms.sunDirection?.value;
      if (!(sun instanceof Vector4)) throw new Error("Missing world sunlight");
      const direction = packet.sun_direction_ned();
      expect(sun.toArray()).toEqual([direction[1], -(direction[2] ?? NaN), -(direction[0] ?? NaN), direction[3]]);
      for (const [name, srgb] of [["shadeTint", lake_scene_ambient_color_srgb()], ["sunTint", lake_scene_sunlight_color_srgb()]] as const) {
        const actual: unknown = mesh.material.uniforms[name]?.value;
        if (!(actual instanceof Color)) throw new Error("Missing shared light colour");
        const expected = new Color(srgb);
        expected.multiplyScalar(1 / Math.max(expected.r, expected.g, expected.b));
        expect(actual.toArray()).toEqual(expected.toArray());
      }
      expect(mesh.layers.isEnabled(0)).toBe(true);
      expect(mesh.layers.isEnabled(1)).toBe(true);
      expect(mesh.material.depthWrite).toBe(false);
      expect(mesh.position.toArray()).toEqual([0, 0, 0]);
    } finally { packet.free(); disposeLakeClouds(mesh); }
  });

  it.each([
    { sky: null, reason: "unrecorded" },
    { sky: { ...condition, cloudFraction: 0 }, reason: "clear" },
    { sky: { ...condition, cloudBaseMeters: 0 }, reason: "ground_level_unsupported" }
  ])("preserves explicit absence: $reason", ({ sky, reason }) => {
    const mesh = createLakeClouds(sky);
    try {
      expect(mesh.userData.cloudAvailability).toBe(reason);
      expect(mesh.visible).toBe(false);
      expect(mesh.geometry.getAttribute("position").count).toBe(0);
    } finally { disposeLakeClouds(mesh); }
  });

  it("releases the bridge on transfer failure and the GPU projection on disposal", () => {
    const free = vi.spyOn(LakeCloudsBridge.prototype, "free");
    const read = vi.spyOn(LakeCloudsBridge.prototype, "vertices").mockImplementationOnce(() => { throw new Error("transfer"); });
    try {
      expect(() => createLakeClouds(condition)).toThrow("transfer");
      expect(free).toHaveBeenCalledTimes(1);
    } finally { read.mockRestore(); free.mockRestore(); }
    const mesh = createLakeClouds(condition);
    const atlas: unknown = mesh.material.uniforms.cloudAtlas?.value;
    if (!(atlas instanceof DataTexture)) throw new Error("Missing cloud atlas");
    const normals: unknown = mesh.material.uniforms.normalAtlas?.value;
    if (!(normals instanceof DataTexture)) throw new Error("Missing normal atlas");
    const released = [vi.spyOn(atlas, "dispose"), vi.spyOn(normals, "dispose"),
      vi.spyOn(mesh.geometry, "dispose"), vi.spyOn(mesh.material, "dispose")];
    disposeLakeClouds(mesh);
    for (const release of released) expect(release).toHaveBeenCalledTimes(1);
  });

  it("attempts independent normal, geometry and material cleanup after an atlas listener fails", () => {
    const mesh = createLakeClouds(condition);
    const atlas: unknown = mesh.material.uniforms.cloudAtlas?.value;
    if (!(atlas instanceof DataTexture)) throw new Error("Missing cloud atlas");
    const cause = new Error("atlas listener");
    atlas.addEventListener("dispose", () => { throw cause; });
    const normals: unknown = mesh.material.uniforms.normalAtlas?.value;
    if (!(normals instanceof DataTexture)) throw new Error("Missing normal atlas");
    const normalCleanup = vi.spyOn(normals, "dispose");
    const geometry = vi.spyOn(mesh.geometry, "dispose");
    const material = vi.spyOn(mesh.material, "dispose");
    expect(() => { disposeLakeClouds(mesh); }).toThrow(AggregateError);
    expect(normalCleanup).toHaveBeenCalledTimes(1);
    expect(geometry).toHaveBeenCalledTimes(1);
    expect(material).toHaveBeenCalledTimes(1);
  });
});

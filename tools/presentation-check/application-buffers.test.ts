import { BufferAttribute } from "three";
import { describe, expect, it, vi } from "vitest";
import type { ApplicationBufferDescriptor, ApplicationMeshAsset, ApplicationWaterChunkAsset } from "../../web/src/generated/application-contracts.js";
import { ThreeApplicationBuffers } from "../../web/src/render/engines/three/application-buffers.js";

function buffersFixture() {
  const bytes = new Map<number, Uint8Array>();
  let serial = 0;
  const descriptor = (values: readonly number[], components: number, data_type: "f32" | "u32" = "f32"): ApplicationBufferDescriptor => {
    const raw = new Uint8Array(values.length * 4);
    const view = new DataView(raw.buffer);
    values.forEach((value, index) => {
      if (data_type === "f32") view.setFloat32(index * 4, value, true);
      else view.setUint32(index * 4, value, true);
    });
    const buffer_id = ++serial;
    bytes.set(buffer_id, raw);
    return { buffer_id, components, count: values.length / components, stride_bytes: components * 4, data_type };
  };
  const position = descriptor([0, 0, 0, 1, 0, 0, 0, 0, 1], 3);
  const normal = descriptor([0, 1, 0, 0, 1, 0, 0, 1, 0], 3);
  const index = descriptor([0, 2, 1], 1, "u32");
  const uv = descriptor([0, 0, 1, 0, 0, 1], 2);
  const mesh: ApplicationMeshAsset = { mesh_id: 7, name: "physical-upload-fixture", attachment: "aircraft", frustum_culled: false,
    local_pose: { position: { x: 0, y: 0, z: 0 }, orientation: { w: 1, x: 0, y: 0, z: 0 } },
    position, normal, index, uv, color: null, materials: [] };
  const buffers = new ThreeApplicationBuffers(id => {
    const values = bytes.get(id);
    if (values === undefined) throw new Error("Unavailable shared buffer");
    return values.slice();
  });
  return { bytes, descriptor, mesh, buffers };
}

describe("Shared GPU upload buffers", () => {
  it("copies little-endian attributes once and publishes complete reusable position/normal pairs", () => {
    const fixture = buffersFixture();
    const { buffers, mesh, bytes, descriptor } = fixture;
    try {
      const geometry = buffers.geometry(mesh);
      expect(buffers.geometry(mesh)).toBe(geometry);
      expect([...geometry.getIndex()?.array ?? []]).toEqual([0, 2, 1]);
      expect([...geometry.getAttribute("position").array]).toEqual([0, 0, 0, 1, 0, 0, 0, 0, 1]);
      const position = descriptor([0, -0.1, 0, 1, -0.1, 0, 0, -0.2, 1], 3);
      const normal = descriptor([0, 1, 0, 0, 1, 0, 0, 1, 0], 3);
      buffers.update(geometry, { mesh_id: 7, position, normal });
      const attribute = geometry.getAttribute("position");
      if (!(attribute instanceof BufferAttribute)) throw new Error("Unexpected interleaved upload");
      const array = attribute.array;
      const version = attribute.version;
      const positionBytes = bytes.get(position.buffer_id);
      if (positionBytes === undefined) throw new Error("Missing position upload");
      new DataView(positionBytes.buffer).setFloat32(4, -0.3, true);
      buffers.update(geometry, { mesh_id: 7, position, normal });
      expect(geometry.getAttribute("position")).toBe(attribute);
      expect(attribute.array).toBe(array);
      expect(attribute.version).toBe(version + 1);
      expect(array[1]).toBe(Math.fround(-0.3));
      const previous = [...array];
      const normalAttribute = geometry.getAttribute("normal");
      if (!(normalAttribute instanceof BufferAttribute)) throw new Error("Unexpected interleaved normal upload");
      const normalArray = normalAttribute.array;
      const normalVersion = normalAttribute.version;
      const previousNormal = [...normalArray];
      const normalBytes = bytes.get(normal.buffer_id);
      if (normalBytes === undefined) throw new Error("Missing normal upload");
      const enlargedNormal = new Uint8Array(normalBytes.length + 12);
      enlargedNormal.set(normalBytes);
      bytes.set(normal.buffer_id, enlargedNormal);
      new DataView(positionBytes.buffer).setFloat32(4, -0.4, true);
      expect(() => { buffers.update(geometry, { mesh_id: 7, position, normal: { ...normal, count: normal.count + 1 } }); }).toThrow("prepared topology");
      expect([...array]).toEqual(previous);
      expect(attribute.version).toBe(version + 1);
      expect(normalAttribute.array).toBe(normalArray);
      expect([...normalArray]).toEqual(previousNormal);
      expect(normalAttribute.version).toBe(normalVersion);
      const replacement = descriptor([2, 0, 0, 3, 0, 0, 2, 0, 1], 3);
      bytes.delete(normal.buffer_id);
      expect(() => { buffers.update(geometry, { mesh_id: 7, position: replacement, normal }); }).toThrow("Unavailable shared buffer");
      expect([...geometry.getAttribute("position").array]).toEqual(previous);
      expect(() => { buffers.geometry({ ...mesh, index: { ...mesh.index, buffer_id: replacement.buffer_id, components: 1 } }); }).toThrow("layout");
    } finally { buffers.dispose(); }
  });

  it("keeps candidate bounds independent and leases completed images until their final GPU owner releases them", () => {
    const { buffers, mesh, bytes, descriptor } = buffersFixture();
    const grid_spacing = descriptor([1, 1, 1], 1);
    const uv = mesh.uv;
    if (uv == null) throw new Error("Missing UV upload fixture");
    const chunk: ApplicationWaterChunkAsset = { position: mesh.position, normal: mesh.normal, index: mesh.index, uv, grid_spacing,
      bounds: { minimum: [-2, -3, -4], maximum: [5, 6, 7], sphere_center: [1, 2, 3], sphere_radius: 8 }, frustum_culled: true };
    const first = buffers.waterGeometry(chunk);
    const second = buffers.waterGeometry({ ...chunk, bounds: { ...chunk.bounds, sphere_radius: 12 } });
    try {
      expect(second).not.toBe(first);
      expect(second.getAttribute("position")).toBe(first.getAttribute("position"));
      expect(first.boundingBox?.min.toArray()).toEqual(chunk.bounds.minimum);
      expect(first.boundingSphere?.radius).toBe(8);
      expect(second.boundingSphere?.radius).toBe(12);
      bytes.set(100, new Uint8Array([1, 2, 3, 4]));
      const asset = { buffer_id: 100, encoding: "srgb_rgba8" as const, size: [1, 1] as const };
      const image = buffers.image(asset, true, 1);
      const dispose = vi.spyOn(image, "dispose");
      expect(buffers.image(asset, true, 1)).toBe(image);
      buffers.releaseImages([[100, 1]]);
      expect(dispose).not.toHaveBeenCalled();
      buffers.releaseImages([[100, 1]]);
      expect(dispose).toHaveBeenCalledOnce();
      buffers.releaseImages([[100, 1]]);
      expect(dispose).toHaveBeenCalledOnce();
      const detail = { ...asset, encoding: "linear_rgba8" as const };
      const high = buffers.image(detail, false, 8);
      const highDispose = vi.spyOn(high, "dispose");
      expect(buffers.image(detail, false, 8)).toBe(high);
      const low = buffers.image(detail, false, 4);
      const lowDispose = vi.spyOn(low, "dispose");
      expect(low).not.toBe(high);
      expect(low.image.data).toEqual(high.image.data);
      expect(high.anisotropy).toBe(8);
      expect(low.anisotropy).toBe(4);
      buffers.releaseImages([[100, 4]]);
      expect(lowDispose).toHaveBeenCalledOnce();
      expect(highDispose).not.toHaveBeenCalled();
      expect(high.anisotropy).toBe(8);
      buffers.releaseImages([[100, 8]]);
      expect(highDispose).not.toHaveBeenCalled();
      buffers.releaseImages([[100, 8]]);
      expect(highDispose).toHaveBeenCalledOnce();
      expect(image.anisotropy).toBe(1);
      bytes.set(101, new Uint8Array(20));
      const texture = buffers.wingTexture({ buffer: { buffer_id: 101, components: 4, count: 5, stride_bytes: 4, data_type: "u8" }, size: [2, 2], mip_level_count: 2 });
      expect(texture.anisotropy).toBe(8);
      expect(texture.generateMipmaps).toBe(false);
      expect(texture.mipmaps.map(mip => [mip.width, mip.height])).toEqual([[2, 2], [1, 1]]);
    } finally { first.dispose(); second.dispose(); buffers.dispose(); }
  });
});

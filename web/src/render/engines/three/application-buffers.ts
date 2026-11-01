import { Box3, BufferAttribute, BufferGeometry, ClampToEdgeWrapping, DataTexture, EquirectangularReflectionMapping,
  LinearFilter, LinearMipmapLinearFilter, NoColorSpace, RepeatWrapping, RGBAFormat, Sphere, SRGBColorSpace, UnsignedByteType, Vector3 } from "three";
import type { ApplicationBufferDescriptor, ApplicationGeometryTexture, ApplicationImageAsset, ApplicationMeshAsset,
  ApplicationMeshUpdate, ApplicationWaterChunkAsset } from "../../../generated/application-contracts.js";

export type ImageLease = readonly [bufferId: number, anisotropy: number];

export class ThreeApplicationBuffers {
  private readonly attributes = new Map<number, BufferAttribute>();
  private readonly geometries = new Map<number, BufferGeometry>();
  private readonly textures = new Map<string, DataTexture>();
  private readonly imageLeases = new Map<string, number>();

  constructor(private readonly bytes: (id: number) => Uint8Array) {}

  private checkedBytes(descriptor: ApplicationBufferDescriptor): Uint8Array {
    const bytes = this.bytes(descriptor.buffer_id);
    const count = descriptor.count * descriptor.components;
    const width = descriptor.data_type === "u8" ? 1 : 4;
    if (descriptor.stride_bytes !== descriptor.components * width || bytes.length !== count * width) throw new Error("Shared GPU buffer layout does not match its bytes");
    return bytes;
  }

  private copyFloat32(view: DataView, values: Float32Array): void {
    for (let index = 0; index < values.length; index++) values[index] = view.getFloat32(index * 4, true);
  }

  private read(descriptor: ApplicationBufferDescriptor): Float32Array | Uint32Array | Uint8Array {
    const bytes = this.checkedBytes(descriptor);
    const count = descriptor.count * descriptor.components;
    if (descriptor.data_type === "u8") return bytes;
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (descriptor.data_type === "f32") {
      const values = new Float32Array(count);
      this.copyFloat32(view, values);
      return values;
    }
    const values = new Uint32Array(count);
    for (let index = 0; index < count; index++) values[index] = view.getUint32(index * 4, true);
    return values;
  }

  private attribute(descriptor: ApplicationBufferDescriptor): BufferAttribute {
    let attribute = this.attributes.get(descriptor.buffer_id);
    if (attribute === undefined) {
      attribute = new BufferAttribute(this.read(descriptor), descriptor.components);
      this.attributes.set(descriptor.buffer_id, attribute);
    }
    return attribute;
  }

  geometry(asset: ApplicationMeshAsset): BufferGeometry {
    let geometry = this.geometries.get(asset.index.buffer_id);
    if (geometry === undefined) {
      geometry = this.meshAttributes(asset.position, asset.normal, asset.index);
      if (asset.uv != null) geometry.setAttribute("uv", this.attribute(asset.uv));
      if (asset.color != null) geometry.setAttribute("color", this.attribute(asset.color));
      this.geometries.set(asset.index.buffer_id, geometry);
    }
    return geometry;
  }

  waterGeometry(asset: ApplicationWaterChunkAsset): BufferGeometry {
    const geometry = this.meshAttributes(asset.position, asset.normal, asset.index);
    geometry.setAttribute("uv", this.attribute(asset.uv));
    geometry.setAttribute("aGridSpacing", this.attribute(asset.grid_spacing));
    geometry.boundingBox = new Box3(new Vector3(...asset.bounds.minimum), new Vector3(...asset.bounds.maximum));
    geometry.boundingSphere = new Sphere(new Vector3(...asset.bounds.sphere_center), asset.bounds.sphere_radius);
    return geometry;
  }

  private meshAttributes(position: ApplicationBufferDescriptor, normal: ApplicationBufferDescriptor, index: ApplicationBufferDescriptor): BufferGeometry {
    const geometry = new BufferGeometry();
    geometry.setAttribute("position", this.attribute(position));
    geometry.setAttribute("normal", this.attribute(normal));
    geometry.setIndex(this.attribute(index));
    return geometry;
  }

  update(geometry: BufferGeometry, update: ApplicationMeshUpdate): void {
    const uploads = ([["position", update.position], ["normal", update.normal]] as const).map(([name, descriptor]) => {
      if (descriptor.data_type !== "f32") throw new Error("Dynamic GPU attributes require Float32 uploads");
      const bytes = this.checkedBytes(descriptor);
      const retained = this.attributes.get(descriptor.buffer_id);
      const attribute = retained ?? new BufferAttribute(new Float32Array(descriptor.count * descriptor.components), descriptor.components);
      if (!(attribute.array instanceof Float32Array) || attribute.array.length !== descriptor.count * descriptor.components
          || attribute.itemSize !== descriptor.components) throw new Error("Dynamic GPU attribute layout differs from its prepared topology");
      return { name, descriptor, attribute, values: attribute.array, view: new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength), retained };
    });
    for (const { name, descriptor, attribute, values, view, retained } of uploads) {
      this.copyFloat32(view, values);
      if (retained === undefined) this.attributes.set(descriptor.buffer_id, attribute);
      else attribute.needsUpdate = true;
      geometry.setAttribute(name, attribute);
    }
  }

  image(asset: ApplicationImageAsset, sky: boolean, anisotropy: number): DataTexture {
    const key = `${asset.buffer_id}:${anisotropy}`;
    let texture = this.textures.get(key);
    if (texture === undefined) {
      texture = new DataTexture(this.bytes(asset.buffer_id), asset.size[0], asset.size[1], RGBAFormat, UnsignedByteType);
      texture.colorSpace = asset.encoding === "srgb_rgba8" ? SRGBColorSpace : NoColorSpace;
      texture.wrapS = RepeatWrapping;
      texture.wrapT = sky ? ClampToEdgeWrapping : RepeatWrapping;
      texture.magFilter = LinearFilter;
      texture.minFilter = LinearMipmapLinearFilter;
      texture.generateMipmaps = true;
      if (sky) texture.mapping = EquirectangularReflectionMapping;
      texture.anisotropy = anisotropy;
      texture.needsUpdate = true;
      this.textures.set(key, texture);
    }
    this.imageLeases.set(key, (this.imageLeases.get(key) ?? 0) + 1);
    return texture;
  }

  releaseImages(leases: readonly ImageLease[]): void {
    const failures: unknown[] = [];
    for (const [id, anisotropy] of leases) {
      const key = `${id}:${anisotropy}`;
      const count = this.imageLeases.get(key);
      if (count === undefined) continue;
      if (count > 1) this.imageLeases.set(key, count - 1);
      else {
        this.imageLeases.delete(key);
        const texture = this.textures.get(key);
        this.textures.delete(key);
        try { texture?.dispose(); } catch (cause) { failures.push(cause); }
      }
    }
    if (failures.length > 0) throw new AggregateError(failures, "Shared image cleanup failed");
  }

  wingTexture(asset: ApplicationGeometryTexture): DataTexture {
    const key = String(asset.buffer.buffer_id);
    let texture = this.textures.get(key);
    if (texture !== undefined) return texture;
    const bytes = this.bytes(asset.buffer.buffer_id);
    const mipmaps: { data: Uint8Array; width: number; height: number }[] = [];
    let offset = 0;
    let [width, height] = asset.size;
    for (let level = 0; level < asset.mip_level_count; level++) {
      const length = width * height * 4;
      mipmaps.push({ width, height, data: bytes.subarray(offset, offset + length) });
      offset += length;
      width = Math.max(1, Math.floor(width / 2));
      height = Math.max(1, Math.floor(height / 2));
    }
    if (offset !== bytes.length || mipmaps.length === 0) throw new Error("Shared wing texture mip layout does not match its bytes");
    texture = new DataTexture(bytes.subarray(0, asset.size[0] * asset.size[1] * 4), asset.size[0], asset.size[1], RGBAFormat, UnsignedByteType);
    texture.mipmaps = mipmaps;
    texture.colorSpace = SRGBColorSpace;
    texture.magFilter = LinearFilter;
    texture.minFilter = LinearMipmapLinearFilter;
    texture.generateMipmaps = false;
    texture.anisotropy = 8;
    texture.needsUpdate = true;
    this.textures.set(key, texture);
    return texture;
  }

  dispose(): void {
    const failures: unknown[] = [];
    for (const resource of [...this.geometries.values(), ...this.textures.values()]) {
      try { resource.dispose(); } catch (cause) { failures.push(cause); }
    }
    this.geometries.clear();
    this.attributes.clear();
    this.textures.clear();
    this.imageLeases.clear();
    if (failures.length > 0) throw new AggregateError(failures, "Shared GPU buffer cleanup failed");
  }
}

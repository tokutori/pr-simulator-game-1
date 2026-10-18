import { BufferGeometry, Color, DataTexture, DoubleSide, Float32BufferAttribute, LinearFilter, Mesh,
  RGBAFormat, RGFormat, ShaderMaterial, SRGBColorSpace, UnsignedByteType, Vector2, Vector4 } from "three";
import { LakeCloudsBridge, lake_scene_ambient_color_srgb,
  lake_scene_sunlight_color_srgb } from "../../../../pkg/birdman_game_wasm.js";
import type { LakeSkyCondition } from "../../contracts/lake-sky.js";

export function createLakeClouds(condition: LakeSkyCondition | null): Mesh<BufferGeometry, ShaderMaterial> {
  const packet = condition === null ? LakeCloudsBridge.unrecorded() : new LakeCloudsBridge(
    condition.sunAzimuthDegrees, condition.sunElevationDegrees, condition.cloudFraction,
    condition.visibilityMeters, condition.cloudBaseMeters);
  const geometry = new BufferGeometry();
  let atlas: DataTexture | null = null;
  let normals: DataTexture | null = null;
  try {
    const vertices = packet.vertices();
    const positions: number[] = [];
    const offsets: number[] = [];
    const uvs: number[] = [];
    const component = (index: number): number => {
      const value = vertices[index];
      if (value === undefined || !Number.isFinite(value)) throw new RangeError("Invalid shared cloud vertex packet");
      return value;
    };
    for (let offset = 0; offset < vertices.length; offset += 7) {
      positions.push(component(offset + 1), -component(offset + 2), -component(offset));
      offsets.push(component(offset + 3), component(offset + 4));
      uvs.push(component(offset + 5), component(offset + 6));
    }
    geometry.setAttribute("position", new Float32BufferAttribute(positions, 3));
    geometry.setAttribute("cloudOffset", new Float32BufferAttribute(offsets, 2));
    geometry.setAttribute("uv", new Float32BufferAttribute(uvs, 2));
    geometry.setIndex(Array.from(packet.indices()));
    atlas = new DataTexture(packet.rgba(), packet.width, packet.height, RGBAFormat, UnsignedByteType);
    atlas.colorSpace = SRGBColorSpace;
    atlas.minFilter = LinearFilter;
    atlas.magFilter = LinearFilter;
    atlas.needsUpdate = true;
    normals = new DataTexture(packet.normal_rg(), packet.width, packet.height, RGFormat, UnsignedByteType);
    normals.minFilter = LinearFilter;
    normals.magFilter = LinearFilter;
    normals.needsUpdate = true;
    const sun = packet.sun_direction_ned();
    const sunComponent = (index: number): number => {
      const value = sun[index];
      if (value === undefined || !Number.isFinite(value)) throw new RangeError("Invalid shared cloud sunlight packet");
      return value;
    };
    const tint = (srgb: number): Color => {
      const color = new Color(srgb);
      return color.multiplyScalar(1 / Math.max(color.r, color.g, color.b));
    };
    const visibility = condition?.visibilityMeters ?? 80_000;
    const material = new ShaderMaterial({
      uniforms: { cloudAtlas: { value: atlas }, normalAtlas: { value: normals },
        sunDirection: { value: new Vector4(sunComponent(1), -sunComponent(2), -sunComponent(0), sunComponent(3)) },
        shadeTint: { value: tint(lake_scene_ambient_color_srgb()) },
        sunTint: { value: tint(lake_scene_sunlight_color_srgb()) },
        hazeRange: { value: new Vector2(visibility * 0.175, visibility) } },
      vertexShader: `
        attribute vec2 cloudOffset;
        uniform vec4 sunDirection;
        varying vec2 cloudUv;
        varying float cloudDistance;
        varying vec3 cloudSun;
        void main() {
          vec3 toEye = cameraPosition - position;
          vec3 right = vec3(toEye.z, 0.0, -toEye.x);
          right = length(right) > 0.0001 ? normalize(right) : vec3(1.0, 0.0, 0.0);
          vec3 worldPoint = position + right * cloudOffset.x + vec3(0.0, cloudOffset.y, 0.0);
          vec4 centerView = viewMatrix * vec4(worldPoint, 1.0);
          cloudUv = uv;
          cloudDistance = length(centerView.xyz);
          vec3 front = vec3(-right.z, 0.0, right.x);
          cloudSun = vec3(dot(sunDirection.xyz, right), sunDirection.y, dot(sunDirection.xyz, front));
          gl_Position = projectionMatrix * centerView;
        }`,
      fragmentShader: `
        uniform sampler2D cloudAtlas;
        uniform sampler2D normalAtlas;
        uniform vec4 sunDirection;
        uniform vec3 shadeTint;
        uniform vec3 sunTint;
        uniform vec2 hazeRange;
        varying vec2 cloudUv;
        varying float cloudDistance;
        varying vec3 cloudSun;
        void main() {
          vec4 cloud = texture2D(cloudAtlas, cloudUv);
          vec2 normalXY = texture2D(normalAtlas, cloudUv).rg * 2.0 - 1.0;
          vec3 normal = normalize(vec3(normalXY, sqrt(max(1.0 - dot(normalXY, normalXY), 0.0))));
          float sunlight = clamp(dot(normal, cloudSun), 0.0, 1.0) * sunDirection.w;
          vec3 color = cloud.rgb * mix(shadeTint, sunTint, sunlight);
          float haze = smoothstep(hazeRange.x, hazeRange.y, cloudDistance);
          gl_FragColor = vec4(color, cloud.a * (1.0 - haze));
          #include <colorspace_fragment>
        }`,
      transparent: true, depthWrite: false, side: DoubleSide, toneMapped: false
    });
    const mesh = new Mesh(geometry, material);
    mesh.name = "world-clouds";
    mesh.userData.cloudAvailability = packet.availability;
    mesh.frustumCulled = false;
    mesh.visible = vertices.length > 0;
    mesh.layers.enable(1);
    return mesh;
  } catch (cause) {
    const failures: unknown[] = [cause];
    for (const release of [() => { geometry.dispose(); }, () => { atlas?.dispose(); }, () => { normals?.dispose(); }]) {
      try { release(); } catch (cleanupCause) { failures.push(cleanupCause); }
    }
    if (failures.length > 1) throw new AggregateError(failures, "Cloud projection failed during transfer and cleanup", { cause });
    throw cause;
  } finally { packet.free(); }
}

export function disposeLakeClouds(clouds: Mesh<BufferGeometry, ShaderMaterial>): void {
  const atlas: unknown = clouds.material.uniforms.cloudAtlas?.value;
  const normals: unknown = clouds.material.uniforms.normalAtlas?.value;
  const failures: unknown[] = [];
  for (const release of [() => { if (atlas instanceof DataTexture) atlas.dispose(); },
    () => { if (normals instanceof DataTexture) normals.dispose(); },
    () => { clouds.geometry.dispose(); }, () => { clouds.material.dispose(); }]) {
    try { release(); } catch (cause) { failures.push(cause); }
  }
  if (failures.length > 0) throw new AggregateError(failures, "Cloud projection cleanup failed");
}

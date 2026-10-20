import "./lake-model-fixture.js";
import { describe, expect, it, vi } from "vitest";
import { Frustum, Matrix4, Mesh, MeshBasicMaterial, PerspectiveCamera, Vector3, Vector4 } from "three";
import { lakeWaterQualityProfile, DEFAULT_LAKE_VISUAL_CONDITION } from "../../web/src/render/contracts/lake-water.js";
import { createLakeWaveProjection } from "../../web/src/render/engines/three/lake-wave-projection.js";
import { LakeDetailImageBridge, LakeWaveProjectionBridge } from "../../web/pkg/birdman_game_wasm.js";
import { createLakeChunks, createLakeGeometry, disposeLakeChunks, lakeWaterDisplacementBounds, updateLakeChunkBounds } from "../../web/src/render/engines/three/lake-water-geometry.js";

describe("shared lake water consumer", () => {
  it("uses the existing component and mesh budgets at each quality level", () => {
    for (const [quality, componentCount, meshSegments] of [["low", 6, 96], ["medium", 10, 144], ["high", 18, 224]] as const) {
      const projection = createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, quality);
      expect(lakeWaterQualityProfile(quality)).toEqual({ componentCount, meshSegments });
      expect(projection.waveCount).toBe(componentCount);
      expect(projection.waveOmegaPhase.slice(componentCount).flat()).toEqual(new Array<number>((24 - componentCount) * 4).fill(0));
    }
  });

  it("preserves representative frequency bands without a TypeScript spectrum implementation", () => {
    const high = createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, "high");
    for (const [quality, indices] of [["low", [0, 4, 8, 9, 13, 17]], ["medium", [0, 4, 5, 6, 8, 9, 10, 13, 14, 17]]] as const) {
      const projection = createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, quality);
      expect(projection.waveOmegaPhase.slice(0, projection.waveCount)).toEqual(indices.map((index) => high.waveOmegaPhase[index]));
    }
  });

  it("keeps geometric phase independent of the detail seed", () => {
    const first = createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, "high");
    const second = createLakeWaveProjection({ ...DEFAULT_LAKE_VISUAL_CONDITION, patternSeed: 41 }, "high");
    expect(second).toEqual(first);
  });

  it("rejects malformed input at the generated WASM boundary", () => {
    expect(() => new LakeWaveProjectionBridge(Number.NaN, 0, 600, 0)).toThrow();
    expect(() => new LakeWaveProjectionBridge(1, 0, 600, 3)).toThrow();
    expect(() => LakeDetailImageBridge.for_visual_condition(1, 0, 600, 1, 0.5, false)).toThrow();
    expect(() => new LakeDetailImageBridge(64, 48.5, 123, new Float64Array([1, 0]), new Float64Array())).toThrow();
    expect(() => new LakeDetailImageBridge(64, 48, 123, new Float64Array([1]), new Float64Array())).toThrow();
    expect(() => new LakeDetailImageBridge(64, 48, 123, new Float64Array([1, 0]), new Float64Array([1, 2]))).toThrow();
  });
});

describe("lake water chunk geometry", () => {
  it.each(["low", "medium", "high"] as const)("preserves every %s triangle and Float32 attribute at shared boundaries", quality => {
    const segments = lakeWaterQualityProfile(quality).meshSegments;
    const projection = createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, quality);
    const original = createLakeGeometry(segments);
    const chunks = createLakeChunks(segments, lakeWaterDisplacementBounds(projection));
    try {
      const positions = original.getAttribute("position");
      const lookup = new Map(Array.from({ length: positions.count }, (_, index) => [`${String(positions.getX(index))}:${String(positions.getY(index))}`, index]));
      const originalIndices = original.index;
      if (originalIndices === null) throw new Error("Missing original triangle indices");
      const triangles = new Set<string>();
      for (let index = 0; index < originalIndices.count; index += 3) {
        triangles.add([0, 1, 2].map(offset => originalIndices.getX(index + offset)).join(":"));
      }
      let submittedVertices = 0;
      let submittedTriangles = 0;
      for (const geometry of chunks) {
        const vertices = geometry.getAttribute("position");
        const indices = geometry.index;
        if (indices === null) throw new Error("Missing chunk triangle indices");
        const sourceIndices = Array.from({ length: vertices.count }, (_, index) => {
          const source = lookup.get(`${String(vertices.getX(index))}:${String(vertices.getY(index))}`);
          if (source === undefined) throw new Error("Chunk added a vertex outside the original grid");
          for (const [name, attribute] of Object.entries(original.attributes)) {
            for (let component = 0; component < attribute.itemSize; component++) {
              if (attribute.getComponent(source, component) !== geometry.getAttribute(name).getComponent(index, component)) {
                throw new Error(`Chunk changed ${name} at original vertex ${String(source)}`);
              }
            }
          }
          return source;
        });
        expect(vertices.count).toBeLessThanOrEqual((Math.ceil(segments / 4) + 1) ** 2);
        for (let index = 0; index < indices.count; index += 3) {
          const triangle = [0, 1, 2].map(offset => sourceIndices[indices.getX(index + offset)]).join(":");
          if (!triangles.delete(triangle)) throw new Error(`Duplicate, inverted or additional triangle ${triangle}`);
        }
        submittedVertices += vertices.count;
        submittedTriangles += indices.count / 3;
      }
      expect(chunks).toHaveLength(16);
      expect(submittedVertices).toBe(16 * (segments / 4 + 1) ** 2);
      expect(submittedTriangles).toBe(segments * segments * 2);
      expect(triangles.size).toBe(0);
    } finally { original.dispose(); disposeLakeChunks(chunks); }
  });

  it("bounds horizontal waves, crest height and maximum detail in shader XZ coordinates", () => {
    const displacement = lakeWaterDisplacementBounds(createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, "high"));
    const chunks = createLakeChunks(224, displacement);
    try {
      for (const geometry of chunks) {
        const positions = geometry.getAttribute("position");
        const box = geometry.boundingBox;
        const sphere = geometry.boundingSphere;
        if (box === null || sphere === null) throw new Error("Missing shader-space culling bounds");
        for (const index of [0, positions.count - 1]) for (const horizontal of [-1, 1]) for (const vertical of [-1, 1]) {
          const point = new Vector3(positions.getX(index) + horizontal * displacement.horizontal,
            vertical * displacement.vertical, -positions.getY(index) - horizontal * displacement.horizontal);
          expect(box.containsPoint(point)).toBe(true);
          expect(sphere.containsPoint(point)).toBe(true);
        }
      }
      const original = chunks.map(geometry => geometry.boundingBox?.clone());
      updateLakeChunkBounds(chunks, { horizontal: displacement.horizontal * 4, vertical: displacement.vertical * 4 });
      updateLakeChunkBounds(chunks, displacement);
      expect(chunks.map(geometry => geometry.boundingBox)).toEqual(original);
      expect(() => { updateLakeChunkBounds(chunks, { horizontal: Number.NaN, vertical: 1 }); }).toThrow(RangeError);
    } finally { disposeLakeChunks(chunks); }
  });

  it("culls offscreen chunks without submitting the complete grid", () => {
    const chunks = createLakeChunks(224, lakeWaterDisplacementBounds(createLakeWaveProjection(DEFAULT_LAKE_VISUAL_CONDITION, "high")));
    const material = new MeshBasicMaterial();
    try {
      const camera = new PerspectiveCamera(60, 16 / 9, 0.05, 10000);
      camera.position.set(0, 10, 0);
      camera.lookAt(0, 0, -1000);
      camera.updateMatrixWorld(true);
      const frustum = new Frustum().setFromProjectionMatrix(new Matrix4().multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse));
      const visible = chunks.filter(geometry => {
        const mesh = new Mesh(geometry, material);
        mesh.updateMatrixWorld(true);
        return mesh.frustumCulled && frustum.intersectsObject(mesh);
      });
      expect(visible.length).toBeGreaterThan(0);
      expect(visible.length).toBeLessThan(chunks.length);
      expect(visible.reduce((sum, geometry) => sum + geometry.getAttribute("position").count, 0)).toBeLessThan(225 ** 2);
    } finally { disposeLakeChunks(chunks); material.dispose(); }
  });

  it.each([1, 2])("attempts every chunk release after %i release failures", failures => {
    const chunks = createLakeChunks(96, { horizontal: 1, vertical: 2 });
    const releases = chunks.map(geometry => vi.spyOn(geometry, "dispose"));
    for (const release of releases.slice(0, failures)) release.mockImplementation(() => { throw new Error("Injected release failure"); });
    expect(() => { disposeLakeChunks(chunks); }).toThrow(failures === 1 ? "Injected release failure" : AggregateError);
    for (const release of releases) expect(release).toHaveBeenCalledOnce();
    for (const release of releases) release.mockRestore();
    disposeLakeChunks(chunks);
  });

  it("commutes reflection projection with perspective-correct world-position interpolation", () => {
    const transform = new Matrix4().set(2, 0, 1, 3, 0, 3, 2, -4, 1, 0, -1, 2, 0, 1, -2, 1);
    const points = [new Vector4(2, 3, -4, 1), new Vector4(-3, 0, -20, 1), new Vector4(8, -1, -7, 1)];
    const weights = [0.2 / 4, 0.3 / 20, 0.5 / 7];
    const total = weights.reduce((sum, weight) => sum + weight, 0);
    const interpolate = (values: readonly Vector4[]) => values.reduce((sum, value, index) => sum.addScaledVector(value, (weights[index] ?? 0) / total), new Vector4(0, 0, 0, 0));
    const before = interpolate(points.map(point => point.clone().applyMatrix4(transform)));
    const after = interpolate(points).applyMatrix4(transform);
    for (const [index, value] of before.toArray().entries()) expect(value).toBeCloseTo(after.toArray()[index] ?? Number.NaN, 12);
  });
});

import "./lake-model-fixture.js";
import { describe, expect, it, vi } from "vitest";
import { BufferAttribute, CylinderGeometry, DataTexture, Group, InstancedMesh, Matrix4, Mesh, MeshLambertMaterial, Vector3 } from "three";
import { createBirdmanAirframe } from "../../web/src/render/engines/three/birdman-airframe.js";
import { CockpitGeometryBridge, TailGeometryBridge, WingGeometryBridge } from "../../web/pkg/birdman_game_wasm.js";

describe("synthetic Birdman airframe", () => {
  it("projects shared registered tail materials and ribs under the physical incidence pivots", () => {
    const free = vi.spyOn(TailGeometryBridge.prototype, "free");
    const airframe = createBirdmanAirframe();
    try {
      expect(free).toHaveBeenCalledTimes(1);
      const tails = ["horizontal-tail-incidence", "vertical-tail-incidence"].map((name) =>
        airframe.root.getObjectByName(name) as Group);
      const source = new TailGeometryBridge();
      try {
        for (const [index, tail] of tails.entries()) {
          const positions = source.surface_positions(index);
          const codes = source.surface_materials(index);
          const patches = tail.children.filter((part) => part.name !== "tail-rib") as Mesh[];
          expect(patches).toHaveLength(index === 0 ? 9 : 5);
          for (const [patch, mesh] of patches.entries()) {
            expect(Array.from(mesh.geometry.getAttribute("position").array))
              .toEqual(Array.from(positions.subarray(patch * 12, (patch + 1) * 12)));
            const material = mesh.material as MeshLambertMaterial;
            expect(material.color.getHex()).toBe(source.material_colors()[codes[patch] ?? -1]);
            expect(material.opacity).toBeCloseTo(source.material_opacities()[codes[patch] ?? -1] ?? -1, 6);
            expect(material.transparent).toBe(codes[patch] === 1);
            expect(material.depthWrite).toBe(codes[patch] !== 1);
          }
          const ribs = tail.children.filter((part) => part.name === "tail-rib") as Mesh[];
          const endpoints = source.rib_endpoints(index);
          expect(ribs).toHaveLength(index === 0 ? 7 : 3);
          for (const [rib, mesh] of ribs.entries()) {
            mesh.updateMatrix();
            for (const [end, height] of [-0.5, 0.5].entries()) {
              const actual = new Vector3(0, height, 0).applyMatrix4(mesh.matrix).toArray();
              for (const [axis, value] of actual.entries()) expect(value)
                .toBeCloseTo(endpoints[rib * 6 + end * 3 + axis] ?? Number.NaN, 11);
            }
          }
        }
      } finally { source.free(); }
      const retained = tails.flatMap((tail) => tail.children.map((part) => (part as Mesh).geometry));
      airframe.setVisualState({ kind: "accepted", factor: 1.6 }, { layout: "tail_incidence", physicalIncidence: {
        horizontalTailRadians: 0.12, verticalTailRadians: -0.08 } },
      { kind: "available", value: { horizontalTailArmMeters: 3.6 } });
      expect(tails[0]?.rotation.x).toBeCloseTo(0.12);
      expect(tails[1]?.rotation.y).toBeCloseTo(0.08);
      expect(tails.flatMap((tail) => tail.children.map((part) => (part as Mesh).geometry))).toEqual(retained);
    } finally { airframe.dispose(); free.mockRestore(); }
  });

  it("retains owned static tail bulk after bridge release", () => {
    const source = new TailGeometryBridge();
    const positions = source.surface_positions(0);
    const initial = positions[0];
    positions[0] = 100;
    expect(source.surface_positions(0)[0]).toBe(initial);
    const ribs = source.rib_endpoints(1);
    source.free();
    expect(positions[0]).toBe(100);
    expect(ribs.length).toBe(18);
    expect(Array.from(ribs).every(Number.isFinite)).toBe(true);
  });

  it("decodes the wing decal once, updates both owned textures and cancels disposed callbacks", () => {
    class Decal {
      naturalWidth = 2171;
      naturalHeight = 724;
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      src = "";
    }
    const decoded: Decal[] = [];
    const createImage = vi.fn(function () { const image = new Decal(); decoded.push(image); return image; });
    const drawImage = vi.fn();
    const rgba = new Uint8ClampedArray(2171 * 724 * 4).fill(255);
    const canvas = { width: 0, height: 0, getContext: () => ({ drawImage,
      getImageData: () => ({ data: rgba }) }) };
    vi.stubGlobal("Image", createImage);
    vi.stubGlobal("document", { createElement: () => canvas });
    const ready = vi.fn();
    const warning = vi.spyOn(console, "warn").mockImplementation(() => undefined);
    const airframe = createBirdmanAirframe(ready);
    try {
      expect(createImage).toHaveBeenCalledTimes(1);
      const textureFor = (name: string): DataTexture => {
        const mesh = airframe.root.getObjectByName(name) as Mesh;
        const material = (mesh.material as MeshLambertMaterial[])[0];
        if (!(material?.map instanceof DataTexture)) throw new Error("Missing indigo texture");
        return material.map;
      };
      const left = textureFor("left-wing");
      const right = textureFor("right-wing");
      const leftData = left.image.data as Uint8Array;
      const rightData = right.image.data as Uint8Array;
      for (const texture of [left, right]) {
        expect([texture.image.width, texture.image.height, texture.image.data?.byteLength])
          .toEqual([1024, 256, 1024 * 256 * 4]);
      }
      const lowerHalf = leftData.slice(1024 * 128 * 4);
      const image = decoded[0];
      if (image === undefined || image.onload === null) throw new Error("Missing decal load callback");
      const late = image.onload;
      late();
      expect(drawImage).toHaveBeenCalledExactlyOnceWith(image, 0, 0);
      expect(ready).toHaveBeenCalledTimes(1);
      for (const pixels of [leftData, rightData]) {
        const index = (50 * 1024 + 512) * 4;
        expect(Array.from(pixels.slice(index, index + 4))).toEqual([220, 224, 218, 255]);
      }
      expect(lowerHalf.every((value, index) => value === leftData[1024 * 128 * 4 + index])).toBe(true);
      const retained = leftData.slice();
      image.naturalWidth = 1;
      late();
      expect(warning).toHaveBeenCalledTimes(1);
      expect(leftData.byteLength).toBe(retained.byteLength);
      expect(retained.every((value, index) => value === leftData[index])).toBe(true);
      expect(ready).toHaveBeenCalledTimes(1);
      const disposeLeft = vi.spyOn(left, "dispose");
      const disposeRight = vi.spyOn(right, "dispose");
      airframe.dispose();
      late();
      expect(disposeLeft).toHaveBeenCalledTimes(1);
      expect(disposeRight).toHaveBeenCalledTimes(1);
      expect(image.onload).toBeNull();
      expect(image.onerror).toBeNull();
      expect(ready).toHaveBeenCalledTimes(1);
    } finally { airframe.dispose(); warning.mockRestore(); vi.unstubAllGlobals(); }
  });

  it("retains the frozen Web T-shaped carbon structure through shared bulk geometry", () => {
    const airframe = createBirdmanAirframe();
    try {
      const beam = airframe.root.getObjectByName("single-carbon-main-beam") as Mesh<CylinderGeometry, MeshLambertMaterial>;
      const spar = airframe.root.getObjectByName("main-wing-spar") as Group;
      expect(beam.geometry).toBeInstanceOf(CylinderGeometry);
      expect(beam.geometry.parameters.radiusTop).toBe(0.075);
      expect(beam.material.color.getHex()).toBe(0x303b3a);
      expect(spar.children).toHaveLength(1);
      const segments = spar.children[0] as InstancedMesh<CylinderGeometry, MeshLambertMaterial>;
      expect(segments).toBeInstanceOf(InstancedMesh);
      expect(segments.count).toBe(24);
      for (const factor of [0.3, 1.6]) {
        airframe.setVisualState({ kind: "accepted", factor }, { layout: "absent" });
        beam.updateMatrix();
        for (const [height, expected] of [[-0.5, [0, -0.73, -0.1375]], [0.5, [0, -0.57, 4.75]]] as const) {
          const actual = new Vector3(0, height, 0).applyMatrix4(beam.matrix).toArray();
          for (const [index, coordinate] of actual.entries()) expect(coordinate).toBeCloseTo(expected[index] ?? Number.NaN, 11);
        }
        const matrix = new Matrix4();
        segments.getMatrixAt(0, matrix);
        const connection = new Vector3(0, -0.5, 0).applyMatrix4(matrix);
        expect(connection.distanceTo(new Vector3(0, -0.73, -0.1375))).toBeLessThan(1e-6);
        expect(segments.material).toBe(beam.material);
      }
    } finally { airframe.dispose(); }
  });

  it("projects the fitted shared frame and unchanged cockpit shell and releases its bulk owner immediately", () => {
    const expected = new CockpitGeometryBridge();
    const endpoints = expected.rod_endpoints();
    expected.free();
    const free = vi.spyOn(CockpitGeometryBridge.prototype, "free");
    const airframe = createBirdmanAirframe();
    try {
      expect(free).toHaveBeenCalledTimes(1);
      const frame = airframe.root.getObjectByName("open-cockpit-frame") as Group;
      expect(frame.children).toHaveLength(18);
      expect(airframe.root.children.filter((part) => part.name === "canopy-rim-segment")).toHaveLength(16);
      for (const [rodIndex, rod] of frame.children.entries()) {
        rod.updateMatrix();
        for (const [endpoint, height] of [-0.5, 0.5].entries()) {
          const actual = new Vector3(0, height, 0).applyMatrix4(rod.matrix).toArray();
          for (const [axis, coordinate] of actual.entries()) {
            expect(coordinate).toBeCloseTo(endpoints[rodIndex * 6 + endpoint * 3 + axis] ?? Number.NaN, 11);
          }
        }
      }
      for (const [name, vertices, indices, samples, color, opacity] of [
        ["transparent-canopy", 150, 720, [[0, [0.025, -1.08, -1.10]], [6, [0, -1.045, -1.10]], [125, [0.22, -1.05, -0.31]]], 0xd6e3e5, 0.23],
        ["rear-cockpit-fairing", 125, 576, [[0, [0.22, -1.05, -0.31]], [25, [0.23, -1.04, -0.06]], [106, [0, -0.83, 1.22]]], 0xe1e3db, 1]
      ] as const) {
        const mesh = airframe.root.getObjectByName(name) as Mesh;
        const position = mesh.geometry.getAttribute("position");
        expect(position.count).toBe(vertices);
        expect(mesh.geometry.index?.count).toBe(indices);
        for (const [index, expected] of samples) {
          const actual = [position.getX(index), position.getY(index), position.getZ(index)];
          for (const [axis, coordinate] of actual.entries()) expect(coordinate).toBeCloseTo(expected[axis] ?? Number.NaN, 6);
        }
        expect((mesh.material as MeshLambertMaterial).color.getHex()).toBe(color);
        expect((mesh.material as MeshLambertMaterial).opacity).toBeCloseTo(opacity, 6);
      }
      airframe.dispose();
      expect(free).toHaveBeenCalledTimes(1);
    } finally { airframe.dispose(); free.mockRestore(); }
  });

  it("returns cockpit bulk arrays owned independently of the released bridge", () => {
    const bridge = new CockpitGeometryBridge();
    const positions = bridge.surface_positions();
    positions[0] = 100;
    expect(bridge.surface_positions()[0]).toBeCloseTo(0.025, 7);
    const indices = bridge.surface_indices();
    const radii = bridge.rod_radii();
    bridge.free();
    expect(positions).toHaveLength((150 + 125) * 3);
    expect(indices).toHaveLength(720 + 576);
    expect(radii).toHaveLength(34);
    expect(positions[0]).toBe(100);
  });

  it.each([0.3, 1, 1.6])("projects every registered wing position and triangle unchanged for coefficient %s", (factor) => {
    const airframe = createBirdmanAirframe();
    const source = new WingGeometryBridge();
    try {
      airframe.setVisualState({ kind: "accepted", factor }, { layout: "absent" });
      source.update_factor(factor);
      const positions = source.surface_positions();
      const indices = source.surface_indices();
      const vertexCount = source.surface_vertex_count;
      const indexCount = source.surface_index_count;
      for (const [surface, name] of ["left-wing", "left-trailing-film", "right-wing", "right-trailing-film"].entries()) {
        const mesh = airframe.root.getObjectByName(name) as Mesh;
        const position = mesh.geometry.getAttribute("position");
        expect(position.count).toBe(vertexCount);
        expect(Array.from(position.array)).toEqual(Array.from(positions.subarray(surface * vertexCount * 3, (surface + 1) * vertexCount * 3)));
        expect(Array.from(mesh.geometry.index?.array ?? [])).toEqual(Array.from(indices.subarray(surface * indexCount, (surface + 1) * indexCount)));
      }
    } finally { airframe.dispose(); source.free(); }
  });

  it("updates accepted coefficient changes in retained buffers and releases the shared owner once", () => {
    const free = vi.spyOn(WingGeometryBridge.prototype, "free");
    const airframe = createBirdmanAirframe();
    try {
      const wing = airframe.root.getObjectByName("left-wing") as Mesh;
      const spar = airframe.root.getObjectByName("main-wing-spar")?.children[0] as InstancedMesh;
      airframe.setVisualState({ kind: "accepted", factor: 1 }, { layout: "absent" });
      const position = wing.geometry.getAttribute("position");
      if (!(position instanceof BufferAttribute)) throw new Error("Main-wing positions require a BufferAttribute");
      const version = position.version;
      const instanceVersion = spar.instanceMatrix.version;
      const positionBuffer = position.array;
      const instanceBuffer = spar.instanceMatrix.array;
      const previousPositions = Array.from(positionBuffer);
      airframe.setVisualState({ kind: "accepted", factor: 1 }, { layout: "absent" });
      expect(position.version).toBe(version);
      expect(spar.instanceMatrix.version).toBe(instanceVersion);
      airframe.setVisualState({ kind: "accepted", factor: 1.004 }, { layout: "absent" });
      expect(position.version).toBeGreaterThan(version);
      expect(spar.instanceMatrix.version).toBeGreaterThan(instanceVersion);
      expect(Array.from(position.array)).not.toEqual(previousPositions);
      const changedVersion = spar.instanceMatrix.version;
      airframe.setVisualState({ kind: "accepted", factor: 1.6 }, { layout: "absent" });
      expect(spar.instanceMatrix.version).toBeGreaterThan(changedVersion);
      const cappedPositionVersion = position.version;
      const cappedInstanceVersion = spar.instanceMatrix.version;
      const cappedPositions = Array.from(position.array);
      airframe.setVisualState({ kind: "accepted", factor: 1.6 }, { layout: "absent" });
      expect(position.version).toBe(cappedPositionVersion);
      expect(spar.instanceMatrix.version).toBe(cappedInstanceVersion);
      expect(Array.from(position.array)).toEqual(cappedPositions);
      expect(wing.geometry.getAttribute("position")).toBe(position);
      expect(position.array).toBe(positionBuffer);
      expect(spar.instanceMatrix.array).toBe(instanceBuffer);
      airframe.dispose();
      airframe.dispose();
      expect(free).toHaveBeenCalledTimes(1);
    } finally { airframe.dispose(); free.mockRestore(); }
  });

  it("hides missing or unrepresentable deformation and restores the exact accepted shape", () => {
    const airframe = createBirdmanAirframe();
    const source = new WingGeometryBridge();
    try {
      const parts = ["left-wing", "right-wing", "left-trailing-film", "right-trailing-film",
        "main-wing-spar-segment", "balsa-stringer", "left-trailing-rib", "right-trailing-rib",
        "left-styrofoam-rib", "right-styrofoam-rib"].map((name) => airframe.root.getObjectByName(name));
      expect(parts.every((part) => part !== undefined)).toBe(true);
      for (const reason of ["unrecorded", "unregistered_aircraft_geometry"] as const) {
        airframe.setVisualState({ kind: "unavailable", reason }, { layout: "absent" });
        expect(parts.every((part) => part?.visible === false)).toBe(true);
        airframe.setVisualState({ kind: "accepted", factor: 1.21 }, { layout: "absent" });
        expect(parts.every((part) => part?.visible === true)).toBe(true);
      }
      const controls = { layout: "tail_incidence", physicalIncidence: {
        horizontalTailRadians: 0.12, verticalTailRadians: -0.08 } } as const;
      expect(() => { airframe.setVisualState(
        { kind: "unavailable", reason: "unregistered_aircraft_geometry" }, controls,
        { kind: "unavailable", reason: "unregistered_aircraft_geometry" }
      ); }).not.toThrow();
      expect(parts.every((part) => part?.visible === false)).toBe(true);
      const tails = ["horizontal-tail-incidence", "vertical-tail-incidence"].map((name) => airframe.root.getObjectByName(name));
      expect(tails.every((tail) => tail?.visible === false)).toBe(true);
      airframe.setVisualState({ kind: "accepted", factor: 1.21 }, controls,
        { kind: "available", value: { horizontalTailArmMeters: 3.6 } });
      expect(tails.every((tail) => tail?.visible === true)).toBe(true);
      for (const factor of [Number.NaN, Infinity, Number.MAX_VALUE]) {
        expect(() => { airframe.setVisualState({ kind: "accepted", factor }, { layout: "absent" }); }).toThrow();
        expect(parts.every((part) => part?.visible === false)).toBe(true);
        airframe.setVisualState({ kind: "accepted", factor: 2 }, { layout: "absent" });
        expect(parts.every((part) => part?.visible === true)).toBe(true);
        source.update_factor(2);
        const wing = airframe.root.getObjectByName("left-wing") as Mesh;
        expect(Array.from(wing.geometry.getAttribute("position").array))
          .toEqual(Array.from(source.surface_positions().subarray(0, source.surface_vertex_count * 3)));
      }
    } finally { airframe.dispose(); source.free(); }
  });

  it("projects all shared rods into four bounded instance batches with the same radii and endpoints", () => {
    const airframe = createBirdmanAirframe();
    const source = new WingGeometryBridge();
    try {
      const kinds = source.rod_kinds();
      const radii = source.rod_radii();
      const groups = ["main-wing-spar-segment", "balsa-stringer", "left-trailing-rib", "right-trailing-rib"]
        .map((name) => airframe.root.getObjectByName(name) as InstancedMesh<CylinderGeometry, MeshLambertMaterial>);
      expect(groups.every((group) => group instanceof InstancedMesh)).toBe(true);
      expect(groups.map((group) => group.count)).toEqual([24, 48, 44, 44]);
      expect(new Set(groups.map((group) => group.geometry)).size).toBe(1);
      expect(groups[0]?.geometry.parameters).toMatchObject({ radiusTop: 1, radiusBottom: 1, height: 1, radialSegments: 8 });
      for (const factor of [0.3, 1, 1.6]) {
        airframe.setVisualState({ kind: "accepted", factor }, { layout: "absent" });
        source.update_factor(factor);
        const endpoints = source.rod_endpoints();
        for (const [groupIndex, group] of groups.entries()) {
          const indices = Array.from(kinds, (_, index) => index).filter((index) => groupIndex === 0 ? kinds[index] === 0
            : groupIndex === 1 ? kinds[index] === 2
              : kinds[index] === 1 && (endpoints[index * 6 + 3] ?? Number.NaN) * (groupIndex === 2 ? -1 : 1) > 0);
          expect(indices).toHaveLength(group.count);
          expect(group.frustumCulled).toBe(true);
          for (const [instance, index] of indices.entries()) {
            const matrix = new Matrix4();
            group.getMatrixAt(instance, matrix);
            expect(matrix.elements.every(Number.isFinite)).toBe(true);
            const radius = radii[index] ?? Number.NaN;
            const scale = new Vector3().setFromMatrixScale(matrix);
            for (const actual of [scale.x, scale.z]) expect(Math.abs(actual - radius)).toBeLessThanOrEqual(radius * 2 ** -20);
            for (const [end, height] of [-0.5, 0.5].entries()) {
              const actual = new Vector3(0, height, 0).applyMatrix4(matrix);
              const expected = new Vector3().fromArray(endpoints, index * 6 + end * 3);
              expect(actual.distanceTo(expected)).toBeLessThanOrEqual(Math.max(1, expected.length()) * 2 ** -21);
              expect(group.boundingSphere?.containsPoint(actual)).toBe(true);
            }
            for (const radial of [new Vector3(1, 0, 0), new Vector3(0, 0, 1)]) {
              expect(group.boundingSphere?.containsPoint(radial.applyMatrix4(matrix))).toBe(true);
            }
          }
        }
      }
      const instances = groups.map((group) => vi.spyOn(group, "dispose"));
      const sharedGeometry = groups[0]?.geometry;
      if (sharedGeometry === undefined) throw new Error("Missing instanced rod geometry");
      const geometry = vi.spyOn(sharedGeometry, "dispose");
      airframe.dispose();
      airframe.dispose();
      for (const dispose of instances) expect(dispose).toHaveBeenCalledTimes(1);
      expect(geometry).toHaveBeenCalledTimes(1);
    } finally { airframe.dispose(); source.free(); }
  });

  it.each([false, true])("releases all instance and shared resources after a dispose listener fails, texture failure=%s", (textureFailure) => {
    const airframe = createBirdmanAirframe();
    const instances: InstancedMesh[] = [];
    airframe.root.traverse((part) => { if (part instanceof InstancedMesh) instances.push(part as InstancedMesh); });
    const first = instances[0];
    if (first === undefined) throw new Error("Missing instanced rods");
    const textures = ["left-wing", "right-wing"].map((name) => {
      const wing = airframe.root.getObjectByName(name) as Mesh;
      const skin = (wing.material as MeshLambertMaterial[])[0]?.map;
      if (!(skin instanceof DataTexture)) throw new Error("Missing owned wing texture");
      return skin;
    });
    const disposals = [...instances.map((part) => vi.spyOn(part, "dispose")),
      vi.spyOn(first.geometry, "dispose"), vi.spyOn(first.material as MeshLambertMaterial, "dispose"),
      ...textures.map((texture) => vi.spyOn(texture, "dispose"))];
    const instanceCause = new Error("Instance cleanup failed");
    const textureCause = new Error("Texture cleanup failed");
    first.addEventListener("dispose", () => { throw instanceCause; });
    if (textureFailure) textures[0]?.addEventListener("dispose", () => { throw textureCause; });
    let failure: unknown;
    try { airframe.dispose(); } catch (cause) { failure = cause; }
    if (textureFailure) {
      expect(failure).toBeInstanceOf(AggregateError);
      expect((failure as AggregateError).errors).toEqual([instanceCause, textureCause]);
      expect((failure as AggregateError).cause).toBe(instanceCause);
    } else expect(failure).toBe(instanceCause);
    airframe.dispose();
    for (const dispose of disposals) expect(dispose).toHaveBeenCalledTimes(1);
  });

  it("returns owned bulk arrays that remain valid after update and bridge release", () => {
    const wing = new WingGeometryBridge();
    const reference = wing.surface_positions();
    const tipHeightIndex = (wing.surface_vertex_count - 1) * 3 + 1;
    const initialTip = reference[tipHeightIndex];
    const initialFirstCoordinate = reference[0];
    reference[0] = 100;
    expect(wing.surface_positions()[0]).toBe(initialFirstCoordinate);
    expect(wing.update_factor(1.6)).toBe(true);
    const updated = wing.surface_positions();
    wing.free();
    expect(reference[tipHeightIndex]).toBe(initialTip);
    expect(updated[tipHeightIndex]).toBeGreaterThan(initialTip ?? Number.NaN);
    expect(updated.length).toBe(reference.length);
  });

  it("keeps finite wing geometry, visual bending and cockpit material boundaries", () => {
    const airframe = createBirdmanAirframe();
    try {
      const left = airframe.root.getObjectByName("left-wing") as Mesh;
      const right = airframe.root.getObjectByName("right-wing") as Mesh;
      const foam = airframe.root.getObjectByName("left-styrofoam-rib") as Mesh;
      const cockpit = airframe.root.getObjectByName("open-cockpit-frame");
      const canopy = airframe.root.getObjectByName("transparent-canopy") as Mesh;
      const fairing = airframe.root.getObjectByName("rear-cockpit-fairing") as Mesh;
      expect(left).toBeInstanceOf(Mesh);
      expect(right).toBeInstanceOf(Mesh);
      expect(cockpit).toBeInstanceOf(Group);
      expect(cockpit?.children.length).toBeGreaterThan(8);
      expect((canopy.material as MeshLambertMaterial).transparent).toBe(true);
      expect((fairing.material as MeshLambertMaterial).transparent).toBe(false);
      const [wingCover, wingUnderside] = left.material as MeshLambertMaterial[];
      expect(wingCover?.map).toBeInstanceOf(DataTexture);
      expect(wingUnderside?.map).toBeNull();
      expect(wingUnderside?.color.getHex()).toBe(0xf2f3ed);
      expect(left.geometry.groups.map((group) => group.materialIndex)).toEqual([0, 1, 1]);
      const leftPosition = left.geometry.getAttribute("position");
      const rightPosition = right.geometry.getAttribute("position");
      const tip = leftPosition.count - 1;
      const initialY = leftPosition.getY(tip);
      const initialFoamY = foam.position.y;
      airframe.setVisualState({ kind: "accepted", factor: 1.6 }, { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: 0.12, verticalTailRadians: -0.08 } },
        { kind: "available", value: { horizontalTailArmMeters: 3.6 } });
      expect(leftPosition.getY(tip)).toBeGreaterThan(initialY);
      expect(foam.position.y).toBeGreaterThan(initialFoamY);
      expect(airframe.root.getObjectByName("horizontal-tail-incidence")?.rotation.x).toBeCloseTo(0.12);
      expect(airframe.root.getObjectByName("vertical-tail-incidence")?.rotation.y).toBeCloseTo(0.08);
      expect(leftPosition.count).toBe(rightPosition.count);
      for (const mesh of [left, right]) for (const name of ["position", "normal"]) {
        const attribute = mesh.geometry.getAttribute(name);
        for (let index = 0; index < attribute.count; index++) {
          expect(Number.isFinite(attribute.getX(index))).toBe(true);
          expect(Number.isFinite(attribute.getY(index))).toBe(true);
          expect(Number.isFinite(attribute.getZ(index))).toBe(true);
        }
      }
    } finally { airframe.dispose(); }
  });
});

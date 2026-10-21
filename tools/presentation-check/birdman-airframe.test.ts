import "./lake-model-fixture.js";
import { describe, expect, it, vi } from "vitest";
import { BufferAttribute, DataTexture, Group, Mesh, MeshLambertMaterial } from "three";
import { createBirdmanAirframe } from "../../web/src/render/engines/three/birdman-airframe.js";
import { AirframeAttachment, AirframeMemberKind, AirframeStructureBridge, CockpitGeometryBridge, TailGeometryBridge, WingGeometryBridge } from "../../web/pkg/birdman_game_wasm.js";

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
          const patches = tail.children.filter((part) => ["physical-tail-cover", "physical-tail-film"].includes(part.name)) as Mesh[];
          expect(patches).toHaveLength(codes.length);
          for (const [patch, mesh] of patches.entries()) {
            expect(Array.from(mesh.geometry.getAttribute("position").array))
              .toEqual(Array.from(positions.subarray(patch * 12, (patch + 1) * 12)));
            const material = mesh.material as MeshLambertMaterial;
            expect(material.color.getHex()).toBe(source.material_colors()[codes[patch] ?? -1]);
            expect(material.opacity).toBeCloseTo(source.material_opacities()[codes[patch] ?? -1] ?? -1, 6);
            expect(material.transparent).toBe(codes[patch] === 1);
            expect(material.depthWrite).toBe(codes[patch] !== 1);
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
    const indices = source.surface_positions(1);
    source.free();
    expect(positions[0]).toBe(100);
    expect(indices.length).toBeGreaterThan(0);
    expect(Array.from(indices).every(Number.isFinite)).toBe(true);
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

  it("projects the fitted shared frame and unchanged cockpit shell and releases its bulk owner immediately", () => {
    const free = vi.spyOn(CockpitGeometryBridge.prototype, "free");
    const airframe = createBirdmanAirframe();
    try {
      expect(free).toHaveBeenCalledTimes(1);
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
    bridge.free();
    expect(positions).toHaveLength((150 + 125) * 3);
    expect(indices).toHaveLength(720 + 576);
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
      const spar = airframe.root.getObjectByName("main-wing-spar") as Mesh;
      const sparPosition = spar.geometry.getAttribute("position") as BufferAttribute;
      airframe.setVisualState({ kind: "accepted", factor: 1 }, { layout: "absent" });
      const position = wing.geometry.getAttribute("position");
      if (!(position instanceof BufferAttribute)) throw new Error("Main-wing positions require a BufferAttribute");
      const version = position.version;
      const instanceVersion = sparPosition.version;
      const positionBuffer = position.array;
      const instanceBuffer = sparPosition.array;
      const previousPositions = Array.from(positionBuffer);
      airframe.setVisualState({ kind: "accepted", factor: 1 }, { layout: "absent" });
      expect(position.version).toBe(version);
      expect(sparPosition.version).toBe(instanceVersion);
      airframe.setVisualState({ kind: "accepted", factor: 1.004 }, { layout: "absent" });
      expect(position.version).toBeGreaterThan(version);
      expect(sparPosition.version).toBeGreaterThan(instanceVersion);
      expect(Array.from(position.array)).not.toEqual(previousPositions);
      const changedVersion = sparPosition.version;
      airframe.setVisualState({ kind: "accepted", factor: 1.6 }, { layout: "absent" });
      expect(sparPosition.version).toBeGreaterThan(changedVersion);
      const cappedPositionVersion = position.version;
      const cappedInstanceVersion = sparPosition.version;
      const cappedPositions = Array.from(position.array);
      airframe.setVisualState({ kind: "accepted", factor: 1.6 }, { layout: "absent" });
      expect(position.version).toBe(cappedPositionVersion);
      expect(sparPosition.version).toBe(cappedInstanceVersion);
      expect(Array.from(position.array)).toEqual(cappedPositions);
      expect(wing.geometry.getAttribute("position")).toBe(position);
      expect(position.array).toBe(positionBuffer);
      expect(sparPosition.array).toBe(instanceBuffer);
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
        "main-wing-spar", "main-wing-stringer", "main-wing-rib", "main-wing-foam"].map((name) => airframe.root.getObjectByName(name));
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
      expect(airframe.root.visible).toBe(false);
      airframe.setVisualState({ kind: "accepted", factor: 1.21 }, controls,
        { kind: "available", value: { horizontalTailArmMeters: 3.6 } });
      expect(tails.every((tail) => tail?.visible === true)).toBe(true);
      expect(airframe.root.visible).toBe(true);
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
      const wing = airframe.root.getObjectByName("left-wing") as Mesh;
      const normalFailure = vi.spyOn(wing.geometry, "computeVertexNormals").mockImplementationOnce(() => {
        wing.geometry.getAttribute("normal").setX(0, Number.NaN);
      });
      try {
        expect(() => { airframe.setVisualState({ kind: "accepted", factor: 1.43 }, { layout: "absent" }); }).toThrow("normals");
        expect(parts.every((part) => part?.visible === false)).toBe(true);
      } finally { normalFailure.mockRestore(); }
      airframe.setVisualState({ kind: "accepted", factor: 1.43 }, { layout: "absent" });
      expect(parts.every((part) => part?.visible === true)).toBe(true);
      expect(Array.from(wing.geometry.getAttribute("normal").array).every(Number.isFinite)).toBe(true);
    } finally { airframe.dispose(); source.free(); }
  });

  it("projects every structural batch from the physical producer with retained topology and buffers", () => {
    const source = new AirframeStructureBridge();
    const airframe = createBirdmanAirframe();
    const kindNames = {
      [AirframeMemberKind.Spar]: "spar", [AirframeMemberKind.Rib]: "rib",
      [AirframeMemberKind.Stringer]: "stringer", [AirframeMemberKind.Foam]: "foam",
      [AirframeMemberKind.MainBeam]: "single-carbon-main-beam",
      [AirframeMemberKind.CockpitFrame]: "open-cockpit-frame", [AirframeMemberKind.CanopyRim]: "canopy-rim"
    };
    let released = false;
    try {
      const parts = Array.from({ length: source.mesh_count }, (_, index) => {
        const attachment = source.attachment(index);
        const name = kindNames[source.member_kind(index)];
        const prefix = attachment === AirframeAttachment.MainWing ? "main-wing-" :
          attachment === AirframeAttachment.HorizontalTail ? "horizontal-tail-" :
            attachment === AirframeAttachment.VerticalTail ? "vertical-tail-" : "";
        const mesh = airframe.root.getObjectByName(prefix + name) as Mesh;
        expect(mesh).toBeInstanceOf(Mesh);
        const expectedParent = attachment === AirframeAttachment.HorizontalTail ? "horizontal-tail-incidence" :
          attachment === AirframeAttachment.VerticalTail ? "vertical-tail-incidence" : "birdman-airframe";
        expect(mesh.parent?.name).toBe(expectedParent);
        if (attachment === AirframeAttachment.HorizontalTail || attachment === AirframeAttachment.VerticalTail) {
          expect(mesh.parent?.position.toArray()).toEqual(Array.from(source.origin(index)));
        }
        return { index, mesh, buffer: mesh.geometry.getAttribute("position").array };
      });
      expect(parts).toHaveLength(11);
      for (const factor of [0, 0.3, 0.7, 1, 1.6]) {
        airframe.setVisualState({ kind: "accepted", factor }, { layout: "tail_incidence",
          physicalIncidence: { horizontalTailRadians: 0.12, verticalTailRadians: -0.08 } },
        { kind: "available", value: { horizontalTailArmMeters: 3.6 } });
        source.update_factor(factor);
        for (const { index, mesh, buffer } of parts) {
          expect(Array.from(mesh.geometry.getAttribute("position").array)).toEqual(Array.from(source.positions(index)));
          expect(mesh.geometry.getAttribute("position").array).toBe(buffer);
          expect(Array.from(mesh.geometry.index?.array ?? [])).toEqual(Array.from(source.indices(index)));
          expect((mesh.material as MeshLambertMaterial).color.getHex()).toBe(source.color(index));
          const normals = mesh.geometry.getAttribute("normal");
          for (let vertex = 0; vertex < normals.count; vertex++) {
            expect(Math.hypot(normals.getX(vertex), normals.getY(vertex), normals.getZ(vertex))).toBeCloseTo(1, 5);
          }
        }
      }
      const owned = source.positions(0);
      const first = owned[0];
      owned[0] = 100;
      expect(source.positions(0)[0]).toBe(first);
      source.free();
      released = true;
      expect(owned[0]).toBe(100);
    } finally { airframe.dispose(); if (!released) source.free(); }
  });

  it.each([false, true])("releases all shared resources after a dispose listener fails, texture failure=%s", (textureFailure) => {
    const airframe = createBirdmanAirframe();
    const first = airframe.root.getObjectByName("main-wing-spar") as Mesh;
    expect(first).toBeInstanceOf(Mesh);
    const textures = ["left-wing", "right-wing"].map((name) => {
      const wing = airframe.root.getObjectByName(name) as Mesh;
      const skin = (wing.material as MeshLambertMaterial[])[0]?.map;
      if (!(skin instanceof DataTexture)) throw new Error("Missing owned wing texture");
      return skin;
    });
    const disposals = [vi.spyOn(first.geometry, "dispose"), vi.spyOn(first.material as MeshLambertMaterial, "dispose"),
      ...textures.map((texture) => vi.spyOn(texture, "dispose"))];
    const instanceCause = new Error("Instance cleanup failed");
    const textureCause = new Error("Texture cleanup failed");
    first.geometry.addEventListener("dispose", () => { throw instanceCause; });
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
      const foam = airframe.root.getObjectByName("main-wing-foam") as Mesh;
      const cockpit = airframe.root.getObjectByName("open-cockpit-frame");
      const canopy = airframe.root.getObjectByName("transparent-canopy") as Mesh;
      const fairing = airframe.root.getObjectByName("rear-cockpit-fairing") as Mesh;
      expect(left).toBeInstanceOf(Mesh);
      expect(right).toBeInstanceOf(Mesh);
      expect(cockpit).toBeInstanceOf(Mesh);
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
      const foamPosition = foam.geometry.getAttribute("position");
      const initialFoamY = foamPosition.getY(0);
      airframe.setVisualState({ kind: "accepted", factor: 1.6 }, { layout: "tail_incidence", physicalIncidence: { horizontalTailRadians: 0.12, verticalTailRadians: -0.08 } },
        { kind: "available", value: { horizontalTailArmMeters: 3.6 } });
      expect(leftPosition.getY(tip)).toBeGreaterThan(initialY);
      expect(foamPosition.getY(0)).toBeGreaterThan(initialFoamY);
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

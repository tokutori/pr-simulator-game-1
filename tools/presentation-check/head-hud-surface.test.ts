import { describe, expect, it } from "vitest";
import { Matrix4, PerspectiveCamera, Quaternion, Scene, StereoCamera, Vector3 } from "three";
import { createHeadHudSurface } from "../../web/src/render/engines/three/head-hud-surface.js";
import { createHeadHudFixture } from "../../web/src/presentation/fixtures.js";
import { NO_HEAD_HUD, resolveHeadHudFrame } from "../../web/src/render/contracts/head-hud.js";
import { composePose, IDENTITY_POSE, pose, quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";

describe("Independent transparent Head HUD surface", () => {
  it("keeps material opacity independent of background alpha and disables depth writes and reflections", () => {
    const surface = createHeadHudSurface(canvas());
    const scene = new Scene();
    scene.add(surface.mesh);
    const view = createHeadHudFixture();
    surface.update(resolveHeadHudFrame(view, IDENTITY_POSE), IDENTITY_POSE);
    expect(surface.mesh.material).toMatchObject({ transparent: true, opacity: 1, depthWrite: false, depthTest: false, toneMapped: false });
    expect(surface.mesh.layers.isEnabled(0)).toBe(true);
    expect(surface.mesh.layers.isEnabled(1)).toBe(false);
    expect(surface.mesh.renderOrder).toBeGreaterThan(0);
    expect(surface.mesh.visible).toBe(true);
    expect(surface.mesh.material.map?.image).toEqual(canvas());
    surface.dispose();
  });

  it("keeps the same mesh across hidden and visible frames and updates only changed immutable views", () => {
    const surface = createHeadHudSurface(canvas());
    const mesh = surface.mesh;
    const texture = mesh.material.map;
    if (texture === null) throw new Error("Missing HUD texture");
    const view = createHeadHudFixture();
    const frame = resolveHeadHudFrame(view, IDENTITY_POSE);
    surface.update(frame, IDENTITY_POSE);
    const firstVersion = texture.version;
    surface.update(frame, pose(vec3(1, 2, 3), IDENTITY_POSE.orientation));
    expect(texture.version).toBe(firstVersion);
    surface.update(resolveHeadHudFrame({ ...view, backgroundAlpha: 0.5 }, IDENTITY_POSE), IDENTITY_POSE);
    expect(texture.version).toBeGreaterThan(firstVersion);
    surface.update(NO_HEAD_HUD, IDENTITY_POSE);
    expect(mesh.visible).toBe(false);
    surface.update(frame, IDENTITY_POSE);
    expect(surface.mesh).toBe(mesh);
    expect(mesh.visible).toBe(true);
    surface.dispose();
  });

  it.each([-0.25, 0, 0.25])("composes aircraft, pilot eye, raw head, and local HUD once at body offset %s", (pilotOffset) => {
    const surface = createHeadHudSurface(canvas());
    const scene = new Scene();
    scene.add(surface.mesh);
    const view = createHeadHudFixture();
    const aircraft = pose(vec3(15, 12, -30), quaternion(Math.cos(0.2), 0, Math.sin(0.2), 0));
    const eye = pose(vec3(0, 0.15, -0.55 - pilotOffset), IDENTITY_POSE.orientation);
    const worldFromTracking = composePose(aircraft, eye);
    const local = poseMatrix(view.localPose).scale(new Vector3(view.size.width, view.size.height, 1));
    for (const head of [IDENTITY_POSE, pose(vec3(0.02, -0.03, 0.04), quaternion(0.9, 0.2, -0.3, 0.1))]) {
      surface.update(resolveHeadHudFrame(view, head), worldFromTracking);
      scene.updateMatrixWorld(true);
      const worldFromHead = poseMatrix(aircraft).multiply(poseMatrix(eye)).multiply(poseMatrix(head));
      expectMatrix(surface.mesh.matrixWorld, worldFromHead.clone().multiply(local));
      const camera = new PerspectiveCamera(60, 1280 / 720, 0.05, 1000);
      worldFromHead.decompose(camera.position, camera.quaternion, camera.scale);
      camera.updateMatrixWorld(true);
      const stereo = new StereoCamera();
      stereo.eyeSep = 0.064;
      stereo.update(camera);
      for (const [index, cameraEye] of [stereo.cameraL, stereo.cameraR].entries()) {
        cameraEye.updateMatrixWorld(true);
        const offset = new Matrix4().makeTranslation((index === 0 ? 1 : -1) * 0.032, 0, 0);
        expectMatrix(cameraEye.matrixWorld.clone().invert().multiply(surface.mesh.matrixWorld), offset.multiply(local));
      }
    }
    surface.dispose();
  });

  it("disposes texture, geometry, and material once and rejects further updates", () => {
    const surface = createHeadHudSurface(canvas());
    const scene = new Scene();
    scene.add(surface.mesh);
    let disposalCount = 0;
    for (const resource of [surface.mesh.material, surface.mesh.geometry, surface.mesh.material.map]) {
      if (resource === null) throw new Error("Missing HUD resource");
      resource.addEventListener("dispose", () => { disposalCount++; });
    }
    surface.dispose();
    surface.dispose();
    expect(disposalCount).toBe(3);
    expect(surface.mesh.parent).toBeNull();
    expect(surface.mesh.visible).toBe(false);
    expect(() => { surface.update(NO_HEAD_HUD, IDENTITY_POSE); }).toThrow(/disposed/);
  });
});

function canvas(): HTMLCanvasElement {
  return { width: 1024, height: 768 } as HTMLCanvasElement;
}

function poseMatrix(value: Pose): Matrix4 {
  return new Matrix4().compose(new Vector3(value.position.x, value.position.y, value.position.z),
    new Quaternion(value.orientation.x, value.orientation.y, value.orientation.z, value.orientation.w), new Vector3(1, 1, 1));
}

function expectMatrix(actual: Matrix4, expected: Matrix4): void {
  for (const [index, value] of actual.elements.entries()) expect(value).toBeCloseTo(expected.elements[index] ?? Number.NaN, 10);
}

import { CanvasTexture, LinearFilter, Mesh, MeshBasicMaterial, PlaneGeometry, SRGBColorSpace } from "three";
import { composePose } from "../../contracts/math.js";
import type { Pose } from "../../contracts/math.js";
import { validateHeadHudLayer } from "../../contracts/head-hud.js";
import type { HeadHudFrame, VisibleHeadHud } from "../../contracts/head-hud.js";

export function createHeadHudSurface(canvas: HTMLCanvasElement) {
  const texture = new CanvasTexture(canvas);
  texture.colorSpace = SRGBColorSpace;
  texture.minFilter = LinearFilter;
  texture.generateMipmaps = false;
  const material = new MeshBasicMaterial({
    map: texture,
    transparent: true,
    opacity: 1,
    depthWrite: false,
    depthTest: false,
    toneMapped: false
  });
  const geometry = new PlaneGeometry(1, 1);
  const mesh = new Mesh(geometry, material);
  mesh.name = "head-hud";
  mesh.visible = false;
  mesh.renderOrder = 1000;
  let currentView: VisibleHeadHud | null = null;
  let disposed = false;

  return Object.freeze({
    mesh,
    update(frame: HeadHudFrame, worldFromTracking: Pose): void {
      if (disposed) throw new Error("Head HUD surface is disposed");
      if (frame.kind === "absent") {
        mesh.visible = false;
        currentView = null;
        return;
      }
      const view = frame.view;
      if (view !== currentView) {
        validateHeadHudLayer(view);
        texture.needsUpdate = true;
        currentView = view;
      }
      const worldFromHud = composePose(composePose(worldFromTracking, frame.trackingFromHead), view.localPose);
      mesh.position.set(worldFromHud.position.x, worldFromHud.position.y, worldFromHud.position.z);
      mesh.quaternion.set(worldFromHud.orientation.x, worldFromHud.orientation.y, worldFromHud.orientation.z, worldFromHud.orientation.w);
      mesh.scale.set(view.size.width, view.size.height, 1);
      mesh.visible = true;
    },
    dispose(): void {
      if (disposed) return;
      mesh.removeFromParent();
      mesh.visible = false;
      geometry.dispose();
      material.dispose();
      texture.dispose();
      currentView = null;
      disposed = true;
    }
  });
}

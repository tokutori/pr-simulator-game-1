import { WebGLRenderTarget } from "three";
import type { DirectionalLight, WebGLRenderer } from "three";

export function applyScreenRenderResources(
  sun: DirectionalLight,
  renderer: Pick<WebGLRenderer, "initRenderTarget">,
  shadowMapSize: number,
  applyResolution: () => boolean,
  commitWater: () => string | undefined = () => undefined
): false | { readonly cleanupFailure?: string } {
  const previous = sun.shadow.map;
  let next: WebGLRenderTarget | null = null;
  try {
    if (previous !== null && (previous.width !== shadowMapSize || previous.height !== shadowMapSize)) {
      if (!(previous instanceof WebGLRenderTarget)) throw new Error("Unsupported directional shadow render target");
      next = previous.clone();
      if (next.depthTexture !== null && next.depthTexture === previous.depthTexture) {
        next.depthTexture = next.depthTexture.clone();
      }
      next.setSize(shadowMapSize, shadowMapSize);
      renderer.initRenderTarget(next);
    }
    if (!applyResolution()) {
      const unused = next;
      next = null;
      if (unused !== null) disposeShadowMap(unused);
      return false;
    }
  } catch (cause: unknown) {
    if (next !== null) {
      try { disposeShadowMap(next); }
      catch (cleanup: unknown) {
        throw new AggregateError([cause, cleanup], `Screen resource preparation failed: ${message(cause)}; ${message(cleanup)}`, { cause: cleanup });
      }
    }
    throw cause;
  }
  sun.shadow.mapSize.set(shadowMapSize, shadowMapSize);
  if (next !== null) sun.shadow.map = next;
  const failures: string[] = [];
  try {
    const cleanup = commitWater();
    if (cleanup !== undefined) failures.push(cleanup);
  } catch (cause: unknown) { failures.push(message(cause)); }
  if (next !== null && previous !== null) {
    try { disposeShadowMap(previous); }
    catch (cause: unknown) { failures.push(message(cause)); }
  }
  return failures.length === 0 ? {} : { cleanupFailure: failures.join("; ") };
}

function disposeShadowMap(target: NonNullable<DirectionalLight["shadow"]["map"]>): void {
  const depth = target.depthTexture;
  target.depthTexture = null;
  const failures: unknown[] = [];
  for (const release of [() => { depth?.dispose(); }, () => { target.dispose(); }]) {
    try { release(); } catch (cause: unknown) { failures.push(cause); }
  }
  if (failures.length > 0) {
    throw new AggregateError(failures, `Shadow map cleanup failed: ${failures.map(message).join("; ")}`);
  }
}

function message(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}

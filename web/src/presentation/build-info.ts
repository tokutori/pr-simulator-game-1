declare const __APP_BUILD_LABEL__: string;

export const APP_BUILD_LABEL = typeof __APP_BUILD_LABEL__ === "string"
  ? __APP_BUILD_LABEL__
  : "v0.1.0 · test";

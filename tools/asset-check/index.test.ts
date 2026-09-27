import { createHash } from "node:crypto";
import { mkdtemp, mkdir, symlink, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, basename, join, resolve } from "node:path";
import { stringify } from "smol-toml";
import { describe, expect, it } from "vitest";
import { checkAssets, parseManifest } from "./index.js";

const asset = {
  path: "assets/test.bin", sha256: createHash("sha256").update("data").digest("hex"),
  source: "original:test", source_version: "1", license: "MIT", license_url: "LICENSE",
  attribution: "Test", processing: "test tool version 1"
};

describe("asset metadata", () => {
  it("accepts the explicit empty manifest", () => {
    expect(parseManifest({ schema_version: 1, assets: [] })).toEqual([]);
  });
  it.each(["../outside", "assets/../outside", "C:/outside", "assets\\test", "assets/.hidden", "assets/manifest.toml"])("rejects unsafe path %s", (path) => {
    expect(() => parseManifest({ schema_version: 1, assets: [{ ...asset, path }] })).toThrow();
  });
  it.each(["source", "source_version", "license", "license_url", "attribution", "processing", "sha256"])("rejects missing %s", (key) => {
    expect(() => parseManifest({ schema_version: 1, assets: [{ ...asset, [key]: "" }] })).toThrow();
  });
  it("rejects case-insensitive duplicate paths", () => {
    expect(() => parseManifest({ schema_version: 1, assets: [asset, { ...asset, path: "assets/TEST.bin" }] })).toThrow();
  });
  it("checks files, hashes, omissions and the public-asset bypass", async () => {
    const root = await mkdtemp(join(tmpdir(), "birdman-assets-"));
    if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith("birdman-assets-")) {
      throw new Error("Temporary cleanup escaped the test directory");
    }
    try {
      await mkdir(join(root, "assets"));
      await mkdir(join(root, "web/src"), { recursive: true });
      await writeFile(join(root, "assets/manifest.toml"), stringify({ schema_version: 1, assets: [asset] }), "utf8");
      await expect(checkAssets(root)).rejects.toThrow("Missing asset");
      await writeFile(join(root, asset.path), "data", "utf8");
      await expect(checkAssets(root)).resolves.toBe(1);
      await writeFile(join(root, asset.path), "tampered", "utf8");
      await expect(checkAssets(root)).rejects.toThrow("Hash mismatch");
      await writeFile(join(root, asset.path), "data", "utf8");
      await writeFile(join(root, "assets/unregistered.bin"), "data", "utf8");
      await expect(checkAssets(root)).rejects.toThrow("Unregistered asset");
      await rm(join(root, "assets/unregistered.bin"));
      await mkdir(join(root, "web/public"), { recursive: true });
      await writeFile(join(root, "web/public/unmanaged.bin"), "data", "utf8");
      await expect(checkAssets(root)).rejects.toThrow("Unmanaged web/public");
      await rm(join(root, "web/public/unmanaged.bin"));
      await writeFile(join(root, "web/src/unmanaged.svg"), "<svg/>", "utf8");
      await expect(checkAssets(root)).rejects.toThrow("Unmanaged web source asset");
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
  it("rejects an assets root that is a symbolic link", async () => {
    const root = await mkdtemp(join(tmpdir(), "birdman-assets-"));
    if (dirname(resolve(root)) !== resolve(tmpdir()) || !basename(root).startsWith("birdman-assets-")) {
      throw new Error("Temporary cleanup escaped the test directory");
    }
    try {
      await mkdir(join(root, "external"));
      await writeFile(join(root, "external/manifest.toml"), stringify({ schema_version: 1, assets: [] }), "utf8");
      await symlink(join(root, "external"), join(root, "assets"), "junction");
      await mkdir(join(root, "web/src"), { recursive: true });
      await expect(checkAssets(root)).rejects.toThrow("Invalid asset directory: assets");
    } finally {
      await rm(root, { recursive: true, force: true });
    }
  });
});

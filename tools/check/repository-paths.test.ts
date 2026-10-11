import { describe, expect, it } from "vitest";
import { repositoryPaths } from "./repository-paths.js";

describe("repository working-tree paths", () => {
  it("excludes deleted cached source and Markdown files while retaining tracked and untracked files", () => {
    const listed = ["web/src/main.ts", "web/src/presentation/vr-panel-canvas.ts", "docs/deleted.md",
      "docs/architecture.md", "web/src/new-view.ts", "docs/new-guide.md", ""].join("\0");
    const deleted = ["web/src/presentation/vr-panel-canvas.ts", "docs/deleted.md", ""].join("\0");
    expect(repositoryPaths(listed, deleted)).toEqual([
      "web/src/main.ts", "docs/architecture.md", "web/src/new-view.ts", "docs/new-guide.md"
    ]);
  });

  it("preserves NUL-delimited names and excludes only the exact deleted path", () => {
    expect(repositoryPaths("docs/空 白\n条件.md\0docs/removed.md\0docs/removed.md.backup\0", "docs/removed.md\0"))
      .toEqual(["docs/空 白\n条件.md", "docs/removed.md.backup"]);
  });

  it("preserves the listed paths when there are no deletions and rejects empty entries", () => {
    expect(repositoryPaths("web/src/main.ts\0docs/architecture.md\0", ""))
      .toEqual(["web/src/main.ts", "docs/architecture.md"]);
    expect(repositoryPaths("", "")).toEqual([]);
  });
});

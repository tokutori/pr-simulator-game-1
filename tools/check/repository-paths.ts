export function repositoryPaths(listedFiles: string, deletedFiles: string): string[] {
  const deleted = new Set(deletedFiles.split("\0"));
  return listedFiles.split("\0").filter((path) => path !== "" && !deleted.has(path));
}

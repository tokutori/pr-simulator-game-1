import { dirname, isAbsolute, resolve } from "node:path";
import ts from "typescript";

export interface SourceText {
  readonly path: string;
  readonly text: string;
}

export function assertRenderImportBoundary(
  sources: readonly SourceText[],
  compilerOptions: ts.CompilerOptions = projectCompilerOptions()
): void {
  const files = new Map(sources.map((source) => [normalizedPath(source.path), source.text]));
  const host: ts.ModuleResolutionHost = {
    fileExists: (path) => files.has(normalizedPath(path)) || ts.sys.fileExists(path),
    readFile: (path) => files.get(normalizedPath(path)) ?? ts.sys.readFile(path),
    directoryExists: (path) => [...files.keys()].some((file) => file.startsWith(`${normalizedPath(path)}/`)) || ts.sys.directoryExists(path)
  };
  const adapter = `${normalizedPath("web/src/render/engines/three")}/`;
  const root = normalizedPath("web/src/main.ts");
  for (const source of sources) {
    const path = normalizedPath(source.path);
    const inThreeAdapter = path.startsWith(adapter);
    const compositionRoot = path === root;
    const syntax = ts.createSourceFile(path, source.text, ts.ScriptTarget.Latest, true);
    const visited = new Set<ts.Node>();

    function assertSpecifier(specifier: string): void {
      const module = ts.resolveModuleName(specifier, path, compilerOptions, host).resolvedModule;
      const requestedPath = specifier.startsWith(".") || isAbsolute(specifier)
        ? normalizedPath(resolve(dirname(path), specifier))
        : undefined;
      const targets = [module?.resolvedFileName, requestedPath]
        .filter((target): target is string => target !== undefined)
        .map(normalizedPath);
      const importsThree = specifier === "three" || specifier.startsWith("three/") ||
        module?.packageId?.name === "three" || module?.packageId?.name === "@types/three" ||
        targets.some((target) => /\/node_modules\/(?:@types\/)?three\//.test(target));
      if (importsThree && !inThreeAdapter) {
        throw new Error(`Three.js import escaped its adapter: ${source.path} -> ${specifier}`);
      }
      if (targets.some((target) => target.startsWith(adapter)) && !inThreeAdapter && !compositionRoot) {
        throw new Error(`Three.js adapter import escaped the composition root: ${source.path} -> ${specifier}`);
      }
    }

    function inspectSpecifier(specifier: ts.Node | undefined): void {
      if (specifier === undefined || !ts.isStringLiteralLike(specifier)) {
        throw new Error(`Dynamic module specifier prevents render-boundary verification: ${source.path}`);
      }
      assertSpecifier(specifier.text);
    }

    function visit(node: ts.Node): void {
      if (visited.has(node)) return;
      visited.add(node);
      if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier !== undefined) {
        inspectSpecifier(node.moduleSpecifier);
      } else if (ts.isJSDocImportTag(node)) {
        inspectSpecifier(node.moduleSpecifier);
      } else if (ts.isImportTypeNode(node)) {
        inspectSpecifier(ts.isLiteralTypeNode(node.argument) ? node.argument.literal : undefined);
      } else if (ts.isExternalModuleReference(node)) {
        inspectSpecifier(node.expression);
      } else if (ts.isCallExpression(node) &&
          (node.expression.kind === ts.SyntaxKind.ImportKeyword || (ts.isIdentifier(node.expression) && node.expression.text === "require"))) {
        inspectSpecifier(node.arguments[0]);
      }
      ts.forEachChild(node, visit);
      for (const comment of ts.getJSDocCommentsAndTags(node)) visit(comment);
    }
    for (const reference of syntax.typeReferenceDirectives) assertSpecifier(reference.fileName);
    for (const reference of syntax.referencedFiles) assertSpecifier(resolve(dirname(path), reference.fileName));
    visit(syntax);
  }
}

function normalizedPath(path: string): string {
  const normalized = resolve(path.replaceAll("\\", "/")).replaceAll("\\", "/");
  return ts.sys.useCaseSensitiveFileNames ? normalized : normalized.toLowerCase();
}

function projectCompilerOptions(): ts.CompilerOptions {
  const config = ts.parseJsonSourceFileConfigFileContent(
    ts.readJsonConfigFile("tsconfig.web.json", (path) => ts.sys.readFile(path)), ts.sys, process.cwd()
  );
  if (config.errors.length > 0) {
    throw new Error(ts.formatDiagnostics(config.errors, {
      getCurrentDirectory: () => process.cwd(),
      getCanonicalFileName: (file) => file,
      getNewLine: () => "\n"
    }));
  }
  return config.options;
}

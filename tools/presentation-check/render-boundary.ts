import ts from "typescript";

export interface SourceText {
  readonly path: string;
  readonly text: string;
}

export function assertRenderImportBoundary(sources: readonly SourceText[]): void {
  for (const source of sources) {
    const path = source.path.replaceAll("\\", "/");
    const inThreeAdapter = path.startsWith("web/src/render/engines/three/");
    const compositionRoot = path === "web/src/main.ts";
    const syntax = ts.createSourceFile(path, source.text, ts.ScriptTarget.Latest, true);
    for (const statement of syntax.statements) {
      if (ts.isImportDeclaration(statement) || ts.isExportDeclaration(statement)) {
        const specifier = statement.moduleSpecifier;
        if (specifier !== undefined && ts.isStringLiteralLike(specifier)) {
          assertSpecifier(path, specifier.text, inThreeAdapter, compositionRoot);
        }
      }
    }
    function visit(node: ts.Node): void {
      if (ts.isCallExpression(node) &&
          (node.expression.kind === ts.SyntaxKind.ImportKeyword || (ts.isIdentifier(node.expression) && node.expression.text === "require"))) {
        const specifier = node.arguments[0];
        if (specifier === undefined || !ts.isStringLiteralLike(specifier)) {
          throw new Error(`Dynamic module specifier prevents render-boundary verification: ${path}`);
        }
        assertSpecifier(path, specifier.text, inThreeAdapter, compositionRoot);
      }
      ts.forEachChild(node, visit);
    }
    visit(syntax);
  }
}

function assertSpecifier(path: string, specifier: string, inThreeAdapter: boolean, compositionRoot: boolean): void {
  const importsThree = specifier === "three" || specifier.startsWith("three/");
  if (importsThree && !inThreeAdapter) {
    throw new Error(`Three.js import escaped its adapter: ${path} -> ${specifier}`);
  }
  if (specifier.includes("render/engines/three/") && !inThreeAdapter && !compositionRoot) {
    throw new Error(`Three.js adapter import escaped the composition root: ${path} -> ${specifier}`);
  }
}

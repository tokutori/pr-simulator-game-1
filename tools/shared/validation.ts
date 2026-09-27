/** Narrows untrusted configuration objects without accepting arrays or null. */
export function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("Expected an object");
  }
  return value as Record<string, unknown>;
}

/** Rejects missing and blank configuration strings at the external boundary. */
export function nonEmpty(value: unknown): string {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error("Expected a non-empty string");
  }
  return value;
}

/** Validates an array of strings received from JSON or TOML. */
export function strings(value: unknown): string[] {
  if (!Array.isArray(value)) throw new Error("Expected an array");
  return value.map((item: unknown) => nonEmpty(item));
}

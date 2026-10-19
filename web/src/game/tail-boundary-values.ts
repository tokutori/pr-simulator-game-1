export function captureJsonInput<Value>(value: Value): Value {
  const ancestors = new Set<object>();
  const capture = (field: unknown): unknown => {
    if (typeof field === "object" && field !== null) {
      if (ancestors.has(field)) throw new RangeError("JSON input contains a cycle");
      ancestors.add(field);
      const result = Array.isArray(field)
        ? Object.setPrototypeOf(Array.from({ length: field.length }, (_, index) => Object.hasOwn(field, index) ? capture(field[index]) : null), null) as unknown[]
        : Object.setPrototypeOf(Object.fromEntries(Object.entries(field).map(([key, child]) => [key, capture(child)])), null) as Record<string, unknown>;
      ancestors.delete(field);
      return result;
    }
    if (typeof field === "function" || typeof field === "symbol" || typeof field === "bigint" || field === undefined
        || typeof field === "number" && !Number.isFinite(field)) {
      throw new RangeError("JSON input requires data fields");
    }
    return field;
  };
  return capture(value) as Value;
}

export function boundaryObject(value: unknown, fields: readonly string[]): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new RangeError("Tail boundary value must be an object");
  }
  const object = value as Record<string, unknown>;
  if (Object.keys(object).length !== fields.length || fields.some((field) => !Object.hasOwn(object, field))) {
    throw new RangeError("Tail boundary fields do not match the declared schema");
  }
  return object;
}

export function boundaryNumber(value: unknown, minimum = -Infinity, maximum = Infinity): number {
  if (typeof value !== "number" || !Number.isFinite(value) || value < minimum || value > maximum) {
    throw new RangeError("Tail boundary number is outside its finite domain");
  }
  return value;
}

export function boundaryInteger(value: unknown, minimum = 0, maximum = Number.MAX_SAFE_INTEGER): number {
  const number = boundaryNumber(value, minimum, maximum);
  if (!Number.isSafeInteger(number)) throw new RangeError("Tail boundary integer must be exact");
  return number;
}

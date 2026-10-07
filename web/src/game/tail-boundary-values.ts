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

export function boundaryTag<const Tags extends readonly string[]>(value: unknown, tags: Tags): Tags[number] {
  if (typeof value !== "string" || !tags.includes(value)) throw new RangeError("Unknown tail boundary tag");
  return value;
}

export function boundaryTuple(value: unknown, length: number): readonly number[] {
  if (!Array.isArray(value) || value.length !== length) throw new RangeError("Invalid tail boundary tuple length");
  return Object.freeze(value.map((component: unknown) => boundaryNumber(component)));
}

export function boundaryExternalTag(value: unknown): Readonly<{ tag: string; payload: unknown }> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new RangeError("Tail failure must use a known externally tagged variant");
  }
  const keys = Object.keys(value);
  const tag = keys[0];
  if (keys.length !== 1 || tag === undefined) throw new RangeError("Tail failure must have exactly one variant");
  const object = value as Record<string, unknown>;
  return { tag, payload: object[tag] };
}

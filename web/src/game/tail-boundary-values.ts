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

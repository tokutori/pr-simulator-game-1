import type { FormatDefinition } from "ajv";

function integerBetween(minimum: number, maximum: number): (value: number) => boolean {
  return (value) => Number.isInteger(value) && value >= minimum && value <= maximum;
}

export const numericFormats: Readonly<Record<string, FormatDefinition<number>>> = {
  double: { type: "number", validate: Number.isFinite },
  float: { type: "number", validate: (value) => Number.isFinite(value) && Math.abs(value) <= 3.4028234663852886e38 },
  int8: { type: "number", validate: integerBetween(-128, 127) },
  uint8: { type: "number", validate: integerBetween(0, 255) },
  int16: { type: "number", validate: integerBetween(-32768, 32767) },
  uint16: { type: "number", validate: integerBetween(0, 65535) },
  int32: { type: "number", validate: integerBetween(-2147483648, 2147483647) },
  uint32: { type: "number", validate: integerBetween(0, 4294967295) },
  int64: { type: "number", validate: Number.isSafeInteger },
  uint64: { type: "number", validate: (value) => Number.isSafeInteger(value) && value >= 0 }
};

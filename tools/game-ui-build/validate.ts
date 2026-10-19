import { readFileSync } from "node:fs";
import { Ajv2020 } from "ajv/dist/2020.js";
import type { ValidateFunction } from "ajv";
import { numericFormats } from "./numeric-formats.js";
import { schemaEntries } from "./generator.js";

const validator = new Ajv2020({ strict: true, strictNumbers: true, ownProperties: true });
validator.addKeyword("x-boundary-bytes");
for (const [name, format] of Object.entries(numericFormats)) validator.addFormat(name, format);
for (const group of ["serialize", "deserialize"] as const) {
  const schema: unknown = JSON.parse(readFileSync(new URL(`./generated/game-ui-${group}.schema.json`, import.meta.url), "utf8"));
  if (typeof schema !== "object" || schema === null) throw new Error(`Missing ${group} contract schema`);
  validator.addSchema(schema);
}

export function contractValidator<Value>(key: typeof schemaEntries[number]["key"]): ValidateFunction<Value> {
  const entry = schemaEntries.find((entry) => entry.key === key);
  if (entry === undefined) throw new Error(`Missing contract ${key}`);
  const validate = validator.getSchema<Value>(`https://tokutori.github.io/pr-simulator-game-1/game-ui/${entry.group}#/$defs/${entry.name}`);
  if (validate === undefined) throw new Error(`Missing generated schema ${entry.name}`);
  return validate;
}

export function readContract<Value>(key: typeof schemaEntries[number]["key"], json: string): Value {
  const value: unknown = JSON.parse(json);
  const validate = contractValidator<Value>(key);
  if (!validate(value)) throw new Error(`${key}: ${validator.errorsText(validate.errors)}`);
  return value;
}

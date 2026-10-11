/* Generated from birdman-game-session::game_ui. Edit the Rust contract and regenerate. */
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __esm = (fn, res, err) => function __init() {
  if (err) throw err[0];
  try {
    return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
  } catch (e) {
    throw err = [e], e;
  }
};
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// tools/game-ui-build/numeric-formats.ts
var numeric_formats_exports = {};
__export(numeric_formats_exports, {
  numericFormats: () => numericFormats
});
function integerBetween(minimum, maximum) {
  return (value) => Number.isInteger(value) && value >= minimum && value <= maximum;
}
var numericFormats;
var init_numeric_formats = __esm({
  "tools/game-ui-build/numeric-formats.ts"() {
    "use strict";
    numericFormats = {
      double: { type: "number", validate: Number.isFinite },
      float: { type: "number", validate: (value) => Number.isFinite(value) && Math.abs(value) <= 34028234663852886e22 },
      int8: { type: "number", validate: integerBetween(-128, 127) },
      uint8: { type: "number", validate: integerBetween(0, 255) },
      int16: { type: "number", validate: integerBetween(-32768, 32767) },
      uint16: { type: "number", validate: integerBetween(0, 65535) },
      int32: { type: "number", validate: integerBetween(-2147483648, 2147483647) },
      uint32: { type: "number", validate: integerBetween(0, 4294967295) },
      int64: { type: "number", validate: Number.isSafeInteger },
      uint64: { type: "number", validate: (value) => Number.isSafeInteger(value) && value >= 0 }
    };
  }
});

// game-ui-validators.ts
var validateGameUiView = validate53;
var schema20 = { "additionalProperties": false, "description": "Pure shared scene and menu projection.", "properties": { "active_overlay": { "anyOf": [{ "$ref": "#/$defs/GameUiOverlay" }, { "type": "null" }], "description": "Shared exclusive overlay name." }, "description": { "description": "Shared explanatory text.", "type": "string" }, "hud": { "anyOf": [{ "$ref": "#/$defs/GameUiHud" }, { "type": "null" }], "description": "Shared telemetry and instrument projection, absent outside a flight or record." }, "layout": { "$ref": "#/$defs/GameUiLayout", "description": "Shared responsive dimensions and scroll policy." }, "panels": { "description": "Ordered semantic panels.", "items": { "$ref": "#/$defs/GameUiPanel" }, "type": "array" }, "scene": { "$ref": "#/$defs/GameUiScene", "description": "Domain-derived scene." }, "schema_version": { "description": "Boundary version.", "format": "uint32", "minimum": 0, "type": "integer" }, "selections": { "$ref": "#/$defs/GameUiState", "description": "Readonly observation of the Rust-owned UI selections." }, "theme": { "$ref": "#/$defs/GameUiTheme", "description": "Shared visual hierarchy." }, "title": { "description": "Shared heading.", "type": "string" }, "view_revision": { "description": "Adapter query-cache revision; pure projections use zero until published.", "format": "uint32", "minimum": 0, "type": "integer" } }, "required": ["schema_version", "view_revision", "selections", "scene", "title", "description", "active_overlay", "panels", "layout", "theme", "hud"], "type": "object" };
var func1 = Object.prototype.hasOwnProperty;
function validate54(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate54.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("Pause" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "Pause" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("PauseSettings" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "PauseSettings" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("PauseHelp" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "PauseHelp" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate54.errors = vErrors;
  return errors === 0;
}
validate54.evaluated = { "dynamicProps": false, "dynamicItems": false };
var schema22 = { "additionalProperties": false, "description": "Shared Web-baseline HUD model; no adapter rederives flight values or labels.", "properties": { "angle_of_attack": { "description": "AoA text, including explicit missing-angle marker.", "type": ["string", "null"] }, "angle_of_attack_degrees": { "description": "Defined angle of attack in degrees.", "format": "double", "type": ["number", "null"] }, "angle_of_attack_label": { "description": "AoA instrument label.", "type": "string" }, "attitude": { "anyOf": [{ "$ref": "#/$defs/GameUiHudAttitude" }, { "type": "null" }], "description": "ADI input already converted into display degrees." }, "controls_description": { "description": "Canonical input instructions.", "type": "string" }, "flight_path_angle_degrees": { "description": "Flight path indicator angle.", "format": "double", "type": ["number", "null"] }, "geometry": { "$ref": "#/$defs/GameUiHudGeometry", "description": "Shared Web-baseline instrument projections, independent of engine pixels." }, "heading": { "description": "Heading instrument text.", "type": ["string", "null"] }, "heading_degrees": { "description": "True heading in [0,360).", "format": "double", "type": ["number", "null"] }, "heading_label": { "description": "Heading instrument label.", "type": "string" }, "layout": { "$ref": "#/$defs/GameUiHudLayout", "description": "Shared Web-baseline instrument placement and responsive dimensions." }, "location": { "description": "Local-frame geographic display.", "type": "string" }, "map_attribution": { "description": "Preserved map attribution.", "type": "string" }, "pilot_position": { "description": "Pilot displacement instrument text.", "type": ["string", "null"] }, "pilot_position_label": { "description": "Pilot instrument label.", "type": "string" }, "readouts": { "description": "Primary readouts with shared wording and precision.", "type": "string" }, "status": { "description": "Domain-derived state label.", "type": "string" }, "supplementary_readouts": { "description": "Angular rates, physical incidences and pilot target text.", "items": { "type": "string" }, "type": "array" }, "telemetry": { "description": "Distance, time and ground-speed text.", "type": "string" }, "warning": { "description": "Terminal or model warning.", "type": ["string", "null"] }, "wind": { "description": "NED wind readout.", "type": ["string", "null"] }, "wind_direction_degrees": { "description": "Ambient air movement direction in [0,360).", "format": "double", "type": ["number", "null"] }, "wind_label": { "description": "Wind instrument label.", "type": "string" } }, "required": ["layout", "geometry", "status", "attitude", "flight_path_angle_degrees", "warning", "heading_degrees", "wind_direction_degrees", "angle_of_attack_degrees", "readouts", "heading", "heading_label", "pilot_position", "pilot_position_label", "wind", "wind_label", "angle_of_attack", "angle_of_attack_label", "telemetry", "location", "map_attribution", "supplementary_readouts", "controls_description"], "type": "object" };
var formats0 = (init_numeric_formats(), __toCommonJS(numeric_formats_exports)).numericFormats.double;
function validate57(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate57.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.roll_degrees === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "roll_degrees" }, message: "must have required property 'roll_degrees'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.pitch_degrees === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "pitch_degrees" }, message: "must have required property 'pitch_degrees'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "pitch_degrees" || key0 === "roll_degrees")) {
        const err2 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.pitch_degrees !== void 0) {
      let data0 = data.pitch_degrees;
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err3 = { instancePath: instancePath + "/pitch_degrees", schemaPath: "#/properties/pitch_degrees/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err3];
          } else {
            vErrors.push(err3);
          }
          errors++;
        }
      } else {
        const err4 = { instancePath: instancePath + "/pitch_degrees", schemaPath: "#/properties/pitch_degrees/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.roll_degrees !== void 0) {
      let data1 = data.roll_degrees;
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err5 = { instancePath: instancePath + "/roll_degrees", schemaPath: "#/properties/roll_degrees/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        }
      } else {
        const err6 = { instancePath: instancePath + "/roll_degrees", schemaPath: "#/properties/roll_degrees/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
  } else {
    const err7 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err7];
    } else {
      vErrors.push(err7);
    }
    errors++;
  }
  validate57.errors = vErrors;
  return errors === 0;
}
validate57.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var schema24 = { "additionalProperties": false, "description": "Shared instrument geometry derived from the authoritative HUD angles.", "properties": { "adi": { "anyOf": [{ "$ref": "#/$defs/GameUiHudAdiGeometry" }, { "type": "null" }], "description": "Visible ADI layers, absent when both attitude and flight path cues are hidden." }, "angle_of_attack_ratio": { "description": "AoA position between the displayed -10 and +20 degree endpoints.", "format": "double", "type": ["number", "null"] }, "heading_ticks": { "description": "Ordered heading tape marks and compass labels.", "items": { "$ref": "#/$defs/GameUiHudHeadingTick" }, "type": "array" } }, "required": ["adi", "heading_ticks", "angle_of_attack_ratio"], "type": "object" };
var schema25 = { "description": "Visible ADI region with an attitude layer or an independent flight path cue.", "oneOf": [{ "additionalProperties": false, "description": "Attitude horizon with an optional flight path marker.", "properties": { "flight_path_y_ratio": { "description": "Flight path marker position relative to the ADI height.", "format": "double", "type": ["number", "null"] }, "horizon": { "$ref": "#/$defs/GameUiHudAttitudeGeometry", "description": "Horizon rotation and translation." }, "kind": { "const": "attitude", "type": "string" } }, "required": ["kind", "horizon", "flight_path_y_ratio"], "type": "object" }, { "additionalProperties": false, "description": "Flight path marker without an attitude horizon.", "properties": { "kind": { "const": "flight_path_only", "type": "string" }, "y_ratio": { "description": "Flight path marker position relative to the ADI height.", "format": "double", "type": "number" } }, "required": ["kind", "y_ratio"], "type": "object" }] };
function validate61(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate61.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.rotation_degrees === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "rotation_degrees" }, message: "must have required property 'rotation_degrees'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.pitch_offset_ratio === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "pitch_offset_ratio" }, message: "must have required property 'pitch_offset_ratio'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "pitch_offset_ratio" || key0 === "rotation_degrees")) {
        const err2 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.pitch_offset_ratio !== void 0) {
      let data0 = data.pitch_offset_ratio;
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err3 = { instancePath: instancePath + "/pitch_offset_ratio", schemaPath: "#/properties/pitch_offset_ratio/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err3];
          } else {
            vErrors.push(err3);
          }
          errors++;
        }
      } else {
        const err4 = { instancePath: instancePath + "/pitch_offset_ratio", schemaPath: "#/properties/pitch_offset_ratio/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.rotation_degrees !== void 0) {
      let data1 = data.rotation_degrees;
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err5 = { instancePath: instancePath + "/rotation_degrees", schemaPath: "#/properties/rotation_degrees/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        }
      } else {
        const err6 = { instancePath: instancePath + "/rotation_degrees", schemaPath: "#/properties/rotation_degrees/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
  } else {
    const err7 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err7];
    } else {
      vErrors.push(err7);
    }
    errors++;
  }
  validate61.errors = vErrors;
  return errors === 0;
}
validate61.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate60(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate60.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.horizon === void 0) {
      const err1 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "horizon" }, message: "must have required property 'horizon'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.flight_path_y_ratio === void 0) {
      const err2 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "flight_path_y_ratio" }, message: "must have required property 'flight_path_y_ratio'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "flight_path_y_ratio" || key0 === "horizon" || key0 === "kind")) {
        const err3 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.flight_path_y_ratio !== void 0) {
      let data0 = data.flight_path_y_ratio;
      if (!(typeof data0 == "number" && isFinite(data0)) && data0 !== null) {
        const err4 = { instancePath: instancePath + "/flight_path_y_ratio", schemaPath: "#/oneOf/0/properties/flight_path_y_ratio/type", keyword: "type", params: { type: schema25.oneOf[0].properties.flight_path_y_ratio.type }, message: "must be number,null" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err5 = { instancePath: instancePath + "/flight_path_y_ratio", schemaPath: "#/oneOf/0/properties/flight_path_y_ratio/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        }
      }
    }
    if (data.horizon !== void 0) {
      if (!validate61(data.horizon, { instancePath: instancePath + "/horizon", parentData: data, parentDataProperty: "horizon", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate61.errors : vErrors.concat(validate61.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data2 = data.kind;
      if (typeof data2 !== "string") {
        const err6 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("attitude" !== data2) {
        const err7 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "attitude" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
  } else {
    const err8 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs9 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err9 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    if (data.y_ratio === void 0) {
      const err10 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "y_ratio" }, message: "must have required property 'y_ratio'" };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "kind" || key1 === "y_ratio")) {
        const err11 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data3 = data.kind;
      if (typeof data3 !== "string") {
        const err12 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
      if ("flight_path_only" !== data3) {
        const err13 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "flight_path_only" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      }
    }
    if (data.y_ratio !== void 0) {
      let data4 = data.y_ratio;
      if (typeof data4 == "number" && isFinite(data4)) {
        if (!formats0.validate(data4)) {
          const err14 = { instancePath: instancePath + "/y_ratio", schemaPath: "#/oneOf/1/properties/y_ratio/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
      } else {
        const err15 = { instancePath: instancePath + "/y_ratio", schemaPath: "#/oneOf/1/properties/y_ratio/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
    }
  } else {
    const err16 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err16];
    } else {
      vErrors.push(err16);
    }
    errors++;
  }
  var _valid0 = _errs9 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
  }
  if (!valid0) {
    const err17 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err17];
    } else {
      vErrors.push(err17);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate60.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate60.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate64(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate64.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.position_ratio === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "position_ratio" }, message: "must have required property 'position_ratio'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.major === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "major" }, message: "must have required property 'major'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.label === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "label" || key0 === "major" || key0 === "position_ratio")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err4 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.major !== void 0) {
      if (typeof data.major !== "boolean") {
        const err5 = { instancePath: instancePath + "/major", schemaPath: "#/properties/major/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.position_ratio !== void 0) {
      let data2 = data.position_ratio;
      if (typeof data2 == "number" && isFinite(data2)) {
        if (!formats0.validate(data2)) {
          const err6 = { instancePath: instancePath + "/position_ratio", schemaPath: "#/properties/position_ratio/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err6];
          } else {
            vErrors.push(err6);
          }
          errors++;
        }
      } else {
        const err7 = { instancePath: instancePath + "/position_ratio", schemaPath: "#/properties/position_ratio/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
  } else {
    const err8 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
  }
  validate64.errors = vErrors;
  return errors === 0;
}
validate64.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate59(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate59.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.adi === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "adi" }, message: "must have required property 'adi'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.heading_ticks === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "heading_ticks" }, message: "must have required property 'heading_ticks'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.angle_of_attack_ratio === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "angle_of_attack_ratio" }, message: "must have required property 'angle_of_attack_ratio'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "adi" || key0 === "angle_of_attack_ratio" || key0 === "heading_ticks")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.adi !== void 0) {
      let data0 = data.adi;
      const _errs3 = errors;
      let valid1 = false;
      const _errs4 = errors;
      if (!validate60(data0, { instancePath: instancePath + "/adi", parentData: data, parentDataProperty: "adi", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate60.errors : vErrors.concat(validate60.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs4 === errors;
      valid1 = valid1 || _valid0;
      const _errs5 = errors;
      if (data0 !== null) {
        const err4 = { instancePath: instancePath + "/adi", schemaPath: "#/properties/adi/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
      var _valid0 = _errs5 === errors;
      valid1 = valid1 || _valid0;
      if (!valid1) {
        const err5 = { instancePath: instancePath + "/adi", schemaPath: "#/properties/adi/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      } else {
        errors = _errs3;
        if (vErrors !== null) {
          if (_errs3) {
            vErrors.length = _errs3;
          } else {
            vErrors = null;
          }
        }
      }
    }
    if (data.angle_of_attack_ratio !== void 0) {
      let data1 = data.angle_of_attack_ratio;
      if (!(typeof data1 == "number" && isFinite(data1)) && data1 !== null) {
        const err6 = { instancePath: instancePath + "/angle_of_attack_ratio", schemaPath: "#/properties/angle_of_attack_ratio/type", keyword: "type", params: { type: schema24.properties.angle_of_attack_ratio.type }, message: "must be number,null" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err7 = { instancePath: instancePath + "/angle_of_attack_ratio", schemaPath: "#/properties/angle_of_attack_ratio/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err7];
          } else {
            vErrors.push(err7);
          }
          errors++;
        }
      }
    }
    if (data.heading_ticks !== void 0) {
      let data2 = data.heading_ticks;
      if (Array.isArray(data2)) {
        const len0 = data2.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate64(data2[i0], { instancePath: instancePath + "/heading_ticks/" + i0, parentData: data2, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate64.errors : vErrors.concat(validate64.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err8 = { instancePath: instancePath + "/heading_ticks", schemaPath: "#/properties/heading_ticks/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
  } else {
    const err9 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err9];
    } else {
      vErrors.push(err9);
    }
    errors++;
  }
  validate59.errors = vErrors;
  return errors === 0;
}
validate59.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var schema28 = { "additionalProperties": false, "description": "Instrument placement in logical pixels with platform safe-area insets applied by adapters.", "properties": { "attitude_size_px": { "description": "ADI width and height.", "items": { "format": "double", "type": "number" }, "maxItems": 2, "minItems": 2, "type": "array" }, "controls_visible": { "description": "Keyboard description visibility on the selected viewport.", "type": "boolean" }, "font_size_px": { "description": "Readout font size.", "format": "double", "type": "number" }, "gap_px": { "description": "Horizontal and vertical grid gaps.", "items": { "format": "double", "type": "number" }, "maxItems": 2, "minItems": 2, "type": "array" }, "instrument_columns": { "description": "Heading, pilot, wind and AoA instrument column count.", "format": "uint32", "minimum": 0, "type": "integer" }, "maximum_height_px": { "description": "Reachable vertical scroll height.", "format": "double", "type": "number" }, "offset_px": { "description": "Left and top viewport offsets.", "items": { "format": "double", "type": "number" }, "maxItems": 2, "minItems": 2, "type": "array" }, "opacity": { "description": "Panel background opacity.", "format": "double", "type": "number" }, "padding_px": { "description": "Horizontal and vertical padding.", "items": { "format": "double", "type": "number" }, "maxItems": 2, "minItems": 2, "type": "array" }, "width_px": { "description": "Responsive panel width.", "format": "double", "type": "number" } }, "required": ["offset_px", "width_px", "maximum_height_px", "padding_px", "gap_px", "attitude_size_px", "font_size_px", "opacity", "controls_visible", "instrument_columns"], "type": "object" };
var formats28 = (init_numeric_formats(), __toCommonJS(numeric_formats_exports)).numericFormats.uint32;
function validate67(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate67.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.offset_px === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "offset_px" }, message: "must have required property 'offset_px'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.width_px === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "width_px" }, message: "must have required property 'width_px'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.maximum_height_px === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "maximum_height_px" }, message: "must have required property 'maximum_height_px'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.padding_px === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "padding_px" }, message: "must have required property 'padding_px'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.gap_px === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "gap_px" }, message: "must have required property 'gap_px'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.attitude_size_px === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "attitude_size_px" }, message: "must have required property 'attitude_size_px'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.font_size_px === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "font_size_px" }, message: "must have required property 'font_size_px'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.opacity === void 0) {
      const err7 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "opacity" }, message: "must have required property 'opacity'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.controls_visible === void 0) {
      const err8 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "controls_visible" }, message: "must have required property 'controls_visible'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.instrument_columns === void 0) {
      const err9 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "instrument_columns" }, message: "must have required property 'instrument_columns'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!func1.call(schema28.properties, key0)) {
        const err10 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
    if (data.attitude_size_px !== void 0) {
      let data0 = data.attitude_size_px;
      if (Array.isArray(data0)) {
        if (data0.length > 2) {
          const err11 = { instancePath: instancePath + "/attitude_size_px", schemaPath: "#/properties/attitude_size_px/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err11];
          } else {
            vErrors.push(err11);
          }
          errors++;
        }
        if (data0.length < 2) {
          const err12 = { instancePath: instancePath + "/attitude_size_px", schemaPath: "#/properties/attitude_size_px/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err12];
          } else {
            vErrors.push(err12);
          }
          errors++;
        }
        const len0 = data0.length;
        for (let i0 = 0; i0 < len0; i0++) {
          let data1 = data0[i0];
          if (typeof data1 == "number" && isFinite(data1)) {
            if (!formats0.validate(data1)) {
              const err13 = { instancePath: instancePath + "/attitude_size_px/" + i0, schemaPath: "#/properties/attitude_size_px/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err13];
              } else {
                vErrors.push(err13);
              }
              errors++;
            }
          } else {
            const err14 = { instancePath: instancePath + "/attitude_size_px/" + i0, schemaPath: "#/properties/attitude_size_px/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err14];
            } else {
              vErrors.push(err14);
            }
            errors++;
          }
        }
      } else {
        const err15 = { instancePath: instancePath + "/attitude_size_px", schemaPath: "#/properties/attitude_size_px/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
    }
    if (data.controls_visible !== void 0) {
      if (typeof data.controls_visible !== "boolean") {
        const err16 = { instancePath: instancePath + "/controls_visible", schemaPath: "#/properties/controls_visible/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err16];
        } else {
          vErrors.push(err16);
        }
        errors++;
      }
    }
    if (data.font_size_px !== void 0) {
      let data3 = data.font_size_px;
      if (typeof data3 == "number" && isFinite(data3)) {
        if (!formats0.validate(data3)) {
          const err17 = { instancePath: instancePath + "/font_size_px", schemaPath: "#/properties/font_size_px/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err17];
          } else {
            vErrors.push(err17);
          }
          errors++;
        }
      } else {
        const err18 = { instancePath: instancePath + "/font_size_px", schemaPath: "#/properties/font_size_px/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err18];
        } else {
          vErrors.push(err18);
        }
        errors++;
      }
    }
    if (data.gap_px !== void 0) {
      let data4 = data.gap_px;
      if (Array.isArray(data4)) {
        if (data4.length > 2) {
          const err19 = { instancePath: instancePath + "/gap_px", schemaPath: "#/properties/gap_px/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err19];
          } else {
            vErrors.push(err19);
          }
          errors++;
        }
        if (data4.length < 2) {
          const err20 = { instancePath: instancePath + "/gap_px", schemaPath: "#/properties/gap_px/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err20];
          } else {
            vErrors.push(err20);
          }
          errors++;
        }
        const len1 = data4.length;
        for (let i1 = 0; i1 < len1; i1++) {
          let data5 = data4[i1];
          if (typeof data5 == "number" && isFinite(data5)) {
            if (!formats0.validate(data5)) {
              const err21 = { instancePath: instancePath + "/gap_px/" + i1, schemaPath: "#/properties/gap_px/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err21];
              } else {
                vErrors.push(err21);
              }
              errors++;
            }
          } else {
            const err22 = { instancePath: instancePath + "/gap_px/" + i1, schemaPath: "#/properties/gap_px/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err22];
            } else {
              vErrors.push(err22);
            }
            errors++;
          }
        }
      } else {
        const err23 = { instancePath: instancePath + "/gap_px", schemaPath: "#/properties/gap_px/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err23];
        } else {
          vErrors.push(err23);
        }
        errors++;
      }
    }
    if (data.instrument_columns !== void 0) {
      let data6 = data.instrument_columns;
      if (!(typeof data6 == "number" && (!(data6 % 1) && !isNaN(data6)) && isFinite(data6))) {
        const err24 = { instancePath: instancePath + "/instrument_columns", schemaPath: "#/properties/instrument_columns/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
        if (vErrors === null) {
          vErrors = [err24];
        } else {
          vErrors.push(err24);
        }
        errors++;
      }
      if (typeof data6 == "number" && isFinite(data6)) {
        if (data6 < 0 || isNaN(data6)) {
          const err25 = { instancePath: instancePath + "/instrument_columns", schemaPath: "#/properties/instrument_columns/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
          if (vErrors === null) {
            vErrors = [err25];
          } else {
            vErrors.push(err25);
          }
          errors++;
        }
        if (!formats28.validate(data6)) {
          const err26 = { instancePath: instancePath + "/instrument_columns", schemaPath: "#/properties/instrument_columns/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
          if (vErrors === null) {
            vErrors = [err26];
          } else {
            vErrors.push(err26);
          }
          errors++;
        }
      }
    }
    if (data.maximum_height_px !== void 0) {
      let data7 = data.maximum_height_px;
      if (typeof data7 == "number" && isFinite(data7)) {
        if (!formats0.validate(data7)) {
          const err27 = { instancePath: instancePath + "/maximum_height_px", schemaPath: "#/properties/maximum_height_px/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err27];
          } else {
            vErrors.push(err27);
          }
          errors++;
        }
      } else {
        const err28 = { instancePath: instancePath + "/maximum_height_px", schemaPath: "#/properties/maximum_height_px/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err28];
        } else {
          vErrors.push(err28);
        }
        errors++;
      }
    }
    if (data.offset_px !== void 0) {
      let data8 = data.offset_px;
      if (Array.isArray(data8)) {
        if (data8.length > 2) {
          const err29 = { instancePath: instancePath + "/offset_px", schemaPath: "#/properties/offset_px/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err29];
          } else {
            vErrors.push(err29);
          }
          errors++;
        }
        if (data8.length < 2) {
          const err30 = { instancePath: instancePath + "/offset_px", schemaPath: "#/properties/offset_px/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err30];
          } else {
            vErrors.push(err30);
          }
          errors++;
        }
        const len2 = data8.length;
        for (let i2 = 0; i2 < len2; i2++) {
          let data9 = data8[i2];
          if (typeof data9 == "number" && isFinite(data9)) {
            if (!formats0.validate(data9)) {
              const err31 = { instancePath: instancePath + "/offset_px/" + i2, schemaPath: "#/properties/offset_px/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err31];
              } else {
                vErrors.push(err31);
              }
              errors++;
            }
          } else {
            const err32 = { instancePath: instancePath + "/offset_px/" + i2, schemaPath: "#/properties/offset_px/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err32];
            } else {
              vErrors.push(err32);
            }
            errors++;
          }
        }
      } else {
        const err33 = { instancePath: instancePath + "/offset_px", schemaPath: "#/properties/offset_px/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err33];
        } else {
          vErrors.push(err33);
        }
        errors++;
      }
    }
    if (data.opacity !== void 0) {
      let data10 = data.opacity;
      if (typeof data10 == "number" && isFinite(data10)) {
        if (!formats0.validate(data10)) {
          const err34 = { instancePath: instancePath + "/opacity", schemaPath: "#/properties/opacity/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err34];
          } else {
            vErrors.push(err34);
          }
          errors++;
        }
      } else {
        const err35 = { instancePath: instancePath + "/opacity", schemaPath: "#/properties/opacity/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err35];
        } else {
          vErrors.push(err35);
        }
        errors++;
      }
    }
    if (data.padding_px !== void 0) {
      let data11 = data.padding_px;
      if (Array.isArray(data11)) {
        if (data11.length > 2) {
          const err36 = { instancePath: instancePath + "/padding_px", schemaPath: "#/properties/padding_px/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err36];
          } else {
            vErrors.push(err36);
          }
          errors++;
        }
        if (data11.length < 2) {
          const err37 = { instancePath: instancePath + "/padding_px", schemaPath: "#/properties/padding_px/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err37];
          } else {
            vErrors.push(err37);
          }
          errors++;
        }
        const len3 = data11.length;
        for (let i3 = 0; i3 < len3; i3++) {
          let data12 = data11[i3];
          if (typeof data12 == "number" && isFinite(data12)) {
            if (!formats0.validate(data12)) {
              const err38 = { instancePath: instancePath + "/padding_px/" + i3, schemaPath: "#/properties/padding_px/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err38];
              } else {
                vErrors.push(err38);
              }
              errors++;
            }
          } else {
            const err39 = { instancePath: instancePath + "/padding_px/" + i3, schemaPath: "#/properties/padding_px/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err39];
            } else {
              vErrors.push(err39);
            }
            errors++;
          }
        }
      } else {
        const err40 = { instancePath: instancePath + "/padding_px", schemaPath: "#/properties/padding_px/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err40];
        } else {
          vErrors.push(err40);
        }
        errors++;
      }
    }
    if (data.width_px !== void 0) {
      let data13 = data.width_px;
      if (typeof data13 == "number" && isFinite(data13)) {
        if (!formats0.validate(data13)) {
          const err41 = { instancePath: instancePath + "/width_px", schemaPath: "#/properties/width_px/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err41];
          } else {
            vErrors.push(err41);
          }
          errors++;
        }
      } else {
        const err42 = { instancePath: instancePath + "/width_px", schemaPath: "#/properties/width_px/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err42];
        } else {
          vErrors.push(err42);
        }
        errors++;
      }
    }
  } else {
    const err43 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err43];
    } else {
      vErrors.push(err43);
    }
    errors++;
  }
  validate67.errors = vErrors;
  return errors === 0;
}
validate67.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate56(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate56.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.layout === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "layout" }, message: "must have required property 'layout'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.geometry === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "geometry" }, message: "must have required property 'geometry'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.status === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "status" }, message: "must have required property 'status'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.attitude === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "attitude" }, message: "must have required property 'attitude'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.flight_path_angle_degrees === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "flight_path_angle_degrees" }, message: "must have required property 'flight_path_angle_degrees'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.warning === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "warning" }, message: "must have required property 'warning'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.heading_degrees === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "heading_degrees" }, message: "must have required property 'heading_degrees'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.wind_direction_degrees === void 0) {
      const err7 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "wind_direction_degrees" }, message: "must have required property 'wind_direction_degrees'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.angle_of_attack_degrees === void 0) {
      const err8 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "angle_of_attack_degrees" }, message: "must have required property 'angle_of_attack_degrees'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.readouts === void 0) {
      const err9 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "readouts" }, message: "must have required property 'readouts'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    if (data.heading === void 0) {
      const err10 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "heading" }, message: "must have required property 'heading'" };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
    if (data.heading_label === void 0) {
      const err11 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "heading_label" }, message: "must have required property 'heading_label'" };
      if (vErrors === null) {
        vErrors = [err11];
      } else {
        vErrors.push(err11);
      }
      errors++;
    }
    if (data.pilot_position === void 0) {
      const err12 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "pilot_position" }, message: "must have required property 'pilot_position'" };
      if (vErrors === null) {
        vErrors = [err12];
      } else {
        vErrors.push(err12);
      }
      errors++;
    }
    if (data.pilot_position_label === void 0) {
      const err13 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "pilot_position_label" }, message: "must have required property 'pilot_position_label'" };
      if (vErrors === null) {
        vErrors = [err13];
      } else {
        vErrors.push(err13);
      }
      errors++;
    }
    if (data.wind === void 0) {
      const err14 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "wind" }, message: "must have required property 'wind'" };
      if (vErrors === null) {
        vErrors = [err14];
      } else {
        vErrors.push(err14);
      }
      errors++;
    }
    if (data.wind_label === void 0) {
      const err15 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "wind_label" }, message: "must have required property 'wind_label'" };
      if (vErrors === null) {
        vErrors = [err15];
      } else {
        vErrors.push(err15);
      }
      errors++;
    }
    if (data.angle_of_attack === void 0) {
      const err16 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "angle_of_attack" }, message: "must have required property 'angle_of_attack'" };
      if (vErrors === null) {
        vErrors = [err16];
      } else {
        vErrors.push(err16);
      }
      errors++;
    }
    if (data.angle_of_attack_label === void 0) {
      const err17 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "angle_of_attack_label" }, message: "must have required property 'angle_of_attack_label'" };
      if (vErrors === null) {
        vErrors = [err17];
      } else {
        vErrors.push(err17);
      }
      errors++;
    }
    if (data.telemetry === void 0) {
      const err18 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "telemetry" }, message: "must have required property 'telemetry'" };
      if (vErrors === null) {
        vErrors = [err18];
      } else {
        vErrors.push(err18);
      }
      errors++;
    }
    if (data.location === void 0) {
      const err19 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "location" }, message: "must have required property 'location'" };
      if (vErrors === null) {
        vErrors = [err19];
      } else {
        vErrors.push(err19);
      }
      errors++;
    }
    if (data.map_attribution === void 0) {
      const err20 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "map_attribution" }, message: "must have required property 'map_attribution'" };
      if (vErrors === null) {
        vErrors = [err20];
      } else {
        vErrors.push(err20);
      }
      errors++;
    }
    if (data.supplementary_readouts === void 0) {
      const err21 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "supplementary_readouts" }, message: "must have required property 'supplementary_readouts'" };
      if (vErrors === null) {
        vErrors = [err21];
      } else {
        vErrors.push(err21);
      }
      errors++;
    }
    if (data.controls_description === void 0) {
      const err22 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "controls_description" }, message: "must have required property 'controls_description'" };
      if (vErrors === null) {
        vErrors = [err22];
      } else {
        vErrors.push(err22);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!func1.call(schema22.properties, key0)) {
        const err23 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err23];
        } else {
          vErrors.push(err23);
        }
        errors++;
      }
    }
    if (data.angle_of_attack !== void 0) {
      let data0 = data.angle_of_attack;
      if (typeof data0 !== "string" && data0 !== null) {
        const err24 = { instancePath: instancePath + "/angle_of_attack", schemaPath: "#/properties/angle_of_attack/type", keyword: "type", params: { type: schema22.properties.angle_of_attack.type }, message: "must be string,null" };
        if (vErrors === null) {
          vErrors = [err24];
        } else {
          vErrors.push(err24);
        }
        errors++;
      }
    }
    if (data.angle_of_attack_degrees !== void 0) {
      let data1 = data.angle_of_attack_degrees;
      if (!(typeof data1 == "number" && isFinite(data1)) && data1 !== null) {
        const err25 = { instancePath: instancePath + "/angle_of_attack_degrees", schemaPath: "#/properties/angle_of_attack_degrees/type", keyword: "type", params: { type: schema22.properties.angle_of_attack_degrees.type }, message: "must be number,null" };
        if (vErrors === null) {
          vErrors = [err25];
        } else {
          vErrors.push(err25);
        }
        errors++;
      }
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err26 = { instancePath: instancePath + "/angle_of_attack_degrees", schemaPath: "#/properties/angle_of_attack_degrees/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err26];
          } else {
            vErrors.push(err26);
          }
          errors++;
        }
      }
    }
    if (data.angle_of_attack_label !== void 0) {
      if (typeof data.angle_of_attack_label !== "string") {
        const err27 = { instancePath: instancePath + "/angle_of_attack_label", schemaPath: "#/properties/angle_of_attack_label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err27];
        } else {
          vErrors.push(err27);
        }
        errors++;
      }
    }
    if (data.attitude !== void 0) {
      let data3 = data.attitude;
      const _errs9 = errors;
      let valid1 = false;
      const _errs10 = errors;
      if (!validate57(data3, { instancePath: instancePath + "/attitude", parentData: data, parentDataProperty: "attitude", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate57.errors : vErrors.concat(validate57.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs10 === errors;
      valid1 = valid1 || _valid0;
      const _errs11 = errors;
      if (data3 !== null) {
        const err28 = { instancePath: instancePath + "/attitude", schemaPath: "#/properties/attitude/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
        if (vErrors === null) {
          vErrors = [err28];
        } else {
          vErrors.push(err28);
        }
        errors++;
      }
      var _valid0 = _errs11 === errors;
      valid1 = valid1 || _valid0;
      if (!valid1) {
        const err29 = { instancePath: instancePath + "/attitude", schemaPath: "#/properties/attitude/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
        if (vErrors === null) {
          vErrors = [err29];
        } else {
          vErrors.push(err29);
        }
        errors++;
      } else {
        errors = _errs9;
        if (vErrors !== null) {
          if (_errs9) {
            vErrors.length = _errs9;
          } else {
            vErrors = null;
          }
        }
      }
    }
    if (data.controls_description !== void 0) {
      if (typeof data.controls_description !== "string") {
        const err30 = { instancePath: instancePath + "/controls_description", schemaPath: "#/properties/controls_description/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err30];
        } else {
          vErrors.push(err30);
        }
        errors++;
      }
    }
    if (data.flight_path_angle_degrees !== void 0) {
      let data5 = data.flight_path_angle_degrees;
      if (!(typeof data5 == "number" && isFinite(data5)) && data5 !== null) {
        const err31 = { instancePath: instancePath + "/flight_path_angle_degrees", schemaPath: "#/properties/flight_path_angle_degrees/type", keyword: "type", params: { type: schema22.properties.flight_path_angle_degrees.type }, message: "must be number,null" };
        if (vErrors === null) {
          vErrors = [err31];
        } else {
          vErrors.push(err31);
        }
        errors++;
      }
      if (typeof data5 == "number" && isFinite(data5)) {
        if (!formats0.validate(data5)) {
          const err32 = { instancePath: instancePath + "/flight_path_angle_degrees", schemaPath: "#/properties/flight_path_angle_degrees/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err32];
          } else {
            vErrors.push(err32);
          }
          errors++;
        }
      }
    }
    if (data.geometry !== void 0) {
      if (!validate59(data.geometry, { instancePath: instancePath + "/geometry", parentData: data, parentDataProperty: "geometry", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate59.errors : vErrors.concat(validate59.errors);
        errors = vErrors.length;
      }
    }
    if (data.heading !== void 0) {
      let data7 = data.heading;
      if (typeof data7 !== "string" && data7 !== null) {
        const err33 = { instancePath: instancePath + "/heading", schemaPath: "#/properties/heading/type", keyword: "type", params: { type: schema22.properties.heading.type }, message: "must be string,null" };
        if (vErrors === null) {
          vErrors = [err33];
        } else {
          vErrors.push(err33);
        }
        errors++;
      }
    }
    if (data.heading_degrees !== void 0) {
      let data8 = data.heading_degrees;
      if (!(typeof data8 == "number" && isFinite(data8)) && data8 !== null) {
        const err34 = { instancePath: instancePath + "/heading_degrees", schemaPath: "#/properties/heading_degrees/type", keyword: "type", params: { type: schema22.properties.heading_degrees.type }, message: "must be number,null" };
        if (vErrors === null) {
          vErrors = [err34];
        } else {
          vErrors.push(err34);
        }
        errors++;
      }
      if (typeof data8 == "number" && isFinite(data8)) {
        if (!formats0.validate(data8)) {
          const err35 = { instancePath: instancePath + "/heading_degrees", schemaPath: "#/properties/heading_degrees/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err35];
          } else {
            vErrors.push(err35);
          }
          errors++;
        }
      }
    }
    if (data.heading_label !== void 0) {
      if (typeof data.heading_label !== "string") {
        const err36 = { instancePath: instancePath + "/heading_label", schemaPath: "#/properties/heading_label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err36];
        } else {
          vErrors.push(err36);
        }
        errors++;
      }
    }
    if (data.layout !== void 0) {
      if (!validate67(data.layout, { instancePath: instancePath + "/layout", parentData: data, parentDataProperty: "layout", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate67.errors : vErrors.concat(validate67.errors);
        errors = vErrors.length;
      }
    }
    if (data.location !== void 0) {
      if (typeof data.location !== "string") {
        const err37 = { instancePath: instancePath + "/location", schemaPath: "#/properties/location/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err37];
        } else {
          vErrors.push(err37);
        }
        errors++;
      }
    }
    if (data.map_attribution !== void 0) {
      if (typeof data.map_attribution !== "string") {
        const err38 = { instancePath: instancePath + "/map_attribution", schemaPath: "#/properties/map_attribution/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err38];
        } else {
          vErrors.push(err38);
        }
        errors++;
      }
    }
    if (data.pilot_position !== void 0) {
      let data13 = data.pilot_position;
      if (typeof data13 !== "string" && data13 !== null) {
        const err39 = { instancePath: instancePath + "/pilot_position", schemaPath: "#/properties/pilot_position/type", keyword: "type", params: { type: schema22.properties.pilot_position.type }, message: "must be string,null" };
        if (vErrors === null) {
          vErrors = [err39];
        } else {
          vErrors.push(err39);
        }
        errors++;
      }
    }
    if (data.pilot_position_label !== void 0) {
      if (typeof data.pilot_position_label !== "string") {
        const err40 = { instancePath: instancePath + "/pilot_position_label", schemaPath: "#/properties/pilot_position_label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err40];
        } else {
          vErrors.push(err40);
        }
        errors++;
      }
    }
    if (data.readouts !== void 0) {
      if (typeof data.readouts !== "string") {
        const err41 = { instancePath: instancePath + "/readouts", schemaPath: "#/properties/readouts/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err41];
        } else {
          vErrors.push(err41);
        }
        errors++;
      }
    }
    if (data.status !== void 0) {
      if (typeof data.status !== "string") {
        const err42 = { instancePath: instancePath + "/status", schemaPath: "#/properties/status/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err42];
        } else {
          vErrors.push(err42);
        }
        errors++;
      }
    }
    if (data.supplementary_readouts !== void 0) {
      let data17 = data.supplementary_readouts;
      if (Array.isArray(data17)) {
        const len0 = data17.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (typeof data17[i0] !== "string") {
            const err43 = { instancePath: instancePath + "/supplementary_readouts/" + i0, schemaPath: "#/properties/supplementary_readouts/items/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err43];
            } else {
              vErrors.push(err43);
            }
            errors++;
          }
        }
      } else {
        const err44 = { instancePath: instancePath + "/supplementary_readouts", schemaPath: "#/properties/supplementary_readouts/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err44];
        } else {
          vErrors.push(err44);
        }
        errors++;
      }
    }
    if (data.telemetry !== void 0) {
      if (typeof data.telemetry !== "string") {
        const err45 = { instancePath: instancePath + "/telemetry", schemaPath: "#/properties/telemetry/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err45];
        } else {
          vErrors.push(err45);
        }
        errors++;
      }
    }
    if (data.warning !== void 0) {
      let data20 = data.warning;
      if (typeof data20 !== "string" && data20 !== null) {
        const err46 = { instancePath: instancePath + "/warning", schemaPath: "#/properties/warning/type", keyword: "type", params: { type: schema22.properties.warning.type }, message: "must be string,null" };
        if (vErrors === null) {
          vErrors = [err46];
        } else {
          vErrors.push(err46);
        }
        errors++;
      }
    }
    if (data.wind !== void 0) {
      let data21 = data.wind;
      if (typeof data21 !== "string" && data21 !== null) {
        const err47 = { instancePath: instancePath + "/wind", schemaPath: "#/properties/wind/type", keyword: "type", params: { type: schema22.properties.wind.type }, message: "must be string,null" };
        if (vErrors === null) {
          vErrors = [err47];
        } else {
          vErrors.push(err47);
        }
        errors++;
      }
    }
    if (data.wind_direction_degrees !== void 0) {
      let data22 = data.wind_direction_degrees;
      if (!(typeof data22 == "number" && isFinite(data22)) && data22 !== null) {
        const err48 = { instancePath: instancePath + "/wind_direction_degrees", schemaPath: "#/properties/wind_direction_degrees/type", keyword: "type", params: { type: schema22.properties.wind_direction_degrees.type }, message: "must be number,null" };
        if (vErrors === null) {
          vErrors = [err48];
        } else {
          vErrors.push(err48);
        }
        errors++;
      }
      if (typeof data22 == "number" && isFinite(data22)) {
        if (!formats0.validate(data22)) {
          const err49 = { instancePath: instancePath + "/wind_direction_degrees", schemaPath: "#/properties/wind_direction_degrees/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err49];
          } else {
            vErrors.push(err49);
          }
          errors++;
        }
      }
    }
    if (data.wind_label !== void 0) {
      if (typeof data.wind_label !== "string") {
        const err50 = { instancePath: instancePath + "/wind_label", schemaPath: "#/properties/wind_label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err50];
        } else {
          vErrors.push(err50);
        }
        errors++;
      }
    }
  } else {
    const err51 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err51];
    } else {
      vErrors.push(err51);
    }
    errors++;
  }
  validate56.errors = vErrors;
  return errors === 0;
}
validate56.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var schema29 = { "additionalProperties": false, "description": "Responsive shell selected using the Web baseline.", "properties": { "choice_columns": { "description": "Responsive choice grid count.", "format": "uint32", "minimum": 0, "type": "integer" }, "clip_x": { "description": "Horizontal overflow is clipped.", "type": "boolean" }, "compact": { "description": "Small-screen branch selected below the Web 600px breakpoint.", "type": "boolean" }, "gap_px": { "description": "Shared control gap.", "format": "double", "type": "number" }, "maximum_height_px": { "description": "Maximum reachable scroll viewport height.", "format": "double", "type": "number" }, "padding_px": { "description": "Shared shell padding.", "format": "double", "type": "number" }, "passthrough": { "description": "Input-transparent forward view outside explicit controls.", "type": "boolean" }, "scroll_y": { "description": "Vertical scrolling is admitted for the shell.", "type": "boolean" }, "shell": { "$ref": "#/$defs/GameUiShell", "description": "Shared shell placement." }, "width_px": { "description": "Shell width in logical pixels.", "format": "double", "type": "number" } }, "required": ["shell", "width_px", "maximum_height_px", "padding_px", "gap_px", "choice_columns", "compact", "scroll_y", "clip_x", "passthrough"], "type": "object" };
function validate71(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate71.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("center" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "center" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("flight_bar" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "flight_bar" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("pause_settings" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "pause_settings" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("analysis" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "analysis" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("cinematic" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "cinematic" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate71.errors = vErrors;
  return errors === 0;
}
validate71.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate70(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate70.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.shell === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "shell" }, message: "must have required property 'shell'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.width_px === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "width_px" }, message: "must have required property 'width_px'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.maximum_height_px === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "maximum_height_px" }, message: "must have required property 'maximum_height_px'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.padding_px === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "padding_px" }, message: "must have required property 'padding_px'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.gap_px === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "gap_px" }, message: "must have required property 'gap_px'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.choice_columns === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "choice_columns" }, message: "must have required property 'choice_columns'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.compact === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "compact" }, message: "must have required property 'compact'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.scroll_y === void 0) {
      const err7 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "scroll_y" }, message: "must have required property 'scroll_y'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.clip_x === void 0) {
      const err8 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "clip_x" }, message: "must have required property 'clip_x'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.passthrough === void 0) {
      const err9 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "passthrough" }, message: "must have required property 'passthrough'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!func1.call(schema29.properties, key0)) {
        const err10 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
    if (data.choice_columns !== void 0) {
      let data0 = data.choice_columns;
      if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0))) {
        const err11 = { instancePath: instancePath + "/choice_columns", schemaPath: "#/properties/choice_columns/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
      if (typeof data0 == "number" && isFinite(data0)) {
        if (data0 < 0 || isNaN(data0)) {
          const err12 = { instancePath: instancePath + "/choice_columns", schemaPath: "#/properties/choice_columns/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
          if (vErrors === null) {
            vErrors = [err12];
          } else {
            vErrors.push(err12);
          }
          errors++;
        }
        if (!formats28.validate(data0)) {
          const err13 = { instancePath: instancePath + "/choice_columns", schemaPath: "#/properties/choice_columns/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
          if (vErrors === null) {
            vErrors = [err13];
          } else {
            vErrors.push(err13);
          }
          errors++;
        }
      }
    }
    if (data.clip_x !== void 0) {
      if (typeof data.clip_x !== "boolean") {
        const err14 = { instancePath: instancePath + "/clip_x", schemaPath: "#/properties/clip_x/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err14];
        } else {
          vErrors.push(err14);
        }
        errors++;
      }
    }
    if (data.compact !== void 0) {
      if (typeof data.compact !== "boolean") {
        const err15 = { instancePath: instancePath + "/compact", schemaPath: "#/properties/compact/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
    }
    if (data.gap_px !== void 0) {
      let data3 = data.gap_px;
      if (typeof data3 == "number" && isFinite(data3)) {
        if (!formats0.validate(data3)) {
          const err16 = { instancePath: instancePath + "/gap_px", schemaPath: "#/properties/gap_px/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err16];
          } else {
            vErrors.push(err16);
          }
          errors++;
        }
      } else {
        const err17 = { instancePath: instancePath + "/gap_px", schemaPath: "#/properties/gap_px/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err17];
        } else {
          vErrors.push(err17);
        }
        errors++;
      }
    }
    if (data.maximum_height_px !== void 0) {
      let data4 = data.maximum_height_px;
      if (typeof data4 == "number" && isFinite(data4)) {
        if (!formats0.validate(data4)) {
          const err18 = { instancePath: instancePath + "/maximum_height_px", schemaPath: "#/properties/maximum_height_px/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err18];
          } else {
            vErrors.push(err18);
          }
          errors++;
        }
      } else {
        const err19 = { instancePath: instancePath + "/maximum_height_px", schemaPath: "#/properties/maximum_height_px/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
    }
    if (data.padding_px !== void 0) {
      let data5 = data.padding_px;
      if (typeof data5 == "number" && isFinite(data5)) {
        if (!formats0.validate(data5)) {
          const err20 = { instancePath: instancePath + "/padding_px", schemaPath: "#/properties/padding_px/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err20];
          } else {
            vErrors.push(err20);
          }
          errors++;
        }
      } else {
        const err21 = { instancePath: instancePath + "/padding_px", schemaPath: "#/properties/padding_px/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err21];
        } else {
          vErrors.push(err21);
        }
        errors++;
      }
    }
    if (data.passthrough !== void 0) {
      if (typeof data.passthrough !== "boolean") {
        const err22 = { instancePath: instancePath + "/passthrough", schemaPath: "#/properties/passthrough/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err22];
        } else {
          vErrors.push(err22);
        }
        errors++;
      }
    }
    if (data.scroll_y !== void 0) {
      if (typeof data.scroll_y !== "boolean") {
        const err23 = { instancePath: instancePath + "/scroll_y", schemaPath: "#/properties/scroll_y/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err23];
        } else {
          vErrors.push(err23);
        }
        errors++;
      }
    }
    if (data.shell !== void 0) {
      if (!validate71(data.shell, { instancePath: instancePath + "/shell", parentData: data, parentDataProperty: "shell", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate71.errors : vErrors.concat(validate71.errors);
        errors = vErrors.length;
      }
    }
    if (data.width_px !== void 0) {
      let data9 = data.width_px;
      if (typeof data9 == "number" && isFinite(data9)) {
        if (!formats0.validate(data9)) {
          const err24 = { instancePath: instancePath + "/width_px", schemaPath: "#/properties/width_px/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err24];
          } else {
            vErrors.push(err24);
          }
          errors++;
        }
      } else {
        const err25 = { instancePath: instancePath + "/width_px", schemaPath: "#/properties/width_px/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err25];
        } else {
          vErrors.push(err25);
        }
        errors++;
      }
    }
  } else {
    const err26 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err26];
    } else {
      vErrors.push(err26);
    }
    errors++;
  }
  validate70.errors = vErrors;
  return errors === 0;
}
validate70.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate75(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate75.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("menu" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "menu" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("head" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "head" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("cockpit" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "cockpit" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("world" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "world" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
      }
    }
  }
  if (!valid0) {
    const err8 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate75.errors = vErrors;
  return errors === 0;
}
validate75.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate81(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate81.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("beginner" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "beginner" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("standard" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "standard" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("expert" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "expert" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("realistic" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "realistic" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("custom" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "custom" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate81.errors = vErrors;
  return errors === 0;
}
validate81.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate83(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate83.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("full" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "full" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("standard" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "standard" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("minimal" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "minimal" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("realistic" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "realistic" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("custom" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "custom" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate83.errors = vErrors;
  return errors === 0;
}
validate83.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate85(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate85.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("strong" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "strong" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("assisted" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "assisted" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("light" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "light" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("manual" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "manual" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
      }
    }
  }
  if (!valid0) {
    const err8 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate85.errors = vErrors;
  return errors === 0;
}
validate85.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate87(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate87.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("calm" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "calm" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("mild" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "mild" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("typical" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "typical" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("challenging" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "challenging" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("near_limit" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "near_limit" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate87.errors = vErrors;
  return errors === 0;
}
validate87.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate89(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate89.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("telemetry" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "telemetry" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("attitude" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "attitude" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("wind" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "wind" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("flight_path" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "flight_path" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("angle_of_attack" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "angle_of_attack" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
          const _errs11 = errors;
          if (typeof data !== "string") {
            const err10 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err10];
            } else {
              vErrors.push(err10);
            }
            errors++;
          }
          if ("warnings" !== data) {
            const err11 = { instancePath, schemaPath: "#/oneOf/5/const", keyword: "const", params: { allowedValue: "warnings" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err11];
            } else {
              vErrors.push(err11);
            }
            errors++;
          }
          var _valid0 = _errs11 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err12 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err12];
    } else {
      vErrors.push(err12);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate89.errors = vErrors;
  return errors === 0;
}
validate89.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate80(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate80.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "kind")) {
        const err1 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err1];
        } else {
          vErrors.push(err1);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data0 = data.kind;
      if (typeof data0 !== "string") {
        const err2 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
      if ("open_setup" !== data0) {
        const err3 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "open_setup" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
  } else {
    const err4 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err4];
    } else {
      vErrors.push(err4);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs6 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err5 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "kind")) {
        const err6 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data1 = data.kind;
      if (typeof data1 !== "string") {
        const err7 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      if ("return_to_title" !== data1) {
        const err8 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "return_to_title" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
  } else {
    const err9 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err9];
    } else {
      vErrors.push(err9);
    }
    errors++;
  }
  var _valid0 = _errs6 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs11 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err10 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "kind")) {
          const err11 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err11];
          } else {
            vErrors.push(err11);
          }
          errors++;
        }
      }
      if (data.kind !== void 0) {
        let data2 = data.kind;
        if (typeof data2 !== "string") {
          const err12 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err12];
          } else {
            vErrors.push(err12);
          }
          errors++;
        }
        if ("prepare" !== data2) {
          const err13 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "prepare" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err13];
          } else {
            vErrors.push(err13);
          }
          errors++;
        }
      }
    } else {
      const err14 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err14];
      } else {
        vErrors.push(err14);
      }
      errors++;
    }
    var _valid0 = _errs11 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
      const _errs16 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.kind === void 0) {
          const err15 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
          if (vErrors === null) {
            vErrors = [err15];
          } else {
            vErrors.push(err15);
          }
          errors++;
        }
        for (const key3 in data) {
          if (!(key3 === "kind")) {
            const err16 = { instancePath, schemaPath: "#/oneOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err16];
            } else {
              vErrors.push(err16);
            }
            errors++;
          }
        }
        if (data.kind !== void 0) {
          let data3 = data.kind;
          if (typeof data3 !== "string") {
            const err17 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err17];
            } else {
              vErrors.push(err17);
            }
            errors++;
          }
          if ("cancel_briefing" !== data3) {
            const err18 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/const", keyword: "const", params: { allowedValue: "cancel_briefing" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err18];
            } else {
              vErrors.push(err18);
            }
            errors++;
          }
        }
      } else {
        const err19 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
      var _valid0 = _errs16 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
        const _errs21 = errors;
        if (data && typeof data == "object" && !Array.isArray(data)) {
          if (data.kind === void 0) {
            const err20 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
            if (vErrors === null) {
              vErrors = [err20];
            } else {
              vErrors.push(err20);
            }
            errors++;
          }
          for (const key4 in data) {
            if (!(key4 === "kind")) {
              const err21 = { instancePath, schemaPath: "#/oneOf/4/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key4 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err21];
              } else {
                vErrors.push(err21);
              }
              errors++;
            }
          }
          if (data.kind !== void 0) {
            let data4 = data.kind;
            if (typeof data4 !== "string") {
              const err22 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err22];
              } else {
                vErrors.push(err22);
              }
              errors++;
            }
            if ("start_countdown" !== data4) {
              const err23 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/const", keyword: "const", params: { allowedValue: "start_countdown" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err23];
              } else {
                vErrors.push(err23);
              }
              errors++;
            }
          }
        } else {
          const err24 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "object" }, message: "must be object" };
          if (vErrors === null) {
            vErrors = [err24];
          } else {
            vErrors.push(err24);
          }
          errors++;
        }
        var _valid0 = _errs21 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props0 !== true) {
              props0 = true;
            }
          }
          const _errs26 = errors;
          if (data && typeof data == "object" && !Array.isArray(data)) {
            if (data.kind === void 0) {
              const err25 = { instancePath, schemaPath: "#/oneOf/5/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
              if (vErrors === null) {
                vErrors = [err25];
              } else {
                vErrors.push(err25);
              }
              errors++;
            }
            for (const key5 in data) {
              if (!(key5 === "kind")) {
                const err26 = { instancePath, schemaPath: "#/oneOf/5/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key5 }, message: "must NOT have additional properties" };
                if (vErrors === null) {
                  vErrors = [err26];
                } else {
                  vErrors.push(err26);
                }
                errors++;
              }
            }
            if (data.kind !== void 0) {
              let data5 = data.kind;
              if (typeof data5 !== "string") {
                const err27 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err27];
                } else {
                  vErrors.push(err27);
                }
                errors++;
              }
              if ("cancel_countdown" !== data5) {
                const err28 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/const", keyword: "const", params: { allowedValue: "cancel_countdown" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err28];
                } else {
                  vErrors.push(err28);
                }
                errors++;
              }
            }
          } else {
            const err29 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "object" }, message: "must be object" };
            if (vErrors === null) {
              vErrors = [err29];
            } else {
              vErrors.push(err29);
            }
            errors++;
          }
          var _valid0 = _errs26 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true) {
                props0 = true;
              }
            }
            const _errs31 = errors;
            if (data && typeof data == "object" && !Array.isArray(data)) {
              if (data.kind === void 0) {
                const err30 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                if (vErrors === null) {
                  vErrors = [err30];
                } else {
                  vErrors.push(err30);
                }
                errors++;
              }
              for (const key6 in data) {
                if (!(key6 === "kind")) {
                  const err31 = { instancePath, schemaPath: "#/oneOf/6/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key6 }, message: "must NOT have additional properties" };
                  if (vErrors === null) {
                    vErrors = [err31];
                  } else {
                    vErrors.push(err31);
                  }
                  errors++;
                }
              }
              if (data.kind !== void 0) {
                let data6 = data.kind;
                if (typeof data6 !== "string") {
                  const err32 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err32];
                  } else {
                    vErrors.push(err32);
                  }
                  errors++;
                }
                if ("pause" !== data6) {
                  const err33 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/const", keyword: "const", params: { allowedValue: "pause" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err33];
                  } else {
                    vErrors.push(err33);
                  }
                  errors++;
                }
              }
            } else {
              const err34 = { instancePath, schemaPath: "#/oneOf/6/type", keyword: "type", params: { type: "object" }, message: "must be object" };
              if (vErrors === null) {
                vErrors = [err34];
              } else {
                vErrors.push(err34);
              }
              errors++;
            }
            var _valid0 = _errs31 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props0 !== true) {
                  props0 = true;
                }
              }
              const _errs36 = errors;
              if (data && typeof data == "object" && !Array.isArray(data)) {
                if (data.kind === void 0) {
                  const err35 = { instancePath, schemaPath: "#/oneOf/7/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                  if (vErrors === null) {
                    vErrors = [err35];
                  } else {
                    vErrors.push(err35);
                  }
                  errors++;
                }
                for (const key7 in data) {
                  if (!(key7 === "kind")) {
                    const err36 = { instancePath, schemaPath: "#/oneOf/7/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key7 }, message: "must NOT have additional properties" };
                    if (vErrors === null) {
                      vErrors = [err36];
                    } else {
                      vErrors.push(err36);
                    }
                    errors++;
                  }
                }
                if (data.kind !== void 0) {
                  let data7 = data.kind;
                  if (typeof data7 !== "string") {
                    const err37 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err37];
                    } else {
                      vErrors.push(err37);
                    }
                    errors++;
                  }
                  if ("resume" !== data7) {
                    const err38 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/const", keyword: "const", params: { allowedValue: "resume" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err38];
                    } else {
                      vErrors.push(err38);
                    }
                    errors++;
                  }
                }
              } else {
                const err39 = { instancePath, schemaPath: "#/oneOf/7/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                if (vErrors === null) {
                  vErrors = [err39];
                } else {
                  vErrors.push(err39);
                }
                errors++;
              }
              var _valid0 = _errs36 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props0 !== true) {
                    props0 = true;
                  }
                }
                const _errs41 = errors;
                if (data && typeof data == "object" && !Array.isArray(data)) {
                  if (data.kind === void 0) {
                    const err40 = { instancePath, schemaPath: "#/oneOf/8/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                    if (vErrors === null) {
                      vErrors = [err40];
                    } else {
                      vErrors.push(err40);
                    }
                    errors++;
                  }
                  for (const key8 in data) {
                    if (!(key8 === "kind")) {
                      const err41 = { instancePath, schemaPath: "#/oneOf/8/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key8 }, message: "must NOT have additional properties" };
                      if (vErrors === null) {
                        vErrors = [err41];
                      } else {
                        vErrors.push(err41);
                      }
                      errors++;
                    }
                  }
                  if (data.kind !== void 0) {
                    let data8 = data.kind;
                    if (typeof data8 !== "string") {
                      const err42 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/8/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err42];
                      } else {
                        vErrors.push(err42);
                      }
                      errors++;
                    }
                    if ("abort" !== data8) {
                      const err43 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/8/properties/kind/const", keyword: "const", params: { allowedValue: "abort" }, message: "must be equal to constant" };
                      if (vErrors === null) {
                        vErrors = [err43];
                      } else {
                        vErrors.push(err43);
                      }
                      errors++;
                    }
                  }
                } else {
                  const err44 = { instancePath, schemaPath: "#/oneOf/8/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                  if (vErrors === null) {
                    vErrors = [err44];
                  } else {
                    vErrors.push(err44);
                  }
                  errors++;
                }
                var _valid0 = _errs41 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                    if (props0 !== true) {
                      props0 = true;
                    }
                  }
                  const _errs46 = errors;
                  if (data && typeof data == "object" && !Array.isArray(data)) {
                    if (data.kind === void 0) {
                      const err45 = { instancePath, schemaPath: "#/oneOf/9/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                      if (vErrors === null) {
                        vErrors = [err45];
                      } else {
                        vErrors.push(err45);
                      }
                      errors++;
                    }
                    for (const key9 in data) {
                      if (!(key9 === "kind")) {
                        const err46 = { instancePath, schemaPath: "#/oneOf/9/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key9 }, message: "must NOT have additional properties" };
                        if (vErrors === null) {
                          vErrors = [err46];
                        } else {
                          vErrors.push(err46);
                        }
                        errors++;
                      }
                    }
                    if (data.kind !== void 0) {
                      let data9 = data.kind;
                      if (typeof data9 !== "string") {
                        const err47 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/9/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                        if (vErrors === null) {
                          vErrors = [err47];
                        } else {
                          vErrors.push(err47);
                        }
                        errors++;
                      }
                      if ("retry" !== data9) {
                        const err48 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/9/properties/kind/const", keyword: "const", params: { allowedValue: "retry" }, message: "must be equal to constant" };
                        if (vErrors === null) {
                          vErrors = [err48];
                        } else {
                          vErrors.push(err48);
                        }
                        errors++;
                      }
                    }
                  } else {
                    const err49 = { instancePath, schemaPath: "#/oneOf/9/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                    if (vErrors === null) {
                      vErrors = [err49];
                    } else {
                      vErrors.push(err49);
                    }
                    errors++;
                  }
                  var _valid0 = _errs46 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                      if (props0 !== true) {
                        props0 = true;
                      }
                    }
                    const _errs51 = errors;
                    if (data && typeof data == "object" && !Array.isArray(data)) {
                      if (data.kind === void 0) {
                        const err50 = { instancePath, schemaPath: "#/oneOf/10/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                        if (vErrors === null) {
                          vErrors = [err50];
                        } else {
                          vErrors.push(err50);
                        }
                        errors++;
                      }
                      for (const key10 in data) {
                        if (!(key10 === "kind")) {
                          const err51 = { instancePath, schemaPath: "#/oneOf/10/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key10 }, message: "must NOT have additional properties" };
                          if (vErrors === null) {
                            vErrors = [err51];
                          } else {
                            vErrors.push(err51);
                          }
                          errors++;
                        }
                      }
                      if (data.kind !== void 0) {
                        let data10 = data.kind;
                        if (typeof data10 !== "string") {
                          const err52 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/10/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                          if (vErrors === null) {
                            vErrors = [err52];
                          } else {
                            vErrors.push(err52);
                          }
                          errors++;
                        }
                        if ("retry_briefing" !== data10) {
                          const err53 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/10/properties/kind/const", keyword: "const", params: { allowedValue: "retry_briefing" }, message: "must be equal to constant" };
                          if (vErrors === null) {
                            vErrors = [err53];
                          } else {
                            vErrors.push(err53);
                          }
                          errors++;
                        }
                      }
                    } else {
                      const err54 = { instancePath, schemaPath: "#/oneOf/10/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                      if (vErrors === null) {
                        vErrors = [err54];
                      } else {
                        vErrors.push(err54);
                      }
                      errors++;
                    }
                    var _valid0 = _errs51 === errors;
                    if (_valid0 && valid0) {
                      valid0 = false;
                      passing0 = [passing0, 10];
                    } else {
                      if (_valid0) {
                        valid0 = true;
                        passing0 = 10;
                        if (props0 !== true) {
                          props0 = true;
                        }
                      }
                      const _errs56 = errors;
                      if (data && typeof data == "object" && !Array.isArray(data)) {
                        if (data.kind === void 0) {
                          const err55 = { instancePath, schemaPath: "#/oneOf/11/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                          if (vErrors === null) {
                            vErrors = [err55];
                          } else {
                            vErrors.push(err55);
                          }
                          errors++;
                        }
                        for (const key11 in data) {
                          if (!(key11 === "kind")) {
                            const err56 = { instancePath, schemaPath: "#/oneOf/11/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key11 }, message: "must NOT have additional properties" };
                            if (vErrors === null) {
                              vErrors = [err56];
                            } else {
                              vErrors.push(err56);
                            }
                            errors++;
                          }
                        }
                        if (data.kind !== void 0) {
                          let data11 = data.kind;
                          if (typeof data11 !== "string") {
                            const err57 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/11/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                            if (vErrors === null) {
                              vErrors = [err57];
                            } else {
                              vErrors.push(err57);
                            }
                            errors++;
                          }
                          if ("enter_replay" !== data11) {
                            const err58 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/11/properties/kind/const", keyword: "const", params: { allowedValue: "enter_replay" }, message: "must be equal to constant" };
                            if (vErrors === null) {
                              vErrors = [err58];
                            } else {
                              vErrors.push(err58);
                            }
                            errors++;
                          }
                        }
                      } else {
                        const err59 = { instancePath, schemaPath: "#/oneOf/11/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                        if (vErrors === null) {
                          vErrors = [err59];
                        } else {
                          vErrors.push(err59);
                        }
                        errors++;
                      }
                      var _valid0 = _errs56 === errors;
                      if (_valid0 && valid0) {
                        valid0 = false;
                        passing0 = [passing0, 11];
                      } else {
                        if (_valid0) {
                          valid0 = true;
                          passing0 = 11;
                          if (props0 !== true) {
                            props0 = true;
                          }
                        }
                        const _errs61 = errors;
                        if (data && typeof data == "object" && !Array.isArray(data)) {
                          if (data.kind === void 0) {
                            const err60 = { instancePath, schemaPath: "#/oneOf/12/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                            if (vErrors === null) {
                              vErrors = [err60];
                            } else {
                              vErrors.push(err60);
                            }
                            errors++;
                          }
                          for (const key12 in data) {
                            if (!(key12 === "kind")) {
                              const err61 = { instancePath, schemaPath: "#/oneOf/12/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key12 }, message: "must NOT have additional properties" };
                              if (vErrors === null) {
                                vErrors = [err61];
                              } else {
                                vErrors.push(err61);
                              }
                              errors++;
                            }
                          }
                          if (data.kind !== void 0) {
                            let data12 = data.kind;
                            if (typeof data12 !== "string") {
                              const err62 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/12/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                              if (vErrors === null) {
                                vErrors = [err62];
                              } else {
                                vErrors.push(err62);
                              }
                              errors++;
                            }
                            if ("leave_replay" !== data12) {
                              const err63 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/12/properties/kind/const", keyword: "const", params: { allowedValue: "leave_replay" }, message: "must be equal to constant" };
                              if (vErrors === null) {
                                vErrors = [err63];
                              } else {
                                vErrors.push(err63);
                              }
                              errors++;
                            }
                          }
                        } else {
                          const err64 = { instancePath, schemaPath: "#/oneOf/12/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                          if (vErrors === null) {
                            vErrors = [err64];
                          } else {
                            vErrors.push(err64);
                          }
                          errors++;
                        }
                        var _valid0 = _errs61 === errors;
                        if (_valid0 && valid0) {
                          valid0 = false;
                          passing0 = [passing0, 12];
                        } else {
                          if (_valid0) {
                            valid0 = true;
                            passing0 = 12;
                            if (props0 !== true) {
                              props0 = true;
                            }
                          }
                          const _errs66 = errors;
                          if (data && typeof data == "object" && !Array.isArray(data)) {
                            if (data.kind === void 0) {
                              const err65 = { instancePath, schemaPath: "#/oneOf/13/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                              if (vErrors === null) {
                                vErrors = [err65];
                              } else {
                                vErrors.push(err65);
                              }
                              errors++;
                            }
                            for (const key13 in data) {
                              if (!(key13 === "kind")) {
                                const err66 = { instancePath, schemaPath: "#/oneOf/13/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key13 }, message: "must NOT have additional properties" };
                                if (vErrors === null) {
                                  vErrors = [err66];
                                } else {
                                  vErrors.push(err66);
                                }
                                errors++;
                              }
                            }
                            if (data.kind !== void 0) {
                              let data13 = data.kind;
                              if (typeof data13 !== "string") {
                                const err67 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/13/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                if (vErrors === null) {
                                  vErrors = [err67];
                                } else {
                                  vErrors.push(err67);
                                }
                                errors++;
                              }
                              if ("enter_attract" !== data13) {
                                const err68 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/13/properties/kind/const", keyword: "const", params: { allowedValue: "enter_attract" }, message: "must be equal to constant" };
                                if (vErrors === null) {
                                  vErrors = [err68];
                                } else {
                                  vErrors.push(err68);
                                }
                                errors++;
                              }
                            }
                          } else {
                            const err69 = { instancePath, schemaPath: "#/oneOf/13/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                            if (vErrors === null) {
                              vErrors = [err69];
                            } else {
                              vErrors.push(err69);
                            }
                            errors++;
                          }
                          var _valid0 = _errs66 === errors;
                          if (_valid0 && valid0) {
                            valid0 = false;
                            passing0 = [passing0, 13];
                          } else {
                            if (_valid0) {
                              valid0 = true;
                              passing0 = 13;
                              if (props0 !== true) {
                                props0 = true;
                              }
                            }
                            const _errs71 = errors;
                            if (data && typeof data == "object" && !Array.isArray(data)) {
                              if (data.kind === void 0) {
                                const err70 = { instancePath, schemaPath: "#/oneOf/14/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                if (vErrors === null) {
                                  vErrors = [err70];
                                } else {
                                  vErrors.push(err70);
                                }
                                errors++;
                              }
                              for (const key14 in data) {
                                if (!(key14 === "kind")) {
                                  const err71 = { instancePath, schemaPath: "#/oneOf/14/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key14 }, message: "must NOT have additional properties" };
                                  if (vErrors === null) {
                                    vErrors = [err71];
                                  } else {
                                    vErrors.push(err71);
                                  }
                                  errors++;
                                }
                              }
                              if (data.kind !== void 0) {
                                let data14 = data.kind;
                                if (typeof data14 !== "string") {
                                  const err72 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/14/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                  if (vErrors === null) {
                                    vErrors = [err72];
                                  } else {
                                    vErrors.push(err72);
                                  }
                                  errors++;
                                }
                                if ("leave_attract" !== data14) {
                                  const err73 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/14/properties/kind/const", keyword: "const", params: { allowedValue: "leave_attract" }, message: "must be equal to constant" };
                                  if (vErrors === null) {
                                    vErrors = [err73];
                                  } else {
                                    vErrors.push(err73);
                                  }
                                  errors++;
                                }
                              }
                            } else {
                              const err74 = { instancePath, schemaPath: "#/oneOf/14/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                              if (vErrors === null) {
                                vErrors = [err74];
                              } else {
                                vErrors.push(err74);
                              }
                              errors++;
                            }
                            var _valid0 = _errs71 === errors;
                            if (_valid0 && valid0) {
                              valid0 = false;
                              passing0 = [passing0, 14];
                            } else {
                              if (_valid0) {
                                valid0 = true;
                                passing0 = 14;
                                if (props0 !== true) {
                                  props0 = true;
                                }
                              }
                              const _errs76 = errors;
                              if (data && typeof data == "object" && !Array.isArray(data)) {
                                if (data.kind === void 0) {
                                  const err75 = { instancePath, schemaPath: "#/oneOf/15/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                  if (vErrors === null) {
                                    vErrors = [err75];
                                  } else {
                                    vErrors.push(err75);
                                  }
                                  errors++;
                                }
                                if (data.preset === void 0) {
                                  const err76 = { instancePath, schemaPath: "#/oneOf/15/required", keyword: "required", params: { missingProperty: "preset" }, message: "must have required property 'preset'" };
                                  if (vErrors === null) {
                                    vErrors = [err76];
                                  } else {
                                    vErrors.push(err76);
                                  }
                                  errors++;
                                }
                                for (const key15 in data) {
                                  if (!(key15 === "kind" || key15 === "preset")) {
                                    const err77 = { instancePath, schemaPath: "#/oneOf/15/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key15 }, message: "must NOT have additional properties" };
                                    if (vErrors === null) {
                                      vErrors = [err77];
                                    } else {
                                      vErrors.push(err77);
                                    }
                                    errors++;
                                  }
                                }
                                if (data.kind !== void 0) {
                                  let data15 = data.kind;
                                  if (typeof data15 !== "string") {
                                    const err78 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/15/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                    if (vErrors === null) {
                                      vErrors = [err78];
                                    } else {
                                      vErrors.push(err78);
                                    }
                                    errors++;
                                  }
                                  if ("select_preset" !== data15) {
                                    const err79 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/15/properties/kind/const", keyword: "const", params: { allowedValue: "select_preset" }, message: "must be equal to constant" };
                                    if (vErrors === null) {
                                      vErrors = [err79];
                                    } else {
                                      vErrors.push(err79);
                                    }
                                    errors++;
                                  }
                                }
                                if (data.preset !== void 0) {
                                  if (!validate81(data.preset, { instancePath: instancePath + "/preset", parentData: data, parentDataProperty: "preset", rootData, dynamicAnchors })) {
                                    vErrors = vErrors === null ? validate81.errors : vErrors.concat(validate81.errors);
                                    errors = vErrors.length;
                                  }
                                }
                              } else {
                                const err80 = { instancePath, schemaPath: "#/oneOf/15/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                if (vErrors === null) {
                                  vErrors = [err80];
                                } else {
                                  vErrors.push(err80);
                                }
                                errors++;
                              }
                              var _valid0 = _errs76 === errors;
                              if (_valid0 && valid0) {
                                valid0 = false;
                                passing0 = [passing0, 15];
                              } else {
                                if (_valid0) {
                                  valid0 = true;
                                  passing0 = 15;
                                  if (props0 !== true) {
                                    props0 = true;
                                  }
                                }
                                const _errs82 = errors;
                                if (data && typeof data == "object" && !Array.isArray(data)) {
                                  if (data.kind === void 0) {
                                    const err81 = { instancePath, schemaPath: "#/oneOf/16/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                    if (vErrors === null) {
                                      vErrors = [err81];
                                    } else {
                                      vErrors.push(err81);
                                    }
                                    errors++;
                                  }
                                  if (data.information === void 0) {
                                    const err82 = { instancePath, schemaPath: "#/oneOf/16/required", keyword: "required", params: { missingProperty: "information" }, message: "must have required property 'information'" };
                                    if (vErrors === null) {
                                      vErrors = [err82];
                                    } else {
                                      vErrors.push(err82);
                                    }
                                    errors++;
                                  }
                                  for (const key16 in data) {
                                    if (!(key16 === "information" || key16 === "kind")) {
                                      const err83 = { instancePath, schemaPath: "#/oneOf/16/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key16 }, message: "must NOT have additional properties" };
                                      if (vErrors === null) {
                                        vErrors = [err83];
                                      } else {
                                        vErrors.push(err83);
                                      }
                                      errors++;
                                    }
                                  }
                                  if (data.information !== void 0) {
                                    if (!validate83(data.information, { instancePath: instancePath + "/information", parentData: data, parentDataProperty: "information", rootData, dynamicAnchors })) {
                                      vErrors = vErrors === null ? validate83.errors : vErrors.concat(validate83.errors);
                                      errors = vErrors.length;
                                    }
                                  }
                                  if (data.kind !== void 0) {
                                    let data18 = data.kind;
                                    if (typeof data18 !== "string") {
                                      const err84 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/16/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                      if (vErrors === null) {
                                        vErrors = [err84];
                                      } else {
                                        vErrors.push(err84);
                                      }
                                      errors++;
                                    }
                                    if ("select_information" !== data18) {
                                      const err85 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/16/properties/kind/const", keyword: "const", params: { allowedValue: "select_information" }, message: "must be equal to constant" };
                                      if (vErrors === null) {
                                        vErrors = [err85];
                                      } else {
                                        vErrors.push(err85);
                                      }
                                      errors++;
                                    }
                                  }
                                } else {
                                  const err86 = { instancePath, schemaPath: "#/oneOf/16/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                  if (vErrors === null) {
                                    vErrors = [err86];
                                  } else {
                                    vErrors.push(err86);
                                  }
                                  errors++;
                                }
                                var _valid0 = _errs82 === errors;
                                if (_valid0 && valid0) {
                                  valid0 = false;
                                  passing0 = [passing0, 16];
                                } else {
                                  if (_valid0) {
                                    valid0 = true;
                                    passing0 = 16;
                                    if (props0 !== true) {
                                      props0 = true;
                                    }
                                  }
                                  const _errs88 = errors;
                                  if (data && typeof data == "object" && !Array.isArray(data)) {
                                    if (data.kind === void 0) {
                                      const err87 = { instancePath, schemaPath: "#/oneOf/17/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                      if (vErrors === null) {
                                        vErrors = [err87];
                                      } else {
                                        vErrors.push(err87);
                                      }
                                      errors++;
                                    }
                                    if (data.assistance === void 0) {
                                      const err88 = { instancePath, schemaPath: "#/oneOf/17/required", keyword: "required", params: { missingProperty: "assistance" }, message: "must have required property 'assistance'" };
                                      if (vErrors === null) {
                                        vErrors = [err88];
                                      } else {
                                        vErrors.push(err88);
                                      }
                                      errors++;
                                    }
                                    for (const key17 in data) {
                                      if (!(key17 === "assistance" || key17 === "kind")) {
                                        const err89 = { instancePath, schemaPath: "#/oneOf/17/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key17 }, message: "must NOT have additional properties" };
                                        if (vErrors === null) {
                                          vErrors = [err89];
                                        } else {
                                          vErrors.push(err89);
                                        }
                                        errors++;
                                      }
                                    }
                                    if (data.assistance !== void 0) {
                                      if (!validate85(data.assistance, { instancePath: instancePath + "/assistance", parentData: data, parentDataProperty: "assistance", rootData, dynamicAnchors })) {
                                        vErrors = vErrors === null ? validate85.errors : vErrors.concat(validate85.errors);
                                        errors = vErrors.length;
                                      }
                                    }
                                    if (data.kind !== void 0) {
                                      let data20 = data.kind;
                                      if (typeof data20 !== "string") {
                                        const err90 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/17/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                        if (vErrors === null) {
                                          vErrors = [err90];
                                        } else {
                                          vErrors.push(err90);
                                        }
                                        errors++;
                                      }
                                      if ("select_assistance" !== data20) {
                                        const err91 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/17/properties/kind/const", keyword: "const", params: { allowedValue: "select_assistance" }, message: "must be equal to constant" };
                                        if (vErrors === null) {
                                          vErrors = [err91];
                                        } else {
                                          vErrors.push(err91);
                                        }
                                        errors++;
                                      }
                                    }
                                  } else {
                                    const err92 = { instancePath, schemaPath: "#/oneOf/17/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                    if (vErrors === null) {
                                      vErrors = [err92];
                                    } else {
                                      vErrors.push(err92);
                                    }
                                    errors++;
                                  }
                                  var _valid0 = _errs88 === errors;
                                  if (_valid0 && valid0) {
                                    valid0 = false;
                                    passing0 = [passing0, 17];
                                  } else {
                                    if (_valid0) {
                                      valid0 = true;
                                      passing0 = 17;
                                      if (props0 !== true) {
                                        props0 = true;
                                      }
                                    }
                                    const _errs94 = errors;
                                    if (data && typeof data == "object" && !Array.isArray(data)) {
                                      if (data.kind === void 0) {
                                        const err93 = { instancePath, schemaPath: "#/oneOf/18/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                        if (vErrors === null) {
                                          vErrors = [err93];
                                        } else {
                                          vErrors.push(err93);
                                        }
                                        errors++;
                                      }
                                      if (data.weather === void 0) {
                                        const err94 = { instancePath, schemaPath: "#/oneOf/18/required", keyword: "required", params: { missingProperty: "weather" }, message: "must have required property 'weather'" };
                                        if (vErrors === null) {
                                          vErrors = [err94];
                                        } else {
                                          vErrors.push(err94);
                                        }
                                        errors++;
                                      }
                                      for (const key18 in data) {
                                        if (!(key18 === "kind" || key18 === "weather")) {
                                          const err95 = { instancePath, schemaPath: "#/oneOf/18/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key18 }, message: "must NOT have additional properties" };
                                          if (vErrors === null) {
                                            vErrors = [err95];
                                          } else {
                                            vErrors.push(err95);
                                          }
                                          errors++;
                                        }
                                      }
                                      if (data.kind !== void 0) {
                                        let data21 = data.kind;
                                        if (typeof data21 !== "string") {
                                          const err96 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/18/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                          if (vErrors === null) {
                                            vErrors = [err96];
                                          } else {
                                            vErrors.push(err96);
                                          }
                                          errors++;
                                        }
                                        if ("select_weather" !== data21) {
                                          const err97 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/18/properties/kind/const", keyword: "const", params: { allowedValue: "select_weather" }, message: "must be equal to constant" };
                                          if (vErrors === null) {
                                            vErrors = [err97];
                                          } else {
                                            vErrors.push(err97);
                                          }
                                          errors++;
                                        }
                                      }
                                      if (data.weather !== void 0) {
                                        if (!validate87(data.weather, { instancePath: instancePath + "/weather", parentData: data, parentDataProperty: "weather", rootData, dynamicAnchors })) {
                                          vErrors = vErrors === null ? validate87.errors : vErrors.concat(validate87.errors);
                                          errors = vErrors.length;
                                        }
                                      }
                                    } else {
                                      const err98 = { instancePath, schemaPath: "#/oneOf/18/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                      if (vErrors === null) {
                                        vErrors = [err98];
                                      } else {
                                        vErrors.push(err98);
                                      }
                                      errors++;
                                    }
                                    var _valid0 = _errs94 === errors;
                                    if (_valid0 && valid0) {
                                      valid0 = false;
                                      passing0 = [passing0, 18];
                                    } else {
                                      if (_valid0) {
                                        valid0 = true;
                                        passing0 = 18;
                                        if (props0 !== true) {
                                          props0 = true;
                                        }
                                      }
                                      const _errs100 = errors;
                                      if (data && typeof data == "object" && !Array.isArray(data)) {
                                        if (data.kind === void 0) {
                                          const err99 = { instancePath, schemaPath: "#/oneOf/19/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                          if (vErrors === null) {
                                            vErrors = [err99];
                                          } else {
                                            vErrors.push(err99);
                                          }
                                          errors++;
                                        }
                                        if (data.cue === void 0) {
                                          const err100 = { instancePath, schemaPath: "#/oneOf/19/required", keyword: "required", params: { missingProperty: "cue" }, message: "must have required property 'cue'" };
                                          if (vErrors === null) {
                                            vErrors = [err100];
                                          } else {
                                            vErrors.push(err100);
                                          }
                                          errors++;
                                        }
                                        if (data.visible === void 0) {
                                          const err101 = { instancePath, schemaPath: "#/oneOf/19/required", keyword: "required", params: { missingProperty: "visible" }, message: "must have required property 'visible'" };
                                          if (vErrors === null) {
                                            vErrors = [err101];
                                          } else {
                                            vErrors.push(err101);
                                          }
                                          errors++;
                                        }
                                        for (const key19 in data) {
                                          if (!(key19 === "cue" || key19 === "kind" || key19 === "visible")) {
                                            const err102 = { instancePath, schemaPath: "#/oneOf/19/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key19 }, message: "must NOT have additional properties" };
                                            if (vErrors === null) {
                                              vErrors = [err102];
                                            } else {
                                              vErrors.push(err102);
                                            }
                                            errors++;
                                          }
                                        }
                                        if (data.cue !== void 0) {
                                          if (!validate89(data.cue, { instancePath: instancePath + "/cue", parentData: data, parentDataProperty: "cue", rootData, dynamicAnchors })) {
                                            vErrors = vErrors === null ? validate89.errors : vErrors.concat(validate89.errors);
                                            errors = vErrors.length;
                                          }
                                        }
                                        if (data.kind !== void 0) {
                                          let data24 = data.kind;
                                          if (typeof data24 !== "string") {
                                            const err103 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/19/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                            if (vErrors === null) {
                                              vErrors = [err103];
                                            } else {
                                              vErrors.push(err103);
                                            }
                                            errors++;
                                          }
                                          if ("set_cue" !== data24) {
                                            const err104 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/19/properties/kind/const", keyword: "const", params: { allowedValue: "set_cue" }, message: "must be equal to constant" };
                                            if (vErrors === null) {
                                              vErrors = [err104];
                                            } else {
                                              vErrors.push(err104);
                                            }
                                            errors++;
                                          }
                                        }
                                        if (data.visible !== void 0) {
                                          if (typeof data.visible !== "boolean") {
                                            const err105 = { instancePath: instancePath + "/visible", schemaPath: "#/oneOf/19/properties/visible/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                                            if (vErrors === null) {
                                              vErrors = [err105];
                                            } else {
                                              vErrors.push(err105);
                                            }
                                            errors++;
                                          }
                                        }
                                      } else {
                                        const err106 = { instancePath, schemaPath: "#/oneOf/19/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                        if (vErrors === null) {
                                          vErrors = [err106];
                                        } else {
                                          vErrors.push(err106);
                                        }
                                        errors++;
                                      }
                                      var _valid0 = _errs100 === errors;
                                      if (_valid0 && valid0) {
                                        valid0 = false;
                                        passing0 = [passing0, 19];
                                      } else {
                                        if (_valid0) {
                                          valid0 = true;
                                          passing0 = 19;
                                          if (props0 !== true) {
                                            props0 = true;
                                          }
                                        }
                                      }
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err107 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err107];
    } else {
      vErrors.push(err107);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate80.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate80.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate93(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate93.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("low" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "low" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("medium" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "medium" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("high" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "high" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate93.errors = vErrors;
  return errors === 0;
}
validate93.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate95(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate95.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("screen" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "screen" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("webxr" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "webxr" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("phone_vr" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "phone_vr" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate95.errors = vErrors;
  return errors === 0;
}
validate95.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate97(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate97.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("auto" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "auto" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("pilot" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "pilot" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("chase" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "chase" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("orbit" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "orbit" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("platform" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "platform" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
          const _errs11 = errors;
          if (typeof data !== "string") {
            const err10 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err10];
            } else {
              vErrors.push(err10);
            }
            errors++;
          }
          if ("shore" !== data) {
            const err11 = { instancePath, schemaPath: "#/oneOf/5/const", keyword: "const", params: { allowedValue: "shore" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err11];
            } else {
              vErrors.push(err11);
            }
            errors++;
          }
          var _valid0 = _errs11 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
            }
            const _errs13 = errors;
            if (typeof data !== "string") {
              const err12 = { instancePath, schemaPath: "#/oneOf/6/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err12];
              } else {
                vErrors.push(err12);
              }
              errors++;
            }
            if ("overhead" !== data) {
              const err13 = { instancePath, schemaPath: "#/oneOf/6/const", keyword: "const", params: { allowedValue: "overhead" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err13];
              } else {
                vErrors.push(err13);
              }
              errors++;
            }
            var _valid0 = _errs13 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
              }
              const _errs15 = errors;
              if (typeof data !== "string") {
                const err14 = { instancePath, schemaPath: "#/oneOf/7/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err14];
                } else {
                  vErrors.push(err14);
                }
                errors++;
              }
              if ("side" !== data) {
                const err15 = { instancePath, schemaPath: "#/oneOf/7/const", keyword: "const", params: { allowedValue: "side" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err15];
                } else {
                  vErrors.push(err15);
                }
                errors++;
              }
              var _valid0 = _errs15 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                }
                const _errs17 = errors;
                if (typeof data !== "string") {
                  const err16 = { instancePath, schemaPath: "#/oneOf/8/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err16];
                  } else {
                    vErrors.push(err16);
                  }
                  errors++;
                }
                if ("front" !== data) {
                  const err17 = { instancePath, schemaPath: "#/oneOf/8/const", keyword: "const", params: { allowedValue: "front" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err17];
                  } else {
                    vErrors.push(err17);
                  }
                  errors++;
                }
                var _valid0 = _errs17 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                  }
                  const _errs19 = errors;
                  if (typeof data !== "string") {
                    const err18 = { instancePath, schemaPath: "#/oneOf/9/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err18];
                    } else {
                      vErrors.push(err18);
                    }
                    errors++;
                  }
                  if ("telephoto" !== data) {
                    const err19 = { instancePath, schemaPath: "#/oneOf/9/const", keyword: "const", params: { allowedValue: "telephoto" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err19];
                    } else {
                      vErrors.push(err19);
                    }
                    errors++;
                  }
                  var _valid0 = _errs19 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err20 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err20];
    } else {
      vErrors.push(err20);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate97.errors = vErrors;
  return errors === 0;
}
validate97.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate92(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate92.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.quality === void 0) {
      const err1 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "quality" }, message: "must have required property 'quality'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "kind" || key0 === "quality")) {
        const err2 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data0 = data.kind;
      if (typeof data0 !== "string") {
        const err3 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
      if ("set_water_quality" !== data0) {
        const err4 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "set_water_quality" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.quality !== void 0) {
      if (!validate93(data.quality, { instancePath: instancePath + "/quality", parentData: data, parentDataProperty: "quality", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate93.errors : vErrors.concat(validate93.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err5 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err5];
    } else {
      vErrors.push(err5);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs7 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err6 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.mode === void 0) {
      const err7 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "mode" }, message: "must have required property 'mode'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "kind" || key1 === "mode")) {
        const err8 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data2 = data.kind;
      if (typeof data2 !== "string") {
        const err9 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
      if ("set_presentation" !== data2) {
        const err10 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "set_presentation" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
    if (data.mode !== void 0) {
      if (!validate95(data.mode, { instancePath: instancePath + "/mode", parentData: data, parentDataProperty: "mode", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate95.errors : vErrors.concat(validate95.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err11 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
  }
  var _valid0 = _errs7 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs13 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err12 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "kind")) {
          const err13 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err13];
          } else {
            vErrors.push(err13);
          }
          errors++;
        }
      }
      if (data.kind !== void 0) {
        let data4 = data.kind;
        if (typeof data4 !== "string") {
          const err14 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
        if ("recenter_phone_tracking" !== data4) {
          const err15 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "recenter_phone_tracking" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err15];
          } else {
            vErrors.push(err15);
          }
          errors++;
        }
      }
    } else {
      const err16 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err16];
      } else {
        vErrors.push(err16);
      }
      errors++;
    }
    var _valid0 = _errs13 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
      const _errs18 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.kind === void 0) {
          const err17 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
          if (vErrors === null) {
            vErrors = [err17];
          } else {
            vErrors.push(err17);
          }
          errors++;
        }
        for (const key3 in data) {
          if (!(key3 === "kind")) {
            const err18 = { instancePath, schemaPath: "#/oneOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err18];
            } else {
              vErrors.push(err18);
            }
            errors++;
          }
        }
        if (data.kind !== void 0) {
          let data5 = data.kind;
          if (typeof data5 !== "string") {
            const err19 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err19];
            } else {
              vErrors.push(err19);
            }
            errors++;
          }
          if ("recenter_menu" !== data5) {
            const err20 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/const", keyword: "const", params: { allowedValue: "recenter_menu" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err20];
            } else {
              vErrors.push(err20);
            }
            errors++;
          }
        }
      } else {
        const err21 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err21];
        } else {
          vErrors.push(err21);
        }
        errors++;
      }
      var _valid0 = _errs18 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
        const _errs23 = errors;
        if (data && typeof data == "object" && !Array.isArray(data)) {
          if (data.kind === void 0) {
            const err22 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
            if (vErrors === null) {
              vErrors = [err22];
            } else {
              vErrors.push(err22);
            }
            errors++;
          }
          for (const key4 in data) {
            if (!(key4 === "kind")) {
              const err23 = { instancePath, schemaPath: "#/oneOf/4/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key4 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err23];
              } else {
                vErrors.push(err23);
              }
              errors++;
            }
          }
          if (data.kind !== void 0) {
            let data6 = data.kind;
            if (typeof data6 !== "string") {
              const err24 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err24];
              } else {
                vErrors.push(err24);
              }
              errors++;
            }
            if ("download_record_json" !== data6) {
              const err25 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/const", keyword: "const", params: { allowedValue: "download_record_json" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err25];
              } else {
                vErrors.push(err25);
              }
              errors++;
            }
          }
        } else {
          const err26 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "object" }, message: "must be object" };
          if (vErrors === null) {
            vErrors = [err26];
          } else {
            vErrors.push(err26);
          }
          errors++;
        }
        var _valid0 = _errs23 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props0 !== true) {
              props0 = true;
            }
          }
          const _errs28 = errors;
          if (data && typeof data == "object" && !Array.isArray(data)) {
            if (data.kind === void 0) {
              const err27 = { instancePath, schemaPath: "#/oneOf/5/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
              if (vErrors === null) {
                vErrors = [err27];
              } else {
                vErrors.push(err27);
              }
              errors++;
            }
            for (const key5 in data) {
              if (!(key5 === "kind")) {
                const err28 = { instancePath, schemaPath: "#/oneOf/5/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key5 }, message: "must NOT have additional properties" };
                if (vErrors === null) {
                  vErrors = [err28];
                } else {
                  vErrors.push(err28);
                }
                errors++;
              }
            }
            if (data.kind !== void 0) {
              let data7 = data.kind;
              if (typeof data7 !== "string") {
                const err29 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err29];
                } else {
                  vErrors.push(err29);
                }
                errors++;
              }
              if ("download_flight_csv" !== data7) {
                const err30 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/const", keyword: "const", params: { allowedValue: "download_flight_csv" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err30];
                } else {
                  vErrors.push(err30);
                }
                errors++;
              }
            }
          } else {
            const err31 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "object" }, message: "must be object" };
            if (vErrors === null) {
              vErrors = [err31];
            } else {
              vErrors.push(err31);
            }
            errors++;
          }
          var _valid0 = _errs28 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true) {
                props0 = true;
              }
            }
            const _errs33 = errors;
            if (data && typeof data == "object" && !Array.isArray(data)) {
              if (data.kind === void 0) {
                const err32 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                if (vErrors === null) {
                  vErrors = [err32];
                } else {
                  vErrors.push(err32);
                }
                errors++;
              }
              if (data.id === void 0) {
                const err33 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
                if (vErrors === null) {
                  vErrors = [err33];
                } else {
                  vErrors.push(err33);
                }
                errors++;
              }
              for (const key6 in data) {
                if (!(key6 === "id" || key6 === "kind")) {
                  const err34 = { instancePath, schemaPath: "#/oneOf/6/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key6 }, message: "must NOT have additional properties" };
                  if (vErrors === null) {
                    vErrors = [err34];
                  } else {
                    vErrors.push(err34);
                  }
                  errors++;
                }
              }
              if (data.id !== void 0) {
                let data8 = data.id;
                if (!(typeof data8 == "number" && (!(data8 % 1) && !isNaN(data8)) && isFinite(data8))) {
                  const err35 = { instancePath: instancePath + "/id", schemaPath: "#/oneOf/6/properties/id/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
                  if (vErrors === null) {
                    vErrors = [err35];
                  } else {
                    vErrors.push(err35);
                  }
                  errors++;
                }
                if (typeof data8 == "number" && isFinite(data8)) {
                  if (data8 < 0 || isNaN(data8)) {
                    const err36 = { instancePath: instancePath + "/id", schemaPath: "#/oneOf/6/properties/id/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                    if (vErrors === null) {
                      vErrors = [err36];
                    } else {
                      vErrors.push(err36);
                    }
                    errors++;
                  }
                  if (!formats28.validate(data8)) {
                    const err37 = { instancePath: instancePath + "/id", schemaPath: "#/oneOf/6/properties/id/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
                    if (vErrors === null) {
                      vErrors = [err37];
                    } else {
                      vErrors.push(err37);
                    }
                    errors++;
                  }
                }
              }
              if (data.kind !== void 0) {
                let data9 = data.kind;
                if (typeof data9 !== "string") {
                  const err38 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err38];
                  } else {
                    vErrors.push(err38);
                  }
                  errors++;
                }
                if ("open_stored_record" !== data9) {
                  const err39 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/const", keyword: "const", params: { allowedValue: "open_stored_record" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err39];
                  } else {
                    vErrors.push(err39);
                  }
                  errors++;
                }
              }
            } else {
              const err40 = { instancePath, schemaPath: "#/oneOf/6/type", keyword: "type", params: { type: "object" }, message: "must be object" };
              if (vErrors === null) {
                vErrors = [err40];
              } else {
                vErrors.push(err40);
              }
              errors++;
            }
            var _valid0 = _errs33 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props0 !== true) {
                  props0 = true;
                }
              }
              const _errs40 = errors;
              if (data && typeof data == "object" && !Array.isArray(data)) {
                if (data.kind === void 0) {
                  const err41 = { instancePath, schemaPath: "#/oneOf/7/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                  if (vErrors === null) {
                    vErrors = [err41];
                  } else {
                    vErrors.push(err41);
                  }
                  errors++;
                }
                if (data.camera === void 0) {
                  const err42 = { instancePath, schemaPath: "#/oneOf/7/required", keyword: "required", params: { missingProperty: "camera" }, message: "must have required property 'camera'" };
                  if (vErrors === null) {
                    vErrors = [err42];
                  } else {
                    vErrors.push(err42);
                  }
                  errors++;
                }
                for (const key7 in data) {
                  if (!(key7 === "camera" || key7 === "kind")) {
                    const err43 = { instancePath, schemaPath: "#/oneOf/7/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key7 }, message: "must NOT have additional properties" };
                    if (vErrors === null) {
                      vErrors = [err43];
                    } else {
                      vErrors.push(err43);
                    }
                    errors++;
                  }
                }
                if (data.camera !== void 0) {
                  if (!validate97(data.camera, { instancePath: instancePath + "/camera", parentData: data, parentDataProperty: "camera", rootData, dynamicAnchors })) {
                    vErrors = vErrors === null ? validate97.errors : vErrors.concat(validate97.errors);
                    errors = vErrors.length;
                  }
                }
                if (data.kind !== void 0) {
                  let data11 = data.kind;
                  if (typeof data11 !== "string") {
                    const err44 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err44];
                    } else {
                      vErrors.push(err44);
                    }
                    errors++;
                  }
                  if ("set_replay_camera" !== data11) {
                    const err45 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/const", keyword: "const", params: { allowedValue: "set_replay_camera" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err45];
                    } else {
                      vErrors.push(err45);
                    }
                    errors++;
                  }
                }
              } else {
                const err46 = { instancePath, schemaPath: "#/oneOf/7/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                if (vErrors === null) {
                  vErrors = [err46];
                } else {
                  vErrors.push(err46);
                }
                errors++;
              }
              var _valid0 = _errs40 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props0 !== true) {
                    props0 = true;
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err47 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err47];
    } else {
      vErrors.push(err47);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate92.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate92.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate100(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate100.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("summary" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "summary" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("analysis" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "analysis" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
  }
  if (!valid0) {
    const err4 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err4];
    } else {
      vErrors.push(err4);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate100.errors = vErrors;
  return errors === 0;
}
validate100.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate102(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate102.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("map" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "map" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("altitude" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "altitude" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("speed" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "speed" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate102.errors = vErrors;
  return errors === 0;
}
validate102.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate104(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate104.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("menu" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "menu" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("settings" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "settings" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("help" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "help" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate104.errors = vErrors;
  return errors === 0;
}
validate104.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate106(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate106.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("cinematic" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "cinematic" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("telemetry" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "telemetry" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("analysis" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "analysis" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate106.errors = vErrors;
  return errors === 0;
}
validate106.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate79(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate79.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.command === void 0) {
      const err1 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "command" }, message: "must have required property 'command'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "command" || key0 === "kind")) {
        const err2 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.command !== void 0) {
      if (!validate80(data.command, { instancePath: instancePath + "/command", parentData: data, parentDataProperty: "command", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate80.errors : vErrors.concat(validate80.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data1 = data.kind;
      if (typeof data1 !== "string") {
        const err3 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
      if ("session" !== data1) {
        const err4 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "session" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
  } else {
    const err5 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err5];
    } else {
      vErrors.push(err5);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props1 = true;
  }
  const _errs7 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err6 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.command === void 0) {
      const err7 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "command" }, message: "must have required property 'command'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "command" || key1 === "kind")) {
        const err8 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.command !== void 0) {
      if (!validate92(data.command, { instancePath: instancePath + "/command", parentData: data, parentDataProperty: "command", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate92.errors : vErrors.concat(validate92.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data3 = data.kind;
      if (typeof data3 !== "string") {
        const err9 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
      if ("platform" !== data3) {
        const err10 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "platform" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
  } else {
    const err11 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
  }
  var _valid0 = _errs7 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props1 !== true) {
        props1 = true;
      }
    }
    const _errs13 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err12 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "kind")) {
          const err13 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err13];
          } else {
            vErrors.push(err13);
          }
          errors++;
        }
      }
      if (data.kind !== void 0) {
        let data4 = data.kind;
        if (typeof data4 !== "string") {
          const err14 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
        if ("toggle_technical" !== data4) {
          const err15 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "toggle_technical" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err15];
          } else {
            vErrors.push(err15);
          }
          errors++;
        }
      }
    } else {
      const err16 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err16];
      } else {
        vErrors.push(err16);
      }
      errors++;
    }
    var _valid0 = _errs13 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props1 !== true) {
          props1 = true;
        }
      }
      const _errs18 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.kind === void 0) {
          const err17 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
          if (vErrors === null) {
            vErrors = [err17];
          } else {
            vErrors.push(err17);
          }
          errors++;
        }
        if (data.tab === void 0) {
          const err18 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "tab" }, message: "must have required property 'tab'" };
          if (vErrors === null) {
            vErrors = [err18];
          } else {
            vErrors.push(err18);
          }
          errors++;
        }
        for (const key3 in data) {
          if (!(key3 === "kind" || key3 === "tab")) {
            const err19 = { instancePath, schemaPath: "#/oneOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err19];
            } else {
              vErrors.push(err19);
            }
            errors++;
          }
        }
        if (data.kind !== void 0) {
          let data5 = data.kind;
          if (typeof data5 !== "string") {
            const err20 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err20];
            } else {
              vErrors.push(err20);
            }
            errors++;
          }
          if ("set_result_tab" !== data5) {
            const err21 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/const", keyword: "const", params: { allowedValue: "set_result_tab" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err21];
            } else {
              vErrors.push(err21);
            }
            errors++;
          }
        }
        if (data.tab !== void 0) {
          if (!validate100(data.tab, { instancePath: instancePath + "/tab", parentData: data, parentDataProperty: "tab", rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate100.errors : vErrors.concat(validate100.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err22 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err22];
        } else {
          vErrors.push(err22);
        }
        errors++;
      }
      var _valid0 = _errs18 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props1 !== true) {
            props1 = true;
          }
        }
        const _errs24 = errors;
        if (data && typeof data == "object" && !Array.isArray(data)) {
          if (data.kind === void 0) {
            const err23 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
            if (vErrors === null) {
              vErrors = [err23];
            } else {
              vErrors.push(err23);
            }
            errors++;
          }
          if (data.chart === void 0) {
            const err24 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "chart" }, message: "must have required property 'chart'" };
            if (vErrors === null) {
              vErrors = [err24];
            } else {
              vErrors.push(err24);
            }
            errors++;
          }
          for (const key4 in data) {
            if (!(key4 === "chart" || key4 === "kind")) {
              const err25 = { instancePath, schemaPath: "#/oneOf/4/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key4 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err25];
              } else {
                vErrors.push(err25);
              }
              errors++;
            }
          }
          if (data.chart !== void 0) {
            if (!validate102(data.chart, { instancePath: instancePath + "/chart", parentData: data, parentDataProperty: "chart", rootData, dynamicAnchors })) {
              vErrors = vErrors === null ? validate102.errors : vErrors.concat(validate102.errors);
              errors = vErrors.length;
            }
          }
          if (data.kind !== void 0) {
            let data8 = data.kind;
            if (typeof data8 !== "string") {
              const err26 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err26];
              } else {
                vErrors.push(err26);
              }
              errors++;
            }
            if ("set_analysis_chart" !== data8) {
              const err27 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/const", keyword: "const", params: { allowedValue: "set_analysis_chart" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err27];
              } else {
                vErrors.push(err27);
              }
              errors++;
            }
          }
        } else {
          const err28 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "object" }, message: "must be object" };
          if (vErrors === null) {
            vErrors = [err28];
          } else {
            vErrors.push(err28);
          }
          errors++;
        }
        var _valid0 = _errs24 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props1 !== true) {
              props1 = true;
            }
          }
          const _errs30 = errors;
          if (data && typeof data == "object" && !Array.isArray(data)) {
            if (data.kind === void 0) {
              const err29 = { instancePath, schemaPath: "#/oneOf/5/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
              if (vErrors === null) {
                vErrors = [err29];
              } else {
                vErrors.push(err29);
              }
              errors++;
            }
            if (data.overlay === void 0) {
              const err30 = { instancePath, schemaPath: "#/oneOf/5/required", keyword: "required", params: { missingProperty: "overlay" }, message: "must have required property 'overlay'" };
              if (vErrors === null) {
                vErrors = [err30];
              } else {
                vErrors.push(err30);
              }
              errors++;
            }
            for (const key5 in data) {
              if (!(key5 === "kind" || key5 === "overlay")) {
                const err31 = { instancePath, schemaPath: "#/oneOf/5/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key5 }, message: "must NOT have additional properties" };
                if (vErrors === null) {
                  vErrors = [err31];
                } else {
                  vErrors.push(err31);
                }
                errors++;
              }
            }
            if (data.kind !== void 0) {
              let data9 = data.kind;
              if (typeof data9 !== "string") {
                const err32 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err32];
                } else {
                  vErrors.push(err32);
                }
                errors++;
              }
              if ("set_pause_overlay" !== data9) {
                const err33 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/const", keyword: "const", params: { allowedValue: "set_pause_overlay" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err33];
                } else {
                  vErrors.push(err33);
                }
                errors++;
              }
            }
            if (data.overlay !== void 0) {
              if (!validate104(data.overlay, { instancePath: instancePath + "/overlay", parentData: data, parentDataProperty: "overlay", rootData, dynamicAnchors })) {
                vErrors = vErrors === null ? validate104.errors : vErrors.concat(validate104.errors);
                errors = vErrors.length;
              }
            }
          } else {
            const err34 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "object" }, message: "must be object" };
            if (vErrors === null) {
              vErrors = [err34];
            } else {
              vErrors.push(err34);
            }
            errors++;
          }
          var _valid0 = _errs30 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props1 !== true) {
                props1 = true;
              }
            }
            const _errs36 = errors;
            if (data && typeof data == "object" && !Array.isArray(data)) {
              if (data.kind === void 0) {
                const err35 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                if (vErrors === null) {
                  vErrors = [err35];
                } else {
                  vErrors.push(err35);
                }
                errors++;
              }
              if (data.view === void 0) {
                const err36 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "view" }, message: "must have required property 'view'" };
                if (vErrors === null) {
                  vErrors = [err36];
                } else {
                  vErrors.push(err36);
                }
                errors++;
              }
              for (const key6 in data) {
                if (!(key6 === "kind" || key6 === "view")) {
                  const err37 = { instancePath, schemaPath: "#/oneOf/6/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key6 }, message: "must NOT have additional properties" };
                  if (vErrors === null) {
                    vErrors = [err37];
                  } else {
                    vErrors.push(err37);
                  }
                  errors++;
                }
              }
              if (data.kind !== void 0) {
                let data11 = data.kind;
                if (typeof data11 !== "string") {
                  const err38 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err38];
                  } else {
                    vErrors.push(err38);
                  }
                  errors++;
                }
                if ("set_replay_view" !== data11) {
                  const err39 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/const", keyword: "const", params: { allowedValue: "set_replay_view" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err39];
                  } else {
                    vErrors.push(err39);
                  }
                  errors++;
                }
              }
              if (data.view !== void 0) {
                if (!validate106(data.view, { instancePath: instancePath + "/view", parentData: data, parentDataProperty: "view", rootData, dynamicAnchors })) {
                  vErrors = vErrors === null ? validate106.errors : vErrors.concat(validate106.errors);
                  errors = vErrors.length;
                }
              }
            } else {
              const err40 = { instancePath, schemaPath: "#/oneOf/6/type", keyword: "type", params: { type: "object" }, message: "must be object" };
              if (vErrors === null) {
                vErrors = [err40];
              } else {
                vErrors.push(err40);
              }
              errors++;
            }
            var _valid0 = _errs36 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props1 !== true) {
                  props1 = true;
                }
              }
              const _errs42 = errors;
              if (data && typeof data == "object" && !Array.isArray(data)) {
                if (data.kind === void 0) {
                  const err41 = { instancePath, schemaPath: "#/oneOf/7/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                  if (vErrors === null) {
                    vErrors = [err41];
                  } else {
                    vErrors.push(err41);
                  }
                  errors++;
                }
                for (const key7 in data) {
                  if (!(key7 === "kind")) {
                    const err42 = { instancePath, schemaPath: "#/oneOf/7/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key7 }, message: "must NOT have additional properties" };
                    if (vErrors === null) {
                      vErrors = [err42];
                    } else {
                      vErrors.push(err42);
                    }
                    errors++;
                  }
                }
                if (data.kind !== void 0) {
                  let data13 = data.kind;
                  if (typeof data13 !== "string") {
                    const err43 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err43];
                    } else {
                      vErrors.push(err43);
                    }
                    errors++;
                  }
                  if ("cycle_replay_camera" !== data13) {
                    const err44 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/const", keyword: "const", params: { allowedValue: "cycle_replay_camera" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err44];
                    } else {
                      vErrors.push(err44);
                    }
                    errors++;
                  }
                }
              } else {
                const err45 = { instancePath, schemaPath: "#/oneOf/7/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                if (vErrors === null) {
                  vErrors = [err45];
                } else {
                  vErrors.push(err45);
                }
                errors++;
              }
              var _valid0 = _errs42 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props1 !== true) {
                    props1 = true;
                  }
                }
                const _errs47 = errors;
                if (data && typeof data == "object" && !Array.isArray(data)) {
                  if (data.kind === void 0) {
                    const err46 = { instancePath, schemaPath: "#/oneOf/8/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                    if (vErrors === null) {
                      vErrors = [err46];
                    } else {
                      vErrors.push(err46);
                    }
                    errors++;
                  }
                  if (data.time_seconds === void 0) {
                    const err47 = { instancePath, schemaPath: "#/oneOf/8/required", keyword: "required", params: { missingProperty: "time_seconds" }, message: "must have required property 'time_seconds'" };
                    if (vErrors === null) {
                      vErrors = [err47];
                    } else {
                      vErrors.push(err47);
                    }
                    errors++;
                  }
                  for (const key8 in data) {
                    if (!(key8 === "kind" || key8 === "time_seconds")) {
                      const err48 = { instancePath, schemaPath: "#/oneOf/8/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key8 }, message: "must NOT have additional properties" };
                      if (vErrors === null) {
                        vErrors = [err48];
                      } else {
                        vErrors.push(err48);
                      }
                      errors++;
                    }
                  }
                  if (data.kind !== void 0) {
                    let data14 = data.kind;
                    if (typeof data14 !== "string") {
                      const err49 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/8/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err49];
                      } else {
                        vErrors.push(err49);
                      }
                      errors++;
                    }
                    if ("seek_analysis" !== data14) {
                      const err50 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/8/properties/kind/const", keyword: "const", params: { allowedValue: "seek_analysis" }, message: "must be equal to constant" };
                      if (vErrors === null) {
                        vErrors = [err50];
                      } else {
                        vErrors.push(err50);
                      }
                      errors++;
                    }
                  }
                  if (data.time_seconds !== void 0) {
                    let data15 = data.time_seconds;
                    if (typeof data15 == "number" && isFinite(data15)) {
                      if (!formats0.validate(data15)) {
                        const err51 = { instancePath: instancePath + "/time_seconds", schemaPath: "#/oneOf/8/properties/time_seconds/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
                        if (vErrors === null) {
                          vErrors = [err51];
                        } else {
                          vErrors.push(err51);
                        }
                        errors++;
                      }
                    } else {
                      const err52 = { instancePath: instancePath + "/time_seconds", schemaPath: "#/oneOf/8/properties/time_seconds/type", keyword: "type", params: { type: "number" }, message: "must be number" };
                      if (vErrors === null) {
                        vErrors = [err52];
                      } else {
                        vErrors.push(err52);
                      }
                      errors++;
                    }
                  }
                } else {
                  const err53 = { instancePath, schemaPath: "#/oneOf/8/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                  if (vErrors === null) {
                    vErrors = [err53];
                  } else {
                    vErrors.push(err53);
                  }
                  errors++;
                }
                var _valid0 = _errs47 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                    if (props1 !== true) {
                      props1 = true;
                    }
                  }
                  const _errs54 = errors;
                  if (data && typeof data == "object" && !Array.isArray(data)) {
                    if (data.kind === void 0) {
                      const err54 = { instancePath, schemaPath: "#/oneOf/9/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                      if (vErrors === null) {
                        vErrors = [err54];
                      } else {
                        vErrors.push(err54);
                      }
                      errors++;
                    }
                    if (data.time_seconds === void 0) {
                      const err55 = { instancePath, schemaPath: "#/oneOf/9/required", keyword: "required", params: { missingProperty: "time_seconds" }, message: "must have required property 'time_seconds'" };
                      if (vErrors === null) {
                        vErrors = [err55];
                      } else {
                        vErrors.push(err55);
                      }
                      errors++;
                    }
                    for (const key9 in data) {
                      if (!(key9 === "kind" || key9 === "time_seconds")) {
                        const err56 = { instancePath, schemaPath: "#/oneOf/9/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key9 }, message: "must NOT have additional properties" };
                        if (vErrors === null) {
                          vErrors = [err56];
                        } else {
                          vErrors.push(err56);
                        }
                        errors++;
                      }
                    }
                    if (data.kind !== void 0) {
                      let data16 = data.kind;
                      if (typeof data16 !== "string") {
                        const err57 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/9/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                        if (vErrors === null) {
                          vErrors = [err57];
                        } else {
                          vErrors.push(err57);
                        }
                        errors++;
                      }
                      if ("seek_replay" !== data16) {
                        const err58 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/9/properties/kind/const", keyword: "const", params: { allowedValue: "seek_replay" }, message: "must be equal to constant" };
                        if (vErrors === null) {
                          vErrors = [err58];
                        } else {
                          vErrors.push(err58);
                        }
                        errors++;
                      }
                    }
                    if (data.time_seconds !== void 0) {
                      let data17 = data.time_seconds;
                      if (typeof data17 == "number" && isFinite(data17)) {
                        if (!formats0.validate(data17)) {
                          const err59 = { instancePath: instancePath + "/time_seconds", schemaPath: "#/oneOf/9/properties/time_seconds/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
                          if (vErrors === null) {
                            vErrors = [err59];
                          } else {
                            vErrors.push(err59);
                          }
                          errors++;
                        }
                      } else {
                        const err60 = { instancePath: instancePath + "/time_seconds", schemaPath: "#/oneOf/9/properties/time_seconds/type", keyword: "type", params: { type: "number" }, message: "must be number" };
                        if (vErrors === null) {
                          vErrors = [err60];
                        } else {
                          vErrors.push(err60);
                        }
                        errors++;
                      }
                    }
                  } else {
                    const err61 = { instancePath, schemaPath: "#/oneOf/9/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                    if (vErrors === null) {
                      vErrors = [err61];
                    } else {
                      vErrors.push(err61);
                    }
                    errors++;
                  }
                  var _valid0 = _errs54 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                      if (props1 !== true) {
                        props1 = true;
                      }
                    }
                    const _errs61 = errors;
                    if (data && typeof data == "object" && !Array.isArray(data)) {
                      if (data.kind === void 0) {
                        const err62 = { instancePath, schemaPath: "#/oneOf/10/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                        if (vErrors === null) {
                          vErrors = [err62];
                        } else {
                          vErrors.push(err62);
                        }
                        errors++;
                      }
                      for (const key10 in data) {
                        if (!(key10 === "kind")) {
                          const err63 = { instancePath, schemaPath: "#/oneOf/10/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key10 }, message: "must NOT have additional properties" };
                          if (vErrors === null) {
                            vErrors = [err63];
                          } else {
                            vErrors.push(err63);
                          }
                          errors++;
                        }
                      }
                      if (data.kind !== void 0) {
                        let data18 = data.kind;
                        if (typeof data18 !== "string") {
                          const err64 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/10/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                          if (vErrors === null) {
                            vErrors = [err64];
                          } else {
                            vErrors.push(err64);
                          }
                          errors++;
                        }
                        if ("toggle_replay_playing" !== data18) {
                          const err65 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/10/properties/kind/const", keyword: "const", params: { allowedValue: "toggle_replay_playing" }, message: "must be equal to constant" };
                          if (vErrors === null) {
                            vErrors = [err65];
                          } else {
                            vErrors.push(err65);
                          }
                          errors++;
                        }
                      }
                    } else {
                      const err66 = { instancePath, schemaPath: "#/oneOf/10/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                      if (vErrors === null) {
                        vErrors = [err66];
                      } else {
                        vErrors.push(err66);
                      }
                      errors++;
                    }
                    var _valid0 = _errs61 === errors;
                    if (_valid0 && valid0) {
                      valid0 = false;
                      passing0 = [passing0, 10];
                    } else {
                      if (_valid0) {
                        valid0 = true;
                        passing0 = 10;
                        if (props1 !== true) {
                          props1 = true;
                        }
                      }
                      const _errs66 = errors;
                      if (data && typeof data == "object" && !Array.isArray(data)) {
                        if (data.kind === void 0) {
                          const err67 = { instancePath, schemaPath: "#/oneOf/11/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                          if (vErrors === null) {
                            vErrors = [err67];
                          } else {
                            vErrors.push(err67);
                          }
                          errors++;
                        }
                        if (data.code === void 0) {
                          const err68 = { instancePath, schemaPath: "#/oneOf/11/required", keyword: "required", params: { missingProperty: "code" }, message: "must have required property 'code'" };
                          if (vErrors === null) {
                            vErrors = [err68];
                          } else {
                            vErrors.push(err68);
                          }
                          errors++;
                        }
                        for (const key11 in data) {
                          if (!(key11 === "code" || key11 === "kind")) {
                            const err69 = { instancePath, schemaPath: "#/oneOf/11/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key11 }, message: "must NOT have additional properties" };
                            if (vErrors === null) {
                              vErrors = [err69];
                            } else {
                              vErrors.push(err69);
                            }
                            errors++;
                          }
                        }
                        if (data.code !== void 0) {
                          let data19 = data.code;
                          if (!(typeof data19 == "number" && (!(data19 % 1) && !isNaN(data19)) && isFinite(data19))) {
                            const err70 = { instancePath: instancePath + "/code", schemaPath: "#/oneOf/11/properties/code/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
                            if (vErrors === null) {
                              vErrors = [err70];
                            } else {
                              vErrors.push(err70);
                            }
                            errors++;
                          }
                          if (typeof data19 == "number" && isFinite(data19)) {
                            if (data19 < 0 || isNaN(data19)) {
                              const err71 = { instancePath: instancePath + "/code", schemaPath: "#/oneOf/11/properties/code/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                              if (vErrors === null) {
                                vErrors = [err71];
                              } else {
                                vErrors.push(err71);
                              }
                              errors++;
                            }
                            if (!formats28.validate(data19)) {
                              const err72 = { instancePath: instancePath + "/code", schemaPath: "#/oneOf/11/properties/code/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
                              if (vErrors === null) {
                                vErrors = [err72];
                              } else {
                                vErrors.push(err72);
                              }
                              errors++;
                            }
                          }
                        }
                        if (data.kind !== void 0) {
                          let data20 = data.kind;
                          if (typeof data20 !== "string") {
                            const err73 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/11/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                            if (vErrors === null) {
                              vErrors = [err73];
                            } else {
                              vErrors.push(err73);
                            }
                            errors++;
                          }
                          if ("set_replay_rate" !== data20) {
                            const err74 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/11/properties/kind/const", keyword: "const", params: { allowedValue: "set_replay_rate" }, message: "must be equal to constant" };
                            if (vErrors === null) {
                              vErrors = [err74];
                            } else {
                              vErrors.push(err74);
                            }
                            errors++;
                          }
                        }
                      } else {
                        const err75 = { instancePath, schemaPath: "#/oneOf/11/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                        if (vErrors === null) {
                          vErrors = [err75];
                        } else {
                          vErrors.push(err75);
                        }
                        errors++;
                      }
                      var _valid0 = _errs66 === errors;
                      if (_valid0 && valid0) {
                        valid0 = false;
                        passing0 = [passing0, 11];
                      } else {
                        if (_valid0) {
                          valid0 = true;
                          passing0 = 11;
                          if (props1 !== true) {
                            props1 = true;
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err76 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err76];
    } else {
      vErrors.push(err76);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate79.errors = vErrors;
  evaluated0.props = props1;
  return errors === 0;
}
validate79.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate110(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate110.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("primary" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "primary" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("secondary" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "secondary" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("normal" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "normal" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate110.errors = vErrors;
  return errors === 0;
}
validate110.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate109(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate109.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.emphasis === void 0) {
      const err1 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "emphasis" }, message: "must have required property 'emphasis'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "emphasis" || key0 === "kind")) {
        const err2 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.emphasis !== void 0) {
      if (!validate110(data.emphasis, { instancePath: instancePath + "/emphasis", parentData: data, parentDataProperty: "emphasis", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate110.errors : vErrors.concat(validate110.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data1 = data.kind;
      if (typeof data1 !== "string") {
        const err3 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
      if ("action" !== data1) {
        const err4 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "action" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
  } else {
    const err5 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err5];
    } else {
      vErrors.push(err5);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs7 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err6 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.group === void 0) {
      const err7 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "group" }, message: "must have required property 'group'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.group_label === void 0) {
      const err8 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "group_label" }, message: "must have required property 'group_label'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.selected === void 0) {
      const err9 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "selected" }, message: "must have required property 'selected'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    if (data.description === void 0) {
      const err10 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "description" }, message: "must have required property 'description'" };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "description" || key1 === "group" || key1 === "group_label" || key1 === "kind" || key1 === "selected")) {
        const err11 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
    }
    if (data.description !== void 0) {
      if (typeof data.description !== "string") {
        const err12 = { instancePath: instancePath + "/description", schemaPath: "#/oneOf/1/properties/description/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
    }
    if (data.group !== void 0) {
      if (typeof data.group !== "string") {
        const err13 = { instancePath: instancePath + "/group", schemaPath: "#/oneOf/1/properties/group/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      }
    }
    if (data.group_label !== void 0) {
      if (typeof data.group_label !== "string") {
        const err14 = { instancePath: instancePath + "/group_label", schemaPath: "#/oneOf/1/properties/group_label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err14];
        } else {
          vErrors.push(err14);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data5 = data.kind;
      if (typeof data5 !== "string") {
        const err15 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
      if ("choice" !== data5) {
        const err16 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "choice" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err16];
        } else {
          vErrors.push(err16);
        }
        errors++;
      }
    }
    if (data.selected !== void 0) {
      if (typeof data.selected !== "boolean") {
        const err17 = { instancePath: instancePath + "/selected", schemaPath: "#/oneOf/1/properties/selected/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err17];
        } else {
          vErrors.push(err17);
        }
        errors++;
      }
    }
  } else {
    const err18 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err18];
    } else {
      vErrors.push(err18);
    }
    errors++;
  }
  var _valid0 = _errs7 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs20 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err19 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
      if (data.expanded === void 0) {
        const err20 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "expanded" }, message: "must have required property 'expanded'" };
        if (vErrors === null) {
          vErrors = [err20];
        } else {
          vErrors.push(err20);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "expanded" || key2 === "kind")) {
          const err21 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err21];
          } else {
            vErrors.push(err21);
          }
          errors++;
        }
      }
      if (data.expanded !== void 0) {
        if (typeof data.expanded !== "boolean") {
          const err22 = { instancePath: instancePath + "/expanded", schemaPath: "#/oneOf/2/properties/expanded/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
          if (vErrors === null) {
            vErrors = [err22];
          } else {
            vErrors.push(err22);
          }
          errors++;
        }
      }
      if (data.kind !== void 0) {
        let data8 = data.kind;
        if (typeof data8 !== "string") {
          const err23 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err23];
          } else {
            vErrors.push(err23);
          }
          errors++;
        }
        if ("disclosure" !== data8) {
          const err24 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "disclosure" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err24];
          } else {
            vErrors.push(err24);
          }
          errors++;
        }
      }
    } else {
      const err25 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err25];
      } else {
        vErrors.push(err25);
      }
      errors++;
    }
    var _valid0 = _errs20 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
    }
  }
  if (!valid0) {
    const err26 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err26];
    } else {
      vErrors.push(err26);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate109.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate109.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate114(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate114.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("analysis" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "analysis" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("replay" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "replay" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
  }
  if (!valid0) {
    const err4 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err4];
    } else {
      vErrors.push(err4);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate114.errors = vErrors;
  return errors === 0;
}
validate114.evaluated = { "dynamicProps": false, "dynamicItems": false };
var schema53 = { "additionalProperties": false, "description": "Complete engine-independent recorded chart.", "properties": { "cursor_points": { "description": "Record-interpolated cursor positions.", "items": { "$ref": "#/$defs/GameUiChartCursorPoint" }, "type": "array" }, "cursor_x": { "description": "Time cursor; maps use cursor points only.", "format": "double", "type": ["number", "null"] }, "equal_axis_scale": { "description": "Preserves metre-for-metre map scale.", "type": "boolean" }, "markers": { "description": "Launch, terminal and venue markers.", "items": { "$ref": "#/$defs/GameUiChartMarker" }, "type": "array" }, "presentation": { "$ref": "#/$defs/GameUiChartPresentation", "description": "Shared chart layout and labels in logical display pixels." }, "reference_lines": { "description": "Horizontal reference lines.", "items": { "$ref": "#/$defs/GameUiChartReferenceLine" }, "type": "array" }, "series": { "description": "Ordered curves including registered venue geometry.", "items": { "$ref": "#/$defs/GameUiChartSeries" }, "type": "array" }, "time_markers": { "description": "Recorded-time map markers.", "items": { "$ref": "#/$defs/GameUiChartMarker" }, "type": "array" }, "vectors": { "description": "Clipped ambient-wind arrows.", "items": { "$ref": "#/$defs/GameUiChartVector" }, "type": "array" }, "x_axis_label": { "description": "Horizontal axis label and unit.", "type": "string" }, "x_maximum": { "description": "Horizontal upper bound.", "format": "double", "type": "number" }, "x_minimum": { "description": "Horizontal lower bound.", "format": "double", "type": "number" }, "y_axis_label": { "description": "Vertical axis label and unit.", "type": "string" }, "y_maximum": { "description": "Vertical upper bound.", "format": "double", "type": "number" }, "y_minimum": { "description": "Vertical lower bound.", "format": "double", "type": "number" } }, "required": ["presentation", "x_axis_label", "y_axis_label", "x_minimum", "x_maximum", "y_minimum", "y_maximum", "equal_axis_scale", "series", "vectors", "markers", "time_markers", "reference_lines", "cursor_x", "cursor_points"], "type": "object" };
function validate119(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate119.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.x === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "x" }, message: "must have required property 'x'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.y === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "y" }, message: "must have required property 'y'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "x" || key0 === "y")) {
        const err2 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.x !== void 0) {
      let data0 = data.x;
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err3 = { instancePath: instancePath + "/x", schemaPath: "#/properties/x/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err3];
          } else {
            vErrors.push(err3);
          }
          errors++;
        }
      } else {
        const err4 = { instancePath: instancePath + "/x", schemaPath: "#/properties/x/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.y !== void 0) {
      let data1 = data.y;
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err5 = { instancePath: instancePath + "/y", schemaPath: "#/properties/y/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        }
      } else {
        const err6 = { instancePath: instancePath + "/y", schemaPath: "#/properties/y/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
  } else {
    const err7 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err7];
    } else {
      vErrors.push(err7);
    }
    errors++;
  }
  validate119.errors = vErrors;
  return errors === 0;
}
validate119.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate118(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate118.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.point === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "point" }, message: "must have required property 'point'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.color === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "color" }, message: "must have required property 'color'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "color" || key0 === "point")) {
        const err2 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.color !== void 0) {
      if (typeof data.color !== "string") {
        const err3 = { instancePath: instancePath + "/color", schemaPath: "#/properties/color/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.point !== void 0) {
      if (!validate119(data.point, { instancePath: instancePath + "/point", parentData: data, parentDataProperty: "point", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate119.errors : vErrors.concat(validate119.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err4 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err4];
    } else {
      vErrors.push(err4);
    }
    errors++;
  }
  validate118.errors = vErrors;
  return errors === 0;
}
validate118.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate122(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate122.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.label === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.color === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "color" }, message: "must have required property 'color'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.point === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "point" }, message: "must have required property 'point'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "color" || key0 === "label" || key0 === "point")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.color !== void 0) {
      if (typeof data.color !== "string") {
        const err4 = { instancePath: instancePath + "/color", schemaPath: "#/properties/color/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err5 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.point !== void 0) {
      if (!validate119(data.point, { instancePath: instancePath + "/point", parentData: data, parentDataProperty: "point", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate119.errors : vErrors.concat(validate119.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err6 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  }
  validate122.errors = vErrors;
  return errors === 0;
}
validate122.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var schema57 = { "additionalProperties": false, "description": "Shared static plot geometry and Web-baseline typography.", "properties": { "axes_color": { "description": "Shared axis stroke colour.", "type": "string" }, "axis_font_size": { "description": "Axis font size in logical pixels.", "format": "double", "type": "number" }, "grid_color": { "description": "Shared grid stroke colour.", "type": "string" }, "grid_lines": { "description": "Static grid segments in descriptor coordinates.", "items": { "items": { "$ref": "#/$defs/GameUiChartPoint" }, "maxItems": 2, "minItems": 2, "type": "array" }, "type": "array" }, "grid_opacity": { "description": "Grid stroke opacity.", "format": "double", "type": "number" }, "legend": { "description": "Ordered shared legend entries.", "items": { "$ref": "#/$defs/GameUiChartLegend" }, "type": "array" }, "legend_font_rem": { "description": "Legend font size in root-font units.", "format": "double", "type": "number" }, "legend_gap_rem": { "description": "Legend item gap in root-font units.", "format": "double", "type": "number" }, "legend_gradient_radius_rem": { "description": "Gradient swatch corner radius in root-font units.", "format": "double", "type": "number" }, "legend_gradient_rem": { "description": "Gradient legend swatch dimensions in root-font units.", "items": { "format": "double", "type": "number" }, "maxItems": 2, "minItems": 2, "type": "array" }, "legend_swatch_gap_rem": { "description": "Gap after each legend swatch in root-font units.", "format": "double", "type": "number" }, "legend_swatch_rem": { "description": "Solid legend swatch dimensions in root-font units.", "items": { "format": "double", "type": "number" }, "maxItems": 2, "minItems": 2, "type": "array" }, "map_guides": { "anyOf": [{ "$ref": "#/$defs/GameUiChartMapGuides" }, { "type": "null" }], "description": "Metre scale and north reference for equal-scale maps." }, "marker_labels": { "description": "Marker labels in the same order as the chart markers.", "items": { "$ref": "#/$defs/GameUiChartText" }, "type": "array" }, "measurement_minimum": { "$ref": "#/$defs/GameUiChartExtent", "description": "Minimum usable measured dimensions in logical pixels." }, "plot": { "description": "Plot rectangle ordered left, top, right, bottom.", "items": { "format": "double", "type": "number" }, "maxItems": 4, "minItems": 4, "type": "array" }, "reference_labels": { "description": "Reference labels in the same order as the reference lines.", "items": { "$ref": "#/$defs/GameUiChartText" }, "type": "array" }, "scroll_overflow": { "description": "Keeps caption, viewport and legend inside one scrollable chart host.", "type": "boolean" }, "tick_font_size": { "description": "Tick font size in logical pixels.", "format": "double", "type": "number" }, "time_marker_labels": { "description": "Recorded-time labels in the same order as the time markers.", "items": { "$ref": "#/$defs/GameUiChartText" }, "type": "array" }, "viewport": { "$ref": "#/$defs/GameUiChartExtent", "description": "Coordinate extent of this descriptor." }, "viewport_minimum_rem": { "description": "Minimum viewport width and height in root-font units.", "items": { "format": "double", "type": "number" }, "maxItems": 2, "minItems": 2, "type": "array" }, "x_axis": { "$ref": "#/$defs/GameUiChartText", "description": "Horizontal axis title." }, "x_ticks": { "description": "Ordered horizontal ticks.", "items": { "$ref": "#/$defs/GameUiChartTick" }, "type": "array" }, "y_axis": { "$ref": "#/$defs/GameUiChartText", "description": "Vertical axis title." }, "y_ticks": { "description": "Ordered vertical ticks.", "items": { "$ref": "#/$defs/GameUiChartTick" }, "type": "array" } }, "required": ["viewport", "measurement_minimum", "scroll_overflow", "plot", "tick_font_size", "axis_font_size", "x_ticks", "y_ticks", "grid_lines", "axes_color", "grid_color", "grid_opacity", "x_axis", "y_axis", "marker_labels", "time_marker_labels", "reference_labels", "legend", "map_guides", "viewport_minimum_rem", "legend_font_rem", "legend_gap_rem", "legend_swatch_rem", "legend_swatch_gap_rem", "legend_gradient_rem", "legend_gradient_radius_rem"], "type": "object" };
var schema58 = { "additionalProperties": false, "description": "One ordered legend item with an optional progress gradient.", "properties": { "color": { "description": "Solid swatch colour.", "type": "string" }, "label": { "description": "Shared label.", "type": "string" }, "segment_gradient": { "description": "First and last progress colours when the series uses a gradient.", "items": { "type": "string" }, "maxItems": 2, "minItems": 2, "type": ["array", "null"] } }, "required": ["label", "color", "segment_gradient"], "type": "object" };
function validate127(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate127.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.label === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.color === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "color" }, message: "must have required property 'color'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.segment_gradient === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "segment_gradient" }, message: "must have required property 'segment_gradient'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "color" || key0 === "label" || key0 === "segment_gradient")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.color !== void 0) {
      if (typeof data.color !== "string") {
        const err4 = { instancePath: instancePath + "/color", schemaPath: "#/properties/color/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err5 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.segment_gradient !== void 0) {
      let data2 = data.segment_gradient;
      if (!Array.isArray(data2) && data2 !== null) {
        const err6 = { instancePath: instancePath + "/segment_gradient", schemaPath: "#/properties/segment_gradient/type", keyword: "type", params: { type: schema58.properties.segment_gradient.type }, message: "must be array,null" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if (Array.isArray(data2)) {
        if (data2.length > 2) {
          const err7 = { instancePath: instancePath + "/segment_gradient", schemaPath: "#/properties/segment_gradient/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err7];
          } else {
            vErrors.push(err7);
          }
          errors++;
        }
        if (data2.length < 2) {
          const err8 = { instancePath: instancePath + "/segment_gradient", schemaPath: "#/properties/segment_gradient/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        const len0 = data2.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (typeof data2[i0] !== "string") {
            const err9 = { instancePath: instancePath + "/segment_gradient/" + i0, schemaPath: "#/properties/segment_gradient/items/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err9];
            } else {
              vErrors.push(err9);
            }
            errors++;
          }
        }
      }
    }
  } else {
    const err10 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  }
  validate127.errors = vErrors;
  return errors === 0;
}
validate127.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate131(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate131.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("start" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "start" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("middle" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "middle" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("end" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "end" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate131.errors = vErrors;
  return errors === 0;
}
validate131.evaluated = { "dynamicProps": false, "dynamicItems": false };
var formats92 = (init_numeric_formats(), __toCommonJS(numeric_formats_exports)).numericFormats.uint16;
function validate130(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate130.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.label === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.position === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "position" }, message: "must have required property 'position'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.font_size === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "font_size" }, message: "must have required property 'font_size'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.font_weight === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "font_weight" }, message: "must have required property 'font_weight'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.rotation_degrees === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "rotation_degrees" }, message: "must have required property 'rotation_degrees'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.anchor === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "anchor" }, message: "must have required property 'anchor'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.color === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "color" }, message: "must have required property 'color'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "anchor" || key0 === "color" || key0 === "font_size" || key0 === "font_weight" || key0 === "label" || key0 === "position" || key0 === "rotation_degrees")) {
        const err7 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
    if (data.anchor !== void 0) {
      if (!validate131(data.anchor, { instancePath: instancePath + "/anchor", parentData: data, parentDataProperty: "anchor", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate131.errors : vErrors.concat(validate131.errors);
        errors = vErrors.length;
      }
    }
    if (data.color !== void 0) {
      if (typeof data.color !== "string") {
        const err8 = { instancePath: instancePath + "/color", schemaPath: "#/properties/color/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.font_size !== void 0) {
      let data2 = data.font_size;
      if (typeof data2 == "number" && isFinite(data2)) {
        if (!formats0.validate(data2)) {
          const err9 = { instancePath: instancePath + "/font_size", schemaPath: "#/properties/font_size/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
      } else {
        const err10 = { instancePath: instancePath + "/font_size", schemaPath: "#/properties/font_size/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
    if (data.font_weight !== void 0) {
      let data3 = data.font_weight;
      if (!(typeof data3 == "number" && (!(data3 % 1) && !isNaN(data3)) && isFinite(data3))) {
        const err11 = { instancePath: instancePath + "/font_weight", schemaPath: "#/properties/font_weight/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
      if (typeof data3 == "number" && isFinite(data3)) {
        if (data3 > 65535 || isNaN(data3)) {
          const err12 = { instancePath: instancePath + "/font_weight", schemaPath: "#/properties/font_weight/maximum", keyword: "maximum", params: { comparison: "<=", limit: 65535 }, message: "must be <= 65535" };
          if (vErrors === null) {
            vErrors = [err12];
          } else {
            vErrors.push(err12);
          }
          errors++;
        }
        if (data3 < 0 || isNaN(data3)) {
          const err13 = { instancePath: instancePath + "/font_weight", schemaPath: "#/properties/font_weight/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
          if (vErrors === null) {
            vErrors = [err13];
          } else {
            vErrors.push(err13);
          }
          errors++;
        }
        if (!formats92.validate(data3)) {
          const err14 = { instancePath: instancePath + "/font_weight", schemaPath: "#/properties/font_weight/format", keyword: "format", params: { format: "uint16" }, message: 'must match format "uint16"' };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err15 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
    }
    if (data.position !== void 0) {
      if (!validate119(data.position, { instancePath: instancePath + "/position", parentData: data, parentDataProperty: "position", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate119.errors : vErrors.concat(validate119.errors);
        errors = vErrors.length;
      }
    }
    if (data.rotation_degrees !== void 0) {
      let data6 = data.rotation_degrees;
      if (typeof data6 == "number" && isFinite(data6)) {
        if (!formats0.validate(data6)) {
          const err16 = { instancePath: instancePath + "/rotation_degrees", schemaPath: "#/properties/rotation_degrees/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err16];
          } else {
            vErrors.push(err16);
          }
          errors++;
        }
      } else {
        const err17 = { instancePath: instancePath + "/rotation_degrees", schemaPath: "#/properties/rotation_degrees/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err17];
        } else {
          vErrors.push(err17);
        }
        errors++;
      }
    }
  } else {
    const err18 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err18];
    } else {
      vErrors.push(err18);
    }
    errors++;
  }
  validate130.errors = vErrors;
  return errors === 0;
}
validate130.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate129(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate129.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.scale_distance_m === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "scale_distance_m" }, message: "must have required property 'scale_distance_m'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.scale_segments === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "scale_segments" }, message: "must have required property 'scale_segments'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.scale_label === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "scale_label" }, message: "must have required property 'scale_label'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.north_segments === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "north_segments" }, message: "must have required property 'north_segments'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.north_label === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "north_label" }, message: "must have required property 'north_label'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "north_label" || key0 === "north_segments" || key0 === "scale_distance_m" || key0 === "scale_label" || key0 === "scale_segments")) {
        const err5 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.north_label !== void 0) {
      if (!validate130(data.north_label, { instancePath: instancePath + "/north_label", parentData: data, parentDataProperty: "north_label", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate130.errors : vErrors.concat(validate130.errors);
        errors = vErrors.length;
      }
    }
    if (data.north_segments !== void 0) {
      let data1 = data.north_segments;
      if (Array.isArray(data1)) {
        if (data1.length > 3) {
          const err6 = { instancePath: instancePath + "/north_segments", schemaPath: "#/properties/north_segments/maxItems", keyword: "maxItems", params: { limit: 3 }, message: "must NOT have more than 3 items" };
          if (vErrors === null) {
            vErrors = [err6];
          } else {
            vErrors.push(err6);
          }
          errors++;
        }
        if (data1.length < 3) {
          const err7 = { instancePath: instancePath + "/north_segments", schemaPath: "#/properties/north_segments/minItems", keyword: "minItems", params: { limit: 3 }, message: "must NOT have fewer than 3 items" };
          if (vErrors === null) {
            vErrors = [err7];
          } else {
            vErrors.push(err7);
          }
          errors++;
        }
        const len0 = data1.length;
        for (let i0 = 0; i0 < len0; i0++) {
          let data2 = data1[i0];
          if (Array.isArray(data2)) {
            if (data2.length > 2) {
              const err8 = { instancePath: instancePath + "/north_segments/" + i0, schemaPath: "#/properties/north_segments/items/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
              if (vErrors === null) {
                vErrors = [err8];
              } else {
                vErrors.push(err8);
              }
              errors++;
            }
            if (data2.length < 2) {
              const err9 = { instancePath: instancePath + "/north_segments/" + i0, schemaPath: "#/properties/north_segments/items/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
              if (vErrors === null) {
                vErrors = [err9];
              } else {
                vErrors.push(err9);
              }
              errors++;
            }
            const len1 = data2.length;
            for (let i1 = 0; i1 < len1; i1++) {
              if (!validate119(data2[i1], { instancePath: instancePath + "/north_segments/" + i0 + "/" + i1, parentData: data2, parentDataProperty: i1, rootData, dynamicAnchors })) {
                vErrors = vErrors === null ? validate119.errors : vErrors.concat(validate119.errors);
                errors = vErrors.length;
              }
            }
          } else {
            const err10 = { instancePath: instancePath + "/north_segments/" + i0, schemaPath: "#/properties/north_segments/items/type", keyword: "type", params: { type: "array" }, message: "must be array" };
            if (vErrors === null) {
              vErrors = [err10];
            } else {
              vErrors.push(err10);
            }
            errors++;
          }
        }
      } else {
        const err11 = { instancePath: instancePath + "/north_segments", schemaPath: "#/properties/north_segments/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
    }
    if (data.scale_distance_m !== void 0) {
      let data4 = data.scale_distance_m;
      if (typeof data4 == "number" && isFinite(data4)) {
        if (!formats0.validate(data4)) {
          const err12 = { instancePath: instancePath + "/scale_distance_m", schemaPath: "#/properties/scale_distance_m/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err12];
          } else {
            vErrors.push(err12);
          }
          errors++;
        }
      } else {
        const err13 = { instancePath: instancePath + "/scale_distance_m", schemaPath: "#/properties/scale_distance_m/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      }
    }
    if (data.scale_label !== void 0) {
      if (!validate130(data.scale_label, { instancePath: instancePath + "/scale_label", parentData: data, parentDataProperty: "scale_label", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate130.errors : vErrors.concat(validate130.errors);
        errors = vErrors.length;
      }
    }
    if (data.scale_segments !== void 0) {
      let data6 = data.scale_segments;
      if (Array.isArray(data6)) {
        if (data6.length > 3) {
          const err14 = { instancePath: instancePath + "/scale_segments", schemaPath: "#/properties/scale_segments/maxItems", keyword: "maxItems", params: { limit: 3 }, message: "must NOT have more than 3 items" };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
        if (data6.length < 3) {
          const err15 = { instancePath: instancePath + "/scale_segments", schemaPath: "#/properties/scale_segments/minItems", keyword: "minItems", params: { limit: 3 }, message: "must NOT have fewer than 3 items" };
          if (vErrors === null) {
            vErrors = [err15];
          } else {
            vErrors.push(err15);
          }
          errors++;
        }
        const len2 = data6.length;
        for (let i2 = 0; i2 < len2; i2++) {
          let data7 = data6[i2];
          if (Array.isArray(data7)) {
            if (data7.length > 2) {
              const err16 = { instancePath: instancePath + "/scale_segments/" + i2, schemaPath: "#/properties/scale_segments/items/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
              if (vErrors === null) {
                vErrors = [err16];
              } else {
                vErrors.push(err16);
              }
              errors++;
            }
            if (data7.length < 2) {
              const err17 = { instancePath: instancePath + "/scale_segments/" + i2, schemaPath: "#/properties/scale_segments/items/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
              if (vErrors === null) {
                vErrors = [err17];
              } else {
                vErrors.push(err17);
              }
              errors++;
            }
            const len3 = data7.length;
            for (let i3 = 0; i3 < len3; i3++) {
              if (!validate119(data7[i3], { instancePath: instancePath + "/scale_segments/" + i2 + "/" + i3, parentData: data7, parentDataProperty: i3, rootData, dynamicAnchors })) {
                vErrors = vErrors === null ? validate119.errors : vErrors.concat(validate119.errors);
                errors = vErrors.length;
              }
            }
          } else {
            const err18 = { instancePath: instancePath + "/scale_segments/" + i2, schemaPath: "#/properties/scale_segments/items/type", keyword: "type", params: { type: "array" }, message: "must be array" };
            if (vErrors === null) {
              vErrors = [err18];
            } else {
              vErrors.push(err18);
            }
            errors++;
          }
        }
      } else {
        const err19 = { instancePath: instancePath + "/scale_segments", schemaPath: "#/properties/scale_segments/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
    }
  } else {
    const err20 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err20];
    } else {
      vErrors.push(err20);
    }
    errors++;
  }
  validate129.errors = vErrors;
  return errors === 0;
}
validate129.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate140(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate140.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.width === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "width" }, message: "must have required property 'width'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.height === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "height" }, message: "must have required property 'height'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "height" || key0 === "width")) {
        const err2 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.height !== void 0) {
      let data0 = data.height;
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err3 = { instancePath: instancePath + "/height", schemaPath: "#/properties/height/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err3];
          } else {
            vErrors.push(err3);
          }
          errors++;
        }
      } else {
        const err4 = { instancePath: instancePath + "/height", schemaPath: "#/properties/height/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.width !== void 0) {
      let data1 = data.width;
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err5 = { instancePath: instancePath + "/width", schemaPath: "#/properties/width/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        }
      } else {
        const err6 = { instancePath: instancePath + "/width", schemaPath: "#/properties/width/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
  } else {
    const err7 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err7];
    } else {
      vErrors.push(err7);
    }
    errors++;
  }
  validate140.errors = vErrors;
  return errors === 0;
}
validate140.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate146(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate146.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.value === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "value" }, message: "must have required property 'value'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.ratio === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "ratio" }, message: "must have required property 'ratio'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.text === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "text" }, message: "must have required property 'text'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "ratio" || key0 === "text" || key0 === "value")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.ratio !== void 0) {
      let data0 = data.ratio;
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err4 = { instancePath: instancePath + "/ratio", schemaPath: "#/properties/ratio/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err4];
          } else {
            vErrors.push(err4);
          }
          errors++;
        }
      } else {
        const err5 = { instancePath: instancePath + "/ratio", schemaPath: "#/properties/ratio/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.text !== void 0) {
      if (!validate130(data.text, { instancePath: instancePath + "/text", parentData: data, parentDataProperty: "text", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate130.errors : vErrors.concat(validate130.errors);
        errors = vErrors.length;
      }
    }
    if (data.value !== void 0) {
      let data2 = data.value;
      if (typeof data2 == "number" && isFinite(data2)) {
        if (!formats0.validate(data2)) {
          const err6 = { instancePath: instancePath + "/value", schemaPath: "#/properties/value/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err6];
          } else {
            vErrors.push(err6);
          }
          errors++;
        }
      } else {
        const err7 = { instancePath: instancePath + "/value", schemaPath: "#/properties/value/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
  } else {
    const err8 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
  }
  validate146.errors = vErrors;
  return errors === 0;
}
validate146.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate125(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate125.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.viewport === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "viewport" }, message: "must have required property 'viewport'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.measurement_minimum === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "measurement_minimum" }, message: "must have required property 'measurement_minimum'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.scroll_overflow === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "scroll_overflow" }, message: "must have required property 'scroll_overflow'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.plot === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "plot" }, message: "must have required property 'plot'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.tick_font_size === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "tick_font_size" }, message: "must have required property 'tick_font_size'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.axis_font_size === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "axis_font_size" }, message: "must have required property 'axis_font_size'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.x_ticks === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "x_ticks" }, message: "must have required property 'x_ticks'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.y_ticks === void 0) {
      const err7 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "y_ticks" }, message: "must have required property 'y_ticks'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.grid_lines === void 0) {
      const err8 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "grid_lines" }, message: "must have required property 'grid_lines'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.axes_color === void 0) {
      const err9 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "axes_color" }, message: "must have required property 'axes_color'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    if (data.grid_color === void 0) {
      const err10 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "grid_color" }, message: "must have required property 'grid_color'" };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
    if (data.grid_opacity === void 0) {
      const err11 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "grid_opacity" }, message: "must have required property 'grid_opacity'" };
      if (vErrors === null) {
        vErrors = [err11];
      } else {
        vErrors.push(err11);
      }
      errors++;
    }
    if (data.x_axis === void 0) {
      const err12 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "x_axis" }, message: "must have required property 'x_axis'" };
      if (vErrors === null) {
        vErrors = [err12];
      } else {
        vErrors.push(err12);
      }
      errors++;
    }
    if (data.y_axis === void 0) {
      const err13 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "y_axis" }, message: "must have required property 'y_axis'" };
      if (vErrors === null) {
        vErrors = [err13];
      } else {
        vErrors.push(err13);
      }
      errors++;
    }
    if (data.marker_labels === void 0) {
      const err14 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "marker_labels" }, message: "must have required property 'marker_labels'" };
      if (vErrors === null) {
        vErrors = [err14];
      } else {
        vErrors.push(err14);
      }
      errors++;
    }
    if (data.time_marker_labels === void 0) {
      const err15 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "time_marker_labels" }, message: "must have required property 'time_marker_labels'" };
      if (vErrors === null) {
        vErrors = [err15];
      } else {
        vErrors.push(err15);
      }
      errors++;
    }
    if (data.reference_labels === void 0) {
      const err16 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "reference_labels" }, message: "must have required property 'reference_labels'" };
      if (vErrors === null) {
        vErrors = [err16];
      } else {
        vErrors.push(err16);
      }
      errors++;
    }
    if (data.legend === void 0) {
      const err17 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "legend" }, message: "must have required property 'legend'" };
      if (vErrors === null) {
        vErrors = [err17];
      } else {
        vErrors.push(err17);
      }
      errors++;
    }
    if (data.map_guides === void 0) {
      const err18 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "map_guides" }, message: "must have required property 'map_guides'" };
      if (vErrors === null) {
        vErrors = [err18];
      } else {
        vErrors.push(err18);
      }
      errors++;
    }
    if (data.viewport_minimum_rem === void 0) {
      const err19 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "viewport_minimum_rem" }, message: "must have required property 'viewport_minimum_rem'" };
      if (vErrors === null) {
        vErrors = [err19];
      } else {
        vErrors.push(err19);
      }
      errors++;
    }
    if (data.legend_font_rem === void 0) {
      const err20 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "legend_font_rem" }, message: "must have required property 'legend_font_rem'" };
      if (vErrors === null) {
        vErrors = [err20];
      } else {
        vErrors.push(err20);
      }
      errors++;
    }
    if (data.legend_gap_rem === void 0) {
      const err21 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "legend_gap_rem" }, message: "must have required property 'legend_gap_rem'" };
      if (vErrors === null) {
        vErrors = [err21];
      } else {
        vErrors.push(err21);
      }
      errors++;
    }
    if (data.legend_swatch_rem === void 0) {
      const err22 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "legend_swatch_rem" }, message: "must have required property 'legend_swatch_rem'" };
      if (vErrors === null) {
        vErrors = [err22];
      } else {
        vErrors.push(err22);
      }
      errors++;
    }
    if (data.legend_swatch_gap_rem === void 0) {
      const err23 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "legend_swatch_gap_rem" }, message: "must have required property 'legend_swatch_gap_rem'" };
      if (vErrors === null) {
        vErrors = [err23];
      } else {
        vErrors.push(err23);
      }
      errors++;
    }
    if (data.legend_gradient_rem === void 0) {
      const err24 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "legend_gradient_rem" }, message: "must have required property 'legend_gradient_rem'" };
      if (vErrors === null) {
        vErrors = [err24];
      } else {
        vErrors.push(err24);
      }
      errors++;
    }
    if (data.legend_gradient_radius_rem === void 0) {
      const err25 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "legend_gradient_radius_rem" }, message: "must have required property 'legend_gradient_radius_rem'" };
      if (vErrors === null) {
        vErrors = [err25];
      } else {
        vErrors.push(err25);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!func1.call(schema57.properties, key0)) {
        const err26 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err26];
        } else {
          vErrors.push(err26);
        }
        errors++;
      }
    }
    if (data.axes_color !== void 0) {
      if (typeof data.axes_color !== "string") {
        const err27 = { instancePath: instancePath + "/axes_color", schemaPath: "#/properties/axes_color/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err27];
        } else {
          vErrors.push(err27);
        }
        errors++;
      }
    }
    if (data.axis_font_size !== void 0) {
      let data1 = data.axis_font_size;
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err28 = { instancePath: instancePath + "/axis_font_size", schemaPath: "#/properties/axis_font_size/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err28];
          } else {
            vErrors.push(err28);
          }
          errors++;
        }
      } else {
        const err29 = { instancePath: instancePath + "/axis_font_size", schemaPath: "#/properties/axis_font_size/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err29];
        } else {
          vErrors.push(err29);
        }
        errors++;
      }
    }
    if (data.grid_color !== void 0) {
      if (typeof data.grid_color !== "string") {
        const err30 = { instancePath: instancePath + "/grid_color", schemaPath: "#/properties/grid_color/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err30];
        } else {
          vErrors.push(err30);
        }
        errors++;
      }
    }
    if (data.grid_lines !== void 0) {
      let data3 = data.grid_lines;
      if (Array.isArray(data3)) {
        const len0 = data3.length;
        for (let i0 = 0; i0 < len0; i0++) {
          let data4 = data3[i0];
          if (Array.isArray(data4)) {
            if (data4.length > 2) {
              const err31 = { instancePath: instancePath + "/grid_lines/" + i0, schemaPath: "#/properties/grid_lines/items/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
              if (vErrors === null) {
                vErrors = [err31];
              } else {
                vErrors.push(err31);
              }
              errors++;
            }
            if (data4.length < 2) {
              const err32 = { instancePath: instancePath + "/grid_lines/" + i0, schemaPath: "#/properties/grid_lines/items/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
              if (vErrors === null) {
                vErrors = [err32];
              } else {
                vErrors.push(err32);
              }
              errors++;
            }
            const len1 = data4.length;
            for (let i1 = 0; i1 < len1; i1++) {
              if (!validate119(data4[i1], { instancePath: instancePath + "/grid_lines/" + i0 + "/" + i1, parentData: data4, parentDataProperty: i1, rootData, dynamicAnchors })) {
                vErrors = vErrors === null ? validate119.errors : vErrors.concat(validate119.errors);
                errors = vErrors.length;
              }
            }
          } else {
            const err33 = { instancePath: instancePath + "/grid_lines/" + i0, schemaPath: "#/properties/grid_lines/items/type", keyword: "type", params: { type: "array" }, message: "must be array" };
            if (vErrors === null) {
              vErrors = [err33];
            } else {
              vErrors.push(err33);
            }
            errors++;
          }
        }
      } else {
        const err34 = { instancePath: instancePath + "/grid_lines", schemaPath: "#/properties/grid_lines/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err34];
        } else {
          vErrors.push(err34);
        }
        errors++;
      }
    }
    if (data.grid_opacity !== void 0) {
      let data6 = data.grid_opacity;
      if (typeof data6 == "number" && isFinite(data6)) {
        if (!formats0.validate(data6)) {
          const err35 = { instancePath: instancePath + "/grid_opacity", schemaPath: "#/properties/grid_opacity/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err35];
          } else {
            vErrors.push(err35);
          }
          errors++;
        }
      } else {
        const err36 = { instancePath: instancePath + "/grid_opacity", schemaPath: "#/properties/grid_opacity/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err36];
        } else {
          vErrors.push(err36);
        }
        errors++;
      }
    }
    if (data.legend !== void 0) {
      let data7 = data.legend;
      if (Array.isArray(data7)) {
        const len2 = data7.length;
        for (let i2 = 0; i2 < len2; i2++) {
          if (!validate127(data7[i2], { instancePath: instancePath + "/legend/" + i2, parentData: data7, parentDataProperty: i2, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate127.errors : vErrors.concat(validate127.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err37 = { instancePath: instancePath + "/legend", schemaPath: "#/properties/legend/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err37];
        } else {
          vErrors.push(err37);
        }
        errors++;
      }
    }
    if (data.legend_font_rem !== void 0) {
      let data9 = data.legend_font_rem;
      if (typeof data9 == "number" && isFinite(data9)) {
        if (!formats0.validate(data9)) {
          const err38 = { instancePath: instancePath + "/legend_font_rem", schemaPath: "#/properties/legend_font_rem/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err38];
          } else {
            vErrors.push(err38);
          }
          errors++;
        }
      } else {
        const err39 = { instancePath: instancePath + "/legend_font_rem", schemaPath: "#/properties/legend_font_rem/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err39];
        } else {
          vErrors.push(err39);
        }
        errors++;
      }
    }
    if (data.legend_gap_rem !== void 0) {
      let data10 = data.legend_gap_rem;
      if (typeof data10 == "number" && isFinite(data10)) {
        if (!formats0.validate(data10)) {
          const err40 = { instancePath: instancePath + "/legend_gap_rem", schemaPath: "#/properties/legend_gap_rem/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err40];
          } else {
            vErrors.push(err40);
          }
          errors++;
        }
      } else {
        const err41 = { instancePath: instancePath + "/legend_gap_rem", schemaPath: "#/properties/legend_gap_rem/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err41];
        } else {
          vErrors.push(err41);
        }
        errors++;
      }
    }
    if (data.legend_gradient_radius_rem !== void 0) {
      let data11 = data.legend_gradient_radius_rem;
      if (typeof data11 == "number" && isFinite(data11)) {
        if (!formats0.validate(data11)) {
          const err42 = { instancePath: instancePath + "/legend_gradient_radius_rem", schemaPath: "#/properties/legend_gradient_radius_rem/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err42];
          } else {
            vErrors.push(err42);
          }
          errors++;
        }
      } else {
        const err43 = { instancePath: instancePath + "/legend_gradient_radius_rem", schemaPath: "#/properties/legend_gradient_radius_rem/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err43];
        } else {
          vErrors.push(err43);
        }
        errors++;
      }
    }
    if (data.legend_gradient_rem !== void 0) {
      let data12 = data.legend_gradient_rem;
      if (Array.isArray(data12)) {
        if (data12.length > 2) {
          const err44 = { instancePath: instancePath + "/legend_gradient_rem", schemaPath: "#/properties/legend_gradient_rem/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err44];
          } else {
            vErrors.push(err44);
          }
          errors++;
        }
        if (data12.length < 2) {
          const err45 = { instancePath: instancePath + "/legend_gradient_rem", schemaPath: "#/properties/legend_gradient_rem/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err45];
          } else {
            vErrors.push(err45);
          }
          errors++;
        }
        const len3 = data12.length;
        for (let i3 = 0; i3 < len3; i3++) {
          let data13 = data12[i3];
          if (typeof data13 == "number" && isFinite(data13)) {
            if (!formats0.validate(data13)) {
              const err46 = { instancePath: instancePath + "/legend_gradient_rem/" + i3, schemaPath: "#/properties/legend_gradient_rem/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err46];
              } else {
                vErrors.push(err46);
              }
              errors++;
            }
          } else {
            const err47 = { instancePath: instancePath + "/legend_gradient_rem/" + i3, schemaPath: "#/properties/legend_gradient_rem/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err47];
            } else {
              vErrors.push(err47);
            }
            errors++;
          }
        }
      } else {
        const err48 = { instancePath: instancePath + "/legend_gradient_rem", schemaPath: "#/properties/legend_gradient_rem/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err48];
        } else {
          vErrors.push(err48);
        }
        errors++;
      }
    }
    if (data.legend_swatch_gap_rem !== void 0) {
      let data14 = data.legend_swatch_gap_rem;
      if (typeof data14 == "number" && isFinite(data14)) {
        if (!formats0.validate(data14)) {
          const err49 = { instancePath: instancePath + "/legend_swatch_gap_rem", schemaPath: "#/properties/legend_swatch_gap_rem/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err49];
          } else {
            vErrors.push(err49);
          }
          errors++;
        }
      } else {
        const err50 = { instancePath: instancePath + "/legend_swatch_gap_rem", schemaPath: "#/properties/legend_swatch_gap_rem/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err50];
        } else {
          vErrors.push(err50);
        }
        errors++;
      }
    }
    if (data.legend_swatch_rem !== void 0) {
      let data15 = data.legend_swatch_rem;
      if (Array.isArray(data15)) {
        if (data15.length > 2) {
          const err51 = { instancePath: instancePath + "/legend_swatch_rem", schemaPath: "#/properties/legend_swatch_rem/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err51];
          } else {
            vErrors.push(err51);
          }
          errors++;
        }
        if (data15.length < 2) {
          const err52 = { instancePath: instancePath + "/legend_swatch_rem", schemaPath: "#/properties/legend_swatch_rem/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err52];
          } else {
            vErrors.push(err52);
          }
          errors++;
        }
        const len4 = data15.length;
        for (let i4 = 0; i4 < len4; i4++) {
          let data16 = data15[i4];
          if (typeof data16 == "number" && isFinite(data16)) {
            if (!formats0.validate(data16)) {
              const err53 = { instancePath: instancePath + "/legend_swatch_rem/" + i4, schemaPath: "#/properties/legend_swatch_rem/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err53];
              } else {
                vErrors.push(err53);
              }
              errors++;
            }
          } else {
            const err54 = { instancePath: instancePath + "/legend_swatch_rem/" + i4, schemaPath: "#/properties/legend_swatch_rem/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err54];
            } else {
              vErrors.push(err54);
            }
            errors++;
          }
        }
      } else {
        const err55 = { instancePath: instancePath + "/legend_swatch_rem", schemaPath: "#/properties/legend_swatch_rem/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err55];
        } else {
          vErrors.push(err55);
        }
        errors++;
      }
    }
    if (data.map_guides !== void 0) {
      let data17 = data.map_guides;
      const _errs35 = errors;
      let valid11 = false;
      const _errs36 = errors;
      if (!validate129(data17, { instancePath: instancePath + "/map_guides", parentData: data, parentDataProperty: "map_guides", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate129.errors : vErrors.concat(validate129.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs36 === errors;
      valid11 = valid11 || _valid0;
      const _errs37 = errors;
      if (data17 !== null) {
        const err56 = { instancePath: instancePath + "/map_guides", schemaPath: "#/properties/map_guides/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
        if (vErrors === null) {
          vErrors = [err56];
        } else {
          vErrors.push(err56);
        }
        errors++;
      }
      var _valid0 = _errs37 === errors;
      valid11 = valid11 || _valid0;
      if (!valid11) {
        const err57 = { instancePath: instancePath + "/map_guides", schemaPath: "#/properties/map_guides/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
        if (vErrors === null) {
          vErrors = [err57];
        } else {
          vErrors.push(err57);
        }
        errors++;
      } else {
        errors = _errs35;
        if (vErrors !== null) {
          if (_errs35) {
            vErrors.length = _errs35;
          } else {
            vErrors = null;
          }
        }
      }
    }
    if (data.marker_labels !== void 0) {
      let data18 = data.marker_labels;
      if (Array.isArray(data18)) {
        const len5 = data18.length;
        for (let i5 = 0; i5 < len5; i5++) {
          if (!validate130(data18[i5], { instancePath: instancePath + "/marker_labels/" + i5, parentData: data18, parentDataProperty: i5, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate130.errors : vErrors.concat(validate130.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err58 = { instancePath: instancePath + "/marker_labels", schemaPath: "#/properties/marker_labels/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err58];
        } else {
          vErrors.push(err58);
        }
        errors++;
      }
    }
    if (data.measurement_minimum !== void 0) {
      if (!validate140(data.measurement_minimum, { instancePath: instancePath + "/measurement_minimum", parentData: data, parentDataProperty: "measurement_minimum", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate140.errors : vErrors.concat(validate140.errors);
        errors = vErrors.length;
      }
    }
    if (data.plot !== void 0) {
      let data21 = data.plot;
      if (Array.isArray(data21)) {
        if (data21.length > 4) {
          const err59 = { instancePath: instancePath + "/plot", schemaPath: "#/properties/plot/maxItems", keyword: "maxItems", params: { limit: 4 }, message: "must NOT have more than 4 items" };
          if (vErrors === null) {
            vErrors = [err59];
          } else {
            vErrors.push(err59);
          }
          errors++;
        }
        if (data21.length < 4) {
          const err60 = { instancePath: instancePath + "/plot", schemaPath: "#/properties/plot/minItems", keyword: "minItems", params: { limit: 4 }, message: "must NOT have fewer than 4 items" };
          if (vErrors === null) {
            vErrors = [err60];
          } else {
            vErrors.push(err60);
          }
          errors++;
        }
        const len6 = data21.length;
        for (let i6 = 0; i6 < len6; i6++) {
          let data22 = data21[i6];
          if (typeof data22 == "number" && isFinite(data22)) {
            if (!formats0.validate(data22)) {
              const err61 = { instancePath: instancePath + "/plot/" + i6, schemaPath: "#/properties/plot/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err61];
              } else {
                vErrors.push(err61);
              }
              errors++;
            }
          } else {
            const err62 = { instancePath: instancePath + "/plot/" + i6, schemaPath: "#/properties/plot/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err62];
            } else {
              vErrors.push(err62);
            }
            errors++;
          }
        }
      } else {
        const err63 = { instancePath: instancePath + "/plot", schemaPath: "#/properties/plot/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err63];
        } else {
          vErrors.push(err63);
        }
        errors++;
      }
    }
    if (data.reference_labels !== void 0) {
      let data23 = data.reference_labels;
      if (Array.isArray(data23)) {
        const len7 = data23.length;
        for (let i7 = 0; i7 < len7; i7++) {
          if (!validate130(data23[i7], { instancePath: instancePath + "/reference_labels/" + i7, parentData: data23, parentDataProperty: i7, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate130.errors : vErrors.concat(validate130.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err64 = { instancePath: instancePath + "/reference_labels", schemaPath: "#/properties/reference_labels/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err64];
        } else {
          vErrors.push(err64);
        }
        errors++;
      }
    }
    if (data.scroll_overflow !== void 0) {
      if (typeof data.scroll_overflow !== "boolean") {
        const err65 = { instancePath: instancePath + "/scroll_overflow", schemaPath: "#/properties/scroll_overflow/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err65];
        } else {
          vErrors.push(err65);
        }
        errors++;
      }
    }
    if (data.tick_font_size !== void 0) {
      let data26 = data.tick_font_size;
      if (typeof data26 == "number" && isFinite(data26)) {
        if (!formats0.validate(data26)) {
          const err66 = { instancePath: instancePath + "/tick_font_size", schemaPath: "#/properties/tick_font_size/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err66];
          } else {
            vErrors.push(err66);
          }
          errors++;
        }
      } else {
        const err67 = { instancePath: instancePath + "/tick_font_size", schemaPath: "#/properties/tick_font_size/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err67];
        } else {
          vErrors.push(err67);
        }
        errors++;
      }
    }
    if (data.time_marker_labels !== void 0) {
      let data27 = data.time_marker_labels;
      if (Array.isArray(data27)) {
        const len8 = data27.length;
        for (let i8 = 0; i8 < len8; i8++) {
          if (!validate130(data27[i8], { instancePath: instancePath + "/time_marker_labels/" + i8, parentData: data27, parentDataProperty: i8, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate130.errors : vErrors.concat(validate130.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err68 = { instancePath: instancePath + "/time_marker_labels", schemaPath: "#/properties/time_marker_labels/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err68];
        } else {
          vErrors.push(err68);
        }
        errors++;
      }
    }
    if (data.viewport !== void 0) {
      if (!validate140(data.viewport, { instancePath: instancePath + "/viewport", parentData: data, parentDataProperty: "viewport", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate140.errors : vErrors.concat(validate140.errors);
        errors = vErrors.length;
      }
    }
    if (data.viewport_minimum_rem !== void 0) {
      let data30 = data.viewport_minimum_rem;
      if (Array.isArray(data30)) {
        if (data30.length > 2) {
          const err69 = { instancePath: instancePath + "/viewport_minimum_rem", schemaPath: "#/properties/viewport_minimum_rem/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err69];
          } else {
            vErrors.push(err69);
          }
          errors++;
        }
        if (data30.length < 2) {
          const err70 = { instancePath: instancePath + "/viewport_minimum_rem", schemaPath: "#/properties/viewport_minimum_rem/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err70];
          } else {
            vErrors.push(err70);
          }
          errors++;
        }
        const len9 = data30.length;
        for (let i9 = 0; i9 < len9; i9++) {
          let data31 = data30[i9];
          if (typeof data31 == "number" && isFinite(data31)) {
            if (!formats0.validate(data31)) {
              const err71 = { instancePath: instancePath + "/viewport_minimum_rem/" + i9, schemaPath: "#/properties/viewport_minimum_rem/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err71];
              } else {
                vErrors.push(err71);
              }
              errors++;
            }
          } else {
            const err72 = { instancePath: instancePath + "/viewport_minimum_rem/" + i9, schemaPath: "#/properties/viewport_minimum_rem/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err72];
            } else {
              vErrors.push(err72);
            }
            errors++;
          }
        }
      } else {
        const err73 = { instancePath: instancePath + "/viewport_minimum_rem", schemaPath: "#/properties/viewport_minimum_rem/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err73];
        } else {
          vErrors.push(err73);
        }
        errors++;
      }
    }
    if (data.x_axis !== void 0) {
      if (!validate130(data.x_axis, { instancePath: instancePath + "/x_axis", parentData: data, parentDataProperty: "x_axis", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate130.errors : vErrors.concat(validate130.errors);
        errors = vErrors.length;
      }
    }
    if (data.x_ticks !== void 0) {
      let data33 = data.x_ticks;
      if (Array.isArray(data33)) {
        const len10 = data33.length;
        for (let i10 = 0; i10 < len10; i10++) {
          if (!validate146(data33[i10], { instancePath: instancePath + "/x_ticks/" + i10, parentData: data33, parentDataProperty: i10, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate146.errors : vErrors.concat(validate146.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err74 = { instancePath: instancePath + "/x_ticks", schemaPath: "#/properties/x_ticks/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err74];
        } else {
          vErrors.push(err74);
        }
        errors++;
      }
    }
    if (data.y_axis !== void 0) {
      if (!validate130(data.y_axis, { instancePath: instancePath + "/y_axis", parentData: data, parentDataProperty: "y_axis", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate130.errors : vErrors.concat(validate130.errors);
        errors = vErrors.length;
      }
    }
    if (data.y_ticks !== void 0) {
      let data36 = data.y_ticks;
      if (Array.isArray(data36)) {
        const len11 = data36.length;
        for (let i11 = 0; i11 < len11; i11++) {
          if (!validate146(data36[i11], { instancePath: instancePath + "/y_ticks/" + i11, parentData: data36, parentDataProperty: i11, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate146.errors : vErrors.concat(validate146.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err75 = { instancePath: instancePath + "/y_ticks", schemaPath: "#/properties/y_ticks/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err75];
        } else {
          vErrors.push(err75);
        }
        errors++;
      }
    }
  } else {
    const err76 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err76];
    } else {
      vErrors.push(err76);
    }
    errors++;
  }
  validate125.errors = vErrors;
  return errors === 0;
}
validate125.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate152(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate152.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.value === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "value" }, message: "must have required property 'value'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.label === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.color === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "color" }, message: "must have required property 'color'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "color" || key0 === "label" || key0 === "value")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.color !== void 0) {
      if (typeof data.color !== "string") {
        const err4 = { instancePath: instancePath + "/color", schemaPath: "#/properties/color/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err5 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.value !== void 0) {
      let data2 = data.value;
      if (typeof data2 == "number" && isFinite(data2)) {
        if (!formats0.validate(data2)) {
          const err6 = { instancePath: instancePath + "/value", schemaPath: "#/properties/value/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err6];
          } else {
            vErrors.push(err6);
          }
          errors++;
        }
      } else {
        const err7 = { instancePath: instancePath + "/value", schemaPath: "#/properties/value/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
  } else {
    const err8 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
  }
  validate152.errors = vErrors;
  return errors === 0;
}
validate152.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate154(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate154.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.label === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.color === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "color" }, message: "must have required property 'color'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.points === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "points" }, message: "must have required property 'points'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.segment_colors === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "segment_colors" }, message: "must have required property 'segment_colors'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "color" || key0 === "label" || key0 === "points" || key0 === "segment_colors")) {
        const err4 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.color !== void 0) {
      if (typeof data.color !== "string") {
        const err5 = { instancePath: instancePath + "/color", schemaPath: "#/properties/color/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err6 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
    if (data.points !== void 0) {
      let data2 = data.points;
      if (Array.isArray(data2)) {
        const len0 = data2.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate119(data2[i0], { instancePath: instancePath + "/points/" + i0, parentData: data2, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate119.errors : vErrors.concat(validate119.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err7 = { instancePath: instancePath + "/points", schemaPath: "#/properties/points/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
    if (data.segment_colors !== void 0) {
      let data4 = data.segment_colors;
      if (Array.isArray(data4)) {
        const len1 = data4.length;
        for (let i1 = 0; i1 < len1; i1++) {
          if (typeof data4[i1] !== "string") {
            const err8 = { instancePath: instancePath + "/segment_colors/" + i1, schemaPath: "#/properties/segment_colors/items/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err8];
            } else {
              vErrors.push(err8);
            }
            errors++;
          }
        }
      } else {
        const err9 = { instancePath: instancePath + "/segment_colors", schemaPath: "#/properties/segment_colors/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
  } else {
    const err10 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  }
  validate154.errors = vErrors;
  return errors === 0;
}
validate154.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var schema66 = { "additionalProperties": false, "description": "Clipped vector with simultaneously present or absent endpoints.", "properties": { "color": { "description": "Shared colour.", "type": "string" }, "label": { "description": "Legend label.", "type": "string" }, "segment": { "description": "Visible segment, absent for zero wind or outside the chart.", "items": { "$ref": "#/$defs/GameUiChartPoint" }, "maxItems": 2, "minItems": 2, "type": ["array", "null"] } }, "required": ["label", "color", "segment"], "type": "object" };
function validate158(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate158.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.label === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.color === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "color" }, message: "must have required property 'color'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.segment === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "segment" }, message: "must have required property 'segment'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "color" || key0 === "label" || key0 === "segment")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.color !== void 0) {
      if (typeof data.color !== "string") {
        const err4 = { instancePath: instancePath + "/color", schemaPath: "#/properties/color/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err5 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.segment !== void 0) {
      let data2 = data.segment;
      if (!Array.isArray(data2) && data2 !== null) {
        const err6 = { instancePath: instancePath + "/segment", schemaPath: "#/properties/segment/type", keyword: "type", params: { type: schema66.properties.segment.type }, message: "must be array,null" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if (Array.isArray(data2)) {
        if (data2.length > 2) {
          const err7 = { instancePath: instancePath + "/segment", schemaPath: "#/properties/segment/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err7];
          } else {
            vErrors.push(err7);
          }
          errors++;
        }
        if (data2.length < 2) {
          const err8 = { instancePath: instancePath + "/segment", schemaPath: "#/properties/segment/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        const len0 = data2.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate119(data2[i0], { instancePath: instancePath + "/segment/" + i0, parentData: data2, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate119.errors : vErrors.concat(validate119.errors);
            errors = vErrors.length;
          }
        }
      }
    }
  } else {
    const err9 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err9];
    } else {
      vErrors.push(err9);
    }
    errors++;
  }
  validate158.errors = vErrors;
  return errors === 0;
}
validate158.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate117(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate117.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.presentation === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "presentation" }, message: "must have required property 'presentation'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.x_axis_label === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "x_axis_label" }, message: "must have required property 'x_axis_label'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.y_axis_label === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "y_axis_label" }, message: "must have required property 'y_axis_label'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.x_minimum === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "x_minimum" }, message: "must have required property 'x_minimum'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.x_maximum === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "x_maximum" }, message: "must have required property 'x_maximum'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.y_minimum === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "y_minimum" }, message: "must have required property 'y_minimum'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.y_maximum === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "y_maximum" }, message: "must have required property 'y_maximum'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.equal_axis_scale === void 0) {
      const err7 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "equal_axis_scale" }, message: "must have required property 'equal_axis_scale'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.series === void 0) {
      const err8 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "series" }, message: "must have required property 'series'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.vectors === void 0) {
      const err9 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "vectors" }, message: "must have required property 'vectors'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    if (data.markers === void 0) {
      const err10 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "markers" }, message: "must have required property 'markers'" };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
    if (data.time_markers === void 0) {
      const err11 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "time_markers" }, message: "must have required property 'time_markers'" };
      if (vErrors === null) {
        vErrors = [err11];
      } else {
        vErrors.push(err11);
      }
      errors++;
    }
    if (data.reference_lines === void 0) {
      const err12 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "reference_lines" }, message: "must have required property 'reference_lines'" };
      if (vErrors === null) {
        vErrors = [err12];
      } else {
        vErrors.push(err12);
      }
      errors++;
    }
    if (data.cursor_x === void 0) {
      const err13 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "cursor_x" }, message: "must have required property 'cursor_x'" };
      if (vErrors === null) {
        vErrors = [err13];
      } else {
        vErrors.push(err13);
      }
      errors++;
    }
    if (data.cursor_points === void 0) {
      const err14 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "cursor_points" }, message: "must have required property 'cursor_points'" };
      if (vErrors === null) {
        vErrors = [err14];
      } else {
        vErrors.push(err14);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!func1.call(schema53.properties, key0)) {
        const err15 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
    }
    if (data.cursor_points !== void 0) {
      let data0 = data.cursor_points;
      if (Array.isArray(data0)) {
        const len0 = data0.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate118(data0[i0], { instancePath: instancePath + "/cursor_points/" + i0, parentData: data0, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate118.errors : vErrors.concat(validate118.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err16 = { instancePath: instancePath + "/cursor_points", schemaPath: "#/properties/cursor_points/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err16];
        } else {
          vErrors.push(err16);
        }
        errors++;
      }
    }
    if (data.cursor_x !== void 0) {
      let data2 = data.cursor_x;
      if (!(typeof data2 == "number" && isFinite(data2)) && data2 !== null) {
        const err17 = { instancePath: instancePath + "/cursor_x", schemaPath: "#/properties/cursor_x/type", keyword: "type", params: { type: schema53.properties.cursor_x.type }, message: "must be number,null" };
        if (vErrors === null) {
          vErrors = [err17];
        } else {
          vErrors.push(err17);
        }
        errors++;
      }
      if (typeof data2 == "number" && isFinite(data2)) {
        if (!formats0.validate(data2)) {
          const err18 = { instancePath: instancePath + "/cursor_x", schemaPath: "#/properties/cursor_x/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err18];
          } else {
            vErrors.push(err18);
          }
          errors++;
        }
      }
    }
    if (data.equal_axis_scale !== void 0) {
      if (typeof data.equal_axis_scale !== "boolean") {
        const err19 = { instancePath: instancePath + "/equal_axis_scale", schemaPath: "#/properties/equal_axis_scale/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
    }
    if (data.markers !== void 0) {
      let data4 = data.markers;
      if (Array.isArray(data4)) {
        const len1 = data4.length;
        for (let i1 = 0; i1 < len1; i1++) {
          if (!validate122(data4[i1], { instancePath: instancePath + "/markers/" + i1, parentData: data4, parentDataProperty: i1, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate122.errors : vErrors.concat(validate122.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err20 = { instancePath: instancePath + "/markers", schemaPath: "#/properties/markers/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err20];
        } else {
          vErrors.push(err20);
        }
        errors++;
      }
    }
    if (data.presentation !== void 0) {
      if (!validate125(data.presentation, { instancePath: instancePath + "/presentation", parentData: data, parentDataProperty: "presentation", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate125.errors : vErrors.concat(validate125.errors);
        errors = vErrors.length;
      }
    }
    if (data.reference_lines !== void 0) {
      let data7 = data.reference_lines;
      if (Array.isArray(data7)) {
        const len2 = data7.length;
        for (let i2 = 0; i2 < len2; i2++) {
          if (!validate152(data7[i2], { instancePath: instancePath + "/reference_lines/" + i2, parentData: data7, parentDataProperty: i2, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate152.errors : vErrors.concat(validate152.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err21 = { instancePath: instancePath + "/reference_lines", schemaPath: "#/properties/reference_lines/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err21];
        } else {
          vErrors.push(err21);
        }
        errors++;
      }
    }
    if (data.series !== void 0) {
      let data9 = data.series;
      if (Array.isArray(data9)) {
        const len3 = data9.length;
        for (let i3 = 0; i3 < len3; i3++) {
          if (!validate154(data9[i3], { instancePath: instancePath + "/series/" + i3, parentData: data9, parentDataProperty: i3, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate154.errors : vErrors.concat(validate154.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err22 = { instancePath: instancePath + "/series", schemaPath: "#/properties/series/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err22];
        } else {
          vErrors.push(err22);
        }
        errors++;
      }
    }
    if (data.time_markers !== void 0) {
      let data11 = data.time_markers;
      if (Array.isArray(data11)) {
        const len4 = data11.length;
        for (let i4 = 0; i4 < len4; i4++) {
          if (!validate122(data11[i4], { instancePath: instancePath + "/time_markers/" + i4, parentData: data11, parentDataProperty: i4, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate122.errors : vErrors.concat(validate122.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err23 = { instancePath: instancePath + "/time_markers", schemaPath: "#/properties/time_markers/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err23];
        } else {
          vErrors.push(err23);
        }
        errors++;
      }
    }
    if (data.vectors !== void 0) {
      let data13 = data.vectors;
      if (Array.isArray(data13)) {
        const len5 = data13.length;
        for (let i5 = 0; i5 < len5; i5++) {
          if (!validate158(data13[i5], { instancePath: instancePath + "/vectors/" + i5, parentData: data13, parentDataProperty: i5, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate158.errors : vErrors.concat(validate158.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err24 = { instancePath: instancePath + "/vectors", schemaPath: "#/properties/vectors/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err24];
        } else {
          vErrors.push(err24);
        }
        errors++;
      }
    }
    if (data.x_axis_label !== void 0) {
      if (typeof data.x_axis_label !== "string") {
        const err25 = { instancePath: instancePath + "/x_axis_label", schemaPath: "#/properties/x_axis_label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err25];
        } else {
          vErrors.push(err25);
        }
        errors++;
      }
    }
    if (data.x_maximum !== void 0) {
      let data16 = data.x_maximum;
      if (typeof data16 == "number" && isFinite(data16)) {
        if (!formats0.validate(data16)) {
          const err26 = { instancePath: instancePath + "/x_maximum", schemaPath: "#/properties/x_maximum/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err26];
          } else {
            vErrors.push(err26);
          }
          errors++;
        }
      } else {
        const err27 = { instancePath: instancePath + "/x_maximum", schemaPath: "#/properties/x_maximum/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err27];
        } else {
          vErrors.push(err27);
        }
        errors++;
      }
    }
    if (data.x_minimum !== void 0) {
      let data17 = data.x_minimum;
      if (typeof data17 == "number" && isFinite(data17)) {
        if (!formats0.validate(data17)) {
          const err28 = { instancePath: instancePath + "/x_minimum", schemaPath: "#/properties/x_minimum/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err28];
          } else {
            vErrors.push(err28);
          }
          errors++;
        }
      } else {
        const err29 = { instancePath: instancePath + "/x_minimum", schemaPath: "#/properties/x_minimum/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err29];
        } else {
          vErrors.push(err29);
        }
        errors++;
      }
    }
    if (data.y_axis_label !== void 0) {
      if (typeof data.y_axis_label !== "string") {
        const err30 = { instancePath: instancePath + "/y_axis_label", schemaPath: "#/properties/y_axis_label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err30];
        } else {
          vErrors.push(err30);
        }
        errors++;
      }
    }
    if (data.y_maximum !== void 0) {
      let data19 = data.y_maximum;
      if (typeof data19 == "number" && isFinite(data19)) {
        if (!formats0.validate(data19)) {
          const err31 = { instancePath: instancePath + "/y_maximum", schemaPath: "#/properties/y_maximum/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err31];
          } else {
            vErrors.push(err31);
          }
          errors++;
        }
      } else {
        const err32 = { instancePath: instancePath + "/y_maximum", schemaPath: "#/properties/y_maximum/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err32];
        } else {
          vErrors.push(err32);
        }
        errors++;
      }
    }
    if (data.y_minimum !== void 0) {
      let data20 = data.y_minimum;
      if (typeof data20 == "number" && isFinite(data20)) {
        if (!formats0.validate(data20)) {
          const err33 = { instancePath: instancePath + "/y_minimum", schemaPath: "#/properties/y_minimum/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err33];
          } else {
            vErrors.push(err33);
          }
          errors++;
        }
      } else {
        const err34 = { instancePath: instancePath + "/y_minimum", schemaPath: "#/properties/y_minimum/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err34];
        } else {
          vErrors.push(err34);
        }
        errors++;
      }
    }
  } else {
    const err35 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err35];
    } else {
      vErrors.push(err35);
    }
    errors++;
  }
  validate117.errors = vErrors;
  return errors === 0;
}
validate117.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate78(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate78.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.action === void 0) {
      const err1 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "action" }, message: "must have required property 'action'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.presentation === void 0) {
      const err2 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "presentation" }, message: "must have required property 'presentation'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "action" || key0 === "kind" || key0 === "presentation")) {
        const err3 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.action !== void 0) {
      if (!validate79(data.action, { instancePath: instancePath + "/action", parentData: data, parentDataProperty: "action", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate79.errors : vErrors.concat(validate79.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data1 = data.kind;
      if (typeof data1 !== "string") {
        const err4 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
      if ("button" !== data1) {
        const err5 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "button" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.presentation !== void 0) {
      if (!validate109(data.presentation, { instancePath: instancePath + "/presentation", parentData: data, parentDataProperty: "presentation", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate109.errors : vErrors.concat(validate109.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err6 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props2 = true;
  }
  const _errs8 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err7 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.cue === void 0) {
      const err8 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "cue" }, message: "must have required property 'cue'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.value === void 0) {
      const err9 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "value" }, message: "must have required property 'value'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "cue" || key1 === "kind" || key1 === "value")) {
        const err10 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
    if (data.cue !== void 0) {
      if (!validate89(data.cue, { instancePath: instancePath + "/cue", parentData: data, parentDataProperty: "cue", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate89.errors : vErrors.concat(validate89.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data4 = data.kind;
      if (typeof data4 !== "string") {
        const err11 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
      if ("toggle" !== data4) {
        const err12 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "toggle" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
    }
    if (data.value !== void 0) {
      if (typeof data.value !== "boolean") {
        const err13 = { instancePath: instancePath + "/value", schemaPath: "#/oneOf/1/properties/value/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      }
    }
  } else {
    const err14 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err14];
    } else {
      vErrors.push(err14);
    }
    errors++;
  }
  var _valid0 = _errs8 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props2 !== true) {
        props2 = true;
      }
    }
    const _errs16 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err15 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
      if (data.target === void 0) {
        const err16 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "target" }, message: "must have required property 'target'" };
        if (vErrors === null) {
          vErrors = [err16];
        } else {
          vErrors.push(err16);
        }
        errors++;
      }
      if (data.value === void 0) {
        const err17 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "value" }, message: "must have required property 'value'" };
        if (vErrors === null) {
          vErrors = [err17];
        } else {
          vErrors.push(err17);
        }
        errors++;
      }
      if (data.display_value === void 0) {
        const err18 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "display_value" }, message: "must have required property 'display_value'" };
        if (vErrors === null) {
          vErrors = [err18];
        } else {
          vErrors.push(err18);
        }
        errors++;
      }
      if (data.minimum === void 0) {
        const err19 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "minimum" }, message: "must have required property 'minimum'" };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
      if (data.maximum === void 0) {
        const err20 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "maximum" }, message: "must have required property 'maximum'" };
        if (vErrors === null) {
          vErrors = [err20];
        } else {
          vErrors.push(err20);
        }
        errors++;
      }
      if (data.step === void 0) {
        const err21 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "step" }, message: "must have required property 'step'" };
        if (vErrors === null) {
          vErrors = [err21];
        } else {
          vErrors.push(err21);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "display_value" || key2 === "kind" || key2 === "maximum" || key2 === "minimum" || key2 === "step" || key2 === "target" || key2 === "value")) {
          const err22 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err22];
          } else {
            vErrors.push(err22);
          }
          errors++;
        }
      }
      if (data.display_value !== void 0) {
        if (typeof data.display_value !== "string") {
          const err23 = { instancePath: instancePath + "/display_value", schemaPath: "#/oneOf/2/properties/display_value/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err23];
          } else {
            vErrors.push(err23);
          }
          errors++;
        }
      }
      if (data.kind !== void 0) {
        let data7 = data.kind;
        if (typeof data7 !== "string") {
          const err24 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err24];
          } else {
            vErrors.push(err24);
          }
          errors++;
        }
        if ("range" !== data7) {
          const err25 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "range" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err25];
          } else {
            vErrors.push(err25);
          }
          errors++;
        }
      }
      if (data.maximum !== void 0) {
        let data8 = data.maximum;
        if (typeof data8 == "number" && isFinite(data8)) {
          if (!formats0.validate(data8)) {
            const err26 = { instancePath: instancePath + "/maximum", schemaPath: "#/oneOf/2/properties/maximum/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
            if (vErrors === null) {
              vErrors = [err26];
            } else {
              vErrors.push(err26);
            }
            errors++;
          }
        } else {
          const err27 = { instancePath: instancePath + "/maximum", schemaPath: "#/oneOf/2/properties/maximum/type", keyword: "type", params: { type: "number" }, message: "must be number" };
          if (vErrors === null) {
            vErrors = [err27];
          } else {
            vErrors.push(err27);
          }
          errors++;
        }
      }
      if (data.minimum !== void 0) {
        let data9 = data.minimum;
        if (typeof data9 == "number" && isFinite(data9)) {
          if (!formats0.validate(data9)) {
            const err28 = { instancePath: instancePath + "/minimum", schemaPath: "#/oneOf/2/properties/minimum/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
            if (vErrors === null) {
              vErrors = [err28];
            } else {
              vErrors.push(err28);
            }
            errors++;
          }
        } else {
          const err29 = { instancePath: instancePath + "/minimum", schemaPath: "#/oneOf/2/properties/minimum/type", keyword: "type", params: { type: "number" }, message: "must be number" };
          if (vErrors === null) {
            vErrors = [err29];
          } else {
            vErrors.push(err29);
          }
          errors++;
        }
      }
      if (data.step !== void 0) {
        let data10 = data.step;
        if (typeof data10 == "number" && isFinite(data10)) {
          if (!formats0.validate(data10)) {
            const err30 = { instancePath: instancePath + "/step", schemaPath: "#/oneOf/2/properties/step/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
            if (vErrors === null) {
              vErrors = [err30];
            } else {
              vErrors.push(err30);
            }
            errors++;
          }
        } else {
          const err31 = { instancePath: instancePath + "/step", schemaPath: "#/oneOf/2/properties/step/type", keyword: "type", params: { type: "number" }, message: "must be number" };
          if (vErrors === null) {
            vErrors = [err31];
          } else {
            vErrors.push(err31);
          }
          errors++;
        }
      }
      if (data.target !== void 0) {
        if (!validate114(data.target, { instancePath: instancePath + "/target", parentData: data, parentDataProperty: "target", rootData, dynamicAnchors })) {
          vErrors = vErrors === null ? validate114.errors : vErrors.concat(validate114.errors);
          errors = vErrors.length;
        }
      }
      if (data.value !== void 0) {
        let data12 = data.value;
        if (typeof data12 == "number" && isFinite(data12)) {
          if (!formats0.validate(data12)) {
            const err32 = { instancePath: instancePath + "/value", schemaPath: "#/oneOf/2/properties/value/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
            if (vErrors === null) {
              vErrors = [err32];
            } else {
              vErrors.push(err32);
            }
            errors++;
          }
        } else {
          const err33 = { instancePath: instancePath + "/value", schemaPath: "#/oneOf/2/properties/value/type", keyword: "type", params: { type: "number" }, message: "must be number" };
          if (vErrors === null) {
            vErrors = [err33];
          } else {
            vErrors.push(err33);
          }
          errors++;
        }
      }
    } else {
      const err34 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err34];
      } else {
        vErrors.push(err34);
      }
      errors++;
    }
    var _valid0 = _errs16 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props2 !== true) {
          props2 = true;
        }
      }
      const _errs32 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.kind === void 0) {
          const err35 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
          if (vErrors === null) {
            vErrors = [err35];
          } else {
            vErrors.push(err35);
          }
          errors++;
        }
        if (data.value === void 0) {
          const err36 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "value" }, message: "must have required property 'value'" };
          if (vErrors === null) {
            vErrors = [err36];
          } else {
            vErrors.push(err36);
          }
          errors++;
        }
        for (const key3 in data) {
          if (!(key3 === "kind" || key3 === "value")) {
            const err37 = { instancePath, schemaPath: "#/oneOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err37];
            } else {
              vErrors.push(err37);
            }
            errors++;
          }
        }
        if (data.kind !== void 0) {
          let data13 = data.kind;
          if (typeof data13 !== "string") {
            const err38 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err38];
            } else {
              vErrors.push(err38);
            }
            errors++;
          }
          if ("status" !== data13) {
            const err39 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/const", keyword: "const", params: { allowedValue: "status" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err39];
            } else {
              vErrors.push(err39);
            }
            errors++;
          }
        }
        if (data.value !== void 0) {
          if (typeof data.value !== "string") {
            const err40 = { instancePath: instancePath + "/value", schemaPath: "#/oneOf/3/properties/value/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err40];
            } else {
              vErrors.push(err40);
            }
            errors++;
          }
        }
      } else {
        const err41 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err41];
        } else {
          vErrors.push(err41);
        }
        errors++;
      }
      var _valid0 = _errs32 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props2 !== true) {
            props2 = true;
          }
        }
        const _errs39 = errors;
        if (data && typeof data == "object" && !Array.isArray(data)) {
          if (data.kind === void 0) {
            const err42 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
            if (vErrors === null) {
              vErrors = [err42];
            } else {
              vErrors.push(err42);
            }
            errors++;
          }
          if (data.chart === void 0) {
            const err43 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "chart" }, message: "must have required property 'chart'" };
            if (vErrors === null) {
              vErrors = [err43];
            } else {
              vErrors.push(err43);
            }
            errors++;
          }
          if (data.plot === void 0) {
            const err44 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "plot" }, message: "must have required property 'plot'" };
            if (vErrors === null) {
              vErrors = [err44];
            } else {
              vErrors.push(err44);
            }
            errors++;
          }
          for (const key4 in data) {
            if (!(key4 === "chart" || key4 === "kind" || key4 === "plot")) {
              const err45 = { instancePath, schemaPath: "#/oneOf/4/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key4 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err45];
              } else {
                vErrors.push(err45);
              }
              errors++;
            }
          }
          if (data.chart !== void 0) {
            if (!validate102(data.chart, { instancePath: instancePath + "/chart", parentData: data, parentDataProperty: "chart", rootData, dynamicAnchors })) {
              vErrors = vErrors === null ? validate102.errors : vErrors.concat(validate102.errors);
              errors = vErrors.length;
            }
          }
          if (data.kind !== void 0) {
            let data16 = data.kind;
            if (typeof data16 !== "string") {
              const err46 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err46];
              } else {
                vErrors.push(err46);
              }
              errors++;
            }
            if ("chart" !== data16) {
              const err47 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/const", keyword: "const", params: { allowedValue: "chart" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err47];
              } else {
                vErrors.push(err47);
              }
              errors++;
            }
          }
          if (data.plot !== void 0) {
            if (!validate117(data.plot, { instancePath: instancePath + "/plot", parentData: data, parentDataProperty: "plot", rootData, dynamicAnchors })) {
              vErrors = vErrors === null ? validate117.errors : vErrors.concat(validate117.errors);
              errors = vErrors.length;
            }
          }
        } else {
          const err48 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "object" }, message: "must be object" };
          if (vErrors === null) {
            vErrors = [err48];
          } else {
            vErrors.push(err48);
          }
          errors++;
        }
        var _valid0 = _errs39 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props2 !== true) {
              props2 = true;
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err49 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err49];
    } else {
      vErrors.push(err49);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate78.errors = vErrors;
  evaluated0.props = props2;
  return errors === 0;
}
validate78.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate163(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate163.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.x === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "x" }, message: "must have required property 'x'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.y === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "y" }, message: "must have required property 'y'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.width === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "width" }, message: "must have required property 'width'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.height === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "height" }, message: "must have required property 'height'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "height" || key0 === "width" || key0 === "x" || key0 === "y")) {
        const err4 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.height !== void 0) {
      let data0 = data.height;
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err5 = { instancePath: instancePath + "/height", schemaPath: "#/properties/height/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        }
      } else {
        const err6 = { instancePath: instancePath + "/height", schemaPath: "#/properties/height/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
    if (data.width !== void 0) {
      let data1 = data.width;
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err7 = { instancePath: instancePath + "/width", schemaPath: "#/properties/width/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err7];
          } else {
            vErrors.push(err7);
          }
          errors++;
        }
      } else {
        const err8 = { instancePath: instancePath + "/width", schemaPath: "#/properties/width/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.x !== void 0) {
      let data2 = data.x;
      if (typeof data2 == "number" && isFinite(data2)) {
        if (!formats0.validate(data2)) {
          const err9 = { instancePath: instancePath + "/x", schemaPath: "#/properties/x/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
      } else {
        const err10 = { instancePath: instancePath + "/x", schemaPath: "#/properties/x/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
    if (data.y !== void 0) {
      let data3 = data.y;
      if (typeof data3 == "number" && isFinite(data3)) {
        if (!formats0.validate(data3)) {
          const err11 = { instancePath: instancePath + "/y", schemaPath: "#/properties/y/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err11];
          } else {
            vErrors.push(err11);
          }
          errors++;
        }
      } else {
        const err12 = { instancePath: instancePath + "/y", schemaPath: "#/properties/y/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
    }
  } else {
    const err13 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err13];
    } else {
      vErrors.push(err13);
    }
    errors++;
  }
  validate163.errors = vErrors;
  return errors === 0;
}
validate163.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate77(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate77.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.id === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.label === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.enabled === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "enabled" }, message: "must have required property 'enabled'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.section === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "section" }, message: "must have required property 'section'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.rect === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "rect" }, message: "must have required property 'rect'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.content === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "content" }, message: "must have required property 'content'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "content" || key0 === "enabled" || key0 === "id" || key0 === "label" || key0 === "rect" || key0 === "section")) {
        const err6 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
    if (data.content !== void 0) {
      if (!validate78(data.content, { instancePath: instancePath + "/content", parentData: data, parentDataProperty: "content", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate78.errors : vErrors.concat(validate78.errors);
        errors = vErrors.length;
      }
    }
    if (data.enabled !== void 0) {
      if (typeof data.enabled !== "boolean") {
        const err7 = { instancePath: instancePath + "/enabled", schemaPath: "#/properties/enabled/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
    if (data.id !== void 0) {
      if (typeof data.id !== "string") {
        const err8 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err9 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    if (data.rect !== void 0) {
      if (!validate163(data.rect, { instancePath: instancePath + "/rect", parentData: data, parentDataProperty: "rect", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate163.errors : vErrors.concat(validate163.errors);
        errors = vErrors.length;
      }
    }
    if (data.section !== void 0) {
      if (typeof data.section !== "string") {
        const err10 = { instancePath: instancePath + "/section", schemaPath: "#/properties/section/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
  } else {
    const err11 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
  }
  validate77.errors = vErrors;
  return errors === 0;
}
validate77.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate167(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate167.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("column" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "column" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("choice_grid" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "choice_grid" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("row" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "row" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("normalized" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "normalized" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
      }
    }
  }
  if (!valid0) {
    const err8 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate167.errors = vErrors;
  return errors === 0;
}
validate167.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate166(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate166.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.id === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.label === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.layout === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "layout" }, message: "must have required property 'layout'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "id" || key0 === "label" || key0 === "layout")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.id !== void 0) {
      if (typeof data.id !== "string") {
        const err4 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err5 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.layout !== void 0) {
      if (!validate167(data.layout, { instancePath: instancePath + "/layout", parentData: data, parentDataProperty: "layout", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate167.errors : vErrors.concat(validate167.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err6 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  }
  validate166.errors = vErrors;
  return errors === 0;
}
validate166.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate74(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate74.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.id === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.title === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "title" }, message: "must have required property 'title'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.anchor === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "anchor" }, message: "must have required property 'anchor'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.local_position === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "local_position" }, message: "must have required property 'local_position'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.size === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "size" }, message: "must have required property 'size'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.sections === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "sections" }, message: "must have required property 'sections'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.controls === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "controls" }, message: "must have required property 'controls'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "anchor" || key0 === "controls" || key0 === "id" || key0 === "local_position" || key0 === "sections" || key0 === "size" || key0 === "title")) {
        const err7 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
    if (data.anchor !== void 0) {
      if (!validate75(data.anchor, { instancePath: instancePath + "/anchor", parentData: data, parentDataProperty: "anchor", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate75.errors : vErrors.concat(validate75.errors);
        errors = vErrors.length;
      }
    }
    if (data.controls !== void 0) {
      let data1 = data.controls;
      if (Array.isArray(data1)) {
        const len0 = data1.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate77(data1[i0], { instancePath: instancePath + "/controls/" + i0, parentData: data1, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate77.errors : vErrors.concat(validate77.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err8 = { instancePath: instancePath + "/controls", schemaPath: "#/properties/controls/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.id !== void 0) {
      if (typeof data.id !== "string") {
        const err9 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    if (data.local_position !== void 0) {
      let data4 = data.local_position;
      if (Array.isArray(data4)) {
        if (data4.length > 3) {
          const err10 = { instancePath: instancePath + "/local_position", schemaPath: "#/properties/local_position/maxItems", keyword: "maxItems", params: { limit: 3 }, message: "must NOT have more than 3 items" };
          if (vErrors === null) {
            vErrors = [err10];
          } else {
            vErrors.push(err10);
          }
          errors++;
        }
        if (data4.length < 3) {
          const err11 = { instancePath: instancePath + "/local_position", schemaPath: "#/properties/local_position/minItems", keyword: "minItems", params: { limit: 3 }, message: "must NOT have fewer than 3 items" };
          if (vErrors === null) {
            vErrors = [err11];
          } else {
            vErrors.push(err11);
          }
          errors++;
        }
        const len1 = data4.length;
        for (let i1 = 0; i1 < len1; i1++) {
          let data5 = data4[i1];
          if (typeof data5 == "number" && isFinite(data5)) {
            if (!formats0.validate(data5)) {
              const err12 = { instancePath: instancePath + "/local_position/" + i1, schemaPath: "#/properties/local_position/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err12];
              } else {
                vErrors.push(err12);
              }
              errors++;
            }
          } else {
            const err13 = { instancePath: instancePath + "/local_position/" + i1, schemaPath: "#/properties/local_position/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err13];
            } else {
              vErrors.push(err13);
            }
            errors++;
          }
        }
      } else {
        const err14 = { instancePath: instancePath + "/local_position", schemaPath: "#/properties/local_position/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err14];
        } else {
          vErrors.push(err14);
        }
        errors++;
      }
    }
    if (data.sections !== void 0) {
      let data6 = data.sections;
      if (Array.isArray(data6)) {
        const len2 = data6.length;
        for (let i2 = 0; i2 < len2; i2++) {
          if (!validate166(data6[i2], { instancePath: instancePath + "/sections/" + i2, parentData: data6, parentDataProperty: i2, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate166.errors : vErrors.concat(validate166.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err15 = { instancePath: instancePath + "/sections", schemaPath: "#/properties/sections/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
    }
    if (data.size !== void 0) {
      let data8 = data.size;
      if (Array.isArray(data8)) {
        if (data8.length > 2) {
          const err16 = { instancePath: instancePath + "/size", schemaPath: "#/properties/size/maxItems", keyword: "maxItems", params: { limit: 2 }, message: "must NOT have more than 2 items" };
          if (vErrors === null) {
            vErrors = [err16];
          } else {
            vErrors.push(err16);
          }
          errors++;
        }
        if (data8.length < 2) {
          const err17 = { instancePath: instancePath + "/size", schemaPath: "#/properties/size/minItems", keyword: "minItems", params: { limit: 2 }, message: "must NOT have fewer than 2 items" };
          if (vErrors === null) {
            vErrors = [err17];
          } else {
            vErrors.push(err17);
          }
          errors++;
        }
        const len3 = data8.length;
        for (let i3 = 0; i3 < len3; i3++) {
          let data9 = data8[i3];
          if (typeof data9 == "number" && isFinite(data9)) {
            if (!formats0.validate(data9)) {
              const err18 = { instancePath: instancePath + "/size/" + i3, schemaPath: "#/properties/size/items/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
              if (vErrors === null) {
                vErrors = [err18];
              } else {
                vErrors.push(err18);
              }
              errors++;
            }
          } else {
            const err19 = { instancePath: instancePath + "/size/" + i3, schemaPath: "#/properties/size/items/type", keyword: "type", params: { type: "number" }, message: "must be number" };
            if (vErrors === null) {
              vErrors = [err19];
            } else {
              vErrors.push(err19);
            }
            errors++;
          }
        }
      } else {
        const err20 = { instancePath: instancePath + "/size", schemaPath: "#/properties/size/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err20];
        } else {
          vErrors.push(err20);
        }
        errors++;
      }
    }
    if (data.title !== void 0) {
      if (typeof data.title !== "string") {
        const err21 = { instancePath: instancePath + "/title", schemaPath: "#/properties/title/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err21];
        } else {
          vErrors.push(err21);
        }
        errors++;
      }
    }
  } else {
    const err22 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err22];
    } else {
      vErrors.push(err22);
    }
    errors++;
  }
  validate74.errors = vErrors;
  return errors === 0;
}
validate74.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate171(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate171.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("Boot" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "Boot" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("Title" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "Title" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("FlightSetup" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "FlightSetup" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("Briefing" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "Briefing" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("Countdown" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "Countdown" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
          const _errs11 = errors;
          if (typeof data !== "string") {
            const err10 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err10];
            } else {
              vErrors.push(err10);
            }
            errors++;
          }
          if ("Flight" !== data) {
            const err11 = { instancePath, schemaPath: "#/oneOf/5/const", keyword: "const", params: { allowedValue: "Flight" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err11];
            } else {
              vErrors.push(err11);
            }
            errors++;
          }
          var _valid0 = _errs11 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
            }
            const _errs13 = errors;
            if (typeof data !== "string") {
              const err12 = { instancePath, schemaPath: "#/oneOf/6/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err12];
              } else {
                vErrors.push(err12);
              }
              errors++;
            }
            if ("Result" !== data) {
              const err13 = { instancePath, schemaPath: "#/oneOf/6/const", keyword: "const", params: { allowedValue: "Result" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err13];
              } else {
                vErrors.push(err13);
              }
              errors++;
            }
            var _valid0 = _errs13 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
              }
              const _errs15 = errors;
              if (typeof data !== "string") {
                const err14 = { instancePath, schemaPath: "#/oneOf/7/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err14];
                } else {
                  vErrors.push(err14);
                }
                errors++;
              }
              if ("Replay" !== data) {
                const err15 = { instancePath, schemaPath: "#/oneOf/7/const", keyword: "const", params: { allowedValue: "Replay" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err15];
                } else {
                  vErrors.push(err15);
                }
                errors++;
              }
              var _valid0 = _errs15 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err16 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err16];
    } else {
      vErrors.push(err16);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate171.errors = vErrors;
  return errors === 0;
}
validate171.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate173(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate173.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.technical_open === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "technical_open" }, message: "must have required property 'technical_open'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.result_tab === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "result_tab" }, message: "must have required property 'result_tab'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.analysis_chart === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "analysis_chart" }, message: "must have required property 'analysis_chart'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.analysis_time_seconds === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "analysis_time_seconds" }, message: "must have required property 'analysis_time_seconds'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.pause_overlay === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "pause_overlay" }, message: "must have required property 'pause_overlay'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.replay_view === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "replay_view" }, message: "must have required property 'replay_view'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.replay_camera === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "replay_camera" }, message: "must have required property 'replay_camera'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "analysis_chart" || key0 === "analysis_time_seconds" || key0 === "pause_overlay" || key0 === "replay_camera" || key0 === "replay_view" || key0 === "result_tab" || key0 === "technical_open")) {
        const err7 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
    if (data.analysis_chart !== void 0) {
      if (!validate102(data.analysis_chart, { instancePath: instancePath + "/analysis_chart", parentData: data, parentDataProperty: "analysis_chart", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate102.errors : vErrors.concat(validate102.errors);
        errors = vErrors.length;
      }
    }
    if (data.analysis_time_seconds !== void 0) {
      let data1 = data.analysis_time_seconds;
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err8 = { instancePath: instancePath + "/analysis_time_seconds", schemaPath: "#/properties/analysis_time_seconds/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
      } else {
        const err9 = { instancePath: instancePath + "/analysis_time_seconds", schemaPath: "#/properties/analysis_time_seconds/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    if (data.pause_overlay !== void 0) {
      if (!validate104(data.pause_overlay, { instancePath: instancePath + "/pause_overlay", parentData: data, parentDataProperty: "pause_overlay", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate104.errors : vErrors.concat(validate104.errors);
        errors = vErrors.length;
      }
    }
    if (data.replay_camera !== void 0) {
      if (!validate97(data.replay_camera, { instancePath: instancePath + "/replay_camera", parentData: data, parentDataProperty: "replay_camera", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate97.errors : vErrors.concat(validate97.errors);
        errors = vErrors.length;
      }
    }
    if (data.replay_view !== void 0) {
      if (!validate106(data.replay_view, { instancePath: instancePath + "/replay_view", parentData: data, parentDataProperty: "replay_view", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate106.errors : vErrors.concat(validate106.errors);
        errors = vErrors.length;
      }
    }
    if (data.result_tab !== void 0) {
      if (!validate100(data.result_tab, { instancePath: instancePath + "/result_tab", parentData: data, parentDataProperty: "result_tab", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate100.errors : vErrors.concat(validate100.errors);
        errors = vErrors.length;
      }
    }
    if (data.technical_open !== void 0) {
      if (typeof data.technical_open !== "boolean") {
        const err10 = { instancePath: instancePath + "/technical_open", schemaPath: "#/properties/technical_open/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
  } else {
    const err11 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
  }
  validate173.errors = vErrors;
  return errors === 0;
}
validate173.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var schema72 = { "additionalProperties": false, "description": "Engine-independent colors, opacity and minimum hit dimensions.", "properties": { "button_min_height_px": { "description": "Ordinary button minimum height.", "format": "double", "type": "number" }, "choice_background": { "description": "Unselected choice fill.", "type": "string" }, "focus_outline": { "description": "Keyboard focus outline.", "type": "string" }, "hud_opacity": { "description": "Head-relative HUD background opacity, preserving forward visibility.", "format": "double", "type": "number" }, "muted_text": { "description": "Secondary text color.", "type": "string" }, "normal_background": { "description": "Ordinary action fill.", "type": "string" }, "panel_background": { "description": "Panel background color.", "type": "string" }, "panel_opacity": { "description": "Centered menu opacity.", "format": "double", "type": "number" }, "primary_background": { "description": "Primary or selected action fill.", "type": "string" }, "primary_min_height_px": { "description": "Principal button minimum height.", "format": "double", "type": "number" }, "text": { "description": "Main text color.", "type": "string" } }, "required": ["text", "muted_text", "panel_background", "panel_opacity", "hud_opacity", "normal_background", "primary_background", "choice_background", "focus_outline", "button_min_height_px", "primary_min_height_px"], "type": "object" };
function validate180(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate180.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.text === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "text" }, message: "must have required property 'text'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.muted_text === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "muted_text" }, message: "must have required property 'muted_text'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.panel_background === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "panel_background" }, message: "must have required property 'panel_background'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.panel_opacity === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "panel_opacity" }, message: "must have required property 'panel_opacity'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.hud_opacity === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "hud_opacity" }, message: "must have required property 'hud_opacity'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.normal_background === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "normal_background" }, message: "must have required property 'normal_background'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.primary_background === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "primary_background" }, message: "must have required property 'primary_background'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.choice_background === void 0) {
      const err7 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "choice_background" }, message: "must have required property 'choice_background'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.focus_outline === void 0) {
      const err8 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "focus_outline" }, message: "must have required property 'focus_outline'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.button_min_height_px === void 0) {
      const err9 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "button_min_height_px" }, message: "must have required property 'button_min_height_px'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    if (data.primary_min_height_px === void 0) {
      const err10 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "primary_min_height_px" }, message: "must have required property 'primary_min_height_px'" };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!func1.call(schema72.properties, key0)) {
        const err11 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
    }
    if (data.button_min_height_px !== void 0) {
      let data0 = data.button_min_height_px;
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err12 = { instancePath: instancePath + "/button_min_height_px", schemaPath: "#/properties/button_min_height_px/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err12];
          } else {
            vErrors.push(err12);
          }
          errors++;
        }
      } else {
        const err13 = { instancePath: instancePath + "/button_min_height_px", schemaPath: "#/properties/button_min_height_px/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      }
    }
    if (data.choice_background !== void 0) {
      if (typeof data.choice_background !== "string") {
        const err14 = { instancePath: instancePath + "/choice_background", schemaPath: "#/properties/choice_background/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err14];
        } else {
          vErrors.push(err14);
        }
        errors++;
      }
    }
    if (data.focus_outline !== void 0) {
      if (typeof data.focus_outline !== "string") {
        const err15 = { instancePath: instancePath + "/focus_outline", schemaPath: "#/properties/focus_outline/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
    }
    if (data.hud_opacity !== void 0) {
      let data3 = data.hud_opacity;
      if (typeof data3 == "number" && isFinite(data3)) {
        if (!formats0.validate(data3)) {
          const err16 = { instancePath: instancePath + "/hud_opacity", schemaPath: "#/properties/hud_opacity/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err16];
          } else {
            vErrors.push(err16);
          }
          errors++;
        }
      } else {
        const err17 = { instancePath: instancePath + "/hud_opacity", schemaPath: "#/properties/hud_opacity/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err17];
        } else {
          vErrors.push(err17);
        }
        errors++;
      }
    }
    if (data.muted_text !== void 0) {
      if (typeof data.muted_text !== "string") {
        const err18 = { instancePath: instancePath + "/muted_text", schemaPath: "#/properties/muted_text/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err18];
        } else {
          vErrors.push(err18);
        }
        errors++;
      }
    }
    if (data.normal_background !== void 0) {
      if (typeof data.normal_background !== "string") {
        const err19 = { instancePath: instancePath + "/normal_background", schemaPath: "#/properties/normal_background/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
    }
    if (data.panel_background !== void 0) {
      if (typeof data.panel_background !== "string") {
        const err20 = { instancePath: instancePath + "/panel_background", schemaPath: "#/properties/panel_background/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err20];
        } else {
          vErrors.push(err20);
        }
        errors++;
      }
    }
    if (data.panel_opacity !== void 0) {
      let data7 = data.panel_opacity;
      if (typeof data7 == "number" && isFinite(data7)) {
        if (!formats0.validate(data7)) {
          const err21 = { instancePath: instancePath + "/panel_opacity", schemaPath: "#/properties/panel_opacity/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err21];
          } else {
            vErrors.push(err21);
          }
          errors++;
        }
      } else {
        const err22 = { instancePath: instancePath + "/panel_opacity", schemaPath: "#/properties/panel_opacity/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err22];
        } else {
          vErrors.push(err22);
        }
        errors++;
      }
    }
    if (data.primary_background !== void 0) {
      if (typeof data.primary_background !== "string") {
        const err23 = { instancePath: instancePath + "/primary_background", schemaPath: "#/properties/primary_background/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err23];
        } else {
          vErrors.push(err23);
        }
        errors++;
      }
    }
    if (data.primary_min_height_px !== void 0) {
      let data9 = data.primary_min_height_px;
      if (typeof data9 == "number" && isFinite(data9)) {
        if (!formats0.validate(data9)) {
          const err24 = { instancePath: instancePath + "/primary_min_height_px", schemaPath: "#/properties/primary_min_height_px/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err24];
          } else {
            vErrors.push(err24);
          }
          errors++;
        }
      } else {
        const err25 = { instancePath: instancePath + "/primary_min_height_px", schemaPath: "#/properties/primary_min_height_px/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err25];
        } else {
          vErrors.push(err25);
        }
        errors++;
      }
    }
    if (data.text !== void 0) {
      if (typeof data.text !== "string") {
        const err26 = { instancePath: instancePath + "/text", schemaPath: "#/properties/text/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err26];
        } else {
          vErrors.push(err26);
        }
        errors++;
      }
    }
  } else {
    const err27 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err27];
    } else {
      vErrors.push(err27);
    }
    errors++;
  }
  validate180.errors = vErrors;
  return errors === 0;
}
validate180.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate53(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate53.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.schema_version === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "schema_version" }, message: "must have required property 'schema_version'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.view_revision === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "view_revision" }, message: "must have required property 'view_revision'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.selections === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "selections" }, message: "must have required property 'selections'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.scene === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "scene" }, message: "must have required property 'scene'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.title === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "title" }, message: "must have required property 'title'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.description === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "description" }, message: "must have required property 'description'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.active_overlay === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "active_overlay" }, message: "must have required property 'active_overlay'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.panels === void 0) {
      const err7 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "panels" }, message: "must have required property 'panels'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.layout === void 0) {
      const err8 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "layout" }, message: "must have required property 'layout'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.theme === void 0) {
      const err9 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "theme" }, message: "must have required property 'theme'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    if (data.hud === void 0) {
      const err10 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "hud" }, message: "must have required property 'hud'" };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!func1.call(schema20.properties, key0)) {
        const err11 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
    }
    if (data.active_overlay !== void 0) {
      let data0 = data.active_overlay;
      const _errs3 = errors;
      let valid1 = false;
      const _errs4 = errors;
      if (!validate54(data0, { instancePath: instancePath + "/active_overlay", parentData: data, parentDataProperty: "active_overlay", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate54.errors : vErrors.concat(validate54.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs4 === errors;
      valid1 = valid1 || _valid0;
      const _errs5 = errors;
      if (data0 !== null) {
        const err12 = { instancePath: instancePath + "/active_overlay", schemaPath: "#/properties/active_overlay/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
      var _valid0 = _errs5 === errors;
      valid1 = valid1 || _valid0;
      if (!valid1) {
        const err13 = { instancePath: instancePath + "/active_overlay", schemaPath: "#/properties/active_overlay/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      } else {
        errors = _errs3;
        if (vErrors !== null) {
          if (_errs3) {
            vErrors.length = _errs3;
          } else {
            vErrors = null;
          }
        }
      }
    }
    if (data.description !== void 0) {
      if (typeof data.description !== "string") {
        const err14 = { instancePath: instancePath + "/description", schemaPath: "#/properties/description/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err14];
        } else {
          vErrors.push(err14);
        }
        errors++;
      }
    }
    if (data.hud !== void 0) {
      let data2 = data.hud;
      const _errs10 = errors;
      let valid2 = false;
      const _errs11 = errors;
      if (!validate56(data2, { instancePath: instancePath + "/hud", parentData: data, parentDataProperty: "hud", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate56.errors : vErrors.concat(validate56.errors);
        errors = vErrors.length;
      }
      var _valid1 = _errs11 === errors;
      valid2 = valid2 || _valid1;
      const _errs12 = errors;
      if (data2 !== null) {
        const err15 = { instancePath: instancePath + "/hud", schemaPath: "#/properties/hud/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
      var _valid1 = _errs12 === errors;
      valid2 = valid2 || _valid1;
      if (!valid2) {
        const err16 = { instancePath: instancePath + "/hud", schemaPath: "#/properties/hud/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
        if (vErrors === null) {
          vErrors = [err16];
        } else {
          vErrors.push(err16);
        }
        errors++;
      } else {
        errors = _errs10;
        if (vErrors !== null) {
          if (_errs10) {
            vErrors.length = _errs10;
          } else {
            vErrors = null;
          }
        }
      }
    }
    if (data.layout !== void 0) {
      if (!validate70(data.layout, { instancePath: instancePath + "/layout", parentData: data, parentDataProperty: "layout", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate70.errors : vErrors.concat(validate70.errors);
        errors = vErrors.length;
      }
    }
    if (data.panels !== void 0) {
      let data4 = data.panels;
      if (Array.isArray(data4)) {
        const len0 = data4.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate74(data4[i0], { instancePath: instancePath + "/panels/" + i0, parentData: data4, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate74.errors : vErrors.concat(validate74.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err17 = { instancePath: instancePath + "/panels", schemaPath: "#/properties/panels/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err17];
        } else {
          vErrors.push(err17);
        }
        errors++;
      }
    }
    if (data.scene !== void 0) {
      if (!validate171(data.scene, { instancePath: instancePath + "/scene", parentData: data, parentDataProperty: "scene", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate171.errors : vErrors.concat(validate171.errors);
        errors = vErrors.length;
      }
    }
    if (data.schema_version !== void 0) {
      let data7 = data.schema_version;
      if (!(typeof data7 == "number" && (!(data7 % 1) && !isNaN(data7)) && isFinite(data7))) {
        const err18 = { instancePath: instancePath + "/schema_version", schemaPath: "#/properties/schema_version/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
        if (vErrors === null) {
          vErrors = [err18];
        } else {
          vErrors.push(err18);
        }
        errors++;
      }
      if (typeof data7 == "number" && isFinite(data7)) {
        if (data7 < 0 || isNaN(data7)) {
          const err19 = { instancePath: instancePath + "/schema_version", schemaPath: "#/properties/schema_version/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
          if (vErrors === null) {
            vErrors = [err19];
          } else {
            vErrors.push(err19);
          }
          errors++;
        }
        if (!formats28.validate(data7)) {
          const err20 = { instancePath: instancePath + "/schema_version", schemaPath: "#/properties/schema_version/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
          if (vErrors === null) {
            vErrors = [err20];
          } else {
            vErrors.push(err20);
          }
          errors++;
        }
      }
    }
    if (data.selections !== void 0) {
      if (!validate173(data.selections, { instancePath: instancePath + "/selections", parentData: data, parentDataProperty: "selections", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate173.errors : vErrors.concat(validate173.errors);
        errors = vErrors.length;
      }
    }
    if (data.theme !== void 0) {
      if (!validate180(data.theme, { instancePath: instancePath + "/theme", parentData: data, parentDataProperty: "theme", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate180.errors : vErrors.concat(validate180.errors);
        errors = vErrors.length;
      }
    }
    if (data.title !== void 0) {
      if (typeof data.title !== "string") {
        const err21 = { instancePath: instancePath + "/title", schemaPath: "#/properties/title/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err21];
        } else {
          vErrors.push(err21);
        }
        errors++;
      }
    }
    if (data.view_revision !== void 0) {
      let data11 = data.view_revision;
      if (!(typeof data11 == "number" && (!(data11 % 1) && !isNaN(data11)) && isFinite(data11))) {
        const err22 = { instancePath: instancePath + "/view_revision", schemaPath: "#/properties/view_revision/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
        if (vErrors === null) {
          vErrors = [err22];
        } else {
          vErrors.push(err22);
        }
        errors++;
      }
      if (typeof data11 == "number" && isFinite(data11)) {
        if (data11 < 0 || isNaN(data11)) {
          const err23 = { instancePath: instancePath + "/view_revision", schemaPath: "#/properties/view_revision/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
          if (vErrors === null) {
            vErrors = [err23];
          } else {
            vErrors.push(err23);
          }
          errors++;
        }
        if (!formats28.validate(data11)) {
          const err24 = { instancePath: instancePath + "/view_revision", schemaPath: "#/properties/view_revision/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
          if (vErrors === null) {
            vErrors = [err24];
          } else {
            vErrors.push(err24);
          }
          errors++;
        }
      }
    }
  } else {
    const err25 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err25];
    } else {
      vErrors.push(err25);
    }
    errors++;
  }
  validate53.errors = vErrors;
  return errors === 0;
}
validate53.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var validateGameUiAction = validate183;
function validate185(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate185.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("beginner" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "beginner" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("standard" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "standard" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("expert" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "expert" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("realistic" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "realistic" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("custom" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "custom" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate185.errors = vErrors;
  return errors === 0;
}
validate185.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate187(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate187.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("full" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "full" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("standard" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "standard" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("minimal" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "minimal" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("realistic" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "realistic" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("custom" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "custom" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate187.errors = vErrors;
  return errors === 0;
}
validate187.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate189(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate189.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("strong" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "strong" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("assisted" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "assisted" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("light" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "light" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("manual" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "manual" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
      }
    }
  }
  if (!valid0) {
    const err8 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate189.errors = vErrors;
  return errors === 0;
}
validate189.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate191(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate191.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("calm" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "calm" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("mild" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "mild" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("typical" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "typical" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("challenging" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "challenging" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("near_limit" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "near_limit" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err10 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate191.errors = vErrors;
  return errors === 0;
}
validate191.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate193(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate193.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("telemetry" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "telemetry" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("attitude" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "attitude" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("wind" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "wind" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("flight_path" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "flight_path" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("angle_of_attack" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "angle_of_attack" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
          const _errs11 = errors;
          if (typeof data !== "string") {
            const err10 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err10];
            } else {
              vErrors.push(err10);
            }
            errors++;
          }
          if ("warnings" !== data) {
            const err11 = { instancePath, schemaPath: "#/oneOf/5/const", keyword: "const", params: { allowedValue: "warnings" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err11];
            } else {
              vErrors.push(err11);
            }
            errors++;
          }
          var _valid0 = _errs11 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err12 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err12];
    } else {
      vErrors.push(err12);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate193.errors = vErrors;
  return errors === 0;
}
validate193.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate184(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate184.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "kind")) {
        const err1 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err1];
        } else {
          vErrors.push(err1);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data0 = data.kind;
      if (typeof data0 !== "string") {
        const err2 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
      if ("open_setup" !== data0) {
        const err3 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "open_setup" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
  } else {
    const err4 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err4];
    } else {
      vErrors.push(err4);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs6 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err5 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "kind")) {
        const err6 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data1 = data.kind;
      if (typeof data1 !== "string") {
        const err7 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      if ("return_to_title" !== data1) {
        const err8 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "return_to_title" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
  } else {
    const err9 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err9];
    } else {
      vErrors.push(err9);
    }
    errors++;
  }
  var _valid0 = _errs6 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs11 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err10 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "kind")) {
          const err11 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err11];
          } else {
            vErrors.push(err11);
          }
          errors++;
        }
      }
      if (data.kind !== void 0) {
        let data2 = data.kind;
        if (typeof data2 !== "string") {
          const err12 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err12];
          } else {
            vErrors.push(err12);
          }
          errors++;
        }
        if ("prepare" !== data2) {
          const err13 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "prepare" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err13];
          } else {
            vErrors.push(err13);
          }
          errors++;
        }
      }
    } else {
      const err14 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err14];
      } else {
        vErrors.push(err14);
      }
      errors++;
    }
    var _valid0 = _errs11 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
      const _errs16 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.kind === void 0) {
          const err15 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
          if (vErrors === null) {
            vErrors = [err15];
          } else {
            vErrors.push(err15);
          }
          errors++;
        }
        for (const key3 in data) {
          if (!(key3 === "kind")) {
            const err16 = { instancePath, schemaPath: "#/oneOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err16];
            } else {
              vErrors.push(err16);
            }
            errors++;
          }
        }
        if (data.kind !== void 0) {
          let data3 = data.kind;
          if (typeof data3 !== "string") {
            const err17 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err17];
            } else {
              vErrors.push(err17);
            }
            errors++;
          }
          if ("cancel_briefing" !== data3) {
            const err18 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/const", keyword: "const", params: { allowedValue: "cancel_briefing" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err18];
            } else {
              vErrors.push(err18);
            }
            errors++;
          }
        }
      } else {
        const err19 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
      var _valid0 = _errs16 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
        const _errs21 = errors;
        if (data && typeof data == "object" && !Array.isArray(data)) {
          if (data.kind === void 0) {
            const err20 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
            if (vErrors === null) {
              vErrors = [err20];
            } else {
              vErrors.push(err20);
            }
            errors++;
          }
          for (const key4 in data) {
            if (!(key4 === "kind")) {
              const err21 = { instancePath, schemaPath: "#/oneOf/4/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key4 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err21];
              } else {
                vErrors.push(err21);
              }
              errors++;
            }
          }
          if (data.kind !== void 0) {
            let data4 = data.kind;
            if (typeof data4 !== "string") {
              const err22 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err22];
              } else {
                vErrors.push(err22);
              }
              errors++;
            }
            if ("start_countdown" !== data4) {
              const err23 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/const", keyword: "const", params: { allowedValue: "start_countdown" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err23];
              } else {
                vErrors.push(err23);
              }
              errors++;
            }
          }
        } else {
          const err24 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "object" }, message: "must be object" };
          if (vErrors === null) {
            vErrors = [err24];
          } else {
            vErrors.push(err24);
          }
          errors++;
        }
        var _valid0 = _errs21 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props0 !== true) {
              props0 = true;
            }
          }
          const _errs26 = errors;
          if (data && typeof data == "object" && !Array.isArray(data)) {
            if (data.kind === void 0) {
              const err25 = { instancePath, schemaPath: "#/oneOf/5/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
              if (vErrors === null) {
                vErrors = [err25];
              } else {
                vErrors.push(err25);
              }
              errors++;
            }
            for (const key5 in data) {
              if (!(key5 === "kind")) {
                const err26 = { instancePath, schemaPath: "#/oneOf/5/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key5 }, message: "must NOT have additional properties" };
                if (vErrors === null) {
                  vErrors = [err26];
                } else {
                  vErrors.push(err26);
                }
                errors++;
              }
            }
            if (data.kind !== void 0) {
              let data5 = data.kind;
              if (typeof data5 !== "string") {
                const err27 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err27];
                } else {
                  vErrors.push(err27);
                }
                errors++;
              }
              if ("cancel_countdown" !== data5) {
                const err28 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/const", keyword: "const", params: { allowedValue: "cancel_countdown" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err28];
                } else {
                  vErrors.push(err28);
                }
                errors++;
              }
            }
          } else {
            const err29 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "object" }, message: "must be object" };
            if (vErrors === null) {
              vErrors = [err29];
            } else {
              vErrors.push(err29);
            }
            errors++;
          }
          var _valid0 = _errs26 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true) {
                props0 = true;
              }
            }
            const _errs31 = errors;
            if (data && typeof data == "object" && !Array.isArray(data)) {
              if (data.kind === void 0) {
                const err30 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                if (vErrors === null) {
                  vErrors = [err30];
                } else {
                  vErrors.push(err30);
                }
                errors++;
              }
              for (const key6 in data) {
                if (!(key6 === "kind")) {
                  const err31 = { instancePath, schemaPath: "#/oneOf/6/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key6 }, message: "must NOT have additional properties" };
                  if (vErrors === null) {
                    vErrors = [err31];
                  } else {
                    vErrors.push(err31);
                  }
                  errors++;
                }
              }
              if (data.kind !== void 0) {
                let data6 = data.kind;
                if (typeof data6 !== "string") {
                  const err32 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err32];
                  } else {
                    vErrors.push(err32);
                  }
                  errors++;
                }
                if ("pause" !== data6) {
                  const err33 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/const", keyword: "const", params: { allowedValue: "pause" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err33];
                  } else {
                    vErrors.push(err33);
                  }
                  errors++;
                }
              }
            } else {
              const err34 = { instancePath, schemaPath: "#/oneOf/6/type", keyword: "type", params: { type: "object" }, message: "must be object" };
              if (vErrors === null) {
                vErrors = [err34];
              } else {
                vErrors.push(err34);
              }
              errors++;
            }
            var _valid0 = _errs31 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props0 !== true) {
                  props0 = true;
                }
              }
              const _errs36 = errors;
              if (data && typeof data == "object" && !Array.isArray(data)) {
                if (data.kind === void 0) {
                  const err35 = { instancePath, schemaPath: "#/oneOf/7/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                  if (vErrors === null) {
                    vErrors = [err35];
                  } else {
                    vErrors.push(err35);
                  }
                  errors++;
                }
                for (const key7 in data) {
                  if (!(key7 === "kind")) {
                    const err36 = { instancePath, schemaPath: "#/oneOf/7/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key7 }, message: "must NOT have additional properties" };
                    if (vErrors === null) {
                      vErrors = [err36];
                    } else {
                      vErrors.push(err36);
                    }
                    errors++;
                  }
                }
                if (data.kind !== void 0) {
                  let data7 = data.kind;
                  if (typeof data7 !== "string") {
                    const err37 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err37];
                    } else {
                      vErrors.push(err37);
                    }
                    errors++;
                  }
                  if ("resume" !== data7) {
                    const err38 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/const", keyword: "const", params: { allowedValue: "resume" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err38];
                    } else {
                      vErrors.push(err38);
                    }
                    errors++;
                  }
                }
              } else {
                const err39 = { instancePath, schemaPath: "#/oneOf/7/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                if (vErrors === null) {
                  vErrors = [err39];
                } else {
                  vErrors.push(err39);
                }
                errors++;
              }
              var _valid0 = _errs36 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props0 !== true) {
                    props0 = true;
                  }
                }
                const _errs41 = errors;
                if (data && typeof data == "object" && !Array.isArray(data)) {
                  if (data.kind === void 0) {
                    const err40 = { instancePath, schemaPath: "#/oneOf/8/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                    if (vErrors === null) {
                      vErrors = [err40];
                    } else {
                      vErrors.push(err40);
                    }
                    errors++;
                  }
                  for (const key8 in data) {
                    if (!(key8 === "kind")) {
                      const err41 = { instancePath, schemaPath: "#/oneOf/8/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key8 }, message: "must NOT have additional properties" };
                      if (vErrors === null) {
                        vErrors = [err41];
                      } else {
                        vErrors.push(err41);
                      }
                      errors++;
                    }
                  }
                  if (data.kind !== void 0) {
                    let data8 = data.kind;
                    if (typeof data8 !== "string") {
                      const err42 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/8/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err42];
                      } else {
                        vErrors.push(err42);
                      }
                      errors++;
                    }
                    if ("abort" !== data8) {
                      const err43 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/8/properties/kind/const", keyword: "const", params: { allowedValue: "abort" }, message: "must be equal to constant" };
                      if (vErrors === null) {
                        vErrors = [err43];
                      } else {
                        vErrors.push(err43);
                      }
                      errors++;
                    }
                  }
                } else {
                  const err44 = { instancePath, schemaPath: "#/oneOf/8/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                  if (vErrors === null) {
                    vErrors = [err44];
                  } else {
                    vErrors.push(err44);
                  }
                  errors++;
                }
                var _valid0 = _errs41 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                    if (props0 !== true) {
                      props0 = true;
                    }
                  }
                  const _errs46 = errors;
                  if (data && typeof data == "object" && !Array.isArray(data)) {
                    if (data.kind === void 0) {
                      const err45 = { instancePath, schemaPath: "#/oneOf/9/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                      if (vErrors === null) {
                        vErrors = [err45];
                      } else {
                        vErrors.push(err45);
                      }
                      errors++;
                    }
                    for (const key9 in data) {
                      if (!(key9 === "kind")) {
                        const err46 = { instancePath, schemaPath: "#/oneOf/9/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key9 }, message: "must NOT have additional properties" };
                        if (vErrors === null) {
                          vErrors = [err46];
                        } else {
                          vErrors.push(err46);
                        }
                        errors++;
                      }
                    }
                    if (data.kind !== void 0) {
                      let data9 = data.kind;
                      if (typeof data9 !== "string") {
                        const err47 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/9/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                        if (vErrors === null) {
                          vErrors = [err47];
                        } else {
                          vErrors.push(err47);
                        }
                        errors++;
                      }
                      if ("retry" !== data9) {
                        const err48 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/9/properties/kind/const", keyword: "const", params: { allowedValue: "retry" }, message: "must be equal to constant" };
                        if (vErrors === null) {
                          vErrors = [err48];
                        } else {
                          vErrors.push(err48);
                        }
                        errors++;
                      }
                    }
                  } else {
                    const err49 = { instancePath, schemaPath: "#/oneOf/9/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                    if (vErrors === null) {
                      vErrors = [err49];
                    } else {
                      vErrors.push(err49);
                    }
                    errors++;
                  }
                  var _valid0 = _errs46 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                      if (props0 !== true) {
                        props0 = true;
                      }
                    }
                    const _errs51 = errors;
                    if (data && typeof data == "object" && !Array.isArray(data)) {
                      if (data.kind === void 0) {
                        const err50 = { instancePath, schemaPath: "#/oneOf/10/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                        if (vErrors === null) {
                          vErrors = [err50];
                        } else {
                          vErrors.push(err50);
                        }
                        errors++;
                      }
                      for (const key10 in data) {
                        if (!(key10 === "kind")) {
                          const err51 = { instancePath, schemaPath: "#/oneOf/10/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key10 }, message: "must NOT have additional properties" };
                          if (vErrors === null) {
                            vErrors = [err51];
                          } else {
                            vErrors.push(err51);
                          }
                          errors++;
                        }
                      }
                      if (data.kind !== void 0) {
                        let data10 = data.kind;
                        if (typeof data10 !== "string") {
                          const err52 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/10/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                          if (vErrors === null) {
                            vErrors = [err52];
                          } else {
                            vErrors.push(err52);
                          }
                          errors++;
                        }
                        if ("retry_briefing" !== data10) {
                          const err53 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/10/properties/kind/const", keyword: "const", params: { allowedValue: "retry_briefing" }, message: "must be equal to constant" };
                          if (vErrors === null) {
                            vErrors = [err53];
                          } else {
                            vErrors.push(err53);
                          }
                          errors++;
                        }
                      }
                    } else {
                      const err54 = { instancePath, schemaPath: "#/oneOf/10/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                      if (vErrors === null) {
                        vErrors = [err54];
                      } else {
                        vErrors.push(err54);
                      }
                      errors++;
                    }
                    var _valid0 = _errs51 === errors;
                    if (_valid0 && valid0) {
                      valid0 = false;
                      passing0 = [passing0, 10];
                    } else {
                      if (_valid0) {
                        valid0 = true;
                        passing0 = 10;
                        if (props0 !== true) {
                          props0 = true;
                        }
                      }
                      const _errs56 = errors;
                      if (data && typeof data == "object" && !Array.isArray(data)) {
                        if (data.kind === void 0) {
                          const err55 = { instancePath, schemaPath: "#/oneOf/11/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                          if (vErrors === null) {
                            vErrors = [err55];
                          } else {
                            vErrors.push(err55);
                          }
                          errors++;
                        }
                        for (const key11 in data) {
                          if (!(key11 === "kind")) {
                            const err56 = { instancePath, schemaPath: "#/oneOf/11/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key11 }, message: "must NOT have additional properties" };
                            if (vErrors === null) {
                              vErrors = [err56];
                            } else {
                              vErrors.push(err56);
                            }
                            errors++;
                          }
                        }
                        if (data.kind !== void 0) {
                          let data11 = data.kind;
                          if (typeof data11 !== "string") {
                            const err57 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/11/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                            if (vErrors === null) {
                              vErrors = [err57];
                            } else {
                              vErrors.push(err57);
                            }
                            errors++;
                          }
                          if ("enter_replay" !== data11) {
                            const err58 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/11/properties/kind/const", keyword: "const", params: { allowedValue: "enter_replay" }, message: "must be equal to constant" };
                            if (vErrors === null) {
                              vErrors = [err58];
                            } else {
                              vErrors.push(err58);
                            }
                            errors++;
                          }
                        }
                      } else {
                        const err59 = { instancePath, schemaPath: "#/oneOf/11/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                        if (vErrors === null) {
                          vErrors = [err59];
                        } else {
                          vErrors.push(err59);
                        }
                        errors++;
                      }
                      var _valid0 = _errs56 === errors;
                      if (_valid0 && valid0) {
                        valid0 = false;
                        passing0 = [passing0, 11];
                      } else {
                        if (_valid0) {
                          valid0 = true;
                          passing0 = 11;
                          if (props0 !== true) {
                            props0 = true;
                          }
                        }
                        const _errs61 = errors;
                        if (data && typeof data == "object" && !Array.isArray(data)) {
                          if (data.kind === void 0) {
                            const err60 = { instancePath, schemaPath: "#/oneOf/12/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                            if (vErrors === null) {
                              vErrors = [err60];
                            } else {
                              vErrors.push(err60);
                            }
                            errors++;
                          }
                          for (const key12 in data) {
                            if (!(key12 === "kind")) {
                              const err61 = { instancePath, schemaPath: "#/oneOf/12/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key12 }, message: "must NOT have additional properties" };
                              if (vErrors === null) {
                                vErrors = [err61];
                              } else {
                                vErrors.push(err61);
                              }
                              errors++;
                            }
                          }
                          if (data.kind !== void 0) {
                            let data12 = data.kind;
                            if (typeof data12 !== "string") {
                              const err62 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/12/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                              if (vErrors === null) {
                                vErrors = [err62];
                              } else {
                                vErrors.push(err62);
                              }
                              errors++;
                            }
                            if ("leave_replay" !== data12) {
                              const err63 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/12/properties/kind/const", keyword: "const", params: { allowedValue: "leave_replay" }, message: "must be equal to constant" };
                              if (vErrors === null) {
                                vErrors = [err63];
                              } else {
                                vErrors.push(err63);
                              }
                              errors++;
                            }
                          }
                        } else {
                          const err64 = { instancePath, schemaPath: "#/oneOf/12/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                          if (vErrors === null) {
                            vErrors = [err64];
                          } else {
                            vErrors.push(err64);
                          }
                          errors++;
                        }
                        var _valid0 = _errs61 === errors;
                        if (_valid0 && valid0) {
                          valid0 = false;
                          passing0 = [passing0, 12];
                        } else {
                          if (_valid0) {
                            valid0 = true;
                            passing0 = 12;
                            if (props0 !== true) {
                              props0 = true;
                            }
                          }
                          const _errs66 = errors;
                          if (data && typeof data == "object" && !Array.isArray(data)) {
                            if (data.kind === void 0) {
                              const err65 = { instancePath, schemaPath: "#/oneOf/13/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                              if (vErrors === null) {
                                vErrors = [err65];
                              } else {
                                vErrors.push(err65);
                              }
                              errors++;
                            }
                            for (const key13 in data) {
                              if (!(key13 === "kind")) {
                                const err66 = { instancePath, schemaPath: "#/oneOf/13/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key13 }, message: "must NOT have additional properties" };
                                if (vErrors === null) {
                                  vErrors = [err66];
                                } else {
                                  vErrors.push(err66);
                                }
                                errors++;
                              }
                            }
                            if (data.kind !== void 0) {
                              let data13 = data.kind;
                              if (typeof data13 !== "string") {
                                const err67 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/13/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                if (vErrors === null) {
                                  vErrors = [err67];
                                } else {
                                  vErrors.push(err67);
                                }
                                errors++;
                              }
                              if ("enter_attract" !== data13) {
                                const err68 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/13/properties/kind/const", keyword: "const", params: { allowedValue: "enter_attract" }, message: "must be equal to constant" };
                                if (vErrors === null) {
                                  vErrors = [err68];
                                } else {
                                  vErrors.push(err68);
                                }
                                errors++;
                              }
                            }
                          } else {
                            const err69 = { instancePath, schemaPath: "#/oneOf/13/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                            if (vErrors === null) {
                              vErrors = [err69];
                            } else {
                              vErrors.push(err69);
                            }
                            errors++;
                          }
                          var _valid0 = _errs66 === errors;
                          if (_valid0 && valid0) {
                            valid0 = false;
                            passing0 = [passing0, 13];
                          } else {
                            if (_valid0) {
                              valid0 = true;
                              passing0 = 13;
                              if (props0 !== true) {
                                props0 = true;
                              }
                            }
                            const _errs71 = errors;
                            if (data && typeof data == "object" && !Array.isArray(data)) {
                              if (data.kind === void 0) {
                                const err70 = { instancePath, schemaPath: "#/oneOf/14/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                if (vErrors === null) {
                                  vErrors = [err70];
                                } else {
                                  vErrors.push(err70);
                                }
                                errors++;
                              }
                              for (const key14 in data) {
                                if (!(key14 === "kind")) {
                                  const err71 = { instancePath, schemaPath: "#/oneOf/14/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key14 }, message: "must NOT have additional properties" };
                                  if (vErrors === null) {
                                    vErrors = [err71];
                                  } else {
                                    vErrors.push(err71);
                                  }
                                  errors++;
                                }
                              }
                              if (data.kind !== void 0) {
                                let data14 = data.kind;
                                if (typeof data14 !== "string") {
                                  const err72 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/14/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                  if (vErrors === null) {
                                    vErrors = [err72];
                                  } else {
                                    vErrors.push(err72);
                                  }
                                  errors++;
                                }
                                if ("leave_attract" !== data14) {
                                  const err73 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/14/properties/kind/const", keyword: "const", params: { allowedValue: "leave_attract" }, message: "must be equal to constant" };
                                  if (vErrors === null) {
                                    vErrors = [err73];
                                  } else {
                                    vErrors.push(err73);
                                  }
                                  errors++;
                                }
                              }
                            } else {
                              const err74 = { instancePath, schemaPath: "#/oneOf/14/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                              if (vErrors === null) {
                                vErrors = [err74];
                              } else {
                                vErrors.push(err74);
                              }
                              errors++;
                            }
                            var _valid0 = _errs71 === errors;
                            if (_valid0 && valid0) {
                              valid0 = false;
                              passing0 = [passing0, 14];
                            } else {
                              if (_valid0) {
                                valid0 = true;
                                passing0 = 14;
                                if (props0 !== true) {
                                  props0 = true;
                                }
                              }
                              const _errs76 = errors;
                              if (data && typeof data == "object" && !Array.isArray(data)) {
                                if (data.kind === void 0) {
                                  const err75 = { instancePath, schemaPath: "#/oneOf/15/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                  if (vErrors === null) {
                                    vErrors = [err75];
                                  } else {
                                    vErrors.push(err75);
                                  }
                                  errors++;
                                }
                                if (data.preset === void 0) {
                                  const err76 = { instancePath, schemaPath: "#/oneOf/15/required", keyword: "required", params: { missingProperty: "preset" }, message: "must have required property 'preset'" };
                                  if (vErrors === null) {
                                    vErrors = [err76];
                                  } else {
                                    vErrors.push(err76);
                                  }
                                  errors++;
                                }
                                for (const key15 in data) {
                                  if (!(key15 === "kind" || key15 === "preset")) {
                                    const err77 = { instancePath, schemaPath: "#/oneOf/15/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key15 }, message: "must NOT have additional properties" };
                                    if (vErrors === null) {
                                      vErrors = [err77];
                                    } else {
                                      vErrors.push(err77);
                                    }
                                    errors++;
                                  }
                                }
                                if (data.kind !== void 0) {
                                  let data15 = data.kind;
                                  if (typeof data15 !== "string") {
                                    const err78 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/15/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                    if (vErrors === null) {
                                      vErrors = [err78];
                                    } else {
                                      vErrors.push(err78);
                                    }
                                    errors++;
                                  }
                                  if ("select_preset" !== data15) {
                                    const err79 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/15/properties/kind/const", keyword: "const", params: { allowedValue: "select_preset" }, message: "must be equal to constant" };
                                    if (vErrors === null) {
                                      vErrors = [err79];
                                    } else {
                                      vErrors.push(err79);
                                    }
                                    errors++;
                                  }
                                }
                                if (data.preset !== void 0) {
                                  if (!validate185(data.preset, { instancePath: instancePath + "/preset", parentData: data, parentDataProperty: "preset", rootData, dynamicAnchors })) {
                                    vErrors = vErrors === null ? validate185.errors : vErrors.concat(validate185.errors);
                                    errors = vErrors.length;
                                  }
                                }
                              } else {
                                const err80 = { instancePath, schemaPath: "#/oneOf/15/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                if (vErrors === null) {
                                  vErrors = [err80];
                                } else {
                                  vErrors.push(err80);
                                }
                                errors++;
                              }
                              var _valid0 = _errs76 === errors;
                              if (_valid0 && valid0) {
                                valid0 = false;
                                passing0 = [passing0, 15];
                              } else {
                                if (_valid0) {
                                  valid0 = true;
                                  passing0 = 15;
                                  if (props0 !== true) {
                                    props0 = true;
                                  }
                                }
                                const _errs82 = errors;
                                if (data && typeof data == "object" && !Array.isArray(data)) {
                                  if (data.kind === void 0) {
                                    const err81 = { instancePath, schemaPath: "#/oneOf/16/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                    if (vErrors === null) {
                                      vErrors = [err81];
                                    } else {
                                      vErrors.push(err81);
                                    }
                                    errors++;
                                  }
                                  if (data.information === void 0) {
                                    const err82 = { instancePath, schemaPath: "#/oneOf/16/required", keyword: "required", params: { missingProperty: "information" }, message: "must have required property 'information'" };
                                    if (vErrors === null) {
                                      vErrors = [err82];
                                    } else {
                                      vErrors.push(err82);
                                    }
                                    errors++;
                                  }
                                  for (const key16 in data) {
                                    if (!(key16 === "information" || key16 === "kind")) {
                                      const err83 = { instancePath, schemaPath: "#/oneOf/16/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key16 }, message: "must NOT have additional properties" };
                                      if (vErrors === null) {
                                        vErrors = [err83];
                                      } else {
                                        vErrors.push(err83);
                                      }
                                      errors++;
                                    }
                                  }
                                  if (data.information !== void 0) {
                                    if (!validate187(data.information, { instancePath: instancePath + "/information", parentData: data, parentDataProperty: "information", rootData, dynamicAnchors })) {
                                      vErrors = vErrors === null ? validate187.errors : vErrors.concat(validate187.errors);
                                      errors = vErrors.length;
                                    }
                                  }
                                  if (data.kind !== void 0) {
                                    let data18 = data.kind;
                                    if (typeof data18 !== "string") {
                                      const err84 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/16/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                      if (vErrors === null) {
                                        vErrors = [err84];
                                      } else {
                                        vErrors.push(err84);
                                      }
                                      errors++;
                                    }
                                    if ("select_information" !== data18) {
                                      const err85 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/16/properties/kind/const", keyword: "const", params: { allowedValue: "select_information" }, message: "must be equal to constant" };
                                      if (vErrors === null) {
                                        vErrors = [err85];
                                      } else {
                                        vErrors.push(err85);
                                      }
                                      errors++;
                                    }
                                  }
                                } else {
                                  const err86 = { instancePath, schemaPath: "#/oneOf/16/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                  if (vErrors === null) {
                                    vErrors = [err86];
                                  } else {
                                    vErrors.push(err86);
                                  }
                                  errors++;
                                }
                                var _valid0 = _errs82 === errors;
                                if (_valid0 && valid0) {
                                  valid0 = false;
                                  passing0 = [passing0, 16];
                                } else {
                                  if (_valid0) {
                                    valid0 = true;
                                    passing0 = 16;
                                    if (props0 !== true) {
                                      props0 = true;
                                    }
                                  }
                                  const _errs88 = errors;
                                  if (data && typeof data == "object" && !Array.isArray(data)) {
                                    if (data.kind === void 0) {
                                      const err87 = { instancePath, schemaPath: "#/oneOf/17/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                      if (vErrors === null) {
                                        vErrors = [err87];
                                      } else {
                                        vErrors.push(err87);
                                      }
                                      errors++;
                                    }
                                    if (data.assistance === void 0) {
                                      const err88 = { instancePath, schemaPath: "#/oneOf/17/required", keyword: "required", params: { missingProperty: "assistance" }, message: "must have required property 'assistance'" };
                                      if (vErrors === null) {
                                        vErrors = [err88];
                                      } else {
                                        vErrors.push(err88);
                                      }
                                      errors++;
                                    }
                                    for (const key17 in data) {
                                      if (!(key17 === "assistance" || key17 === "kind")) {
                                        const err89 = { instancePath, schemaPath: "#/oneOf/17/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key17 }, message: "must NOT have additional properties" };
                                        if (vErrors === null) {
                                          vErrors = [err89];
                                        } else {
                                          vErrors.push(err89);
                                        }
                                        errors++;
                                      }
                                    }
                                    if (data.assistance !== void 0) {
                                      if (!validate189(data.assistance, { instancePath: instancePath + "/assistance", parentData: data, parentDataProperty: "assistance", rootData, dynamicAnchors })) {
                                        vErrors = vErrors === null ? validate189.errors : vErrors.concat(validate189.errors);
                                        errors = vErrors.length;
                                      }
                                    }
                                    if (data.kind !== void 0) {
                                      let data20 = data.kind;
                                      if (typeof data20 !== "string") {
                                        const err90 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/17/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                        if (vErrors === null) {
                                          vErrors = [err90];
                                        } else {
                                          vErrors.push(err90);
                                        }
                                        errors++;
                                      }
                                      if ("select_assistance" !== data20) {
                                        const err91 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/17/properties/kind/const", keyword: "const", params: { allowedValue: "select_assistance" }, message: "must be equal to constant" };
                                        if (vErrors === null) {
                                          vErrors = [err91];
                                        } else {
                                          vErrors.push(err91);
                                        }
                                        errors++;
                                      }
                                    }
                                  } else {
                                    const err92 = { instancePath, schemaPath: "#/oneOf/17/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                    if (vErrors === null) {
                                      vErrors = [err92];
                                    } else {
                                      vErrors.push(err92);
                                    }
                                    errors++;
                                  }
                                  var _valid0 = _errs88 === errors;
                                  if (_valid0 && valid0) {
                                    valid0 = false;
                                    passing0 = [passing0, 17];
                                  } else {
                                    if (_valid0) {
                                      valid0 = true;
                                      passing0 = 17;
                                      if (props0 !== true) {
                                        props0 = true;
                                      }
                                    }
                                    const _errs94 = errors;
                                    if (data && typeof data == "object" && !Array.isArray(data)) {
                                      if (data.kind === void 0) {
                                        const err93 = { instancePath, schemaPath: "#/oneOf/18/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                        if (vErrors === null) {
                                          vErrors = [err93];
                                        } else {
                                          vErrors.push(err93);
                                        }
                                        errors++;
                                      }
                                      if (data.weather === void 0) {
                                        const err94 = { instancePath, schemaPath: "#/oneOf/18/required", keyword: "required", params: { missingProperty: "weather" }, message: "must have required property 'weather'" };
                                        if (vErrors === null) {
                                          vErrors = [err94];
                                        } else {
                                          vErrors.push(err94);
                                        }
                                        errors++;
                                      }
                                      for (const key18 in data) {
                                        if (!(key18 === "kind" || key18 === "weather")) {
                                          const err95 = { instancePath, schemaPath: "#/oneOf/18/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key18 }, message: "must NOT have additional properties" };
                                          if (vErrors === null) {
                                            vErrors = [err95];
                                          } else {
                                            vErrors.push(err95);
                                          }
                                          errors++;
                                        }
                                      }
                                      if (data.kind !== void 0) {
                                        let data21 = data.kind;
                                        if (typeof data21 !== "string") {
                                          const err96 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/18/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                          if (vErrors === null) {
                                            vErrors = [err96];
                                          } else {
                                            vErrors.push(err96);
                                          }
                                          errors++;
                                        }
                                        if ("select_weather" !== data21) {
                                          const err97 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/18/properties/kind/const", keyword: "const", params: { allowedValue: "select_weather" }, message: "must be equal to constant" };
                                          if (vErrors === null) {
                                            vErrors = [err97];
                                          } else {
                                            vErrors.push(err97);
                                          }
                                          errors++;
                                        }
                                      }
                                      if (data.weather !== void 0) {
                                        if (!validate191(data.weather, { instancePath: instancePath + "/weather", parentData: data, parentDataProperty: "weather", rootData, dynamicAnchors })) {
                                          vErrors = vErrors === null ? validate191.errors : vErrors.concat(validate191.errors);
                                          errors = vErrors.length;
                                        }
                                      }
                                    } else {
                                      const err98 = { instancePath, schemaPath: "#/oneOf/18/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                      if (vErrors === null) {
                                        vErrors = [err98];
                                      } else {
                                        vErrors.push(err98);
                                      }
                                      errors++;
                                    }
                                    var _valid0 = _errs94 === errors;
                                    if (_valid0 && valid0) {
                                      valid0 = false;
                                      passing0 = [passing0, 18];
                                    } else {
                                      if (_valid0) {
                                        valid0 = true;
                                        passing0 = 18;
                                        if (props0 !== true) {
                                          props0 = true;
                                        }
                                      }
                                      const _errs100 = errors;
                                      if (data && typeof data == "object" && !Array.isArray(data)) {
                                        if (data.kind === void 0) {
                                          const err99 = { instancePath, schemaPath: "#/oneOf/19/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                                          if (vErrors === null) {
                                            vErrors = [err99];
                                          } else {
                                            vErrors.push(err99);
                                          }
                                          errors++;
                                        }
                                        if (data.cue === void 0) {
                                          const err100 = { instancePath, schemaPath: "#/oneOf/19/required", keyword: "required", params: { missingProperty: "cue" }, message: "must have required property 'cue'" };
                                          if (vErrors === null) {
                                            vErrors = [err100];
                                          } else {
                                            vErrors.push(err100);
                                          }
                                          errors++;
                                        }
                                        if (data.visible === void 0) {
                                          const err101 = { instancePath, schemaPath: "#/oneOf/19/required", keyword: "required", params: { missingProperty: "visible" }, message: "must have required property 'visible'" };
                                          if (vErrors === null) {
                                            vErrors = [err101];
                                          } else {
                                            vErrors.push(err101);
                                          }
                                          errors++;
                                        }
                                        for (const key19 in data) {
                                          if (!(key19 === "cue" || key19 === "kind" || key19 === "visible")) {
                                            const err102 = { instancePath, schemaPath: "#/oneOf/19/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key19 }, message: "must NOT have additional properties" };
                                            if (vErrors === null) {
                                              vErrors = [err102];
                                            } else {
                                              vErrors.push(err102);
                                            }
                                            errors++;
                                          }
                                        }
                                        if (data.cue !== void 0) {
                                          if (!validate193(data.cue, { instancePath: instancePath + "/cue", parentData: data, parentDataProperty: "cue", rootData, dynamicAnchors })) {
                                            vErrors = vErrors === null ? validate193.errors : vErrors.concat(validate193.errors);
                                            errors = vErrors.length;
                                          }
                                        }
                                        if (data.kind !== void 0) {
                                          let data24 = data.kind;
                                          if (typeof data24 !== "string") {
                                            const err103 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/19/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                                            if (vErrors === null) {
                                              vErrors = [err103];
                                            } else {
                                              vErrors.push(err103);
                                            }
                                            errors++;
                                          }
                                          if ("set_cue" !== data24) {
                                            const err104 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/19/properties/kind/const", keyword: "const", params: { allowedValue: "set_cue" }, message: "must be equal to constant" };
                                            if (vErrors === null) {
                                              vErrors = [err104];
                                            } else {
                                              vErrors.push(err104);
                                            }
                                            errors++;
                                          }
                                        }
                                        if (data.visible !== void 0) {
                                          if (typeof data.visible !== "boolean") {
                                            const err105 = { instancePath: instancePath + "/visible", schemaPath: "#/oneOf/19/properties/visible/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
                                            if (vErrors === null) {
                                              vErrors = [err105];
                                            } else {
                                              vErrors.push(err105);
                                            }
                                            errors++;
                                          }
                                        }
                                      } else {
                                        const err106 = { instancePath, schemaPath: "#/oneOf/19/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                                        if (vErrors === null) {
                                          vErrors = [err106];
                                        } else {
                                          vErrors.push(err106);
                                        }
                                        errors++;
                                      }
                                      var _valid0 = _errs100 === errors;
                                      if (_valid0 && valid0) {
                                        valid0 = false;
                                        passing0 = [passing0, 19];
                                      } else {
                                        if (_valid0) {
                                          valid0 = true;
                                          passing0 = 19;
                                          if (props0 !== true) {
                                            props0 = true;
                                          }
                                        }
                                      }
                                    }
                                  }
                                }
                              }
                            }
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err107 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err107];
    } else {
      vErrors.push(err107);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate184.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate184.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate197(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate197.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("low" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "low" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("medium" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "medium" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("high" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "high" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate197.errors = vErrors;
  return errors === 0;
}
validate197.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate199(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate199.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("screen" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "screen" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("webxr" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "webxr" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("phone_vr" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "phone_vr" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate199.errors = vErrors;
  return errors === 0;
}
validate199.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate201(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate201.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("auto" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "auto" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("pilot" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "pilot" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("chase" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "chase" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
      const _errs7 = errors;
      if (typeof data !== "string") {
        const err6 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if ("orbit" !== data) {
        const err7 = { instancePath, schemaPath: "#/oneOf/3/const", keyword: "const", params: { allowedValue: "orbit" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
        }
        const _errs9 = errors;
        if (typeof data !== "string") {
          const err8 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
        if ("platform" !== data) {
          const err9 = { instancePath, schemaPath: "#/oneOf/4/const", keyword: "const", params: { allowedValue: "platform" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err9];
          } else {
            vErrors.push(err9);
          }
          errors++;
        }
        var _valid0 = _errs9 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
          }
          const _errs11 = errors;
          if (typeof data !== "string") {
            const err10 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err10];
            } else {
              vErrors.push(err10);
            }
            errors++;
          }
          if ("shore" !== data) {
            const err11 = { instancePath, schemaPath: "#/oneOf/5/const", keyword: "const", params: { allowedValue: "shore" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err11];
            } else {
              vErrors.push(err11);
            }
            errors++;
          }
          var _valid0 = _errs11 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
            }
            const _errs13 = errors;
            if (typeof data !== "string") {
              const err12 = { instancePath, schemaPath: "#/oneOf/6/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err12];
              } else {
                vErrors.push(err12);
              }
              errors++;
            }
            if ("overhead" !== data) {
              const err13 = { instancePath, schemaPath: "#/oneOf/6/const", keyword: "const", params: { allowedValue: "overhead" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err13];
              } else {
                vErrors.push(err13);
              }
              errors++;
            }
            var _valid0 = _errs13 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
              }
              const _errs15 = errors;
              if (typeof data !== "string") {
                const err14 = { instancePath, schemaPath: "#/oneOf/7/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err14];
                } else {
                  vErrors.push(err14);
                }
                errors++;
              }
              if ("side" !== data) {
                const err15 = { instancePath, schemaPath: "#/oneOf/7/const", keyword: "const", params: { allowedValue: "side" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err15];
                } else {
                  vErrors.push(err15);
                }
                errors++;
              }
              var _valid0 = _errs15 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                }
                const _errs17 = errors;
                if (typeof data !== "string") {
                  const err16 = { instancePath, schemaPath: "#/oneOf/8/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err16];
                  } else {
                    vErrors.push(err16);
                  }
                  errors++;
                }
                if ("front" !== data) {
                  const err17 = { instancePath, schemaPath: "#/oneOf/8/const", keyword: "const", params: { allowedValue: "front" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err17];
                  } else {
                    vErrors.push(err17);
                  }
                  errors++;
                }
                var _valid0 = _errs17 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                  }
                  const _errs19 = errors;
                  if (typeof data !== "string") {
                    const err18 = { instancePath, schemaPath: "#/oneOf/9/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err18];
                    } else {
                      vErrors.push(err18);
                    }
                    errors++;
                  }
                  if ("telephoto" !== data) {
                    const err19 = { instancePath, schemaPath: "#/oneOf/9/const", keyword: "const", params: { allowedValue: "telephoto" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err19];
                    } else {
                      vErrors.push(err19);
                    }
                    errors++;
                  }
                  var _valid0 = _errs19 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err20 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err20];
    } else {
      vErrors.push(err20);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate201.errors = vErrors;
  return errors === 0;
}
validate201.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate196(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate196.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.quality === void 0) {
      const err1 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "quality" }, message: "must have required property 'quality'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "kind" || key0 === "quality")) {
        const err2 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data0 = data.kind;
      if (typeof data0 !== "string") {
        const err3 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
      if ("set_water_quality" !== data0) {
        const err4 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "set_water_quality" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.quality !== void 0) {
      if (!validate197(data.quality, { instancePath: instancePath + "/quality", parentData: data, parentDataProperty: "quality", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate197.errors : vErrors.concat(validate197.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err5 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err5];
    } else {
      vErrors.push(err5);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs7 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err6 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.mode === void 0) {
      const err7 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "mode" }, message: "must have required property 'mode'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "kind" || key1 === "mode")) {
        const err8 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data2 = data.kind;
      if (typeof data2 !== "string") {
        const err9 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
      if ("set_presentation" !== data2) {
        const err10 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "set_presentation" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
    if (data.mode !== void 0) {
      if (!validate199(data.mode, { instancePath: instancePath + "/mode", parentData: data, parentDataProperty: "mode", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate199.errors : vErrors.concat(validate199.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err11 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
  }
  var _valid0 = _errs7 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs13 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err12 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "kind")) {
          const err13 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err13];
          } else {
            vErrors.push(err13);
          }
          errors++;
        }
      }
      if (data.kind !== void 0) {
        let data4 = data.kind;
        if (typeof data4 !== "string") {
          const err14 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
        if ("recenter_phone_tracking" !== data4) {
          const err15 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "recenter_phone_tracking" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err15];
          } else {
            vErrors.push(err15);
          }
          errors++;
        }
      }
    } else {
      const err16 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err16];
      } else {
        vErrors.push(err16);
      }
      errors++;
    }
    var _valid0 = _errs13 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
      const _errs18 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.kind === void 0) {
          const err17 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
          if (vErrors === null) {
            vErrors = [err17];
          } else {
            vErrors.push(err17);
          }
          errors++;
        }
        for (const key3 in data) {
          if (!(key3 === "kind")) {
            const err18 = { instancePath, schemaPath: "#/oneOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err18];
            } else {
              vErrors.push(err18);
            }
            errors++;
          }
        }
        if (data.kind !== void 0) {
          let data5 = data.kind;
          if (typeof data5 !== "string") {
            const err19 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err19];
            } else {
              vErrors.push(err19);
            }
            errors++;
          }
          if ("recenter_menu" !== data5) {
            const err20 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/const", keyword: "const", params: { allowedValue: "recenter_menu" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err20];
            } else {
              vErrors.push(err20);
            }
            errors++;
          }
        }
      } else {
        const err21 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err21];
        } else {
          vErrors.push(err21);
        }
        errors++;
      }
      var _valid0 = _errs18 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
        const _errs23 = errors;
        if (data && typeof data == "object" && !Array.isArray(data)) {
          if (data.kind === void 0) {
            const err22 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
            if (vErrors === null) {
              vErrors = [err22];
            } else {
              vErrors.push(err22);
            }
            errors++;
          }
          for (const key4 in data) {
            if (!(key4 === "kind")) {
              const err23 = { instancePath, schemaPath: "#/oneOf/4/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key4 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err23];
              } else {
                vErrors.push(err23);
              }
              errors++;
            }
          }
          if (data.kind !== void 0) {
            let data6 = data.kind;
            if (typeof data6 !== "string") {
              const err24 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err24];
              } else {
                vErrors.push(err24);
              }
              errors++;
            }
            if ("download_record_json" !== data6) {
              const err25 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/const", keyword: "const", params: { allowedValue: "download_record_json" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err25];
              } else {
                vErrors.push(err25);
              }
              errors++;
            }
          }
        } else {
          const err26 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "object" }, message: "must be object" };
          if (vErrors === null) {
            vErrors = [err26];
          } else {
            vErrors.push(err26);
          }
          errors++;
        }
        var _valid0 = _errs23 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props0 !== true) {
              props0 = true;
            }
          }
          const _errs28 = errors;
          if (data && typeof data == "object" && !Array.isArray(data)) {
            if (data.kind === void 0) {
              const err27 = { instancePath, schemaPath: "#/oneOf/5/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
              if (vErrors === null) {
                vErrors = [err27];
              } else {
                vErrors.push(err27);
              }
              errors++;
            }
            for (const key5 in data) {
              if (!(key5 === "kind")) {
                const err28 = { instancePath, schemaPath: "#/oneOf/5/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key5 }, message: "must NOT have additional properties" };
                if (vErrors === null) {
                  vErrors = [err28];
                } else {
                  vErrors.push(err28);
                }
                errors++;
              }
            }
            if (data.kind !== void 0) {
              let data7 = data.kind;
              if (typeof data7 !== "string") {
                const err29 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err29];
                } else {
                  vErrors.push(err29);
                }
                errors++;
              }
              if ("download_flight_csv" !== data7) {
                const err30 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/const", keyword: "const", params: { allowedValue: "download_flight_csv" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err30];
                } else {
                  vErrors.push(err30);
                }
                errors++;
              }
            }
          } else {
            const err31 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "object" }, message: "must be object" };
            if (vErrors === null) {
              vErrors = [err31];
            } else {
              vErrors.push(err31);
            }
            errors++;
          }
          var _valid0 = _errs28 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props0 !== true) {
                props0 = true;
              }
            }
            const _errs33 = errors;
            if (data && typeof data == "object" && !Array.isArray(data)) {
              if (data.kind === void 0) {
                const err32 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                if (vErrors === null) {
                  vErrors = [err32];
                } else {
                  vErrors.push(err32);
                }
                errors++;
              }
              if (data.id === void 0) {
                const err33 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
                if (vErrors === null) {
                  vErrors = [err33];
                } else {
                  vErrors.push(err33);
                }
                errors++;
              }
              for (const key6 in data) {
                if (!(key6 === "id" || key6 === "kind")) {
                  const err34 = { instancePath, schemaPath: "#/oneOf/6/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key6 }, message: "must NOT have additional properties" };
                  if (vErrors === null) {
                    vErrors = [err34];
                  } else {
                    vErrors.push(err34);
                  }
                  errors++;
                }
              }
              if (data.id !== void 0) {
                let data8 = data.id;
                if (!(typeof data8 == "number" && (!(data8 % 1) && !isNaN(data8)) && isFinite(data8))) {
                  const err35 = { instancePath: instancePath + "/id", schemaPath: "#/oneOf/6/properties/id/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
                  if (vErrors === null) {
                    vErrors = [err35];
                  } else {
                    vErrors.push(err35);
                  }
                  errors++;
                }
                if (typeof data8 == "number" && isFinite(data8)) {
                  if (data8 < 0 || isNaN(data8)) {
                    const err36 = { instancePath: instancePath + "/id", schemaPath: "#/oneOf/6/properties/id/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                    if (vErrors === null) {
                      vErrors = [err36];
                    } else {
                      vErrors.push(err36);
                    }
                    errors++;
                  }
                  if (!formats28.validate(data8)) {
                    const err37 = { instancePath: instancePath + "/id", schemaPath: "#/oneOf/6/properties/id/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
                    if (vErrors === null) {
                      vErrors = [err37];
                    } else {
                      vErrors.push(err37);
                    }
                    errors++;
                  }
                }
              }
              if (data.kind !== void 0) {
                let data9 = data.kind;
                if (typeof data9 !== "string") {
                  const err38 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err38];
                  } else {
                    vErrors.push(err38);
                  }
                  errors++;
                }
                if ("open_stored_record" !== data9) {
                  const err39 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/const", keyword: "const", params: { allowedValue: "open_stored_record" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err39];
                  } else {
                    vErrors.push(err39);
                  }
                  errors++;
                }
              }
            } else {
              const err40 = { instancePath, schemaPath: "#/oneOf/6/type", keyword: "type", params: { type: "object" }, message: "must be object" };
              if (vErrors === null) {
                vErrors = [err40];
              } else {
                vErrors.push(err40);
              }
              errors++;
            }
            var _valid0 = _errs33 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props0 !== true) {
                  props0 = true;
                }
              }
              const _errs40 = errors;
              if (data && typeof data == "object" && !Array.isArray(data)) {
                if (data.kind === void 0) {
                  const err41 = { instancePath, schemaPath: "#/oneOf/7/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                  if (vErrors === null) {
                    vErrors = [err41];
                  } else {
                    vErrors.push(err41);
                  }
                  errors++;
                }
                if (data.camera === void 0) {
                  const err42 = { instancePath, schemaPath: "#/oneOf/7/required", keyword: "required", params: { missingProperty: "camera" }, message: "must have required property 'camera'" };
                  if (vErrors === null) {
                    vErrors = [err42];
                  } else {
                    vErrors.push(err42);
                  }
                  errors++;
                }
                for (const key7 in data) {
                  if (!(key7 === "camera" || key7 === "kind")) {
                    const err43 = { instancePath, schemaPath: "#/oneOf/7/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key7 }, message: "must NOT have additional properties" };
                    if (vErrors === null) {
                      vErrors = [err43];
                    } else {
                      vErrors.push(err43);
                    }
                    errors++;
                  }
                }
                if (data.camera !== void 0) {
                  if (!validate201(data.camera, { instancePath: instancePath + "/camera", parentData: data, parentDataProperty: "camera", rootData, dynamicAnchors })) {
                    vErrors = vErrors === null ? validate201.errors : vErrors.concat(validate201.errors);
                    errors = vErrors.length;
                  }
                }
                if (data.kind !== void 0) {
                  let data11 = data.kind;
                  if (typeof data11 !== "string") {
                    const err44 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err44];
                    } else {
                      vErrors.push(err44);
                    }
                    errors++;
                  }
                  if ("set_replay_camera" !== data11) {
                    const err45 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/const", keyword: "const", params: { allowedValue: "set_replay_camera" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err45];
                    } else {
                      vErrors.push(err45);
                    }
                    errors++;
                  }
                }
              } else {
                const err46 = { instancePath, schemaPath: "#/oneOf/7/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                if (vErrors === null) {
                  vErrors = [err46];
                } else {
                  vErrors.push(err46);
                }
                errors++;
              }
              var _valid0 = _errs40 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props0 !== true) {
                    props0 = true;
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err47 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err47];
    } else {
      vErrors.push(err47);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate196.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate196.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate204(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate204.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("summary" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "summary" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("analysis" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "analysis" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
  }
  if (!valid0) {
    const err4 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err4];
    } else {
      vErrors.push(err4);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate204.errors = vErrors;
  return errors === 0;
}
validate204.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate206(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate206.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("map" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "map" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("altitude" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "altitude" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("speed" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "speed" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate206.errors = vErrors;
  return errors === 0;
}
validate206.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate208(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate208.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("menu" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "menu" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("settings" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "settings" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("help" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "help" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate208.errors = vErrors;
  return errors === 0;
}
validate208.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate210(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate210.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (typeof data !== "string") {
    const err0 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err0];
    } else {
      vErrors.push(err0);
    }
    errors++;
  }
  if ("cinematic" !== data) {
    const err1 = { instancePath, schemaPath: "#/oneOf/0/const", keyword: "const", params: { allowedValue: "cinematic" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err1];
    } else {
      vErrors.push(err1);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
  }
  const _errs3 = errors;
  if (typeof data !== "string") {
    const err2 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "string" }, message: "must be string" };
    if (vErrors === null) {
      vErrors = [err2];
    } else {
      vErrors.push(err2);
    }
    errors++;
  }
  if ("telemetry" !== data) {
    const err3 = { instancePath, schemaPath: "#/oneOf/1/const", keyword: "const", params: { allowedValue: "telemetry" }, message: "must be equal to constant" };
    if (vErrors === null) {
      vErrors = [err3];
    } else {
      vErrors.push(err3);
    }
    errors++;
  }
  var _valid0 = _errs3 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
    }
    const _errs5 = errors;
    if (typeof data !== "string") {
      const err4 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "string" }, message: "must be string" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if ("analysis" !== data) {
      const err5 = { instancePath, schemaPath: "#/oneOf/2/const", keyword: "const", params: { allowedValue: "analysis" }, message: "must be equal to constant" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    var _valid0 = _errs5 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
      }
    }
  }
  if (!valid0) {
    const err6 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err6];
    } else {
      vErrors.push(err6);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate210.errors = vErrors;
  return errors === 0;
}
validate210.evaluated = { "dynamicProps": false, "dynamicItems": false };
function validate183(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate183.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.command === void 0) {
      const err1 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "command" }, message: "must have required property 'command'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "command" || key0 === "kind")) {
        const err2 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.command !== void 0) {
      if (!validate184(data.command, { instancePath: instancePath + "/command", parentData: data, parentDataProperty: "command", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate184.errors : vErrors.concat(validate184.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data1 = data.kind;
      if (typeof data1 !== "string") {
        const err3 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
      if ("session" !== data1) {
        const err4 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "session" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
  } else {
    const err5 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err5];
    } else {
      vErrors.push(err5);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props1 = true;
  }
  const _errs7 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err6 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.command === void 0) {
      const err7 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "command" }, message: "must have required property 'command'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "command" || key1 === "kind")) {
        const err8 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.command !== void 0) {
      if (!validate196(data.command, { instancePath: instancePath + "/command", parentData: data, parentDataProperty: "command", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate196.errors : vErrors.concat(validate196.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data3 = data.kind;
      if (typeof data3 !== "string") {
        const err9 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
      if ("platform" !== data3) {
        const err10 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "platform" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
  } else {
    const err11 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
  }
  var _valid0 = _errs7 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props1 !== true) {
        props1 = true;
      }
    }
    const _errs13 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err12 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "kind")) {
          const err13 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err13];
          } else {
            vErrors.push(err13);
          }
          errors++;
        }
      }
      if (data.kind !== void 0) {
        let data4 = data.kind;
        if (typeof data4 !== "string") {
          const err14 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
        if ("toggle_technical" !== data4) {
          const err15 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "toggle_technical" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err15];
          } else {
            vErrors.push(err15);
          }
          errors++;
        }
      }
    } else {
      const err16 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err16];
      } else {
        vErrors.push(err16);
      }
      errors++;
    }
    var _valid0 = _errs13 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props1 !== true) {
          props1 = true;
        }
      }
      const _errs18 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.kind === void 0) {
          const err17 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
          if (vErrors === null) {
            vErrors = [err17];
          } else {
            vErrors.push(err17);
          }
          errors++;
        }
        if (data.tab === void 0) {
          const err18 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "tab" }, message: "must have required property 'tab'" };
          if (vErrors === null) {
            vErrors = [err18];
          } else {
            vErrors.push(err18);
          }
          errors++;
        }
        for (const key3 in data) {
          if (!(key3 === "kind" || key3 === "tab")) {
            const err19 = { instancePath, schemaPath: "#/oneOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err19];
            } else {
              vErrors.push(err19);
            }
            errors++;
          }
        }
        if (data.kind !== void 0) {
          let data5 = data.kind;
          if (typeof data5 !== "string") {
            const err20 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err20];
            } else {
              vErrors.push(err20);
            }
            errors++;
          }
          if ("set_result_tab" !== data5) {
            const err21 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/const", keyword: "const", params: { allowedValue: "set_result_tab" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err21];
            } else {
              vErrors.push(err21);
            }
            errors++;
          }
        }
        if (data.tab !== void 0) {
          if (!validate204(data.tab, { instancePath: instancePath + "/tab", parentData: data, parentDataProperty: "tab", rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate204.errors : vErrors.concat(validate204.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err22 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err22];
        } else {
          vErrors.push(err22);
        }
        errors++;
      }
      var _valid0 = _errs18 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props1 !== true) {
            props1 = true;
          }
        }
        const _errs24 = errors;
        if (data && typeof data == "object" && !Array.isArray(data)) {
          if (data.kind === void 0) {
            const err23 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
            if (vErrors === null) {
              vErrors = [err23];
            } else {
              vErrors.push(err23);
            }
            errors++;
          }
          if (data.chart === void 0) {
            const err24 = { instancePath, schemaPath: "#/oneOf/4/required", keyword: "required", params: { missingProperty: "chart" }, message: "must have required property 'chart'" };
            if (vErrors === null) {
              vErrors = [err24];
            } else {
              vErrors.push(err24);
            }
            errors++;
          }
          for (const key4 in data) {
            if (!(key4 === "chart" || key4 === "kind")) {
              const err25 = { instancePath, schemaPath: "#/oneOf/4/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key4 }, message: "must NOT have additional properties" };
              if (vErrors === null) {
                vErrors = [err25];
              } else {
                vErrors.push(err25);
              }
              errors++;
            }
          }
          if (data.chart !== void 0) {
            if (!validate206(data.chart, { instancePath: instancePath + "/chart", parentData: data, parentDataProperty: "chart", rootData, dynamicAnchors })) {
              vErrors = vErrors === null ? validate206.errors : vErrors.concat(validate206.errors);
              errors = vErrors.length;
            }
          }
          if (data.kind !== void 0) {
            let data8 = data.kind;
            if (typeof data8 !== "string") {
              const err26 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
              if (vErrors === null) {
                vErrors = [err26];
              } else {
                vErrors.push(err26);
              }
              errors++;
            }
            if ("set_analysis_chart" !== data8) {
              const err27 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/4/properties/kind/const", keyword: "const", params: { allowedValue: "set_analysis_chart" }, message: "must be equal to constant" };
              if (vErrors === null) {
                vErrors = [err27];
              } else {
                vErrors.push(err27);
              }
              errors++;
            }
          }
        } else {
          const err28 = { instancePath, schemaPath: "#/oneOf/4/type", keyword: "type", params: { type: "object" }, message: "must be object" };
          if (vErrors === null) {
            vErrors = [err28];
          } else {
            vErrors.push(err28);
          }
          errors++;
        }
        var _valid0 = _errs24 === errors;
        if (_valid0 && valid0) {
          valid0 = false;
          passing0 = [passing0, 4];
        } else {
          if (_valid0) {
            valid0 = true;
            passing0 = 4;
            if (props1 !== true) {
              props1 = true;
            }
          }
          const _errs30 = errors;
          if (data && typeof data == "object" && !Array.isArray(data)) {
            if (data.kind === void 0) {
              const err29 = { instancePath, schemaPath: "#/oneOf/5/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
              if (vErrors === null) {
                vErrors = [err29];
              } else {
                vErrors.push(err29);
              }
              errors++;
            }
            if (data.overlay === void 0) {
              const err30 = { instancePath, schemaPath: "#/oneOf/5/required", keyword: "required", params: { missingProperty: "overlay" }, message: "must have required property 'overlay'" };
              if (vErrors === null) {
                vErrors = [err30];
              } else {
                vErrors.push(err30);
              }
              errors++;
            }
            for (const key5 in data) {
              if (!(key5 === "kind" || key5 === "overlay")) {
                const err31 = { instancePath, schemaPath: "#/oneOf/5/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key5 }, message: "must NOT have additional properties" };
                if (vErrors === null) {
                  vErrors = [err31];
                } else {
                  vErrors.push(err31);
                }
                errors++;
              }
            }
            if (data.kind !== void 0) {
              let data9 = data.kind;
              if (typeof data9 !== "string") {
                const err32 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                if (vErrors === null) {
                  vErrors = [err32];
                } else {
                  vErrors.push(err32);
                }
                errors++;
              }
              if ("set_pause_overlay" !== data9) {
                const err33 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/5/properties/kind/const", keyword: "const", params: { allowedValue: "set_pause_overlay" }, message: "must be equal to constant" };
                if (vErrors === null) {
                  vErrors = [err33];
                } else {
                  vErrors.push(err33);
                }
                errors++;
              }
            }
            if (data.overlay !== void 0) {
              if (!validate208(data.overlay, { instancePath: instancePath + "/overlay", parentData: data, parentDataProperty: "overlay", rootData, dynamicAnchors })) {
                vErrors = vErrors === null ? validate208.errors : vErrors.concat(validate208.errors);
                errors = vErrors.length;
              }
            }
          } else {
            const err34 = { instancePath, schemaPath: "#/oneOf/5/type", keyword: "type", params: { type: "object" }, message: "must be object" };
            if (vErrors === null) {
              vErrors = [err34];
            } else {
              vErrors.push(err34);
            }
            errors++;
          }
          var _valid0 = _errs30 === errors;
          if (_valid0 && valid0) {
            valid0 = false;
            passing0 = [passing0, 5];
          } else {
            if (_valid0) {
              valid0 = true;
              passing0 = 5;
              if (props1 !== true) {
                props1 = true;
              }
            }
            const _errs36 = errors;
            if (data && typeof data == "object" && !Array.isArray(data)) {
              if (data.kind === void 0) {
                const err35 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                if (vErrors === null) {
                  vErrors = [err35];
                } else {
                  vErrors.push(err35);
                }
                errors++;
              }
              if (data.view === void 0) {
                const err36 = { instancePath, schemaPath: "#/oneOf/6/required", keyword: "required", params: { missingProperty: "view" }, message: "must have required property 'view'" };
                if (vErrors === null) {
                  vErrors = [err36];
                } else {
                  vErrors.push(err36);
                }
                errors++;
              }
              for (const key6 in data) {
                if (!(key6 === "kind" || key6 === "view")) {
                  const err37 = { instancePath, schemaPath: "#/oneOf/6/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key6 }, message: "must NOT have additional properties" };
                  if (vErrors === null) {
                    vErrors = [err37];
                  } else {
                    vErrors.push(err37);
                  }
                  errors++;
                }
              }
              if (data.kind !== void 0) {
                let data11 = data.kind;
                if (typeof data11 !== "string") {
                  const err38 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                  if (vErrors === null) {
                    vErrors = [err38];
                  } else {
                    vErrors.push(err38);
                  }
                  errors++;
                }
                if ("set_replay_view" !== data11) {
                  const err39 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/6/properties/kind/const", keyword: "const", params: { allowedValue: "set_replay_view" }, message: "must be equal to constant" };
                  if (vErrors === null) {
                    vErrors = [err39];
                  } else {
                    vErrors.push(err39);
                  }
                  errors++;
                }
              }
              if (data.view !== void 0) {
                if (!validate210(data.view, { instancePath: instancePath + "/view", parentData: data, parentDataProperty: "view", rootData, dynamicAnchors })) {
                  vErrors = vErrors === null ? validate210.errors : vErrors.concat(validate210.errors);
                  errors = vErrors.length;
                }
              }
            } else {
              const err40 = { instancePath, schemaPath: "#/oneOf/6/type", keyword: "type", params: { type: "object" }, message: "must be object" };
              if (vErrors === null) {
                vErrors = [err40];
              } else {
                vErrors.push(err40);
              }
              errors++;
            }
            var _valid0 = _errs36 === errors;
            if (_valid0 && valid0) {
              valid0 = false;
              passing0 = [passing0, 6];
            } else {
              if (_valid0) {
                valid0 = true;
                passing0 = 6;
                if (props1 !== true) {
                  props1 = true;
                }
              }
              const _errs42 = errors;
              if (data && typeof data == "object" && !Array.isArray(data)) {
                if (data.kind === void 0) {
                  const err41 = { instancePath, schemaPath: "#/oneOf/7/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                  if (vErrors === null) {
                    vErrors = [err41];
                  } else {
                    vErrors.push(err41);
                  }
                  errors++;
                }
                for (const key7 in data) {
                  if (!(key7 === "kind")) {
                    const err42 = { instancePath, schemaPath: "#/oneOf/7/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key7 }, message: "must NOT have additional properties" };
                    if (vErrors === null) {
                      vErrors = [err42];
                    } else {
                      vErrors.push(err42);
                    }
                    errors++;
                  }
                }
                if (data.kind !== void 0) {
                  let data13 = data.kind;
                  if (typeof data13 !== "string") {
                    const err43 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                    if (vErrors === null) {
                      vErrors = [err43];
                    } else {
                      vErrors.push(err43);
                    }
                    errors++;
                  }
                  if ("cycle_replay_camera" !== data13) {
                    const err44 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/7/properties/kind/const", keyword: "const", params: { allowedValue: "cycle_replay_camera" }, message: "must be equal to constant" };
                    if (vErrors === null) {
                      vErrors = [err44];
                    } else {
                      vErrors.push(err44);
                    }
                    errors++;
                  }
                }
              } else {
                const err45 = { instancePath, schemaPath: "#/oneOf/7/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                if (vErrors === null) {
                  vErrors = [err45];
                } else {
                  vErrors.push(err45);
                }
                errors++;
              }
              var _valid0 = _errs42 === errors;
              if (_valid0 && valid0) {
                valid0 = false;
                passing0 = [passing0, 7];
              } else {
                if (_valid0) {
                  valid0 = true;
                  passing0 = 7;
                  if (props1 !== true) {
                    props1 = true;
                  }
                }
                const _errs47 = errors;
                if (data && typeof data == "object" && !Array.isArray(data)) {
                  if (data.kind === void 0) {
                    const err46 = { instancePath, schemaPath: "#/oneOf/8/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                    if (vErrors === null) {
                      vErrors = [err46];
                    } else {
                      vErrors.push(err46);
                    }
                    errors++;
                  }
                  if (data.time_seconds === void 0) {
                    const err47 = { instancePath, schemaPath: "#/oneOf/8/required", keyword: "required", params: { missingProperty: "time_seconds" }, message: "must have required property 'time_seconds'" };
                    if (vErrors === null) {
                      vErrors = [err47];
                    } else {
                      vErrors.push(err47);
                    }
                    errors++;
                  }
                  for (const key8 in data) {
                    if (!(key8 === "kind" || key8 === "time_seconds")) {
                      const err48 = { instancePath, schemaPath: "#/oneOf/8/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key8 }, message: "must NOT have additional properties" };
                      if (vErrors === null) {
                        vErrors = [err48];
                      } else {
                        vErrors.push(err48);
                      }
                      errors++;
                    }
                  }
                  if (data.kind !== void 0) {
                    let data14 = data.kind;
                    if (typeof data14 !== "string") {
                      const err49 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/8/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                      if (vErrors === null) {
                        vErrors = [err49];
                      } else {
                        vErrors.push(err49);
                      }
                      errors++;
                    }
                    if ("seek_analysis" !== data14) {
                      const err50 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/8/properties/kind/const", keyword: "const", params: { allowedValue: "seek_analysis" }, message: "must be equal to constant" };
                      if (vErrors === null) {
                        vErrors = [err50];
                      } else {
                        vErrors.push(err50);
                      }
                      errors++;
                    }
                  }
                  if (data.time_seconds !== void 0) {
                    let data15 = data.time_seconds;
                    if (typeof data15 == "number" && isFinite(data15)) {
                      if (!formats0.validate(data15)) {
                        const err51 = { instancePath: instancePath + "/time_seconds", schemaPath: "#/oneOf/8/properties/time_seconds/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
                        if (vErrors === null) {
                          vErrors = [err51];
                        } else {
                          vErrors.push(err51);
                        }
                        errors++;
                      }
                    } else {
                      const err52 = { instancePath: instancePath + "/time_seconds", schemaPath: "#/oneOf/8/properties/time_seconds/type", keyword: "type", params: { type: "number" }, message: "must be number" };
                      if (vErrors === null) {
                        vErrors = [err52];
                      } else {
                        vErrors.push(err52);
                      }
                      errors++;
                    }
                  }
                } else {
                  const err53 = { instancePath, schemaPath: "#/oneOf/8/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                  if (vErrors === null) {
                    vErrors = [err53];
                  } else {
                    vErrors.push(err53);
                  }
                  errors++;
                }
                var _valid0 = _errs47 === errors;
                if (_valid0 && valid0) {
                  valid0 = false;
                  passing0 = [passing0, 8];
                } else {
                  if (_valid0) {
                    valid0 = true;
                    passing0 = 8;
                    if (props1 !== true) {
                      props1 = true;
                    }
                  }
                  const _errs54 = errors;
                  if (data && typeof data == "object" && !Array.isArray(data)) {
                    if (data.kind === void 0) {
                      const err54 = { instancePath, schemaPath: "#/oneOf/9/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                      if (vErrors === null) {
                        vErrors = [err54];
                      } else {
                        vErrors.push(err54);
                      }
                      errors++;
                    }
                    if (data.time_seconds === void 0) {
                      const err55 = { instancePath, schemaPath: "#/oneOf/9/required", keyword: "required", params: { missingProperty: "time_seconds" }, message: "must have required property 'time_seconds'" };
                      if (vErrors === null) {
                        vErrors = [err55];
                      } else {
                        vErrors.push(err55);
                      }
                      errors++;
                    }
                    for (const key9 in data) {
                      if (!(key9 === "kind" || key9 === "time_seconds")) {
                        const err56 = { instancePath, schemaPath: "#/oneOf/9/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key9 }, message: "must NOT have additional properties" };
                        if (vErrors === null) {
                          vErrors = [err56];
                        } else {
                          vErrors.push(err56);
                        }
                        errors++;
                      }
                    }
                    if (data.kind !== void 0) {
                      let data16 = data.kind;
                      if (typeof data16 !== "string") {
                        const err57 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/9/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                        if (vErrors === null) {
                          vErrors = [err57];
                        } else {
                          vErrors.push(err57);
                        }
                        errors++;
                      }
                      if ("seek_replay" !== data16) {
                        const err58 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/9/properties/kind/const", keyword: "const", params: { allowedValue: "seek_replay" }, message: "must be equal to constant" };
                        if (vErrors === null) {
                          vErrors = [err58];
                        } else {
                          vErrors.push(err58);
                        }
                        errors++;
                      }
                    }
                    if (data.time_seconds !== void 0) {
                      let data17 = data.time_seconds;
                      if (typeof data17 == "number" && isFinite(data17)) {
                        if (!formats0.validate(data17)) {
                          const err59 = { instancePath: instancePath + "/time_seconds", schemaPath: "#/oneOf/9/properties/time_seconds/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
                          if (vErrors === null) {
                            vErrors = [err59];
                          } else {
                            vErrors.push(err59);
                          }
                          errors++;
                        }
                      } else {
                        const err60 = { instancePath: instancePath + "/time_seconds", schemaPath: "#/oneOf/9/properties/time_seconds/type", keyword: "type", params: { type: "number" }, message: "must be number" };
                        if (vErrors === null) {
                          vErrors = [err60];
                        } else {
                          vErrors.push(err60);
                        }
                        errors++;
                      }
                    }
                  } else {
                    const err61 = { instancePath, schemaPath: "#/oneOf/9/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                    if (vErrors === null) {
                      vErrors = [err61];
                    } else {
                      vErrors.push(err61);
                    }
                    errors++;
                  }
                  var _valid0 = _errs54 === errors;
                  if (_valid0 && valid0) {
                    valid0 = false;
                    passing0 = [passing0, 9];
                  } else {
                    if (_valid0) {
                      valid0 = true;
                      passing0 = 9;
                      if (props1 !== true) {
                        props1 = true;
                      }
                    }
                    const _errs61 = errors;
                    if (data && typeof data == "object" && !Array.isArray(data)) {
                      if (data.kind === void 0) {
                        const err62 = { instancePath, schemaPath: "#/oneOf/10/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                        if (vErrors === null) {
                          vErrors = [err62];
                        } else {
                          vErrors.push(err62);
                        }
                        errors++;
                      }
                      for (const key10 in data) {
                        if (!(key10 === "kind")) {
                          const err63 = { instancePath, schemaPath: "#/oneOf/10/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key10 }, message: "must NOT have additional properties" };
                          if (vErrors === null) {
                            vErrors = [err63];
                          } else {
                            vErrors.push(err63);
                          }
                          errors++;
                        }
                      }
                      if (data.kind !== void 0) {
                        let data18 = data.kind;
                        if (typeof data18 !== "string") {
                          const err64 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/10/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                          if (vErrors === null) {
                            vErrors = [err64];
                          } else {
                            vErrors.push(err64);
                          }
                          errors++;
                        }
                        if ("toggle_replay_playing" !== data18) {
                          const err65 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/10/properties/kind/const", keyword: "const", params: { allowedValue: "toggle_replay_playing" }, message: "must be equal to constant" };
                          if (vErrors === null) {
                            vErrors = [err65];
                          } else {
                            vErrors.push(err65);
                          }
                          errors++;
                        }
                      }
                    } else {
                      const err66 = { instancePath, schemaPath: "#/oneOf/10/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                      if (vErrors === null) {
                        vErrors = [err66];
                      } else {
                        vErrors.push(err66);
                      }
                      errors++;
                    }
                    var _valid0 = _errs61 === errors;
                    if (_valid0 && valid0) {
                      valid0 = false;
                      passing0 = [passing0, 10];
                    } else {
                      if (_valid0) {
                        valid0 = true;
                        passing0 = 10;
                        if (props1 !== true) {
                          props1 = true;
                        }
                      }
                      const _errs66 = errors;
                      if (data && typeof data == "object" && !Array.isArray(data)) {
                        if (data.kind === void 0) {
                          const err67 = { instancePath, schemaPath: "#/oneOf/11/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
                          if (vErrors === null) {
                            vErrors = [err67];
                          } else {
                            vErrors.push(err67);
                          }
                          errors++;
                        }
                        if (data.code === void 0) {
                          const err68 = { instancePath, schemaPath: "#/oneOf/11/required", keyword: "required", params: { missingProperty: "code" }, message: "must have required property 'code'" };
                          if (vErrors === null) {
                            vErrors = [err68];
                          } else {
                            vErrors.push(err68);
                          }
                          errors++;
                        }
                        for (const key11 in data) {
                          if (!(key11 === "code" || key11 === "kind")) {
                            const err69 = { instancePath, schemaPath: "#/oneOf/11/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key11 }, message: "must NOT have additional properties" };
                            if (vErrors === null) {
                              vErrors = [err69];
                            } else {
                              vErrors.push(err69);
                            }
                            errors++;
                          }
                        }
                        if (data.code !== void 0) {
                          let data19 = data.code;
                          if (!(typeof data19 == "number" && (!(data19 % 1) && !isNaN(data19)) && isFinite(data19))) {
                            const err70 = { instancePath: instancePath + "/code", schemaPath: "#/oneOf/11/properties/code/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
                            if (vErrors === null) {
                              vErrors = [err70];
                            } else {
                              vErrors.push(err70);
                            }
                            errors++;
                          }
                          if (typeof data19 == "number" && isFinite(data19)) {
                            if (data19 < 0 || isNaN(data19)) {
                              const err71 = { instancePath: instancePath + "/code", schemaPath: "#/oneOf/11/properties/code/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
                              if (vErrors === null) {
                                vErrors = [err71];
                              } else {
                                vErrors.push(err71);
                              }
                              errors++;
                            }
                            if (!formats28.validate(data19)) {
                              const err72 = { instancePath: instancePath + "/code", schemaPath: "#/oneOf/11/properties/code/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
                              if (vErrors === null) {
                                vErrors = [err72];
                              } else {
                                vErrors.push(err72);
                              }
                              errors++;
                            }
                          }
                        }
                        if (data.kind !== void 0) {
                          let data20 = data.kind;
                          if (typeof data20 !== "string") {
                            const err73 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/11/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
                            if (vErrors === null) {
                              vErrors = [err73];
                            } else {
                              vErrors.push(err73);
                            }
                            errors++;
                          }
                          if ("set_replay_rate" !== data20) {
                            const err74 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/11/properties/kind/const", keyword: "const", params: { allowedValue: "set_replay_rate" }, message: "must be equal to constant" };
                            if (vErrors === null) {
                              vErrors = [err74];
                            } else {
                              vErrors.push(err74);
                            }
                            errors++;
                          }
                        }
                      } else {
                        const err75 = { instancePath, schemaPath: "#/oneOf/11/type", keyword: "type", params: { type: "object" }, message: "must be object" };
                        if (vErrors === null) {
                          vErrors = [err75];
                        } else {
                          vErrors.push(err75);
                        }
                        errors++;
                      }
                      var _valid0 = _errs66 === errors;
                      if (_valid0 && valid0) {
                        valid0 = false;
                        passing0 = [passing0, 11];
                      } else {
                        if (_valid0) {
                          valid0 = true;
                          passing0 = 11;
                          if (props1 !== true) {
                            props1 = true;
                          }
                        }
                      }
                    }
                  }
                }
              }
            }
          }
        }
      }
    }
  }
  if (!valid0) {
    const err76 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err76];
    } else {
      vErrors.push(err76);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate183.errors = vErrors;
  evaluated0.props = props1;
  return errors === 0;
}
validate183.evaluated = { "dynamicProps": true, "dynamicItems": false };
var validateGameUiPlatformInput = validate212;
var schema89 = { "additionalProperties": false, "description": "Browser or OS observations that never own game physics or phase.", "properties": { "analysis_available": { "description": "Platform supports full record plots.", "type": "boolean" }, "chart_extent": { "anyOf": [{ "$ref": "#/$defs/GameUiChartExtent" }, { "type": "null" }], "description": "Measured chart content extent; absent before the platform has measured it." }, "flight_controller_failure": { "description": "Captured flight-controller resource failure; the session retains its last valid state.", "type": ["string", "null"] }, "flight_log_available": { "description": "Platform supports record downloads.", "type": "boolean" }, "flight_log_pending": { "description": "A download request for the current resource owner remains pending.", "type": "boolean" }, "head_hud_failure": { "description": "Captured head-HUD resource failure, separate from the domain and physical state.", "type": ["string", "null"] }, "notification": { "description": "Platform operation feedback.", "type": "string" }, "operation_pending": { "description": "Blocks operations while the owning adapter is transitioning.", "type": "boolean" }, "phone_vr_available": { "description": "Phone sensor headset support.", "type": "boolean" }, "presentation_mode": { "$ref": "#/$defs/GameUiPresentationMode", "description": "Actual active presentation mode." }, "replay_available": { "description": "Platform supports recorded scenic playback.", "type": "boolean" }, "stored_records": { "description": "Persistence results associated with this owner.", "items": { "$ref": "#/$defs/GameUiStoredRecord" }, "type": "array" }, "stored_records_available": { "description": "Platform supports persisted record lookup.", "type": "boolean" }, "supported_replay_cameras": { "description": "Implemented camera rigs; shared ordering remains the Web ordering.", "items": { "$ref": "#/$defs/GameUiReplayCamera" }, "type": "array" }, "viewport": { "$ref": "#/$defs/GameUiViewport", "description": "Current physical display viewport." }, "water_quality": { "$ref": "#/$defs/GameUiWaterQuality", "description": "Confirmed current render quality." }, "water_quality_operation": { "$ref": "#/$defs/GameUiWaterQualityOperation", "description": "Actual resource preparation, commitment and cleanup observation." }, "webxr_available": { "description": "Browser WebXR support.", "type": "boolean" } }, "required": ["viewport", "presentation_mode", "operation_pending", "webxr_available", "phone_vr_available", "analysis_available", "replay_available", "supported_replay_cameras", "stored_records_available", "flight_log_available", "flight_log_pending", "water_quality", "water_quality_operation", "notification", "stored_records"], "type": "object" };
function validate213(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate213.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.width === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "width" }, message: "must have required property 'width'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.height === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "height" }, message: "must have required property 'height'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "height" || key0 === "width")) {
        const err2 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.height !== void 0) {
      let data0 = data.height;
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err3 = { instancePath: instancePath + "/height", schemaPath: "#/properties/height/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err3];
          } else {
            vErrors.push(err3);
          }
          errors++;
        }
      } else {
        const err4 = { instancePath: instancePath + "/height", schemaPath: "#/properties/height/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.width !== void 0) {
      let data1 = data.width;
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err5 = { instancePath: instancePath + "/width", schemaPath: "#/properties/width/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        }
      } else {
        const err6 = { instancePath: instancePath + "/width", schemaPath: "#/properties/width/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
    }
  } else {
    const err7 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err7];
    } else {
      vErrors.push(err7);
    }
    errors++;
  }
  validate213.errors = vErrors;
  return errors === 0;
}
validate213.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate216(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate216.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.id === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.saved_at === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "saved_at" }, message: "must have required property 'saved_at'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.personal_best === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "personal_best" }, message: "must have required property 'personal_best'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "id" || key0 === "personal_best" || key0 === "saved_at")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.id !== void 0) {
      let data0 = data.id;
      if (!(typeof data0 == "number" && (!(data0 % 1) && !isNaN(data0)) && isFinite(data0))) {
        const err4 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
      if (typeof data0 == "number" && isFinite(data0)) {
        if (data0 < 0 || isNaN(data0)) {
          const err5 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
          if (vErrors === null) {
            vErrors = [err5];
          } else {
            vErrors.push(err5);
          }
          errors++;
        }
        if (!formats28.validate(data0)) {
          const err6 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
          if (vErrors === null) {
            vErrors = [err6];
          } else {
            vErrors.push(err6);
          }
          errors++;
        }
      }
    }
    if (data.personal_best !== void 0) {
      if (typeof data.personal_best !== "boolean") {
        const err7 = { instancePath: instancePath + "/personal_best", schemaPath: "#/properties/personal_best/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
    if (data.saved_at !== void 0) {
      if (typeof data.saved_at !== "string") {
        const err8 = { instancePath: instancePath + "/saved_at", schemaPath: "#/properties/saved_at/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
  } else {
    const err9 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err9];
    } else {
      vErrors.push(err9);
    }
    errors++;
  }
  validate216.errors = vErrors;
  return errors === 0;
}
validate216.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate219(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate219.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.width === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "width" }, message: "must have required property 'width'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.height === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "height" }, message: "must have required property 'height'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.root_font_size === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "root_font_size" }, message: "must have required property 'root_font_size'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "height" || key0 === "root_font_size" || key0 === "width")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.height !== void 0) {
      let data0 = data.height;
      if (typeof data0 == "number" && isFinite(data0)) {
        if (!formats0.validate(data0)) {
          const err4 = { instancePath: instancePath + "/height", schemaPath: "#/properties/height/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err4];
          } else {
            vErrors.push(err4);
          }
          errors++;
        }
      } else {
        const err5 = { instancePath: instancePath + "/height", schemaPath: "#/properties/height/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.root_font_size !== void 0) {
      let data1 = data.root_font_size;
      if (typeof data1 == "number" && isFinite(data1)) {
        if (!formats0.validate(data1)) {
          const err6 = { instancePath: instancePath + "/root_font_size", schemaPath: "#/properties/root_font_size/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err6];
          } else {
            vErrors.push(err6);
          }
          errors++;
        }
      } else {
        const err7 = { instancePath: instancePath + "/root_font_size", schemaPath: "#/properties/root_font_size/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
    if (data.width !== void 0) {
      let data2 = data.width;
      if (typeof data2 == "number" && isFinite(data2)) {
        if (!formats0.validate(data2)) {
          const err8 = { instancePath: instancePath + "/width", schemaPath: "#/properties/width/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err8];
          } else {
            vErrors.push(err8);
          }
          errors++;
        }
      } else {
        const err9 = { instancePath: instancePath + "/width", schemaPath: "#/properties/width/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
  } else {
    const err10 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  }
  validate219.errors = vErrors;
  return errors === 0;
}
validate219.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate222(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate222.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "kind")) {
        const err1 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err1];
        } else {
          vErrors.push(err1);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data0 = data.kind;
      if (typeof data0 !== "string") {
        const err2 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
      if ("idle" !== data0) {
        const err3 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "idle" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
  } else {
    const err4 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err4];
    } else {
      vErrors.push(err4);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props0 = true;
  }
  const _errs6 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err5 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.requested === void 0) {
      const err6 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "requested" }, message: "must have required property 'requested'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "kind" || key1 === "requested")) {
        const err7 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
    if (data.kind !== void 0) {
      let data1 = data.kind;
      if (typeof data1 !== "string") {
        const err8 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
      if ("applying" !== data1) {
        const err9 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "applying" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
    }
    if (data.requested !== void 0) {
      if (!validate197(data.requested, { instancePath: instancePath + "/requested", parentData: data, parentDataProperty: "requested", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate197.errors : vErrors.concat(validate197.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err10 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err10];
    } else {
      vErrors.push(err10);
    }
    errors++;
  }
  var _valid0 = _errs6 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props0 !== true) {
        props0 = true;
      }
    }
    const _errs12 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err11 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
      if (data.requested === void 0) {
        const err12 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "requested" }, message: "must have required property 'requested'" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
      if (data.cause === void 0) {
        const err13 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "cause" }, message: "must have required property 'cause'" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "cause" || key2 === "kind" || key2 === "requested")) {
          const err14 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
      }
      if (data.cause !== void 0) {
        if (typeof data.cause !== "string") {
          const err15 = { instancePath: instancePath + "/cause", schemaPath: "#/oneOf/2/properties/cause/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err15];
          } else {
            vErrors.push(err15);
          }
          errors++;
        }
      }
      if (data.kind !== void 0) {
        let data4 = data.kind;
        if (typeof data4 !== "string") {
          const err16 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err16];
          } else {
            vErrors.push(err16);
          }
          errors++;
        }
        if ("failed" !== data4) {
          const err17 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "failed" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err17];
          } else {
            vErrors.push(err17);
          }
          errors++;
        }
      }
      if (data.requested !== void 0) {
        if (!validate197(data.requested, { instancePath: instancePath + "/requested", parentData: data, parentDataProperty: "requested", rootData, dynamicAnchors })) {
          vErrors = vErrors === null ? validate197.errors : vErrors.concat(validate197.errors);
          errors = vErrors.length;
        }
      }
    } else {
      const err18 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err18];
      } else {
        vErrors.push(err18);
      }
      errors++;
    }
    var _valid0 = _errs12 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props0 !== true) {
          props0 = true;
        }
      }
      const _errs20 = errors;
      if (data && typeof data == "object" && !Array.isArray(data)) {
        if (data.kind === void 0) {
          const err19 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
          if (vErrors === null) {
            vErrors = [err19];
          } else {
            vErrors.push(err19);
          }
          errors++;
        }
        if (data.cause === void 0) {
          const err20 = { instancePath, schemaPath: "#/oneOf/3/required", keyword: "required", params: { missingProperty: "cause" }, message: "must have required property 'cause'" };
          if (vErrors === null) {
            vErrors = [err20];
          } else {
            vErrors.push(err20);
          }
          errors++;
        }
        for (const key3 in data) {
          if (!(key3 === "cause" || key3 === "kind")) {
            const err21 = { instancePath, schemaPath: "#/oneOf/3/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key3 }, message: "must NOT have additional properties" };
            if (vErrors === null) {
              vErrors = [err21];
            } else {
              vErrors.push(err21);
            }
            errors++;
          }
        }
        if (data.cause !== void 0) {
          if (typeof data.cause !== "string") {
            const err22 = { instancePath: instancePath + "/cause", schemaPath: "#/oneOf/3/properties/cause/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err22];
            } else {
              vErrors.push(err22);
            }
            errors++;
          }
        }
        if (data.kind !== void 0) {
          let data7 = data.kind;
          if (typeof data7 !== "string") {
            const err23 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
            if (vErrors === null) {
              vErrors = [err23];
            } else {
              vErrors.push(err23);
            }
            errors++;
          }
          if ("cleanup_failed" !== data7) {
            const err24 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/3/properties/kind/const", keyword: "const", params: { allowedValue: "cleanup_failed" }, message: "must be equal to constant" };
            if (vErrors === null) {
              vErrors = [err24];
            } else {
              vErrors.push(err24);
            }
            errors++;
          }
        }
      } else {
        const err25 = { instancePath, schemaPath: "#/oneOf/3/type", keyword: "type", params: { type: "object" }, message: "must be object" };
        if (vErrors === null) {
          vErrors = [err25];
        } else {
          vErrors.push(err25);
        }
        errors++;
      }
      var _valid0 = _errs20 === errors;
      if (_valid0 && valid0) {
        valid0 = false;
        passing0 = [passing0, 3];
      } else {
        if (_valid0) {
          valid0 = true;
          passing0 = 3;
          if (props0 !== true) {
            props0 = true;
          }
        }
      }
    }
  }
  if (!valid0) {
    const err26 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err26];
    } else {
      vErrors.push(err26);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate222.errors = vErrors;
  evaluated0.props = props0;
  return errors === 0;
}
validate222.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate212(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate212.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.viewport === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "viewport" }, message: "must have required property 'viewport'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.presentation_mode === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "presentation_mode" }, message: "must have required property 'presentation_mode'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.operation_pending === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "operation_pending" }, message: "must have required property 'operation_pending'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.webxr_available === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "webxr_available" }, message: "must have required property 'webxr_available'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.phone_vr_available === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "phone_vr_available" }, message: "must have required property 'phone_vr_available'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.analysis_available === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "analysis_available" }, message: "must have required property 'analysis_available'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.replay_available === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "replay_available" }, message: "must have required property 'replay_available'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.supported_replay_cameras === void 0) {
      const err7 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "supported_replay_cameras" }, message: "must have required property 'supported_replay_cameras'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    if (data.stored_records_available === void 0) {
      const err8 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "stored_records_available" }, message: "must have required property 'stored_records_available'" };
      if (vErrors === null) {
        vErrors = [err8];
      } else {
        vErrors.push(err8);
      }
      errors++;
    }
    if (data.flight_log_available === void 0) {
      const err9 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "flight_log_available" }, message: "must have required property 'flight_log_available'" };
      if (vErrors === null) {
        vErrors = [err9];
      } else {
        vErrors.push(err9);
      }
      errors++;
    }
    if (data.flight_log_pending === void 0) {
      const err10 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "flight_log_pending" }, message: "must have required property 'flight_log_pending'" };
      if (vErrors === null) {
        vErrors = [err10];
      } else {
        vErrors.push(err10);
      }
      errors++;
    }
    if (data.water_quality === void 0) {
      const err11 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "water_quality" }, message: "must have required property 'water_quality'" };
      if (vErrors === null) {
        vErrors = [err11];
      } else {
        vErrors.push(err11);
      }
      errors++;
    }
    if (data.water_quality_operation === void 0) {
      const err12 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "water_quality_operation" }, message: "must have required property 'water_quality_operation'" };
      if (vErrors === null) {
        vErrors = [err12];
      } else {
        vErrors.push(err12);
      }
      errors++;
    }
    if (data.notification === void 0) {
      const err13 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "notification" }, message: "must have required property 'notification'" };
      if (vErrors === null) {
        vErrors = [err13];
      } else {
        vErrors.push(err13);
      }
      errors++;
    }
    if (data.stored_records === void 0) {
      const err14 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "stored_records" }, message: "must have required property 'stored_records'" };
      if (vErrors === null) {
        vErrors = [err14];
      } else {
        vErrors.push(err14);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!func1.call(schema89.properties, key0)) {
        const err15 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
    }
    if (data.analysis_available !== void 0) {
      if (typeof data.analysis_available !== "boolean") {
        const err16 = { instancePath: instancePath + "/analysis_available", schemaPath: "#/properties/analysis_available/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err16];
        } else {
          vErrors.push(err16);
        }
        errors++;
      }
    }
    if (data.chart_extent !== void 0) {
      let data1 = data.chart_extent;
      const _errs5 = errors;
      let valid1 = false;
      const _errs6 = errors;
      if (!validate213(data1, { instancePath: instancePath + "/chart_extent", parentData: data, parentDataProperty: "chart_extent", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate213.errors : vErrors.concat(validate213.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs6 === errors;
      valid1 = valid1 || _valid0;
      const _errs7 = errors;
      if (data1 !== null) {
        const err17 = { instancePath: instancePath + "/chart_extent", schemaPath: "#/properties/chart_extent/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
        if (vErrors === null) {
          vErrors = [err17];
        } else {
          vErrors.push(err17);
        }
        errors++;
      }
      var _valid0 = _errs7 === errors;
      valid1 = valid1 || _valid0;
      if (!valid1) {
        const err18 = { instancePath: instancePath + "/chart_extent", schemaPath: "#/properties/chart_extent/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
        if (vErrors === null) {
          vErrors = [err18];
        } else {
          vErrors.push(err18);
        }
        errors++;
      } else {
        errors = _errs5;
        if (vErrors !== null) {
          if (_errs5) {
            vErrors.length = _errs5;
          } else {
            vErrors = null;
          }
        }
      }
    }
    if (data.flight_controller_failure !== void 0) {
      let data2 = data.flight_controller_failure;
      if (typeof data2 !== "string" && data2 !== null) {
        const err19 = { instancePath: instancePath + "/flight_controller_failure", schemaPath: "#/properties/flight_controller_failure/type", keyword: "type", params: { type: schema89.properties.flight_controller_failure.type }, message: "must be string,null" };
        if (vErrors === null) {
          vErrors = [err19];
        } else {
          vErrors.push(err19);
        }
        errors++;
      }
    }
    if (data.flight_log_available !== void 0) {
      if (typeof data.flight_log_available !== "boolean") {
        const err20 = { instancePath: instancePath + "/flight_log_available", schemaPath: "#/properties/flight_log_available/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err20];
        } else {
          vErrors.push(err20);
        }
        errors++;
      }
    }
    if (data.flight_log_pending !== void 0) {
      if (typeof data.flight_log_pending !== "boolean") {
        const err21 = { instancePath: instancePath + "/flight_log_pending", schemaPath: "#/properties/flight_log_pending/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err21];
        } else {
          vErrors.push(err21);
        }
        errors++;
      }
    }
    if (data.head_hud_failure !== void 0) {
      let data5 = data.head_hud_failure;
      if (typeof data5 !== "string" && data5 !== null) {
        const err22 = { instancePath: instancePath + "/head_hud_failure", schemaPath: "#/properties/head_hud_failure/type", keyword: "type", params: { type: schema89.properties.head_hud_failure.type }, message: "must be string,null" };
        if (vErrors === null) {
          vErrors = [err22];
        } else {
          vErrors.push(err22);
        }
        errors++;
      }
    }
    if (data.notification !== void 0) {
      if (typeof data.notification !== "string") {
        const err23 = { instancePath: instancePath + "/notification", schemaPath: "#/properties/notification/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err23];
        } else {
          vErrors.push(err23);
        }
        errors++;
      }
    }
    if (data.operation_pending !== void 0) {
      if (typeof data.operation_pending !== "boolean") {
        const err24 = { instancePath: instancePath + "/operation_pending", schemaPath: "#/properties/operation_pending/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err24];
        } else {
          vErrors.push(err24);
        }
        errors++;
      }
    }
    if (data.phone_vr_available !== void 0) {
      if (typeof data.phone_vr_available !== "boolean") {
        const err25 = { instancePath: instancePath + "/phone_vr_available", schemaPath: "#/properties/phone_vr_available/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err25];
        } else {
          vErrors.push(err25);
        }
        errors++;
      }
    }
    if (data.presentation_mode !== void 0) {
      if (!validate199(data.presentation_mode, { instancePath: instancePath + "/presentation_mode", parentData: data, parentDataProperty: "presentation_mode", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate199.errors : vErrors.concat(validate199.errors);
        errors = vErrors.length;
      }
    }
    if (data.replay_available !== void 0) {
      if (typeof data.replay_available !== "boolean") {
        const err26 = { instancePath: instancePath + "/replay_available", schemaPath: "#/properties/replay_available/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err26];
        } else {
          vErrors.push(err26);
        }
        errors++;
      }
    }
    if (data.stored_records !== void 0) {
      let data11 = data.stored_records;
      if (Array.isArray(data11)) {
        const len0 = data11.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate216(data11[i0], { instancePath: instancePath + "/stored_records/" + i0, parentData: data11, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate216.errors : vErrors.concat(validate216.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err27 = { instancePath: instancePath + "/stored_records", schemaPath: "#/properties/stored_records/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err27];
        } else {
          vErrors.push(err27);
        }
        errors++;
      }
    }
    if (data.stored_records_available !== void 0) {
      if (typeof data.stored_records_available !== "boolean") {
        const err28 = { instancePath: instancePath + "/stored_records_available", schemaPath: "#/properties/stored_records_available/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err28];
        } else {
          vErrors.push(err28);
        }
        errors++;
      }
    }
    if (data.supported_replay_cameras !== void 0) {
      let data14 = data.supported_replay_cameras;
      if (Array.isArray(data14)) {
        const len1 = data14.length;
        for (let i1 = 0; i1 < len1; i1++) {
          if (!validate201(data14[i1], { instancePath: instancePath + "/supported_replay_cameras/" + i1, parentData: data14, parentDataProperty: i1, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate201.errors : vErrors.concat(validate201.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err29 = { instancePath: instancePath + "/supported_replay_cameras", schemaPath: "#/properties/supported_replay_cameras/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err29];
        } else {
          vErrors.push(err29);
        }
        errors++;
      }
    }
    if (data.viewport !== void 0) {
      if (!validate219(data.viewport, { instancePath: instancePath + "/viewport", parentData: data, parentDataProperty: "viewport", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate219.errors : vErrors.concat(validate219.errors);
        errors = vErrors.length;
      }
    }
    if (data.water_quality !== void 0) {
      if (!validate197(data.water_quality, { instancePath: instancePath + "/water_quality", parentData: data, parentDataProperty: "water_quality", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate197.errors : vErrors.concat(validate197.errors);
        errors = vErrors.length;
      }
    }
    if (data.water_quality_operation !== void 0) {
      if (!validate222(data.water_quality_operation, { instancePath: instancePath + "/water_quality_operation", parentData: data, parentDataProperty: "water_quality_operation", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate222.errors : vErrors.concat(validate222.errors);
        errors = vErrors.length;
      }
    }
    if (data.webxr_available !== void 0) {
      if (typeof data.webxr_available !== "boolean") {
        const err30 = { instancePath: instancePath + "/webxr_available", schemaPath: "#/properties/webxr_available/type", keyword: "type", params: { type: "boolean" }, message: "must be boolean" };
        if (vErrors === null) {
          vErrors = [err30];
        } else {
          vErrors.push(err30);
        }
        errors++;
      }
    }
  } else {
    const err31 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err31];
    } else {
      vErrors.push(err31);
    }
    errors++;
  }
  validate212.errors = vErrors;
  return errors === 0;
}
validate212.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var validateGameUiUpdate = validate226;
function validate227(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate227.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  const _errs0 = errors;
  let valid0 = false;
  let passing0 = null;
  const _errs1 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err0 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.command === void 0) {
      const err1 = { instancePath, schemaPath: "#/oneOf/0/required", keyword: "required", params: { missingProperty: "command" }, message: "must have required property 'command'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "command" || key0 === "kind")) {
        const err2 = { instancePath, schemaPath: "#/oneOf/0/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.command !== void 0) {
      if (!validate80(data.command, { instancePath: instancePath + "/command", parentData: data, parentDataProperty: "command", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate80.errors : vErrors.concat(validate80.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data1 = data.kind;
      if (typeof data1 !== "string") {
        const err3 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
      if ("session" !== data1) {
        const err4 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/0/properties/kind/const", keyword: "const", params: { allowedValue: "session" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
  } else {
    const err5 = { instancePath, schemaPath: "#/oneOf/0/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err5];
    } else {
      vErrors.push(err5);
    }
    errors++;
  }
  var _valid0 = _errs1 === errors;
  if (_valid0) {
    valid0 = true;
    passing0 = 0;
    var props1 = true;
  }
  const _errs7 = errors;
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.kind === void 0) {
      const err6 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    if (data.command === void 0) {
      const err7 = { instancePath, schemaPath: "#/oneOf/1/required", keyword: "required", params: { missingProperty: "command" }, message: "must have required property 'command'" };
      if (vErrors === null) {
        vErrors = [err7];
      } else {
        vErrors.push(err7);
      }
      errors++;
    }
    for (const key1 in data) {
      if (!(key1 === "command" || key1 === "kind")) {
        const err8 = { instancePath, schemaPath: "#/oneOf/1/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key1 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.command !== void 0) {
      if (!validate92(data.command, { instancePath: instancePath + "/command", parentData: data, parentDataProperty: "command", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate92.errors : vErrors.concat(validate92.errors);
        errors = vErrors.length;
      }
    }
    if (data.kind !== void 0) {
      let data3 = data.kind;
      if (typeof data3 !== "string") {
        const err9 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
      if ("platform" !== data3) {
        const err10 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/1/properties/kind/const", keyword: "const", params: { allowedValue: "platform" }, message: "must be equal to constant" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      }
    }
  } else {
    const err11 = { instancePath, schemaPath: "#/oneOf/1/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
  }
  var _valid0 = _errs7 === errors;
  if (_valid0 && valid0) {
    valid0 = false;
    passing0 = [passing0, 1];
  } else {
    if (_valid0) {
      valid0 = true;
      passing0 = 1;
      if (props1 !== true) {
        props1 = true;
      }
    }
    const _errs13 = errors;
    if (data && typeof data == "object" && !Array.isArray(data)) {
      if (data.kind === void 0) {
        const err12 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "kind" }, message: "must have required property 'kind'" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      }
      if (data.action === void 0) {
        const err13 = { instancePath, schemaPath: "#/oneOf/2/required", keyword: "required", params: { missingProperty: "action" }, message: "must have required property 'action'" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      }
      for (const key2 in data) {
        if (!(key2 === "action" || key2 === "kind")) {
          const err14 = { instancePath, schemaPath: "#/oneOf/2/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key2 }, message: "must NOT have additional properties" };
          if (vErrors === null) {
            vErrors = [err14];
          } else {
            vErrors.push(err14);
          }
          errors++;
        }
      }
      if (data.action !== void 0) {
        if (!validate79(data.action, { instancePath: instancePath + "/action", parentData: data, parentDataProperty: "action", rootData, dynamicAnchors })) {
          vErrors = vErrors === null ? validate79.errors : vErrors.concat(validate79.errors);
          errors = vErrors.length;
        }
      }
      if (data.kind !== void 0) {
        let data5 = data.kind;
        if (typeof data5 !== "string") {
          const err15 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/type", keyword: "type", params: { type: "string" }, message: "must be string" };
          if (vErrors === null) {
            vErrors = [err15];
          } else {
            vErrors.push(err15);
          }
          errors++;
        }
        if ("record" !== data5) {
          const err16 = { instancePath: instancePath + "/kind", schemaPath: "#/oneOf/2/properties/kind/const", keyword: "const", params: { allowedValue: "record" }, message: "must be equal to constant" };
          if (vErrors === null) {
            vErrors = [err16];
          } else {
            vErrors.push(err16);
          }
          errors++;
        }
      }
    } else {
      const err17 = { instancePath, schemaPath: "#/oneOf/2/type", keyword: "type", params: { type: "object" }, message: "must be object" };
      if (vErrors === null) {
        vErrors = [err17];
      } else {
        vErrors.push(err17);
      }
      errors++;
    }
    var _valid0 = _errs13 === errors;
    if (_valid0 && valid0) {
      valid0 = false;
      passing0 = [passing0, 2];
    } else {
      if (_valid0) {
        valid0 = true;
        passing0 = 2;
        if (props1 !== true) {
          props1 = true;
        }
      }
    }
  }
  if (!valid0) {
    const err18 = { instancePath, schemaPath: "#/oneOf", keyword: "oneOf", params: { passingSchemas: passing0 }, message: "must match exactly one schema in oneOf" };
    if (vErrors === null) {
      vErrors = [err18];
    } else {
      vErrors.push(err18);
    }
    errors++;
  } else {
    errors = _errs0;
    if (vErrors !== null) {
      if (_errs0) {
        vErrors.length = _errs0;
      } else {
        vErrors = null;
      }
    }
  }
  validate227.errors = vErrors;
  evaluated0.props = props1;
  return errors === 0;
}
validate227.evaluated = { "dynamicProps": true, "dynamicItems": false };
function validate226(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate226.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.state === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "state" }, message: "must have required property 'state'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.effects === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "effects" }, message: "must have required property 'effects'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "effects" || key0 === "state")) {
        const err2 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.effects !== void 0) {
      let data0 = data.effects;
      if (Array.isArray(data0)) {
        const len0 = data0.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate227(data0[i0], { instancePath: instancePath + "/effects/" + i0, parentData: data0, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate227.errors : vErrors.concat(validate227.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err3 = { instancePath: instancePath + "/effects", schemaPath: "#/properties/effects/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.state !== void 0) {
      if (!validate173(data.state, { instancePath: instancePath + "/state", parentData: data, parentDataProperty: "state", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate173.errors : vErrors.concat(validate173.errors);
        errors = vErrors.length;
      }
    }
  } else {
    const err4 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err4];
    } else {
      vErrors.push(err4);
    }
    errors++;
  }
  validate226.errors = vErrors;
  return errors === 0;
}
validate226.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var validateGameUiFrame = validate233;
function validate234(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate234.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.id === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.label === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "label" }, message: "must have required property 'label'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "id" || key0 === "label")) {
        const err2 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.id !== void 0) {
      if (typeof data.id !== "string") {
        const err3 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.label !== void 0) {
      if (typeof data.label !== "string") {
        const err4 = { instancePath: instancePath + "/label", schemaPath: "#/properties/label/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
  } else {
    const err5 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err5];
    } else {
      vErrors.push(err5);
    }
    errors++;
  }
  validate234.errors = vErrors;
  return errors === 0;
}
validate234.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
var schema98 = { "additionalProperties": false, "description": "Lightweight cursor and selected-sample wind for the retained plot bounds.", "properties": { "cursor_points": { "description": "Shared plot-space sample points.", "items": { "$ref": "#/$defs/GameUiChartCursorPoint" }, "type": "array" }, "cursor_x": { "description": "Time-axis cursor, absent for a horizontal map.", "format": "double", "type": ["number", "null"] }, "id": { "description": "Retained chart control identity.", "type": "string" }, "selected_wind_vector": { "anyOf": [{ "$ref": "#/$defs/GameUiChartVector" }, { "type": "null" }], "description": "Selected-sample wind, replacing the first map vector only." } }, "required": ["id", "cursor_x", "cursor_points", "selected_wind_vector"], "type": "object" };
function validate236(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate236.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.id === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.cursor_x === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "cursor_x" }, message: "must have required property 'cursor_x'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.cursor_points === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "cursor_points" }, message: "must have required property 'cursor_points'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.selected_wind_vector === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "selected_wind_vector" }, message: "must have required property 'selected_wind_vector'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "cursor_points" || key0 === "cursor_x" || key0 === "id" || key0 === "selected_wind_vector")) {
        const err4 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.cursor_points !== void 0) {
      let data0 = data.cursor_points;
      if (Array.isArray(data0)) {
        const len0 = data0.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate118(data0[i0], { instancePath: instancePath + "/cursor_points/" + i0, parentData: data0, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate118.errors : vErrors.concat(validate118.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err5 = { instancePath: instancePath + "/cursor_points", schemaPath: "#/properties/cursor_points/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.cursor_x !== void 0) {
      let data2 = data.cursor_x;
      if (!(typeof data2 == "number" && isFinite(data2)) && data2 !== null) {
        const err6 = { instancePath: instancePath + "/cursor_x", schemaPath: "#/properties/cursor_x/type", keyword: "type", params: { type: schema98.properties.cursor_x.type }, message: "must be number,null" };
        if (vErrors === null) {
          vErrors = [err6];
        } else {
          vErrors.push(err6);
        }
        errors++;
      }
      if (typeof data2 == "number" && isFinite(data2)) {
        if (!formats0.validate(data2)) {
          const err7 = { instancePath: instancePath + "/cursor_x", schemaPath: "#/properties/cursor_x/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err7];
          } else {
            vErrors.push(err7);
          }
          errors++;
        }
      }
    }
    if (data.id !== void 0) {
      if (typeof data.id !== "string") {
        const err8 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.selected_wind_vector !== void 0) {
      let data4 = data.selected_wind_vector;
      const _errs10 = errors;
      let valid3 = false;
      const _errs11 = errors;
      if (!validate158(data4, { instancePath: instancePath + "/selected_wind_vector", parentData: data, parentDataProperty: "selected_wind_vector", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate158.errors : vErrors.concat(validate158.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs11 === errors;
      valid3 = valid3 || _valid0;
      const _errs12 = errors;
      if (data4 !== null) {
        const err9 = { instancePath: instancePath + "/selected_wind_vector", schemaPath: "#/properties/selected_wind_vector/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
      var _valid0 = _errs12 === errors;
      valid3 = valid3 || _valid0;
      if (!valid3) {
        const err10 = { instancePath: instancePath + "/selected_wind_vector", schemaPath: "#/properties/selected_wind_vector/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      } else {
        errors = _errs10;
        if (vErrors !== null) {
          if (_errs10) {
            vErrors.length = _errs10;
          } else {
            vErrors = null;
          }
        }
      }
    }
  } else {
    const err11 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err11];
    } else {
      vErrors.push(err11);
    }
    errors++;
  }
  validate236.errors = vErrors;
  return errors === 0;
}
validate236.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate241(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate241.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.id === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.value === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "value" }, message: "must have required property 'value'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.display_value === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "display_value" }, message: "must have required property 'display_value'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "display_value" || key0 === "id" || key0 === "value")) {
        const err3 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.display_value !== void 0) {
      if (typeof data.display_value !== "string") {
        const err4 = { instancePath: instancePath + "/display_value", schemaPath: "#/properties/display_value/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
    if (data.id !== void 0) {
      if (typeof data.id !== "string") {
        const err5 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err5];
        } else {
          vErrors.push(err5);
        }
        errors++;
      }
    }
    if (data.value !== void 0) {
      let data2 = data.value;
      if (typeof data2 == "number" && isFinite(data2)) {
        if (!formats0.validate(data2)) {
          const err6 = { instancePath: instancePath + "/value", schemaPath: "#/properties/value/format", keyword: "format", params: { format: "double" }, message: 'must match format "double"' };
          if (vErrors === null) {
            vErrors = [err6];
          } else {
            vErrors.push(err6);
          }
          errors++;
        }
      } else {
        const err7 = { instancePath: instancePath + "/value", schemaPath: "#/properties/value/type", keyword: "type", params: { type: "number" }, message: "must be number" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
  } else {
    const err8 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err8];
    } else {
      vErrors.push(err8);
    }
    errors++;
  }
  validate241.errors = vErrors;
  return errors === 0;
}
validate241.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate243(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate243.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.id === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "id" }, message: "must have required property 'id'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.value === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "value" }, message: "must have required property 'value'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "id" || key0 === "value")) {
        const err2 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err2];
        } else {
          vErrors.push(err2);
        }
        errors++;
      }
    }
    if (data.id !== void 0) {
      if (typeof data.id !== "string") {
        const err3 = { instancePath: instancePath + "/id", schemaPath: "#/properties/id/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err3];
        } else {
          vErrors.push(err3);
        }
        errors++;
      }
    }
    if (data.value !== void 0) {
      if (typeof data.value !== "string") {
        const err4 = { instancePath: instancePath + "/value", schemaPath: "#/properties/value/type", keyword: "type", params: { type: "string" }, message: "must be string" };
        if (vErrors === null) {
          vErrors = [err4];
        } else {
          vErrors.push(err4);
        }
        errors++;
      }
    }
  } else {
    const err5 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err5];
    } else {
      vErrors.push(err5);
    }
    errors++;
  }
  validate243.errors = vErrors;
  return errors === 0;
}
validate243.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
function validate233(data, { instancePath = "", parentData, parentDataProperty, rootData = data, dynamicAnchors = {} } = {}) {
  let vErrors = null;
  let errors = 0;
  const evaluated0 = validate233.evaluated;
  if (evaluated0.dynamicProps) {
    evaluated0.props = void 0;
  }
  if (evaluated0.dynamicItems) {
    evaluated0.items = void 0;
  }
  if (data && typeof data == "object" && !Array.isArray(data)) {
    if (data.view_revision === void 0) {
      const err0 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "view_revision" }, message: "must have required property 'view_revision'" };
      if (vErrors === null) {
        vErrors = [err0];
      } else {
        vErrors.push(err0);
      }
      errors++;
    }
    if (data.selections === void 0) {
      const err1 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "selections" }, message: "must have required property 'selections'" };
      if (vErrors === null) {
        vErrors = [err1];
      } else {
        vErrors.push(err1);
      }
      errors++;
    }
    if (data.hud === void 0) {
      const err2 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "hud" }, message: "must have required property 'hud'" };
      if (vErrors === null) {
        vErrors = [err2];
      } else {
        vErrors.push(err2);
      }
      errors++;
    }
    if (data.ranges === void 0) {
      const err3 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "ranges" }, message: "must have required property 'ranges'" };
      if (vErrors === null) {
        vErrors = [err3];
      } else {
        vErrors.push(err3);
      }
      errors++;
    }
    if (data.readouts === void 0) {
      const err4 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "readouts" }, message: "must have required property 'readouts'" };
      if (vErrors === null) {
        vErrors = [err4];
      } else {
        vErrors.push(err4);
      }
      errors++;
    }
    if (data.button_labels === void 0) {
      const err5 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "button_labels" }, message: "must have required property 'button_labels'" };
      if (vErrors === null) {
        vErrors = [err5];
      } else {
        vErrors.push(err5);
      }
      errors++;
    }
    if (data.chart_cursor === void 0) {
      const err6 = { instancePath, schemaPath: "#/required", keyword: "required", params: { missingProperty: "chart_cursor" }, message: "must have required property 'chart_cursor'" };
      if (vErrors === null) {
        vErrors = [err6];
      } else {
        vErrors.push(err6);
      }
      errors++;
    }
    for (const key0 in data) {
      if (!(key0 === "button_labels" || key0 === "chart_cursor" || key0 === "hud" || key0 === "ranges" || key0 === "readouts" || key0 === "selections" || key0 === "view_revision")) {
        const err7 = { instancePath, schemaPath: "#/additionalProperties", keyword: "additionalProperties", params: { additionalProperty: key0 }, message: "must NOT have additional properties" };
        if (vErrors === null) {
          vErrors = [err7];
        } else {
          vErrors.push(err7);
        }
        errors++;
      }
    }
    if (data.button_labels !== void 0) {
      let data0 = data.button_labels;
      if (Array.isArray(data0)) {
        const len0 = data0.length;
        for (let i0 = 0; i0 < len0; i0++) {
          if (!validate234(data0[i0], { instancePath: instancePath + "/button_labels/" + i0, parentData: data0, parentDataProperty: i0, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate234.errors : vErrors.concat(validate234.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err8 = { instancePath: instancePath + "/button_labels", schemaPath: "#/properties/button_labels/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err8];
        } else {
          vErrors.push(err8);
        }
        errors++;
      }
    }
    if (data.chart_cursor !== void 0) {
      let data2 = data.chart_cursor;
      const _errs6 = errors;
      let valid3 = false;
      const _errs7 = errors;
      if (!validate236(data2, { instancePath: instancePath + "/chart_cursor", parentData: data, parentDataProperty: "chart_cursor", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate236.errors : vErrors.concat(validate236.errors);
        errors = vErrors.length;
      }
      var _valid0 = _errs7 === errors;
      valid3 = valid3 || _valid0;
      const _errs8 = errors;
      if (data2 !== null) {
        const err9 = { instancePath: instancePath + "/chart_cursor", schemaPath: "#/properties/chart_cursor/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
        if (vErrors === null) {
          vErrors = [err9];
        } else {
          vErrors.push(err9);
        }
        errors++;
      }
      var _valid0 = _errs8 === errors;
      valid3 = valid3 || _valid0;
      if (!valid3) {
        const err10 = { instancePath: instancePath + "/chart_cursor", schemaPath: "#/properties/chart_cursor/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
        if (vErrors === null) {
          vErrors = [err10];
        } else {
          vErrors.push(err10);
        }
        errors++;
      } else {
        errors = _errs6;
        if (vErrors !== null) {
          if (_errs6) {
            vErrors.length = _errs6;
          } else {
            vErrors = null;
          }
        }
      }
    }
    if (data.hud !== void 0) {
      let data3 = data.hud;
      const _errs11 = errors;
      let valid4 = false;
      const _errs12 = errors;
      if (!validate56(data3, { instancePath: instancePath + "/hud", parentData: data, parentDataProperty: "hud", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate56.errors : vErrors.concat(validate56.errors);
        errors = vErrors.length;
      }
      var _valid1 = _errs12 === errors;
      valid4 = valid4 || _valid1;
      const _errs13 = errors;
      if (data3 !== null) {
        const err11 = { instancePath: instancePath + "/hud", schemaPath: "#/properties/hud/anyOf/1/type", keyword: "type", params: { type: "null" }, message: "must be null" };
        if (vErrors === null) {
          vErrors = [err11];
        } else {
          vErrors.push(err11);
        }
        errors++;
      }
      var _valid1 = _errs13 === errors;
      valid4 = valid4 || _valid1;
      if (!valid4) {
        const err12 = { instancePath: instancePath + "/hud", schemaPath: "#/properties/hud/anyOf", keyword: "anyOf", params: {}, message: "must match a schema in anyOf" };
        if (vErrors === null) {
          vErrors = [err12];
        } else {
          vErrors.push(err12);
        }
        errors++;
      } else {
        errors = _errs11;
        if (vErrors !== null) {
          if (_errs11) {
            vErrors.length = _errs11;
          } else {
            vErrors = null;
          }
        }
      }
    }
    if (data.ranges !== void 0) {
      let data4 = data.ranges;
      if (Array.isArray(data4)) {
        const len1 = data4.length;
        for (let i1 = 0; i1 < len1; i1++) {
          if (!validate241(data4[i1], { instancePath: instancePath + "/ranges/" + i1, parentData: data4, parentDataProperty: i1, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate241.errors : vErrors.concat(validate241.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err13 = { instancePath: instancePath + "/ranges", schemaPath: "#/properties/ranges/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err13];
        } else {
          vErrors.push(err13);
        }
        errors++;
      }
    }
    if (data.readouts !== void 0) {
      let data6 = data.readouts;
      if (Array.isArray(data6)) {
        const len2 = data6.length;
        for (let i2 = 0; i2 < len2; i2++) {
          if (!validate243(data6[i2], { instancePath: instancePath + "/readouts/" + i2, parentData: data6, parentDataProperty: i2, rootData, dynamicAnchors })) {
            vErrors = vErrors === null ? validate243.errors : vErrors.concat(validate243.errors);
            errors = vErrors.length;
          }
        }
      } else {
        const err14 = { instancePath: instancePath + "/readouts", schemaPath: "#/properties/readouts/type", keyword: "type", params: { type: "array" }, message: "must be array" };
        if (vErrors === null) {
          vErrors = [err14];
        } else {
          vErrors.push(err14);
        }
        errors++;
      }
    }
    if (data.selections !== void 0) {
      if (!validate173(data.selections, { instancePath: instancePath + "/selections", parentData: data, parentDataProperty: "selections", rootData, dynamicAnchors })) {
        vErrors = vErrors === null ? validate173.errors : vErrors.concat(validate173.errors);
        errors = vErrors.length;
      }
    }
    if (data.view_revision !== void 0) {
      let data9 = data.view_revision;
      if (!(typeof data9 == "number" && (!(data9 % 1) && !isNaN(data9)) && isFinite(data9))) {
        const err15 = { instancePath: instancePath + "/view_revision", schemaPath: "#/properties/view_revision/type", keyword: "type", params: { type: "integer" }, message: "must be integer" };
        if (vErrors === null) {
          vErrors = [err15];
        } else {
          vErrors.push(err15);
        }
        errors++;
      }
      if (typeof data9 == "number" && isFinite(data9)) {
        if (data9 < 0 || isNaN(data9)) {
          const err16 = { instancePath: instancePath + "/view_revision", schemaPath: "#/properties/view_revision/minimum", keyword: "minimum", params: { comparison: ">=", limit: 0 }, message: "must be >= 0" };
          if (vErrors === null) {
            vErrors = [err16];
          } else {
            vErrors.push(err16);
          }
          errors++;
        }
        if (!formats28.validate(data9)) {
          const err17 = { instancePath: instancePath + "/view_revision", schemaPath: "#/properties/view_revision/format", keyword: "format", params: { format: "uint32" }, message: 'must match format "uint32"' };
          if (vErrors === null) {
            vErrors = [err17];
          } else {
            vErrors.push(err17);
          }
          errors++;
        }
      }
    }
  } else {
    const err18 = { instancePath, schemaPath: "#/type", keyword: "type", params: { type: "object" }, message: "must be object" };
    if (vErrors === null) {
      vErrors = [err18];
    } else {
      vErrors.push(err18);
    }
    errors++;
  }
  validate233.errors = vErrors;
  return errors === 0;
}
validate233.evaluated = { "props": true, "dynamicProps": false, "dynamicItems": false };
export {
  validateGameUiAction,
  validateGameUiFrame,
  validateGameUiPlatformInput,
  validateGameUiUpdate,
  validateGameUiView
};

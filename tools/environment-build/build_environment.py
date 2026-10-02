import argparse
import copy
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile

from jma_normals import GroundNormal, MAX_SNAPSHOT_BYTES, SourceError, parse_normal


def strict_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise SourceError("duplicate recipe field: " + key)
        result[key] = value
    return result


def fields(value, expected: set[str], label: str) -> None:
    if not isinstance(value, dict) or set(value) != expected:
        raise SourceError("invalid recipe fields: " + label)


def number(value, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise SourceError("nonfinite or invalid number: " + label)
    try:
        converted = float(value)
    except OverflowError as error:
        raise SourceError("numeric overflow: " + label) from error
    if not math.isfinite(converted):
        raise SourceError("nonfinite or invalid number: " + label)
    return converted


def vector(value, length: int, label: str) -> list[float]:
    if not isinstance(value, list) or len(value) != length:
        raise SourceError("invalid vector: " + label)
    return [number(component, label) for component in value]


def toward_ne(speed: float, from_degrees: float) -> list[float]:
    angle = math.radians(from_degrees % 360)
    return [-speed * math.cos(angle), -speed * math.sin(angle)]


def generate(recipe: dict, normal: GroundNormal, recipe_bytes: bytes, recipe_name: str) -> dict:
    fields(recipe, {"recipe_version", "observation", "wind_model", "wave_wind_speed_scale", "environment"}, "root")
    if type(recipe["recipe_version"]) is not int or recipe["recipe_version"] != 1:
        raise SourceError("unsupported recipe version")
    observation = recipe["observation"]
    fields(observation, {"station", "month", "first_year", "last_year", "input_sha256", "source", "measurement_scope"}, "observation")
    for key in ("station", "month", "first_year", "last_year", "input_sha256"):
        if observation[key] != getattr(normal, key):
            raise SourceError("observation does not match recipe: " + key)
    model = recipe["wind_model"]
    fields(model, {"reference_speed_scale", "from_offset_degrees", "vertical_mps", "gradient_ned_per_m"}, "wind_model")
    speed_scale = number(model["reference_speed_scale"], "reference_speed_scale")
    if speed_scale < 0:
        raise SourceError("negative reference speed scale")
    reference_ne = toward_ne(normal.mean_speed_mps * speed_scale,
                             normal.prevailing_from_degrees + number(model["from_offset_degrees"], "from_offset_degrees"))
    reference = reference_ne + [number(model["vertical_mps"], "vertical_mps")]
    gradients = model["gradient_ned_per_m"]
    if not isinstance(gradients, list) or len(gradients) != 3:
        raise SourceError("wind gradient must have three velocity rows")
    gradients = [vector(row, 3, "gradient") for row in gradients]
    document = copy.deepcopy(recipe["environment"])
    if not isinstance(document, dict) or any(key in document for key in ("sources", "ground_wind_normals")):
        raise SourceError("environment template must not duplicate generated evidence")
    for component in ("local_frame", "wind_grid", "waves", "sky"):
        if document["provenance"][component]["kind"] not in ("assumed", "game_tuned"):
            raise SourceError("this recipe generator requires explicit assumptions/tuning: " + component)
    grid = document["wind_grid"]
    fields(grid, {"origin_ned_m", "spacing_ned_m", "counts_ned", "representative_position_ned_m"}, "wind_grid")
    origin = vector(grid["origin_ned_m"], 3, "origin")
    spacing = vector(grid["spacing_ned_m"], 3, "spacing")
    representative = vector(grid["representative_position_ned_m"], 3, "representative")
    counts = grid["counts_ned"]
    if (not isinstance(counts, list) or len(counts) != 3
            or any(type(count) is not int or count < 2 for count in counts)
            or math.prod(counts) > 65536 or any(step <= 0 for step in spacing)):
        raise SourceError("invalid grid dimensions")
    velocities = []
    for down in range(counts[2]):
        for east in range(counts[1]):
            for north in range(counts[0]):
                position = [origin[axis] + spacing[axis] * index
                            for axis, index in enumerate((north, east, down))]
                velocity = [reference[component] + sum(
                    gradients[component][axis] * (position[axis] - representative[axis])
                    for axis in range(3)) for component in range(3)]
                if not all(math.isfinite(component) for component in velocity):
                    raise SourceError("nonfinite generated grid velocity")
                velocities.append([round(component, 9) + 0.0 for component in velocity])
    grid["velocities_ned_mps"] = velocities
    wave_scale = number(recipe["wave_wind_speed_scale"], "wave_wind_speed_scale")
    if wave_scale < 0:
        raise SourceError("negative wave history scale")
    if "wind_velocity_ne_mps" in document["waves"]:
        raise SourceError("wave wind must be generated from the explicit history scale")
    document["waves"]["wind_velocity_ne_mps"] = [round(component, 9) + 0.0 for component in toward_ne(
        normal.mean_speed_mps * wave_scale, normal.prevailing_from_degrees)]
    source = copy.deepcopy(observation["source"])
    if "input_sha256" in source:
        raise SourceError("source hash must come from raw snapshot")
    source["input_sha256"] = normal.input_sha256
    sources = [source]
    for title, path, payload in (
        ("Environment generation recipe", recipe_name, recipe_bytes),
        ("Environment generator", "build_environment.py", Path(__file__).read_bytes()),
        ("JMA normal parser", "jma_normals.py", Path(__file__).with_name("jma_normals.py").read_bytes()),
    ):
        sources.append({
            "title": title,
            "url": "https://github.com/tokutori/pr-simulator-game-1/blob/main/tools/environment-build/" + path,
            "version": "SHA-256 pinned source bytes",
            "input_sha256": hashlib.sha256(payload).hexdigest(),
            "license": "MIT", "license_url": "https://github.com/tokutori/pr-simulator-game-1/blob/main/LICENSE",
            "attribution": "pr-simulator-game-1 contributors",
        })
    document["sources"] = sources
    document["ground_wind_normals"] = [{
        "source_index": 0, "station": normal.station, "first_year": normal.first_year,
        "last_year": normal.last_year, "month": normal.month,
        "mean_speed_mps": normal.mean_speed_mps,
        "prevailing_from_degrees": normal.prevailing_from_degrees,
        "measurement_scope": observation["measurement_scope"],
    }]
    return document


def main() -> None:
    arguments = argparse.ArgumentParser(description="Generate and Rust-validate an offline environment asset")
    arguments.add_argument("recipe", type=Path)
    arguments.add_argument("snapshot", type=Path)
    arguments.add_argument("output", type=Path)
    arguments.add_argument("--validator", required=True, type=Path)
    options = arguments.parse_args()
    try:
        if options.recipe.resolve().parent != Path(__file__).resolve().parent:
            raise SourceError("recipe must reside beside the tracked environment generator")
        inputs = (options.recipe.resolve(), options.snapshot.resolve(), options.validator.resolve(),
                  Path(__file__).resolve(), Path(__file__).with_name("jma_normals.py").resolve())
        if options.output.resolve() in inputs:
            raise SourceError("output must not overwrite an input or validator")
        with options.recipe.open("rb") as source:
            recipe_bytes = source.read(1024 * 1024 + 1)
        if len(recipe_bytes) > 1024 * 1024:
            raise SourceError("recipe exceeds 1 MiB")
        recipe = json.loads(recipe_bytes.decode("utf-8"), object_pairs_hook=strict_object)
        observation = recipe["observation"]
        with options.snapshot.open("rb") as source:
            snapshot = source.read(MAX_SNAPSHOT_BYTES + 1)
        normal = parse_normal(snapshot, observation["input_sha256"], observation["station"], observation["month"])
        document = generate(recipe, normal, recipe_bytes, options.recipe.name)
        encoded = (json.dumps(document, ensure_ascii=True, sort_keys=True, indent=2, allow_nan=False) + "\n").encode("utf-8")
        if len(encoded) > 8 * 1024 * 1024:
            raise SourceError("generated document exceeds codec bound")
        with tempfile.TemporaryDirectory() as directory:
            candidate = Path(directory) / "environment.json"
            candidate.write_bytes(encoded)
            subprocess.run([str(options.validator.resolve()), "validate-environment", str(candidate)], check=True)
        with tempfile.NamedTemporaryFile(dir=options.output.parent, delete=False) as output:
            temporary_path = Path(output.name)
            try:
                output.write(encoded)
                output.close()
                os.replace(temporary_path, options.output)
            finally:
                temporary_path.unlink(missing_ok=True)
    except (SourceError, ValueError, KeyError, TypeError, OSError, subprocess.CalledProcessError) as error:
        arguments.exit(2, str(error) + "\n")


if __name__ == "__main__":
    main()

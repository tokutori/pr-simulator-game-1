import copy
import hashlib
import json
import math
from pathlib import Path
import unittest

from build_environment import generate, number, strict_object, toward_ne
from jma_normals import GroundNormal, SourceError


RECIPE_PATH = Path(__file__).with_name("typical-july.recipe.json")


class EnvironmentGenerationTests(unittest.TestCase):
    def setUp(self) -> None:
        self.recipe_bytes = RECIPE_PATH.read_bytes()
        self.recipe = json.loads(self.recipe_bytes)
        observation = self.recipe["observation"]
        self.normal = GroundNormal("彦根", 1991, 2020, 7, 2.5, 315.0, observation["input_sha256"])

    def build(self, recipe=None):
        return generate(self.recipe if recipe is None else recipe, self.normal,
                        self.recipe_bytes, RECIPE_PATH.name)

    def test_ground_evidence_is_separate_and_hashes_are_actual_bytes(self) -> None:
        document = self.build()
        self.assertEqual(document["environment_version"], 6)
        self.assertEqual(document["ground_wind_normals"][0]["mean_speed_mps"], 2.5)
        self.assertEqual(document["sources"][0]["input_sha256"], self.normal.input_sha256)
        self.assertEqual(document["sources"][1]["input_sha256"], hashlib.sha256(self.recipe_bytes).hexdigest())
        self.assertEqual(document, self.build())
        self.assertNotIn("sources", self.recipe["environment"])

    def test_cardinal_from_directions_reverse_to_toward_components(self) -> None:
        for direction, expected in ((0, [-2, 0]), (90, [0, -2]), (180, [2, 0]), (270, [0, 2])):
            result = toward_ne(2, direction)
            for actual, target in zip(result, expected):
                self.assertAlmostEqual(actual, target, places=12)
        north, east = toward_ne(2.5, 315)
        self.assertLess(north, 0)
        self.assertGreater(east, 0)

    def test_affine_grid_is_north_fast_and_keeps_below_water_margin(self) -> None:
        document = self.build()
        grid = document["wind_grid"]
        velocities = grid["velocities_ned_mps"]
        self.assertEqual(len(velocities), math.prod(grid["counts_ned"]))
        self.assertAlmostEqual(velocities[1][0] - velocities[0][0], 0.02, places=8)
        self.assertAlmostEqual(velocities[5][0] - velocities[0][0], 0.01, places=8)
        self.assertAlmostEqual(velocities[25][0] - velocities[0][0], -0.017, places=8)
        self.assertEqual(grid["origin_ned_m"][2] + grid["spacing_ned_m"][2] * 3, 10)
        self.assertTrue(all(sample[2] == 0 for sample in velocities))

    def test_waves_use_separate_history_scale(self) -> None:
        recipe = copy.deepcopy(self.recipe)
        recipe["wave_wind_speed_scale"] = 0
        document = self.build(recipe)
        self.assertEqual(document["waves"]["wind_velocity_ne_mps"], [0, 0])
        self.assertNotEqual(document["wind_grid"]["velocities_ned_mps"][0][:2], [0, 0])

    def test_observation_period_identity_and_hash_mismatch(self) -> None:
        for key, value in (("first_year", 1990), ("last_year", 2021), ("month", 8),
                           ("station", "京都"), ("input_sha256", "0" * 64)):
            recipe = copy.deepcopy(self.recipe)
            recipe["observation"][key] = value
            with self.subTest(key=key), self.assertRaises(SourceError):
                self.build(recipe)

    def test_generated_assumptions_cannot_claim_direct_observation(self) -> None:
        for component in ("local_frame", "wind_grid", "waves", "sky"):
            recipe = copy.deepcopy(self.recipe)
            recipe["environment"]["provenance"][component] = {"kind": "observed", "source_index": 0}
            with self.subTest(component=component), self.assertRaises(SourceError):
                self.build(recipe)

    def test_invalid_dimensions_rejected_before_generation(self) -> None:
        for counts in ([1, 2, 2], [True, 2, 2], [100, 100, 100], [2, 2], [2.0, 2, 2]):
            recipe = copy.deepcopy(self.recipe)
            recipe["environment"]["wind_grid"]["counts_ned"] = counts
            with self.subTest(counts=counts), self.assertRaises(SourceError):
                self.build(recipe)

    def test_nonfinite_huge_boolean_and_negative_scales(self) -> None:
        for value in (10 ** 1000, float("nan"), float("inf"), True, "2.5"):
            with self.subTest(value=str(value)[:20]), self.assertRaises(SourceError):
                number(value, "test")
        for key in ("reference_speed_scale", "vertical_mps"):
            recipe = copy.deepcopy(self.recipe)
            recipe["wind_model"][key] = float("inf")
            with self.subTest(key=key), self.assertRaises(SourceError):
                self.build(recipe)
        recipe = copy.deepcopy(self.recipe)
        recipe["wind_model"]["reference_speed_scale"] = -1
        with self.assertRaises(SourceError):
            self.build(recipe)

    def test_duplicate_unknown_and_generated_fields_are_rejected(self) -> None:
        with self.assertRaises(SourceError):
            json.loads('{"recipe_version":1,"recipe_version":2}', object_pairs_hook=strict_object)
        recipe = copy.deepcopy(self.recipe)
        recipe["extra"] = 1
        with self.assertRaises(SourceError):
            self.build(recipe)
        for key in ("sources", "ground_wind_normals"):
            recipe = copy.deepcopy(self.recipe)
            recipe["environment"][key] = []
            with self.subTest(key=key), self.assertRaises(SourceError):
                self.build(recipe)


if __name__ == "__main__":
    unittest.main()

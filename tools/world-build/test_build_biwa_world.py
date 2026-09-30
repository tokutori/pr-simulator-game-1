import unittest

from build_biwa_world import FINE_PATCHES, shoreline_land_side, simplify_closed_ring, supplemental_land_height


class ShorelineLandSideTests(unittest.TestCase):
    def setUp(self) -> None:
        self.water_ring = [(-100.0, -50.0), (-100.0, 50.0), (100.0, 50.0),
                           (100.0, -50.0), (-100.0, -50.0)]

    def test_classifies_the_land_side_of_a_south_shore(self) -> None:
        line = [(-100.0, -80.0), (-100.0, 80.0)]
        self.assertEqual(shoreline_land_side(line, self.water_ring), -1)
        self.assertEqual(shoreline_land_side(list(reversed(line)), self.water_ring), 1)

    def test_classifies_the_land_side_of_an_east_shore(self) -> None:
        line = [(-120.0, 50.0), (120.0, 50.0)]
        self.assertEqual(shoreline_land_side(line, self.water_ring), -1)
        self.assertEqual(shoreline_land_side(list(reversed(line)), self.water_ring), 1)


class ClosedIslandSimplificationTests(unittest.TestCase):
    def test_keeps_small_islands_as_closed_polygons(self) -> None:
        ring = [(0.0, 0.0), (2.0, 0.0), (1.0, 1.0), (0.0, 0.0)]

        simplified = simplify_closed_ring(ring, 8.0)

        self.assertEqual(simplified, ring[:-1])
        self.assertGreaterEqual(len(set(simplified)), 3)

    def test_simplifies_large_rings_without_repeating_the_closing_vertex(self) -> None:
        ring = [(0.0, 0.0), (0.0, 100.0), (2.0, 200.0), (100.0, 200.0),
                (100.0, 0.0), (0.0, 0.0)]

        simplified = simplify_closed_ring(ring, 8.0)

        self.assertLess(len(simplified), len(ring) - 1)
        self.assertGreaterEqual(len(set(simplified)), 3)
        self.assertNotEqual(simplified[0], simplified[-1])


class SupplementalElevationTests(unittest.TestCase):
    def test_only_fills_missing_land_and_applies_local_alignment(self) -> None:
        self.assertAlmostEqual(supplemental_land_height(100.0, True), 15.929)
        self.assertIsNone(supplemental_land_height(83.5, False))
        self.assertIsNone(supplemental_land_height(-9999.0, True))


class LaunchShoreTerrainCoverageTests(unittest.TestCase):
    def test_high_detail_patch_contains_the_local_hikone_coastline(self) -> None:
        patch = next(entry for entry in FINE_PATCHES if entry["id"] == "terrain-patch-regional-shore")
        north_min = int(patch["northMinMeters"])
        east_min = int(patch["eastMinMeters"])
        step = int(patch["stepMeters"])
        north_max = north_min + (int(patch["rows"]) - 1) * step
        east_max = east_min + (int(patch["columns"]) - 1) * step

        for north, east in ((-4_954.7, -5_531.3), (3_944.5, 1_197.8)):
            self.assertTrue(north_min <= north <= north_max)
            self.assertTrue(east_min <= east <= east_max)
        self.assertEqual(step, 30)
        self.assertEqual(patch["columns"], 401)
        self.assertEqual(patch["rows"], 401)

    def test_wooded_osm_islet_has_source_pixel_resolution(self) -> None:
        patch = next(entry for entry in FINE_PATCHES
                     if entry["id"] == "terrain-patch-wooded-islet-1156534005")
        self.assertEqual(patch["stepMeters"], 10)
        self.assertEqual(patch["sampleRadius"], 0)
        self.assertLessEqual(int(patch["northMinMeters"]), 16_437)
        self.assertGreaterEqual(int(patch["northMinMeters"]) + (int(patch["rows"]) - 1) * 10, 16_464)
        self.assertLessEqual(int(patch["eastMinMeters"]), -6_147)
        self.assertGreaterEqual(int(patch["eastMinMeters"]) + (int(patch["columns"]) - 1) * 10, -6_099)

if __name__ == "__main__":
    unittest.main()

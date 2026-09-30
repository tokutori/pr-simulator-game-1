"""Build redistributable Lake Biwa shoreline and terrain assets.

The OSM extract is an input only and is not bundled. AW3D30 GeoTIFF inputs are
also kept outside the repository. Only OSM shoreline coordinates and filtered,
decimated elevation samples are written to assets/.

Install the pinned host-side dependencies with:
  python -m pip install -r tools/world-build/requirements.txt
"""

from __future__ import annotations

import argparse
import bisect
import hashlib
import json
import math
import xml.etree.ElementTree as ET
from pathlib import Path

ORIGIN_LAT = 35.294075
ORIGIN_LON = 136.254448
LAKE_RELATION_ID = "63499"
WATER_DATUM_METERS = 84.371
# Local robust alignment of Copernicus EGM2008 heights to the AW3D30 EGM96
# surface in OSM-classified land around Hikone. This is a rendering adjustment,
# not a geodetic datum transformation.
COPDEM_TO_AW3D30_ALIGNMENT_METERS = 0.3
NORTH_MIN = -20_000
NORTH_MAX = 36_000
EAST_MIN = -66_000
EAST_MAX = 21_000
TERRAIN_STEP = 300
FINE_PATCHES = (
    # Keep the 12 km flight region and its nearby shorelines at native 30 m
    # sampling; coarse regional grids can miss narrow coastal land.
    {"id": "terrain-patch-regional-shore", "northMinMeters": -6_030, "eastMinMeters": -6_030, "columns": 401, "rows": 401, "stepMeters": 30, "sampleRadius": 1, "blendWidthMeters": 1_800, "blendTo": "broad-terrain"},
    {"id": "terrain-patch-takeshima", "northMinMeters": -150, "eastMinMeters": -7_650, "columns": 51, "rows": 51, "stepMeters": 30},
    {"id": "terrain-patch-chikubushima", "sourceTile": "E136-N35", "northMinMeters": 13_800, "eastMinMeters": -10_500, "columns": 32, "rows": 30, "stepMeters": 30},
    {"id": "terrain-patch-okishima", "northMinMeters": -11_100, "eastMinMeters": -19_200, "columns": 121, "rows": 81, "stepMeters": 30},
    {"id": "terrain-patch-okonozu", "northMinMeters": 14_400, "eastMinMeters": -6_500, "columns": 41, "rows": 41, "stepMeters": 30},
    # This OSM-mapped wooded islet is only about 50 m across. A 30 m terrain
    # grid cannot place enough land vertices inside its outline to form a
    # surface, so sample the source DSM on a 10 m grid using exact source pixels.
    {"id": "terrain-patch-wooded-islet-1156534005", "northMinMeters": 16_380, "eastMinMeters": -6_170, "columns": 14, "rows": 11, "stepMeters": 10, "sampleRadius": 0},
)
RIDGE_PEAKS = (
    {"id": "terrain-patch-ridge-northwest", "northMeters": 20_894, "eastMeters": -27_271},
    {"id": "terrain-patch-ridge-north", "northMeters": 28_457, "eastMeters": 7_508},
    {"id": "terrain-patch-ridge-northwest-far", "northMeters": 32_840, "eastMeters": -20_885},
    {"id": "terrain-patch-ridge-west", "northMeters": -3_277, "eastMeters": -32_495},
)
RIDGE_PATCH_HALF_EXTENT_METERS = 1_200
RIDGE_PATCH_BLEND_METERS = 600
RIDGE_PATCH_STEP_METERS = 90
VENUE_BOUNDS = (-1_500.0, 1_500.0, -1_500.0, 1_500.0)
VENUE_OSM_BOUNDS_URL = "136.2375,35.2805,136.2715,35.3077"
VENUE_FEATURE_TAGS = {
    ("natural", "beach"): "beach",
    ("natural", "sand"): "beach",
    ("landuse", "forest"): "woodland",
    ("natural", "wood"): "woodland",
    ("natural", "tree_row"): "tree-row",
    ("man_made", "pier"): "pier",
    ("man_made", "quay"): "quay",
    ("man_made", "breakwater"): "breakwater",
}
METERS_PER_LAT_DEGREE = 111_132.0
METERS_PER_LON_DEGREE = 111_320.0 * math.cos(math.radians(ORIGIN_LAT))
# Some OSM inner rings are assembled from untagged ways; these stable IDs map
# their shoreline geometry to the island names used by the local scene.
ISLAND_NAMES = {"41039549": "多景島", "41039563": "沖島", "41039555": "オコノ洲", "183556759": "竹生島"}


def project(lat: float, lon: float) -> tuple[float, float]:
    return ((lat - ORIGIN_LAT) * METERS_PER_LAT_DEGREE,
            (lon - ORIGIN_LON) * METERS_PER_LON_DEGREE)


def simplify(points: list[tuple[float, float]], tolerance: float) -> list[tuple[float, float]]:
    if len(points) <= 2:
        return points
    start, end = points[0], points[-1]
    dx, dy = end[0] - start[0], end[1] - start[1]
    denominator = dx * dx + dy * dy
    farthest = 0
    maximum = -1.0
    for index in range(1, len(points) - 1):
        point = points[index]
        fraction = 0.0 if denominator == 0 else max(0.0, min(1.0, ((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / denominator))
        distance = math.dist(point, (start[0] + fraction * dx, start[1] + fraction * dy))
        if distance > maximum:
            farthest, maximum = index, distance
    if maximum <= tolerance:
        return [start, end]
    return simplify(points[:farthest + 1], tolerance)[:-1] + simplify(points[farthest:], tolerance)


def simplify_closed_ring(points: list[tuple[float, float]], tolerance: float) -> list[tuple[float, float]]:
    """Simplify a closed polygon without collapsing small islands to lines or points."""
    ring = points[:-1] if len(points) > 1 and points[0] == points[-1] else points[:]
    ring = [point for index, point in enumerate(ring) if index == 0 or point != ring[index - 1]]
    if len(ring) < 3 or len(set(ring)) < 3:
        raise ValueError("Closed shoreline ring requires at least three distinct points")
    simplified = simplify(ring + [ring[0]], tolerance)
    if len(simplified) > 1 and simplified[0] == simplified[-1]:
        simplified = simplified[:-1]
    area_twice = sum(
        current[1] * following[0] - following[1] * current[0]
        for current, following in zip(simplified, simplified[1:] + simplified[:1])
    )
    if len(set(simplified)) < 3 or abs(area_twice) < 1e-6:
        simplified = ring
    if len(simplified) < 3:
        raise ValueError("Closed shoreline ring simplification produced a degenerate polygon")
    return simplified


def clip_segment(
    a: tuple[float, float], b: tuple[float, float],
    bounds: tuple[float, float, float, float] = (NORTH_MIN, NORTH_MAX, EAST_MIN, EAST_MAX),
) -> tuple[tuple[float, float], tuple[float, float]] | None:
    """Clip a (north, east) segment to (north min/max, east min/max)."""
    low_n, high_n, low_e, high_e = bounds
    dn, de = b[0] - a[0], b[1] - a[1]
    lower, upper = 0.0, 1.0
    for p, q in ((-dn, a[0] - low_n), (dn, high_n - a[0]), (-de, a[1] - low_e), (de, high_e - a[1])):
        if p == 0:
            if q < 0:
                return None
            continue
        ratio = q / p
        if p < 0:
            lower = max(lower, ratio)
        else:
            upper = min(upper, ratio)
        if lower > upper:
            return None
    return ((a[0] + lower * dn, a[1] + lower * de), (a[0] + upper * dn, a[1] + upper * de))


def clip_polyline(
    points: list[tuple[float, float]],
    bounds: tuple[float, float, float, float] = (NORTH_MIN, NORTH_MAX, EAST_MIN, EAST_MAX),
) -> list[list[tuple[float, float]]]:
    fragments: list[list[tuple[float, float]]] = []
    current: list[tuple[float, float]] = []
    for first, second in zip(points, points[1:]):
        clipped = clip_segment(first, second, bounds)
        if clipped is None:
            if len(current) >= 2:
                fragments.append(current)
            current = []
            continue
        start, end = clipped
        if not current or math.dist(current[-1], start) > 0.01:
            if len(current) >= 2:
                fragments.append(current)
            current = [start]
        current.append(end)
    if len(current) >= 2:
        fragments.append(current)
    return fragments


def clip_polygon(points: list[tuple[float, float]], bounds: tuple[float, float, float, float]) -> list[tuple[float, float]]:
    """Clip a closed north/east ring to the venue rectangle."""
    polygon = points[:-1] if len(points) > 1 and points[0] == points[-1] else list(points)
    low_n, high_n, low_e, high_e = bounds
    for axis, boundary, keep_greater in ((0, low_n, True), (0, high_n, False),
                                         (1, low_e, True), (1, high_e, False)):
        if not polygon:
            return []
        clipped: list[tuple[float, float]] = []
        previous = polygon[-1]
        previous_inside = previous[axis] >= boundary if keep_greater else previous[axis] <= boundary
        for current in polygon:
            current_inside = current[axis] >= boundary if keep_greater else current[axis] <= boundary
            if current_inside != previous_inside:
                denominator = current[axis] - previous[axis]
                fraction = 0.0 if denominator == 0 else (boundary - previous[axis]) / denominator
                intersection = (previous[0] + (current[0] - previous[0]) * fraction,
                                previous[1] + (current[1] - previous[1]) * fraction)
                clipped.append(intersection)
            if current_inside:
                clipped.append(current)
            previous = current
            previous_inside = current_inside
        polygon = clipped
    if len(polygon) < 3:
        return []
    return polygon + [polygon[0]]


def read_venue_features(path: Path) -> dict[str, object]:
    """Extract mapped beach, vegetation cover, and fixed shoreline structures."""
    root = ET.parse(path).getroot()
    nodes = {node.attrib["id"]: (float(node.attrib["lat"]), float(node.attrib["lon"]))
             for node in root.findall("node")}
    features: list[dict[str, object]] = []
    for way in root.findall("way"):
        tags = {tag.attrib["k"]: tag.attrib["v"] for tag in way.findall("tag")}
        kind = next((feature_kind for (key, value), feature_kind in VENUE_FEATURE_TAGS.items()
                     if tags.get(key) == value), None)
        if kind is None:
            continue
        coords = [project(*nodes[nd.attrib["ref"]]) for nd in way.findall("nd") if nd.attrib["ref"] in nodes]
        if len(coords) < 2:
            continue
        closed = coords[0] == coords[-1]
        if closed:
            clipped = clip_polygon(coords, VENUE_BOUNDS)
            if not clipped:
                continue
            tolerance = 2.0 if kind in ("beach", "woodland") else 0.5
            simplified = simplify(clipped, tolerance)
            if len(simplified) < 4:
                continue
            coords = simplified
        else:
            fragments = clip_polyline(coords, VENUE_BOUNDS)
            if not fragments:
                continue
            # The venue subset is small and all selected line features are local;
            # preserve each clipped fragment independently when a way crosses a bound.
            for fragment_index, fragment in enumerate(fragments):
                simplified = simplify(fragment, 0.5)
                if len(simplified) < 2:
                    continue
                source_tags = {key: tags[key] for key in ("natural", "landuse", "man_made", "surface") if key in tags}
                features.append({
                    "id": f"osm-way-{way.attrib['id']}-{fragment_index + 1}",
                    "osmWayId": int(way.attrib["id"]),
                    "osmVersion": int(way.attrib.get("version", "1")),
                    "osmTimestamp": way.attrib.get("timestamp", ""),
                    "kind": kind,
                    "name": tags.get("name"),
                    "sourceTags": source_tags,
                    "closed": False,
                    "northEastMeters": [[round(n, 1), round(e, 1)] for n, e in simplified],
                })
            continue
        source_tags = {key: tags[key] for key in ("natural", "landuse", "man_made", "surface") if key in tags}
        features.append({
            "id": f"osm-way-{way.attrib['id']}",
            "osmWayId": int(way.attrib["id"]),
            "osmVersion": int(way.attrib.get("version", "1")),
            "osmTimestamp": way.attrib.get("timestamp", ""),
            "kind": kind,
            "name": tags.get("name"),
            "sourceTags": source_tags,
            "closed": True,
            "northEastMeters": [[round(n, 1), round(e, 1)] for n, e in simplified],
        })
    features.sort(key=lambda item: (str(item["kind"]), int(item["osmWayId"]), str(item["id"])))
    return {
        "schemaVersion": 1,
        "originWgs84": {"latitudeDegrees": ORIGIN_LAT, "longitudeDegrees": ORIGIN_LON},
        "license": "ODbL-1.0",
        "licenseUrl": "https://opendatacommons.org/licenses/odbl/1-0/",
        "attribution": "© OpenStreetMap contributors",
        "sourceUrl": f"https://api.openstreetmap.org/api/0.6/map?bbox={VENUE_OSM_BOUNDS_URL}",
        "sourceSnapshotSha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "boundsMeters": {"northMin": -1_500, "northMax": 1_500, "eastMin": -1_500, "eastMax": 1_500},
        "projection": "local WGS84 tangent approximation; north/east metres relative to launch origin",
        "features": features,
        "processing": "tools/world-build/build_biwa_world.py; selected natural=beach/sand, forest/wood, tree_row, pier/quay/breakwater ways; clipped to 3 km square and simplified by kind; no raw OSM XML bundled",
    }


def joined_ring(ways: list[tuple[str, list[tuple[float, float]]]]) -> list[tuple[float, float]]:
    remaining = list(ways)
    if not remaining:
        return []
    _, first = remaining.pop(0)
    ring = list(first)
    while remaining and ring[-1] != ring[0]:
        match = next((index for index, (_, path) in enumerate(remaining)
                      if path[0] == ring[-1] or path[-1] == ring[-1]), None)
        if match is None:
            return []
        _, path = remaining.pop(match)
        ring.extend(path[1:] if path[0] == ring[-1] else list(reversed(path[:-1])))
    return ring if len(ring) >= 4 and ring[-1] == ring[0] else []


def point_in_ring(north: float, east: float, ring: list[tuple[float, float]]) -> bool:
    inside = False
    for current, following in zip(ring, ring[1:]):
        if (current[0] > north) != (following[0] > north):
            crossing_east = (following[1] - current[1]) * (north - current[0]) / (following[0] - current[0]) + current[1]
            if east < crossing_east:
                inside = not inside
    return inside


def shoreline_land_side(line: list[tuple[float, float]], water_ring: list[tuple[float, float]]) -> int:
    """Return +1 for the line's left side or -1 for its right side on land."""
    segment_indices = sorted(range(len(line) - 1), key=lambda index: abs(index - (len(line) - 2) / 2))
    for distance in (25.0, 50.0, 100.0, 200.0, 500.0, 1_000.0):
        for index in segment_indices:
            start, end = line[index], line[index + 1]
            delta_north, delta_east = end[0] - start[0], end[1] - start[1]
            length = math.hypot(delta_north, delta_east)
            if length < 1.0:
                continue
            midpoint_north = (start[0] + end[0]) / 2
            midpoint_east = (start[1] + end[1]) / 2
            left_is_water = point_in_ring(
                midpoint_north + delta_east / length * distance,
                midpoint_east - delta_north / length * distance,
                water_ring,
            )
            right_is_water = point_in_ring(
                midpoint_north - delta_east / length * distance,
                midpoint_east + delta_north / length * distance,
                water_ring,
            )
            if left_is_water != right_is_water:
                return -1 if left_is_water else 1
    raise ValueError("Could not determine land side for a clipped shoreline fragment")


def ring_east_crossings(north: float, ring: list[tuple[float, float]]) -> list[float]:
    crossings = []
    for current, following in zip(ring, ring[1:]):
        if (current[0] > north) != (following[0] > north):
            fraction = (north - current[0]) / (following[0] - current[0])
            crossings.append(current[1] + (following[1] - current[1]) * fraction)
    return sorted(crossings)


def build_land_mask_asset(water_ring: list[tuple[float, float]], islands: list[dict[str, object]],
                          source_hash: str, relation_version: str, relation_timestamp: str) -> dict[str, object]:
    grids: list[dict[str, object]] = [{
        "id": "broad-terrain", "northMinMeters": NORTH_MIN, "eastMinMeters": EAST_MIN,
        "columns": (EAST_MAX - EAST_MIN) // TERRAIN_STEP + 1,
        "rows": (NORTH_MAX - NORTH_MIN) // TERRAIN_STEP + 1,
        "terrainStepMeters": TERRAIN_STEP,
    }]
    for patch in FINE_PATCHES:
        grids.append({
            "id": str(patch["id"]), "northMinMeters": int(patch["northMinMeters"]),
            "eastMinMeters": int(patch["eastMinMeters"]), "columns": int(patch["columns"]),
            "rows": int(patch["rows"]), "terrainStepMeters": int(patch["stepMeters"]),
        })
    for peak in RIDGE_PEAKS:
        north_min = math.floor((int(peak["northMeters"]) - RIDGE_PATCH_HALF_EXTENT_METERS) / RIDGE_PATCH_STEP_METERS) * RIDGE_PATCH_STEP_METERS
        east_min = math.floor((int(peak["eastMeters"]) - RIDGE_PATCH_HALF_EXTENT_METERS) / RIDGE_PATCH_STEP_METERS) * RIDGE_PATCH_STEP_METERS
        dimension = math.ceil(2 * RIDGE_PATCH_HALF_EXTENT_METERS / RIDGE_PATCH_STEP_METERS) + 1
        grids.append({
            "id": str(peak["id"]), "northMinMeters": north_min, "eastMinMeters": east_min,
            "columns": dimension, "rows": dimension, "terrainStepMeters": RIDGE_PATCH_STEP_METERS,
        })

    island_rings = [entry["ringNorthEastMeters"] for entry in islands]
    encoded_grids = []
    for grid in grids:
        rows, columns = int(grid["rows"]), int(grid["columns"])
        north_min, east_min = int(grid["northMinMeters"]), int(grid["eastMinMeters"])
        step = int(grid["terrainStepMeters"])
        packed = [0] * math.ceil(rows * columns / 4)
        for row in range(rows):
            north = north_min + (rows - row - 1) * step
            outer_crossings = ring_east_crossings(north, water_ring)
            island_crossings = [ring_east_crossings(north, ring) for ring in island_rings]
            for column in range(columns):
                east = east_min + column * step
                water = bisect.bisect_right(outer_crossings, east) % 2 == 1
                island_land = any(bisect.bisect_right(crossings, east) % 2 == 1 for crossings in island_crossings)
                if not water or island_land:
                    index = row * columns + column
                    packed[index // 4] |= 1 << (index % 4)
        encoded_grids.append({**grid, "landMaskHex": "".join(format(value, "x") for value in packed)})
    return {
        "schemaVersion": 1,
        "originWgs84": {"latitudeDegrees": ORIGIN_LAT, "longitudeDegrees": ORIGIN_LON},
        "source": "OpenStreetMap Lake Biwa relation 63499 outer water ring and inner island rings",
        "sourceVersion": f"relation version {relation_version}, {relation_timestamp}",
        "sourceSnapshotSha256": source_hash,
        "license": "ODbL-1.0",
        "licenseUrl": "https://opendatacommons.org/licenses/odbl/1-0/",
        "attribution": "© OpenStreetMap contributors",
        "processing": "tools/world-build/build_biwa_world.py; land/water classification sampled at the matching AW3D30 vertices; four grid samples are packed per hexadecimal digit",
        "grids": encoded_grids,
    }


def read_osm(path: Path) -> tuple[list[list[tuple[float, float]]], list[dict[str, object]], list[tuple[float, float]], str, str]:
    root = ET.parse(path).getroot()
    nodes = {node.attrib["id"]: (float(node.attrib["lat"]), float(node.attrib["lon"]))
             for node in root.findall("node")}
    ways = {way.attrib["id"]: way for way in root.findall("way")}
    relation = next(item for item in root.findall("relation") if item.attrib["id"] == LAKE_RELATION_ID)
    outer: list[tuple[str, list[tuple[float, float]]]] = []
    inner: list[tuple[str, list[tuple[float, float]]]] = []
    shoreline_lines: list[list[tuple[float, float]]] = []
    for member in relation.findall("member"):
        if member.attrib.get("type") != "way":
            continue
        way_id = member.attrib["ref"]
        way = ways[way_id]
        tags = {tag.attrib["k"]: tag.attrib["v"] for tag in way.findall("tag")}
        coords = [nodes[point.attrib["ref"]] for point in way.findall("nd")]
        projected = [project(*coord) for coord in coords]
        role = member.attrib.get("role")
        if role == "outer":
            outer.append((way_id, projected))
            for fragment in clip_polyline(projected):
                middle = fragment[len(fragment) // 2]
                # Preserve the launch-side shoreline more closely than remote coasts.
                tolerance = 2.0 if math.hypot(*middle) < 3_000 else 20.0
                shoreline_lines.append(simplify(fragment, tolerance))
        elif role == "inner":
            inner.append((way_id, projected))

    water_ring = joined_ring(outer)
    if not water_ring:
        raise ValueError("Could not assemble Lake Biwa outer shoreline ring")

    island_groups: list[list[tuple[str, list[tuple[float, float]]]]] = []
    pending = list(inner)
    while pending:
        seed = pending.pop(0)
        group = [seed]
        chain = list(seed[1])
        changed = True
        while changed and chain[-1] != chain[0]:
            changed = False
            for index, candidate in enumerate(pending):
                candidate_path = candidate[1]
                if candidate_path[0] == chain[-1]:
                    chain.extend(candidate_path[1:])
                elif candidate_path[-1] == chain[-1]:
                    chain.extend(reversed(candidate_path[:-1]))
                else:
                    continue
                group.append(candidate)
                pending.pop(index)
                changed = True
                break
        if chain[-1] == chain[0] and len(chain) >= 4:
            island_groups.append(group)

    islands: list[dict[str, object]] = []
    for group in island_groups:
        ring = group[0][1]
        if len(group) > 1:
            ring = list(group[0][1])
            for _, path in group[1:]:
                if path[0] == ring[-1]:
                    ring.extend(path[1:])
                elif path[-1] == ring[-1]:
                    ring.extend(reversed(path[:-1]))
        if any(NORTH_MIN <= n <= NORTH_MAX and EAST_MIN <= e <= EAST_MAX for n, e in ring):
            ids = [way_id for way_id, _ in group]
            name = next((ISLAND_NAMES[way_id] for way_id in ids if way_id in ISLAND_NAMES), None)
            # Tiny islands are kept as individual loops; only simplify their outline.
            simplified = simplify_closed_ring(ring, 8.0)
            islands.append({"wayIds": [int(value) for value in ids], "name": name,
                            "ringNorthEastMeters": [[round(n, 1), round(e, 1)] for n, e in simplified]})

    return shoreline_lines, islands, water_ring, relation.attrib["version"], relation.attrib["timestamp"]


def load_tile(dsm_path: Path, mask_path: Path, west_longitude: int) -> tuple[np.ndarray, np.ndarray]:
    import numpy as np
    import tifffile

    with tifffile.TiffFile(dsm_path) as source:
        elevation = source.pages[0].asarray()
    with tifffile.TiffFile(mask_path) as source:
        mask = source.pages[0].asarray()
    if elevation.shape != (3600, 3600) or mask.shape != elevation.shape:
        raise ValueError(f"Unexpected AW3D30 tile shape for E{west_longitude}")
    return elevation, mask


def load_copdem_tile(path: Path) -> np.ndarray:
    """Load a public Copernicus GLO-30 COG tile used only as land fallback."""
    import tifffile

    with tifffile.TiffFile(path) as source:
        elevation = source.pages[0].asarray()
    if elevation.shape != (3600, 3600):
        raise ValueError(f"Unexpected Copernicus GLO-30 tile shape: {elevation.shape}")
    return elevation


def supplemental_land_height(raw_height: float, is_land: bool) -> float | None:
    """Convert one CopDEM sample to the render datum only for mapped land."""
    if not is_land or not math.isfinite(raw_height) or raw_height <= -9_000:
        return None
    return max(0.0, raw_height - WATER_DATUM_METERS + COPDEM_TO_AW3D30_ALIGNMENT_METERS)


def sample_elevation(tiles: dict[int, tuple[np.ndarray, np.ndarray]], north: float, east: float,
                    radius: int = 4, minimum_samples: int = 8) -> float | None:
    import numpy as np

    lat = ORIGIN_LAT + north / METERS_PER_LAT_DEGREE
    lon = ORIGIN_LON + east / METERS_PER_LON_DEGREE
    west = math.floor(lon)
    tile = tiles.get(west)
    if tile is None or not 35 <= lat < 36:
        return None
    elevation, mask = tile
    column = int((lon - west) * 3600)
    row = int((36 - lat) * 3600)
    if mask[row, column] not in (0, 12):
        return None
    r0, r1 = max(0, row - radius), min(3600, row + radius + 1)
    c0, c1 = max(0, column - radius), min(3600, column + radius + 1)
    height_window = elevation[r0:r1, c0:c1]
    mask_window = mask[r0:r1, c0:c1]
    # 0 = original valid DSM; 12 = PRISM DSM. Exclude GSI DEM (4), all other
    # third-party fill classes, invalid pixels, and water.
    valid = np.isin(mask_window, (0, 12)) & (height_window != -9999)
    samples = height_window[valid]
    if samples.size < minimum_samples:
        return None
    return float(np.median(samples)) - WATER_DATUM_METERS


def sample_grid(tiles: dict[int, tuple[np.ndarray, np.ndarray]], north_min: int, east_min: int,
                columns: int, rows: int, step: int, radius: int = 4, minimum_samples: int = 8,
                land_mask: list[bool] | None = None,
                supplemental_tiles: dict[int, np.ndarray] | None = None) -> list[int | None]:
    heights: list[int | None] = []
    if land_mask is not None and len(land_mask) != columns * rows:
        raise ValueError("Land mask dimensions do not match terrain grid")
    for row in range(rows):
        north = north_min + (rows - row - 1) * step
        for column in range(columns):
            index = row * columns + column
            east = east_min + column * step
            elevation = sample_elevation(tiles, north, east, radius, minimum_samples)
            # CopDEM is a DSM with a different vertical datum. Use it only when
            # AW3D30 has no sample and OSM classifies this vertex as land.
            if elevation is None and supplemental_tiles is not None and land_mask is not None and land_mask[index]:
                lat = ORIGIN_LAT + north / METERS_PER_LAT_DEGREE
                lon = ORIGIN_LON + east / METERS_PER_LON_DEGREE
                west = math.floor(lon)
                supplemental = supplemental_tiles.get(west)
                if supplemental is not None and 35 <= lat < 36:
                    source_column = int((lon - west) * 3600)
                    source_row = int((36 - lat) * 3600)
                    raw_height = float(supplemental[source_row, source_column])
                    elevation = supplemental_land_height(raw_height, True)
            heights.append(None if elevation is None else max(0, round(elevation)))
    return heights


def decode_land_mask_grid(mask_asset: dict[str, object], grid_id: str) -> list[bool]:
    grid = next((entry for entry in mask_asset["grids"] if entry["id"] == grid_id), None)
    if grid is None:
        raise ValueError(f"Missing land-mask grid: {grid_id}")
    columns, rows = int(grid["columns"]), int(grid["rows"])
    encoded = str(grid["landMaskHex"])
    if len(encoded) != math.ceil(columns * rows / 4):
        raise ValueError(f"Invalid land-mask dimensions: {grid_id}")
    return [((int(encoded[index // 4], 16) >> (index % 4)) & 1) == 1
            for index in range(columns * rows)]


def sample_grid_surface(heights: list[int | None], north_min: int, east_min: int,
                        columns: int, rows: int, step: int, north: float, east: float) -> float | None:
    """Sample the same two triangles used by the runtime terrain mesh."""
    column_position = (east - east_min) / step
    row_position = rows - 1 - (north - north_min) / step
    if column_position < 0 or row_position < 0 or column_position > columns - 1 or row_position > rows - 1:
        return None
    column = min(columns - 2, math.floor(column_position))
    row = min(rows - 2, math.floor(row_position))
    u = column_position - column
    v = row_position - row
    northwest = heights[row * columns + column]
    northeast = heights[row * columns + column + 1]
    southwest = heights[(row + 1) * columns + column]
    southeast = heights[(row + 1) * columns + column + 1]
    if u + v <= 1 and northwest is not None and southwest is not None and northeast is not None:
        return max(0.35, northwest) * (1 - u - v) + max(0.35, southwest) * v + max(0.35, northeast) * u
    if u + v >= 1 and northeast is not None and southwest is not None and southeast is not None:
        return max(0.35, northeast) * (1 - v) + max(0.35, southwest) * (1 - u) + max(0.35, southeast) * (u + v - 1)
    return None


def build_ridge_patches(tiles: dict[int, tuple[np.ndarray, np.ndarray]], broad_heights: list[int | None],
                        broad_columns: int, broad_rows: int) -> list[dict[str, object]]:
    """Retain four generalized opposite-shore peaks with a coarse-matched halo."""
    patches: list[dict[str, object]] = []
    step = RIDGE_PATCH_STEP_METERS
    half_extent = RIDGE_PATCH_HALF_EXTENT_METERS
    blend_width = RIDGE_PATCH_BLEND_METERS
    for peak in RIDGE_PEAKS:
        north_min = math.floor((int(peak["northMeters"]) - half_extent) / step) * step
        east_min = math.floor((int(peak["eastMeters"]) - half_extent) / step) * step
        columns = rows = math.ceil(2 * half_extent / step) + 1
        heights: list[float | None] = []
        for row in range(rows):
            north = north_min + (rows - row - 1) * step
            for column in range(columns):
                east = east_min + column * step
                coarse = sample_grid_surface(broad_heights, NORTH_MIN, EAST_MIN,
                                             broad_columns, broad_rows, TERRAIN_STEP, north, east)
                fine = sample_elevation(tiles, north, east, radius=1, minimum_samples=8)
                if coarse is None:
                    heights.append(None if fine is None else round(max(0.0, fine), 1))
                    continue
                if fine is None:
                    fine = coarse
                edge_distance = min(row, rows - 1 - row, column, columns - 1 - column) * step
                blend = min(1.0, edge_distance / blend_width)
                blend = blend * blend * (3 - 2 * blend)
                heights.append(round(max(0.0, coarse + (fine - coarse) * blend), 1))
        patches.append({
            "id": str(peak["id"]),
            "northMinMeters": north_min,
            "eastMinMeters": east_min,
            "columns": columns,
            "rows": rows,
            "terrainStepMeters": step,
            "blendWidthMeters": blend_width,
            "heightSource": "90 m opposite-shore grid sampled from valid AW3D30 DSM pixels, blended to the broad 300 m mesh in the outer 600 m",
            "elevationMetersAboveWater": heights,
        })
    return patches


def build_terrain(tiles: dict[int, tuple[np.ndarray, np.ndarray]], land_mask_asset: dict[str, object],
                 supplemental_tiles: dict[int, np.ndarray]) -> dict[str, object]:
    rows = (NORTH_MAX - NORTH_MIN) // TERRAIN_STEP + 1
    columns = (EAST_MAX - EAST_MIN) // TERRAIN_STEP + 1
    heights = sample_grid(tiles, NORTH_MIN, EAST_MIN, columns, rows, TERRAIN_STEP,
                          land_mask=decode_land_mask_grid(land_mask_asset, "broad-terrain"),
                          supplemental_tiles=supplemental_tiles)
    patches: list[dict[str, object]] = []
    built_surfaces: dict[str, tuple[int, int, int, int, int, list[int | None]]] = {
        "broad-terrain": (NORTH_MIN, EAST_MIN, columns, rows, TERRAIN_STEP, heights)
    }
    for patch in FINE_PATCHES:
        patch_id = str(patch["id"])
        patch_north_min = int(patch["northMinMeters"])
        patch_east_min = int(patch["eastMinMeters"])
        patch_columns = int(patch["columns"])
        patch_rows = int(patch["rows"])
        patch_step = int(patch["stepMeters"])
        patch_heights = sample_grid(
            tiles, patch_north_min, patch_east_min, patch_columns, patch_rows, patch_step,
            int(patch.get("sampleRadius", 1)), 1,
            decode_land_mask_grid(land_mask_asset, patch_id), supplemental_tiles
        )
        patch_asset: dict[str, object] = {
            "id": str(patch["id"]),
            **({"sourceTile": str(patch["sourceTile"])} if "sourceTile" in patch else {}),
            "northMinMeters": patch_north_min,
            "eastMinMeters": patch_east_min,
            "columns": patch_columns,
            "rows": patch_rows,
            "terrainStepMeters": patch_step,
            "elevationMetersAboveWater": patch_heights,
        }
        blend_width = int(patch.get("blendWidthMeters", 0))
        blend_to = str(patch.get("blendTo", ""))
        if blend_width > 0:
            base_surface = built_surfaces.get(blend_to)
            if base_surface is None:
                raise ValueError(f"Missing blend target terrain grid: {blend_to}")
            base_north_min, base_east_min, base_columns, base_rows, base_step, base_heights = base_surface
            for row in range(patch_rows):
                north = patch_north_min + (patch_rows - row - 1) * patch_step
                for column in range(patch_columns):
                    index = row * patch_columns + column
                    fine = patch_heights[index]
                    coarse = sample_grid_surface(
                        base_heights, base_north_min, base_east_min, base_columns, base_rows, base_step,
                        north, patch_east_min + column * patch_step
                    )
                    if coarse is None:
                        continue
                    if fine is None:
                        fine = coarse
                    edge_distance = min(row, patch_rows - 1 - row, column, patch_columns - 1 - column) * patch_step
                    blend = min(1.0, edge_distance / blend_width)
                    blend = blend * blend * (3 - 2 * blend)
                    patch_heights[index] = round(max(0.0, coarse + (fine - coarse) * blend), 1)
            patch_asset["blendWidthMeters"] = blend_width
            patch_asset["blendTo"] = blend_to
            patch_asset["heightSource"] = f"AW3D30 DSM with Copernicus GLO-30 fallback for OSM land; blended to {blend_to} in the outer {blend_width} m"
        patches.append(patch_asset)
        built_surfaces[patch_id] = (patch_north_min, patch_east_min, patch_columns, patch_rows, patch_step, patch_heights)
    patches.extend(build_ridge_patches(tiles, heights, columns, rows))
    return {
        "schemaVersion": 1,
        "originWgs84": {"latitudeDegrees": ORIGIN_LAT, "longitudeDegrees": ORIGIN_LON},
        "source": "JAXA ALOS World 3D-30m AW3D30 v3.2 and Copernicus DEM GLO-30 Public COG",
        "sourceVersion": "AW3D30 2021-02 collection v3.2_global; Copernicus DEM 2021 release, tiles N35 E135/E136",
        "license": "JAXA Research Data Terms of Use and Copernicus WorldDEM-30 free and open license",
        "licenseUrl": "https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM",
        "additionalLicenseUrl": "https://earth.jaxa.jp/en/data/policy/",
        "attribution": "Elevation data: AW3D30 (JAXA); Copernicus WorldDEM-30 provided under COPERNICUS by the European Union and ESA",
        "copernicusModifiedDataNotice": "produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved.",
        "copernicusLiabilityNotice": "The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30.",
        "heightDatum": "AW3D30 EGM96 orthometric height minus 84.371 m Lake Biwa B.S.L. zero; Copernicus EGM2008 fills missing OSM-land vertices after +0.3 m local median alignment to nearby AW3D30; neither is a geodetic datum transformation",
        "waterDatumMeters": WATER_DATUM_METERS,
        "terrainStepMeters": TERRAIN_STEP,
        "northMinMeters": NORTH_MIN,
        "eastMinMeters": EAST_MIN,
        "columns": columns,
        "rows": rows,
        "rowOrder": "north-to-south",
        "elevationMetersAboveWater": heights,
        "finePatches": patches,
        "validLandVertices": sum(value is not None for value in heights),
        "nodataMeaning": "water, invalid, or any AW3D30 mask class other than 0 or 12; missing OSM-land vertices use Copernicus GLO-30",
        "processing": "tools/world-build/build_biwa_world.py; 9x9 AW3D30 source pixel median on the 300 m broad grid; 12.06 km square native 30 m regional flight-and-shore grid blends to broad terrain over 1,800 m; exact valid AW3D30 pixels on 30 m island grids and a 10 m sampling grid for OSM wooded islet 1156534005; opposite-shore peak patches use 90 m median samples and blend over 600 m; missing vertices use Copernicus GLO-30 DSM only where OSM marks land, locally aligned by +0.3 m from the median of 8,828 co-located land samples in 35.20–35.32 N, 136.16–136.30 E; water and GSI DEM class 4 remain excluded; source GeoTIFFs are not bundled",
    }


def write_json(path: Path, payload: object) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    data = (json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")
    path.write_bytes(data)
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("osm", type=Path)
    parser.add_argument("--osm-only", action="store_true", help="Regenerate only the OSM-derived shoreline and analysis-map assets")
    parser.add_argument("--dsm-e135-n35", type=Path)
    parser.add_argument("--mask-e135-n35", type=Path)
    parser.add_argument("--dsm-e136-n35", type=Path)
    parser.add_argument("--mask-e136-n35", type=Path)
    parser.add_argument("--copdem-e135-n35", type=Path)
    parser.add_argument("--copdem-e136-n35", type=Path)
    parser.add_argument("--shoreline-output", type=Path, default=Path("assets/biwa-shoreline.json"))
    parser.add_argument("--analysis-map-output", type=Path, default=Path("assets/biwa-analysis-map.json"))
    parser.add_argument("--land-mask-output", type=Path, default=Path("assets/biwa-land-mask.json"))
    parser.add_argument("--terrain-output", type=Path, default=Path("assets/biwa-terrain.json"))
    parser.add_argument("--venue-osm-input", type=Path,
                        help="OpenStreetMap /map XML snapshot for the documented 3 km venue bounds")
    parser.add_argument("--venue-features-output", type=Path, default=Path("assets/biwa-venue-features.json"))
    args = parser.parse_args()
    required_full_build_inputs = (args.dsm_e135_n35, args.mask_e135_n35, args.dsm_e136_n35,
                                  args.mask_e136_n35, args.copdem_e135_n35, args.copdem_e136_n35,
                                  args.venue_osm_input)
    if not args.osm_only and any(path is None for path in required_full_build_inputs):
        parser.error("full asset build requires both DSM/mask tile pairs and --venue-osm-input")

    shorelines, islands, water_ring, relation_version, relation_timestamp = read_osm(args.osm)
    land_mask = build_land_mask_asset(water_ring, islands, hashlib.sha256(args.osm.read_bytes()).hexdigest(),
                                      relation_version, relation_timestamp)
    land_mask_hash = write_json(args.land_mask_output, land_mask)
    shoreline_land_sides = [shoreline_land_side(line, water_ring) for line in shorelines]
    analysis_bounds = VENUE_BOUNDS
    analysis_shorelines: list[list[tuple[float, float]]] = []
    for line in shorelines:
        for fragment in clip_polyline(line, analysis_bounds):
            if len(fragment) >= 2:
                analysis_shorelines.append(simplify(fragment, 2.0))
    camera_points = [
        {"id": "platform", "northMeters": -16, "eastMeters": -18, "altitudeMeters": 14},
        {"id": "shore", "northMeters": 0, "eastMeters": 250, "altitudeMeters": 10},
        {"id": "telephoto", "northMeters": 300, "eastMeters": 700, "altitudeMeters": 55},
    ]
    analysis_map = {
        "schemaVersion": 1,
        "originWgs84": {"latitudeDegrees": ORIGIN_LAT, "longitudeDegrees": ORIGIN_LON},
        "license": "ODbL-1.0",
        "licenseUrl": "https://opendatacommons.org/licenses/odbl/1-0/",
        "attribution": "© OpenStreetMap contributors",
        "sourceUrl": "https://www.openstreetmap.org/relation/63499",
        "sourceSnapshotSha256": hashlib.sha256(args.osm.read_bytes()).hexdigest(),
        "sourceRelationId": int(LAKE_RELATION_ID),
        "sourceRelationVersion": int(relation_version),
        "sourceRelationTimestamp": relation_timestamp,
        "boundsMeters": {"northMin": -1_500, "northMax": 1_500, "eastMin": -1_500, "eastMax": 1_500},
        "shorelinesNorthEastMeters": [[[round(n, 1), round(e, 1)] for n, e in line] for line in analysis_shorelines],
        "cameraPoints": camera_points,
        "cameraPointsSha256": hashlib.sha256(json.dumps(camera_points, separators=(",", ":")).encode("utf-8")).hexdigest(),
        "cameraPointNote": "Art-directed nearby viewpoints based on OSM shoreline and AW3D30 terrain; approximate, not surveyed.",
    }
    shoreline = {
        "schemaVersion": 3,
        "originWgs84": {"latitudeDegrees": ORIGIN_LAT, "longitudeDegrees": ORIGIN_LON},
        "license": "ODbL-1.0",
        "licenseUrl": "https://opendatacommons.org/licenses/odbl/1-0/",
        "attribution": "© OpenStreetMap contributors",
        "sourceUrl": "https://www.openstreetmap.org/relation/63499",
        "sourceSnapshotSha256": hashlib.sha256(args.osm.read_bytes()).hexdigest(),
        "sourceRelationId": int(LAKE_RELATION_ID),
        "sourceRelationVersion": int(relation_version),
        "sourceRelationTimestamp": relation_timestamp,
        "projection": "local WGS84 tangent approximation; north/east metres relative to launch origin",
        "boundsMeters": {"northMin": NORTH_MIN, "northMax": NORTH_MAX, "eastMin": EAST_MIN, "eastMax": EAST_MAX},
        "simplificationToleranceMeters": {"launchArea": 2.0, "remoteShore": 20.0, "islands": 8.0},
        "shorelinesNorthEastMeters": [[[round(n, 1), round(e, 1)] for n, e in line] for line in shorelines],
        "shorelineLandSideSigns": shoreline_land_sides,
        "islands": islands,
        "processing": "tools/world-build/build_biwa_world.py; clipped Lake Biwa relation 63499 outer ways and inner island rings; each open shoreline land side is classified against the complete outer water ring; raw OSM XML not bundled",
    }
    shoreline_hash = write_json(args.shoreline_output, shoreline)
    analysis_map_hash = write_json(args.analysis_map_output, analysis_map)
    if args.osm_only:
        print(f"Shoreline asset: {args.shoreline_output} sha256={shoreline_hash}; {len(shorelines)} shore fragments, {len(islands)} island rings")
        print(f"Analysis map asset: {args.analysis_map_output} sha256={analysis_map_hash}; {len(analysis_shorelines)} local shoreline fragments and {len(camera_points)} approximate camera points")
        print(f"Land mask asset: {args.land_mask_output} sha256={land_mask_hash}; {len(land_mask['grids'])} terrain grids")
        return
    venue_features = read_venue_features(args.venue_osm_input)
    venue_features_hash = write_json(args.venue_features_output, venue_features)
    tiles = {
        135: load_tile(args.dsm_e135_n35, args.mask_e135_n35, 135),
        136: load_tile(args.dsm_e136_n35, args.mask_e136_n35, 136),
    }
    supplemental_tiles = {
        135: load_copdem_tile(args.copdem_e135_n35),
        136: load_copdem_tile(args.copdem_e136_n35),
    }
    terrain = build_terrain(tiles, land_mask, supplemental_tiles)
    terrain["sourceTileSha256"] = {
        "copdem-n35-e135": hashlib.sha256(args.copdem_e135_n35.read_bytes()).hexdigest(),
        "copdem-n35-e136": hashlib.sha256(args.copdem_e136_n35.read_bytes()).hexdigest(),
    }
    terrain_hash = write_json(args.terrain_output, terrain)
    print(f"Shoreline asset: {args.shoreline_output} sha256={shoreline_hash}; {len(shorelines)} shore fragments, {len(islands)} island rings")
    print(f"Analysis map asset: {args.analysis_map_output} sha256={analysis_map_hash}; {len(analysis_shorelines)} local shoreline fragments and {len(camera_points)} approximate camera points")
    print(f"Land mask asset: {args.land_mask_output} sha256={land_mask_hash}; {len(land_mask['grids'])} terrain grids")
    print(f"Venue features asset: {args.venue_features_output} sha256={venue_features_hash}; {len(venue_features['features'])} selected OSM features")
    print(f"Terrain asset: {args.terrain_output} sha256={terrain_hash}; {terrain['columns']}x{terrain['rows']} grid, {terrain['validLandVertices']} populated broad-grid land vertices and {len(terrain['finePatches'])} independent high-detail regions")


if __name__ == "__main__":
    main()

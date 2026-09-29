"""Extract the local Lake Biwa shoreline from an OSM API XML snapshot.

Usage: python tools/world-build/build_biwa_shoreline.py input.osm assets/biwa-shoreline.json
The OSM snapshot is an input and is not bundled with the game.
"""

import argparse
import json
import math
import xml.etree.ElementTree as ET
from pathlib import Path


ORIGIN_LAT = 35.294075
ORIGIN_LON = 136.254448
LAKE_RELATION_ID = "63499"
SHORE_WAY_ID = "41696803"
METERS_PER_LAT_DEGREE = 111_132.0
METERS_PER_LON_DEGREE = 111_320.0 * math.cos(math.radians(ORIGIN_LAT))


def distance_from_line(point, start, end):
    dx = end[0] - start[0]
    dy = end[1] - start[1]
    if dx == 0 and dy == 0:
        return math.dist(point, start)
    fraction = max(0.0, min(1.0, ((point[0] - start[0]) * dx + (point[1] - start[1]) * dy) / (dx * dx + dy * dy)))
    return math.dist(point, (start[0] + fraction * dx, start[1] + fraction * dy))


def simplify(points, tolerance):
    if len(points) <= 2:
        return points
    farthest = max(range(1, len(points) - 1), key=lambda index: distance_from_line(points[index], points[0], points[-1]))
    if distance_from_line(points[farthest], points[0], points[-1]) <= tolerance:
        return [points[0], points[-1]]
    return simplify(points[:farthest + 1], tolerance)[:-1] + simplify(points[farthest:], tolerance)


def build(source: Path, destination: Path):
    root = ET.parse(source).getroot()
    nodes = {node.attrib["id"]: (float(node.attrib["lat"]), float(node.attrib["lon"])) for node in root.findall("node")}
    relation = next(item for item in root.findall("relation") if item.attrib["id"] == LAKE_RELATION_ID)
    if not any(member.attrib.get("ref") == SHORE_WAY_ID and member.attrib.get("role") == "outer" for member in relation.findall("member")):
        raise ValueError("Shore way is not an outer member of Lake Biwa")
    way = next(item for item in root.findall("way") if item.attrib["id"] == SHORE_WAY_ID)
    coordinates = [nodes[point.attrib["ref"]] for point in way.findall("nd")]
    projected = [((lat - ORIGIN_LAT) * METERS_PER_LAT_DEGREE, (lon - ORIGIN_LON) * METERS_PER_LON_DEGREE) for lat, lon in coordinates]
    nearby = [index for index, (north, east) in enumerate(projected) if abs(north) < 1800 and abs(east) < 1800]
    if not nearby:
        raise ValueError("No shoreline points near the launch coordinate")
    first, last = max(0, min(nearby) - 1), min(len(projected), max(nearby) + 2)
    shore = simplify(projected[first:last], 2.0)
    payload = {
        "schemaVersion": 1,
        "originWgs84": {"latitudeDegrees": ORIGIN_LAT, "longitudeDegrees": ORIGIN_LON},
        "license": "ODbL-1.0",
        "licenseUrl": "https://opendatacommons.org/licenses/odbl/1-0/",
        "attribution": "© OpenStreetMap contributors",
        "sourceUrl": "https://www.openstreetmap.org/relation/63499",
        "sourceWayId": int(SHORE_WAY_ID),
        "sourceWayVersion": int(way.attrib["version"]),
        "sourceWayTimestamp": way.attrib["timestamp"],
        "projection": "local WGS84 tangent approximation; north/east metres relative to origin",
        "simplificationToleranceMeters": 2.0,
        "shorelineNorthEastMeters": [[round(north, 1), round(east, 1)] for north, east in shore],
    }
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(payload, ensure_ascii=False, separators=(",", ":")) + "\n", encoding="utf-8", newline="\n")
    print(f"Wrote {len(shore)} shoreline points to {destination}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    build(args.source, args.destination)

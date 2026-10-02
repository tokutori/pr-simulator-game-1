import argparse
from dataclasses import asdict, dataclass
import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import sys


DIRECTIONS = {
    name: index * 22.5
    for index, name in enumerate((
        "北", "北北東", "北東", "東北東", "東", "東南東", "南東", "南南東",
        "南", "南南西", "南西", "西南西", "西", "西北西", "北西", "北北西",
    ))
}
MAX_SNAPSHOT_BYTES = 2 * 1024 * 1024


class SourceError(ValueError):
    pass


@dataclass(frozen=True)
class GroundNormal:
    station: str
    first_year: int
    last_year: int
    month: int
    mean_speed_mps: float
    prevailing_from_degrees: float
    input_sha256: str


class NormalTableParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.headings: list[str] = []
        self.tables: list[list[list[str]]] = []
        self.layouts: list[list[list[tuple[str, str, str]]]] = []
        self.layout: list[list[tuple[str, str, str]]] = []
        self.row_layout: list[tuple[str, str, str]] = []
        self.heading: list[str] | None = None
        self.table: list[list[str]] | None = None
        self.row: list[str] | None = None
        self.cell: list[str] | None = None
        self.cell_tag: str | None = None

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if self.cell is not None:
            if tag == "br":
                self.cell.append("\n")
            elif tag not in ("span", "a"):
                raise SourceError("unsupported markup inside source cell")
        if tag == "h3":
            if self.heading is not None:
                raise SourceError("nested heading")
            self.heading = []
        if tag == "table" and dict(attrs).get("id") == "tablefix1":
            if self.table is not None:
                raise SourceError("nested source table")
            self.table = []
            self.layout = []
        if self.table is not None and tag == "tr":
            if self.row is not None:
                raise SourceError("unclosed source row")
            self.row = []
            self.row_layout = []
        if self.row is not None and tag in ("th", "td"):
            if self.cell is not None:
                raise SourceError("unclosed source cell")
            self.cell = []
            self.cell_tag = tag
            attributes = dict(attrs)
            self.row_layout.append((tag, attributes.get("rowspan", "1"),
                                    attributes.get("colspan", "1")))

    def handle_data(self, data: str) -> None:
        if self.heading is not None:
            self.heading.append(data)
        if self.cell is not None:
            self.cell.append(data)

    def handle_endtag(self, tag: str) -> None:
        if tag == "h3" and self.heading is not None:
            self.headings.append("".join(self.heading))
            self.heading = None
        if tag in ("th", "td") and self.cell is not None:
            if tag != self.cell_tag:
                raise SourceError("mismatched source cell closing tag")
            if self.row is None:
                raise SourceError("cell outside row")
            self.row.append("".join(self.cell).strip())
            self.cell = None
            self.cell_tag = None
        if tag == "tr" and self.row is not None:
            if self.table is None or self.cell is not None:
                raise SourceError("incomplete source row")
            self.table.append(self.row)
            self.layout.append(self.row_layout)
            self.row = None
        if tag == "table" and self.table is not None:
            if self.row is not None:
                raise SourceError("incomplete source table")
            self.tables.append(self.table)
            self.layouts.append(self.layout)
            self.table = None


def parse_normal(snapshot: bytes, expected_sha256: str, station: str, month: int) -> GroundNormal:
    if not 1 <= month <= 12:
        raise SourceError("month must be in 1..12")
    if not re.fullmatch(r"[0-9a-f]{64}", expected_sha256):
        raise SourceError("expected SHA-256 must be lowercase hexadecimal")
    if len(snapshot) > MAX_SNAPSHOT_BYTES:
        raise SourceError("source snapshot exceeds 2 MiB")
    digest = hashlib.sha256(snapshot).hexdigest()
    if digest != expected_sha256:
        raise SourceError("source snapshot SHA-256 mismatch")
    parser = NormalTableParser()
    try:
        parser.feed(snapshot.decode("utf-8", errors="strict"))
        parser.close()
    except UnicodeDecodeError as error:
        raise SourceError("source snapshot must be UTF-8") from error
    matching = [heading for heading in parser.headings
                if heading.startswith(station + "（")
                and "平年値（年・月ごとの値）" in heading
                and "詳細（風・日照）" in heading]
    if len(matching) != 1 or len(parser.tables) != 1 or parser.table is not None:
        raise SourceError("expected exactly one station heading and source table")
    rows = parser.tables[0]
    expected_headers = [
        ["要素", "風向・風速", "日照時間", "全天日射量", "雲量", "大気現象"],
        ["平均風速(m/s)", "最多風向", "各階級の日数", "合計(時)", "各階級の日数",
         "平均(MJ/㎡)", "平均", "各階級の日数", "雪日数", "霧日数", "雷日数"],
        ["≧10.0m/s", "≧15.0m/s", "≧20.0m/s", "≧30.0m/s", "不照", "日照率≧40%", "＜1.5", "≧8.5"],
    ]
    expected_layout = [
        [("th", "3", "1"), ("th", "1", "6"), ("th", "1", "3"),
         ("th", "1", "1"), ("th", "1", "3"), ("th", "1", "3")],
        [("th", "2", "1"), ("th", "2", "1"), ("th", "1", "4"),
         ("th", "2", "1"), ("th", "1", "2"), ("th", "2", "1"),
         ("th", "2", "1"), ("th", "1", "2"), ("th", "2", "1"),
         ("th", "2", "1"), ("th", "2", "1")],
        [("th", "1", "1")] * 8,
    ]
    normalized_headers = [[re.sub(r"\s+", "", cell) for cell in row] for row in rows[:3]]
    if normalized_headers != expected_headers or parser.layouts[0][:3] != expected_layout:
        raise SourceError("unsupported wind/sunshine column layout")

    def unique_row(label: str) -> list[str]:
        matches = [row for row in rows if row and row[0] == label]
        if len(matches) != 1 or len(matches[0]) != 17:
            raise SourceError("missing, duplicate or malformed row: " + label)
        index = rows.index(matches[0])
        if parser.layouts[0][index] != [("th", "1", "1")] + [("td", "1", "1")] * 16:
            raise SourceError("unsupported data row spans: " + label)
        return matches[0]

    periods = unique_row("統計期間")
    years = unique_row("資料年数")
    selected = unique_row(str(month) + "月")
    period_values = [re.sub(r"\s+", "", value) for value in periods[1:3]]
    period = re.fullmatch(r"([0-9]{4})～([0-9]{4})", period_values[0])
    if period is None or period_values[0] != period_values[1]:
        raise SourceError("wind statistics must have matching explicit periods")
    first_year, last_year = map(int, period.groups())
    if first_year > last_year or years[1:3] != [str(last_year - first_year + 1)] * 2:
        raise SourceError("incomplete wind statistical period")
    if not re.fullmatch(r"[0-9]+(?:\.[0-9]+)?", selected[1]):
        raise SourceError("missing or qualified mean wind speed")
    speed = float(selected[1])
    if not 0 <= speed <= 60:
        raise SourceError("mean wind speed outside supported range")
    if selected[2] not in DIRECTIONS:
        raise SourceError("missing, qualified or unsupported prevailing direction")
    return GroundNormal(station, first_year, last_year, month, speed,
                        DIRECTIONS[selected[2]], digest)


def main() -> None:
    arguments = argparse.ArgumentParser(description="Extract hash-pinned JMA ground wind normals offline")
    arguments.add_argument("snapshot", type=Path)
    arguments.add_argument("--sha256", required=True)
    arguments.add_argument("--station", required=True)
    arguments.add_argument("--month", type=int, required=True)
    options = arguments.parse_args()
    try:
        with options.snapshot.open("rb") as source:
            snapshot = source.read(MAX_SNAPSHOT_BYTES + 1)
        normal = parse_normal(snapshot, options.sha256, options.station, options.month)
    except (SourceError, OSError) as error:
        arguments.exit(2, str(error) + "\n")
    encoded = json.dumps(asdict(normal), ensure_ascii=True, sort_keys=True, allow_nan=False)
    sys.stdout.buffer.write(encoded.encode("utf-8") + b"\n")


if __name__ == "__main__":
    main()

import hashlib
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from jma_normals import DIRECTIONS, SourceError, parse_normal


HEADER = '''<h3>彦根（滋賀県) 平年値（年・月ごとの値） 詳細（風・日照）</h3>
<table id="tablefix1">
<tr><th rowspan="3">要素</th><th colspan="6">風向・風速</th><th colspan="3">日照時間</th><th>全天日射量</th><th colspan="3">雲量</th><th colspan="3">大気現象</th></tr>
<tr><th rowspan="2">平均風速<br>(m/s)</th><th rowspan="2">最多風向</th><th colspan="4">各階級の日数</th><th rowspan="2">合計<br>(時)</th><th colspan="2">各階級の日数</th><th rowspan="2">平均<br>(MJ/㎡)</th><th rowspan="2">平均</th><th colspan="2">各階級の日数</th><th rowspan="2">雪日数</th><th rowspan="2">霧日数</th><th rowspan="2">雷日数</th></tr>
<tr><th>≧10.0m/s</th><th>≧15.0m/s</th><th>≧20.0m/s</th><th>≧30.0m/s</th><th>不照</th><th>日照率≧40%</th><th>＜1.5</th><th>≧8.5</th></tr>'''


def row(label: str, speed: str, direction: str) -> str:
    return ("<tr><th>" + label + "</th><td>" + speed + "</td><td>" + direction
            + "</td>" + "<td>8.0 &#64;</td>" * 14 + "</tr>")


def fixture(speed: str = "2.5", direction: str = "北西") -> str:
    return (HEADER + row("統計期間", "1991～<br>2020", "1991～<br>2020")
            + row("資料年数", "30", "30") + row("1月", "3.7", "北西")
            + row("7月", speed, direction) + row("8月", "2.5", "北西")
            + row("12月", "3.7", "南南東") + "</table>")


def parse(text: str, month: int = 7):
    snapshot = text.encode("utf-8")
    return parse_normal(snapshot, hashlib.sha256(snapshot).hexdigest(), "彦根", month)


class NormalParsingTests(unittest.TestCase):
    def test_cli_emits_identical_utf8_lf_bytes(self) -> None:
        snapshot = fixture().encode("utf-8")
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "snapshot.html"
            source.write_bytes(snapshot)
            command = [sys.executable, str(Path(__file__).with_name("jma_normals.py")),
                       str(source), "--sha256", hashlib.sha256(snapshot).hexdigest(),
                       "--station", "彦根", "--month", "7"]
            first = subprocess.run(command, check=True, capture_output=True).stdout
            second = subprocess.run(command, check=True, capture_output=True).stdout
        self.assertEqual(first, second)
        self.assertNotIn(b"\r", first)
        self.assertTrue(first.endswith(b"\n"))
        first.decode("utf-8", errors="strict")

    def test_selected_month_and_unrelated_quality_annotations(self) -> None:
        normal = parse(fixture())
        self.assertEqual((normal.first_year, normal.last_year, normal.month), (1991, 2020, 7))
        self.assertEqual((normal.mean_speed_mps, normal.prevailing_from_degrees), (2.5, 315))
        self.assertEqual(parse(fixture(), 1).mean_speed_mps, 3.7)
        self.assertEqual(parse(fixture(), 12).prevailing_from_degrees, 157.5)
        self.assertEqual(parse(fixture()), parse(fixture()))

    def test_all_sixteen_directions(self) -> None:
        for direction, angle in DIRECTIONS.items():
            with self.subTest(direction=direction):
                self.assertEqual(parse(fixture(direction=direction)).prevailing_from_degrees, angle)

    def test_missing_qualified_and_invalid_values(self) -> None:
        for speed in ("", "--", "×", "2.5@", "2.5 ]", "NaN", "inf", "-1", "61",
                      "2 5", "2<br>5", "2<script>.5</script>", "2<style>.5</style>"):
            with self.subTest(speed=speed), self.assertRaises(SourceError):
                parse(fixture(speed=speed))
        for direction in ("", "--", "北西@", "NW", "静穏"):
            with self.subTest(direction=direction), self.assertRaises(SourceError):
                parse(fixture(direction=direction))

    def test_mismatched_cell_tags_are_rejected(self) -> None:
        with self.assertRaises(SourceError):
            parse(fixture().replace("<td>2.5</td>", "<td>2.5</th>", 1))

    def test_missing_and_duplicate_structure(self) -> None:
        text = fixture()
        cases = (
            text.replace("彦根", "京都"), text + HEADER.split("<table")[0],
            text + text, text.replace(row("7月", "2.5", "北西"), ""),
            text.replace("</table>", row("7月", "2.5", "北西") + "</table>"),
            text.replace("</table>", row("統計期間", "1991～2020", "1991～2020") + "</table>"),
            text.replace("</table>", ""),
        )
        for case in cases:
            with self.subTest(case=case), self.assertRaises(SourceError):
                parse(case)

    def test_changed_columns_units_spans_and_cell_counts(self) -> None:
        text = fixture()
        for before, after in (
            ("平均風速<br>(m/s)", "最多風向"), ("(m/s)", "(km/h)"),
            ('colspan="6"', 'colspan="5"'), ('rowspan="2"', 'rowspan="1"'),
            ("<th>7月</th>", '<th colspan="2">7月</th>'),
            ("<th>7月</th>", "<th>7月</th><td>9.0</td>"),
            ("<th>7月</th><td>2.5</td>", "<th>7月</th>"),
        ):
            with self.subTest(before=before), self.assertRaises(SourceError):
                parse(text.replace(before, after))

    def test_period_and_sample_years_are_independent(self) -> None:
        text = fixture()
        for before, after in (
            ("<td>1991～<br>2020</td>", "<td>1990～2020</td>"),
            ("<td>1991～<br>2020</td>", "<td>2021～2020</td>"),
            ("<td>1991～<br>2020</td>", "<td>1991～2020@</td>"),
            ("<td>30</td>", "<td>29</td>"),
        ):
            with self.subTest(after=after), self.assertRaises(SourceError):
                parse(text.replace(before, after, 1))

    def test_hash_encoding_size_and_month_boundaries(self) -> None:
        snapshot = fixture().encode("utf-8")
        digest = hashlib.sha256(snapshot).hexdigest()
        for payload, expected, month in (
            (snapshot, "0" * 64, 7), (snapshot, digest.upper(), 7),
            (b"\xff", hashlib.sha256(b"\xff").hexdigest(), 7),
            (snapshot, digest, 0), (snapshot, digest, 13),
            (b" " * (2 * 1024 * 1024 + 1), digest, 7),
        ):
            with self.subTest(month=month, expected=expected), self.assertRaises(SourceError):
                parse_normal(payload, expected, "彦根", month)


if __name__ == "__main__":
    unittest.main()

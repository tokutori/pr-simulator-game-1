# データ利用計画

本表は候補一覧である。利用条件、データversion、配布可否、帰属表示は取得前に一次資料で確認する。
2026-09-27時点でデータの取得・同梱・利用条件の承認は行っていない。

## 琵琶湖湖岸の局所データ（2026-09-30）

発進地点を WGS84 緯度35.294075°、経度136.254448°とし、
OpenStreetMap の琵琶湖 relation 63499 に属する外周 way 41696803 を取得した。
元データの way version は47、更新日時は2026-02-23T09:13:17Z。
取得範囲は経度136.215–136.270°、緯度35.275–35.325°であり、
OpenStreetMap API の `/api/0.6/map?bbox=136.215,35.275,136.270,35.325` を使用した。
`tools/world-build/build_biwa_shoreline.py` で発進地点周辺を切り出し、
局所 NED 座標へ投影して2 mの許容差で簡略化した結果を
`assets/biwa-shoreline.json` に収録する。発進台の寸法・傾斜・高度は
利用者が提供した概数であり、OSM由来ではない。

湖岸データのライセンスは ODbL 1.0。公開する加工済みデータも同条件とし、
ゲーム HUD に「© OpenStreetMap contributors」と
[著作権・ライセンス](https://www.openstreetmap.org/copyright)へのリンクを表示する。
この局所データは測量・航行用ではない。水平位置の近似投影と
OSM境界の精度を超える実地の一致は保証しない。

| 用途 | 候補 | 確認事項 |
|---|---|---|
| 地上風 | 気象庁AMeDAS | 観測高度、時間平均、欠測、湖上への適用性 |
| 一般風 | ERA5 | grid解像度、時刻、鉛直座標、利用条件 |
| 湖上風・波 | 水資源機構 | 観測地点、較正、期間、配布条件 |
| 遠景 | JAXA AW3D30 | DSM/地盤の区別、標高基準、派生成果の配布 |
| 局所地形 | 国土地理院DEM | 精度、測地系、標高基準、表示条件 |
| 土地被覆 | ESA WorldCover | class定義、version、帰属表示 |
| 衛星画像 | Sentinel-2 | 任意用途、雲・時期、帯域、条件 |
| 建物 | PLATEAU | 対象範囲、LOD、都市ごとの条件 |
| 地理情報 | OpenStreetMap | databaseと画像成果物の条件 |

初期候補構成はAW3D30 + 必要範囲のGSI、AMeDAS + ERA5 + 琵琶湖局地風研究、
波浪較正用の水資源機構資料である。広域衛星textureは必須としない。
Typical scenarioの選定期間、統計量、方位、鉛直風はBPG-008で決定する。

処理は取得、範囲切り出し、座標・標高基準の統一、NED変換、LOD生成、量子化、圧縮、hash作成の順とする。
raw dataは配布artifactと区別し、runtimeに取得処理を含めない。
再生成に必要なsource version・tool version・パラメータ・originを保存する。
source再配布と派生asset配布の許諾はそれぞれ確認する。

## 配布assetの入口

ゲーム固有の画像・音声・model・shader・binaryは `assets/` に置き、source、license、hashを
`assets/manifest.toml` へ登録する。`web/public` は空とし、`web/src` にはコード、HTML、CSSのみを置く。
Vite buildではmodule import、`new URL(..., import.meta.url)`、HTMLのresource属性、CSSの`url()`を検査する。
静的文字列を使う一般の`new URL()`は拡張子に関わらずmanifestと照合し、動的引数を拒否する。
`Worker` / `SharedWorker` constructorの第1引数として直接渡すJS/TS source URLはmodule参照として扱い、manifest asset検査から除外する。
HTMLの`src`・`href`・`poster`は属性値全体を単一URLとして検査し、`srcset`・`imagesrcset`は候補ごとに検査する。
asset形式の判定にはViteの解決済み`assetsInclude`を使用し、明示的な`?url`・`?raw`も登録対象とする。
CSSの`image-set()`は登録経路を実装するまで使用しない。
小容量assetのdata URL化も元ファイルの登録を要する。動的URL生成は登録済みasset IDから解決し、
任意のruntime URLをassetとして読み込む経路は設けない。依存package内のassetはpackageの利用条件で管理する。
この規則とbuild graph検査はBPG-001の対象である。実assetの配布・creditsとruntime resolverはBPG-009/013で検証する。

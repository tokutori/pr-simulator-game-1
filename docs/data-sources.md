# データ利用計画

公開assetについては取得元の一次資料、利用条件、対象データ区分、標高基準を確認し、出典・処理条件・hashをmanifestに記録する。生データは配布物に含めない。

## 琵琶湖湖岸の局所データ（2026-09-30）

発進原点は WGS84 緯度35.294075°、経度136.254448°である。OSM の琵琶湖 relation 63499（version 111、2025-08-14T09:30:32Z）の外周岸線と内周の島輪郭を、発進点から北20 km～36 km、東西66 km～21 kmの範囲で切り出して `assets/biwa-shoreline.json` に収録した。relation 全体のAPIスナップショットは入力時のSHA-256をassetに記録し、入力XMLは配布しない。発進地点付近を2 m、遠景を20 m、島輪郭を8 mの許容差で簡略化した。湖岸・島の座標は ODbL 1.0 の加工データとして配布し、HUDにOpenStreetMap contributorsと著作権・ライセンスへのリンクを表示する。

`assets/biwa-analysis-map.json` には発進点を中心とする3 km四方のAnalysis用湖岸線を収録する。飛行軌跡の表示範囲は近景の湖岸線・発進台に合わせ、離れた島で軌跡が過度に縮小しない。3D表示用の島輪郭は `assets/biwa-shoreline.json` に保持し、沖島・多景島・竹生島・オコノ洲を名前付きで含む17輪である。カメラ点は発進台・湖岸・望遠の3視点を用意した。視点の相対座標はOSM湖岸線とAW3D30標高を基に置いた試作値であり、実測位置ではない。asset内に個別SHA-256を記録する。

`assets/biwa-land-mask.json` は同じOSM relationの外周水面と内周島輪郭を、AW3D30の地形格子点へサンプリングした陸水判定である。地形の欠損補間はこのマスクで陸上と判定された点だけを対象とし、水域の欠損は欠損のまま残す。マスクは標高データを含まず、JAXA標高assetとは分離してODbL-1.0の派生データとして扱う。

竹生島の表示名は、OSM inner ringに含まれるway ID 183556759の地理的位置を識別して付与する。輪郭そのものは他の島と同様にOSMから取得する。

`assets/biwa-venue-features.json` はOpenStreetMap map APIの発進点周辺3 km四方のスナップショットから `natural=beach/sand`、`landuse=forest` / `natural=wood`、`natural=tree_row`、`man_made=pier/quay/breakwater` のwayを抽出した加工データである。松原水泳場の砂浜名と桟橋・岸壁・防波堤をAnalysis mapへ表示し、砂浜・樹林の地表色と港湾構造物の3D表示に使う。OSMの局所map APIに存在しない地物や、wayがbbox端で分割されている箇所は欠落しうる。分類はOSMタグの写しであり、樹種・砂質・構造物の高さや現況を保証しない。桟橋・岸壁・防波堤の表示幅と高さは視覚化用の概算である。樹林ポリゴンをAW3D30 DSMの樹冠標高へ上乗せすると二重計上になるため、地物を高さの根拠に使わない。出典と取得snapshot hashはassetに記録する。

対岸稜線用の主標高はJAXA Earth APIの `JAXA.EORC_ALOS.PRISM_AW3D30.v3.2_global`、2021-02 collectionからE135-N35・E136-N35を用いた。`assets/biwa-terrain.json` は300 m広域格子、離陸地点を含む12.06 km四方の30 m湖岸・飛行圏格子、主要4島（多景島・竹生島・沖島・オコノ洲）周辺の30 m格子、OSM woodland属性を持つ小島用の10 mサンプリング格子、対岸4峰周辺の90 m格子からなる描画専用標高assetである。30 m格子は外周1,800 mで広域格子の三角形面へ滑らかにつなぐ。湖岸は90 m格子で細い陸地を取りこぼして接合幅が延びるため、地域の湖岸格子を30 mに保ち、頂点数の削減は遠景の4峰で行う。各段の格子寸法は描画メッシュの頂点間隔であり、元のAW3D30画素は30 mである。10 m小島格子も標高を10 m精度へ高めるものではなく、小島輪郭に複数の頂点を配置するために元DEM画素を再参照する。遠景の対岸4峰は90 mへ集約して頂点数を抑える。粗い格子で島内に三角形を作れない場合は、局所格子を追加する。AW3D30 maskの0（有効AW3D30）と12（PRISM DSM）を優先し、4（国土地理院数値標高モデル）を含む他のmask補完区分は採用しない。AW3D30に標高がない陸上格子点のみ、Copernicus DEM GLO-30 Public COGのN35-E135・N35-E136タイルで補完する。OSMの陸水マスクを適用するため水域の値は補完に使わない。

Copernicus DEMはTanDEM-X由来のDSMで、水平座標WGS84、標高基準EGM2008、GLO-30 Publicは自由利用・加工・再配布可能である。AW3D30のEGM96と直接混ぜず、OSM陸地上の重複8,828標本（緯度35.20–35.32°、経度136.16–136.30°）におけるCopernicus−AW3D30差の中央値−0.325 mを基に、補完値へ+0.3 mを加える。これは局所的な描画位置合わせであり、測地基準変換や現地較正ではない。各タイルのSHA-256は地形assetに記録する。Copernicusの再配布条件が求める帰属・改変通知・免責文を [`THIRD_PARTY_DATA.md`](../THIRD_PARTY_DATA.md)、地形asset、manifestおよびHUDリンクに記載する。

AW3D30の標高はEGM96ジオイド基準である。水面からの高さを作るため、滋賀県が示す琵琶湖B.S.L. 0＝東京湾平均海面（T.P.）+84.371 mを差し引くが、EGM96からT.P.への測地基準変換を行っていない。よって景観用の近似であり、測量・航行・高度計算には使用しない。水面の平均水位も変動する。

多景島のOSM輪郭内にはAW3D30 DSMの有効標本が29点、オコノ洲では100点ある。湖岸・島マスクで水域のDSM標本を除いた結果に残る島内の欠損を避けるため、両島のterrain patchをOSM輪郭内で元格子の半分の間隔に再サンプリングし、輪郭内の有効な陸上DSM標本から各点の高さを最大4点の逆距離加重で補間する。多景島の描画用樹冠面もOSM輪郭で閉じる。深緑の色調変化は利用者指定の見た目を再現するart directionで、独立した植生データではない。DSMに含まれる樹冠高を再加算せず、補間面は描画専用とする。

沖の白石を構成する4つの小岩礁はOSM島輪郭で位置と水面上の外形を表す。対応するAW3D30格子は有効な岩礁標高を持たないため、[高島市の案内](https://www.city.takashima.lg.jp/kanko_bunka_sports/kanko_tokusan_rekishi_bunkazai/3/4633.html)が示す4個の岩・最高14 mを描画の上限にだけ使い、残る3岩の高さは輪郭面積に応じた推定値とする。これは隙間を埋めて景観を描くための近似であり、地形標高、測量値、collision、飛行物理として使用しない。

JAXA Research Data Terms of Useは、条件に従うデータの利用・改変・第三者配布を無償で認め、派生物にもJAXAとデータ名の表示を求める。商用利用も無償だが、事前にJAXAへの通知が必要である。画面に「AW3D30 (JAXA)」を表示し、manifestに利用条件と出典を記録する。JAXA製品仕様・配布条件は[AW3D30データセット](https://www.eorc.jaxa.jp/ALOS/jp/dataset/aw3d30/aw3d30_j.htm)、[AW3D30 v3.2製品説明書](https://www.eorc.jaxa.jp/ALOS/en/aw3d30/aw3d30v3.2_product_e_e1.2.pdf)、[JAXA研究データ利用条件](https://earth.jaxa.jp/en/data/policy/)を参照する。

発進原点はWGS84緯度35.294075°、経度136.254448°とする。座標表示は原点近傍の緯度・経度1度当たり距離による局所平面近似で、測地計算には使わない。発進台は幅12 m、奥行20 m、前縁の水面上高10 m、北西向き、北西側が3.5°下がる長方形で近似する。寸法・傾斜は利用者提供の概数であり、地理データ由来ではない。湖岸の平坦帯、島の表示高や簡略化、AW3D30のDSM・粗い格子は見た目のための近似である。

| 用途 | 候補 | 確認事項 |
|---|---|---|
| 地上風 | 気象庁AMeDAS | 観測高度、時間平均、欠測、湖上への適用性 |
| 一般風 | ERA5 | grid解像度、時刻、鉛直座標、利用条件 |
| 湖上風・波 | 水資源機構 | 観測地点、較正、期間、配布条件 |
| 遠景 | JAXA AW3D30 | 採用済み。DSM/地盤区分、EGM96基準、派生成果条件を記録 |
| 局所地形 | 国土地理院DEM | 精度、測地系、標高基準、表示条件 |
| 土地被覆 | ESA WorldCover | class定義、version、帰属表示 |
| 衛星画像 | Sentinel-2 | 任意用途、雲・時期、帯域、条件 |
| 建物 | PLATEAU | 対象範囲、LOD、都市ごとの条件 |
| 地理情報 | OpenStreetMap | databaseと画像成果物の条件 |

環境風と波浪較正には、AMeDAS、ERA5、琵琶湖局地風研究、水資源機構資料を引き続き候補とする。広域衛星textureは必須としない。
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

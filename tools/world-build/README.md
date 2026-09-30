# 琵琶湖の環境asset生成

`build_biwa_world.py` は入力スナップショットから、利用条件を分けた実行時assetを生成する。Analysis mapには発進点周辺の湖岸線と、試作配置であるcamera_pointsを収録する。OSMの局所 `map` API入力から砂浜・樹林・並木・桟橋・岸壁・防波堤のfeature assetも生成する。

- `assets/biwa-shoreline.json`: OSM relation 63499の湖岸線・島輪郭（ODbL-1.0）。
- `assets/biwa-analysis-map.json`: 同じOSM relationから切り出した発進点周辺の湖岸線とcamera_points（ODbL-1.0）。
- `assets/biwa-terrain.json`: JAXA AW3D30から区分を選別した標高（JAXA研究データ利用条件）。
- AW3D30の無効・欠測画素は、OSMで陸地と判定される頂点に限りCopernicus DEM GLO-30 Publicで補完する。Copernicus GLO-30は自由利用・加工・再配布が可能だが、所定の帰属・免責表示を `THIRD_PARTY_DATA.md` とアプリ内クレジットに維持する。
- `assets/biwa-land-mask.json`: OSMの湖面・島輪郭から生成した地形格子の陸水マスク（ODbL-1.0）。
- `assets/biwa-venue-features.json`: 発進点周辺の砂浜・樹林・並木・港湾構造物（ODbL-1.0）。

OSM XMLとGeoTIFF・maskはリポジトリへ含めない。湖岸線は[OpenStreetMap relation API](https://api.openstreetmap.org/api/0.6/relation/63499/full)、局所地物は[OpenStreetMap map API](https://api.openstreetmap.org/api/0.6/map)から取得する。map APIのbboxは経度・緯度順で `136.2375,35.2805,136.2715,35.3077` とする。局所地物はrelation geometryを補完しないため、bbox境界でwayが切れる場合がある。派生データはODbL-1.0、帰属表示は「© OpenStreetMap contributors」とし、再配布時にはデータベースのshare-alike要件を確認する。AW3D30の `DSM` と `MSK` COGは、[JAXA Earth API collection](https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/JAXA.EORC_ALOS.PRISM_AW3D30.v3.2_global/collection.json)の `E135.00-N35.00-E136.00-N36.00` と `E136.00-N35.00-E137.00-N36.00` を使い、JAXAの利用条件に従う。データversionと入力SHA-256を生成assetに記録する。

host-side依存をインストールする。

```powershell
python -m pip install -r tools/world-build/requirements.txt
```

入力スナップショットを指定して生成する。

```powershell
python tools/world-build/build_biwa_world.py .tmp/lake-biwas-relation.osm `
  --dsm-e135-n35 .tmp/E135-N35-DSM.tiff --mask-e135-n35 .tmp/E135-N35-MSK.tiff `
  --dsm-e136-n35 .tmp/E136-N35-DSM.tiff --mask-e136-n35 .tmp/E136-N35-MSK.tiff `
  --copdem-e135-n35 .tmp/Copernicus_DSM_COG_10_N35_00_E135_00_DEM.tif `
  --copdem-e136-n35 .tmp/Copernicus_DSM_COG_10_N35_00_E136_00_DEM.tif `
  --venue-osm-input .tmp/matsubara-venue.osm
```

広域地形は300 m格子とする。離陸地点を含む12.06 km四方の湖岸・飛行圏を30 m格子で表現し、外周1,800 mで広域面へ滑らかにつなぐ。湖岸近くに90 m格子を使うと細い陸地を取りこぼし、地形接合が長くなるため、この範囲は元のAW3D30と同じ格子間隔を維持する。主要4島には30 m格子、DSMで足場が得られない約50 m幅のOSM wooded isletには元DEMの30 m画素を参照する10 mサンプリング格子を使う。対岸の4峰は90 m格子とし、外周600 mで広域面へ接続する。遠景ほど頂点数を減らし、手元の地形と対岸の稜線へ標本を集中する。AW3D30 mask区分0・12の標高を優先し、欠測部をCopernicus GLO-30で補完する。補完はOSMで陸地と判定された格子点に限り、AW3D30との重複標本から得た+0.3 mの局所中央値補正を行う。これはEGM96/EGM2008の測地変換ではなく景観用の位置合わせである。水域とAW3D30の国土地理院DTM区分4は除外する。利用条件と標高基準の制限は [`docs/data-sources.md`](../../docs/data-sources.md) を参照する。runtimeは生成済みJSONを読み込み、GISデータの通信・解析を行わない。

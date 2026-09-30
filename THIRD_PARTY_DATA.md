# 第三者データとライセンス

プロジェクトのソースコードはMIT Licenseである。
第三者データ・画像・地形・機体assetには各出典の利用条件を適用し、MITへ再ライセンスしない。
ソフトウェア依存のライセンスも各packageの条件を維持する。

配布assetの一覧と個別条件は `assets/manifest.toml` に記載する。候補一覧は `docs/data-sources.md` に記載する。
候補掲載は取得・再配布の許諾確認を意味しない。

## 琵琶湖の局所湖岸

`assets/biwa-shoreline.json` と `assets/biwa-analysis-map.json` は OpenStreetMap の琵琶湖 relation 63499 から、
発進地点周辺の外周湖岸線と名前付き島（沖島・多景島・竹生島・オコノ洲を含む）の内周輪郭を切り出して簡略化した加工済みデータである。
このデータには [Open Database License 1.0](https://opendatacommons.org/licenses/odbl/1-0/) を適用する。
帰属表示は「© OpenStreetMap contributors」であり、ゲーム中の HUD と
データ内のメタデータに掲載する。ソースコードの MIT License をこのデータへ適用しない。

## 琵琶湖の地形標高

`assets/biwa-terrain.json` は JAXA Earth API が配布する AW3D30 v3.2 のDSMとmaskを主標高にし、Copernicus DEM GLO-30 PublicをOSM陸地上の欠測補完に使う描画用地形である。
AW3D30 mask区分0と12を優先し、国土地理院DEM区分4を除外する。水域はOSM陸水maskで補完対象から除く。
JAXA研究データ利用条件およびCopernicus WorldDEM-30 free and open licenseを適用し、各出典と利用条件をゲーム内HUDとmanifestに記載する。JAXAの商用利用は無償だが事前通知が必要である。

Copernicus GLO-30を改変・再配布するassetには、次の出典表示を維持する。

> © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved.

HUDには上記の出典文をリンク付きで常時表示し、加工データ用の告知も初期状態で展開する。

加工した標高assetには、次の表示も維持する。

> produced using Copernicus WorldDEM-30.

配布を対象とする利用条件または法的告知には、次の免責文も含める。

> The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30.

GLO-30はEGM2008、AW3D30はEGM96を標高基準にする。補完値の+0.3 mは当該領域の重複標本から得た視覚的位置合わせであり、測地変換ではない。補完結果も測量・航法・飛行物理に使わない。条件の一次資料は[Copernicus DEM license](https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM)と[AW3D30の利用条件](https://earth.jaxa.jp/en/data/policy/)を参照する。
EGM96標高から琵琶湖B.S.L.への変換は近似であり、測量・航法・物理計算には使用しない。

## 琵琶湖・松原の局所地物

`assets/biwa-venue-features.json` はOpenStreetMap contributorsの局所データから抽出・簡略化した砂浜、樹林、並木、桟橋、岸壁、防波堤の地物データである。OpenStreetMapデータと同じくODbL-1.0を適用し、帰属表示とライセンスへのリンクを維持する。派生データベースを公開する場合はODbLのshare-alike条件を満たす。原データsnapshotは再配布しない。各assetとmanifestにsnapshot hash、API出典、加工方法を記録する。

沖の白石はOpenStreetMapの4つの島輪郭を使い、高島市が公表する「4個の岩」「最高14 m」という事実を描画専用の形状上限に利用する。個々の岩の高さは公開資料にないため、輪郭面積に応じた視覚上の推定値であり、地形標高・衝突形状・飛行物理として使用しない。市の文章を出典として記録し、写真・画像は利用していない。

## 登録要件

`assets/manifest.toml` を正本とし、各assetに次の情報を登録する。

- path: `assets/` 内の相対パス
- sha256: 配布ファイルのSHA-256
- source: 出典URLまたは自作データの生成元識別子
- source_version: 元データのversionまたは取得日と識別子
- license: SPDX識別子または明示的な利用条件名
- license_url: 条件のURLまたはリポジトリ内の条件ファイル
- attribution: 実際に表示する帰属文
- processing: tool versionと処理パラメータ、地理データではoriginと標高基準

自作assetも生成元とライセンスを登録する。`web/public/` の配布assetも同じ管理経路を使用し、
未登録ファイルの別経路配布を禁止する。manifestはファイルの存在、hash、登録漏れ、重複をCIで検査する。
検査はmetadataの存在を保証する。法的な利用可否は採用時の一次資料確認により判断する。
出典ファイルを移植する場合は元のcopyrightとlicense noticeを保持する。

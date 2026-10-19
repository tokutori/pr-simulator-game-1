# 第三者データとライセンス

本プロジェクトのソースコードには[MIT License](LICENSE)を適用する。
第三者のデータ・ソフトウェア・フォントには、それぞれの利用条件を適用する。

配布ファイルと出典・加工方法・ハッシュの正本は[`assets/manifest.toml`](assets/manifest.toml)である。
データの用途と仕様は[データ形式・出典](docs/data-and-formats.md)を参照する。

## ゲーム内と配布物の表示

- Titleと一時停止メニューの「クレジット・利用条件」から、出典・加工告知・免責・同梱コードの利用条件を閲覧できる。
- 飛行画面には短い地図帰属を表示し、データとOSSの詳細はクレジットにまとめる。
- 配布物には本書、必要な著作権・利用条件、第三者ソフトウェアのライセンス一覧を含める。
- データ内の出典と加工情報も保持する。クレジットの表示によって、再配布時の告知義務がなくなるわけではない。

OpenStreetMapの[ゲーム・シミュレーション向け帰属指針](https://osmfoundation.org/wiki/Licence/Attribution_Guidelines#Computer_games_and_simulations)は、クレジットやメニューでの帰属表示を認めている。
長い帰属文や免責文の常時HUD表示は、本プロジェクトの配布条件としない。

## 地図・地形・気象

### OpenStreetMap

次の加工済みデータはOpenStreetMap contributorsのデータを使用する。

| ファイル | 用途 |
| --- | --- |
| `assets/biwa-shoreline.json` | 琵琶湖の湖岸と島の輪郭 |
| `assets/biwa-analysis-map.json` | 飛行解析用の局所湖岸図 |
| `assets/biwa-land-mask.json` | 地形の陸水区分 |
| `assets/biwa-venue-features.json` | 砂浜・樹林・並木・桟橋・岸壁・防波堤 |

湖岸・島・陸水区分は琵琶湖relation 63499を基に抽出・簡略化した。
周辺地物は松原の局所データを基に抽出・簡略化した。
元のXMLは配布しない。

帰属表示は「© OpenStreetMap contributors」である。
[Open Database License 1.0](https://opendatacommons.org/licenses/odbl/1-0/)を適用し、[OpenStreetMapの著作権・ライセンス案内](https://www.openstreetmap.org/copyright)への参照を保持する。
派生データベースの公開・再配布には、ODbLの継承条件を適用する。

### JAXA AW3D30

`assets/biwa-terrain.json`は、JAXA Earth APIが配布するAW3D30 v3.2の標高・マスクを主な資料として加工した描画用地形である。
研究データ名と出典は「AW3D30 (JAXA)」として表示する。
[JAXA研究データ利用条件](https://earth.jaxa.jp/en/data/policy/)を適用する。
商用利用は無償であるが、JAXAへの事前通知が必要である。

注意: 標高基準の調整は描画用の近似である。測量・航法・飛行物理には使用しない。

### Copernicus WorldDEM-30

`assets/biwa-terrain.json`はCopernicus DEM GLO-30 Publicを加工し、陸地の標高欠測を補完する。
[Copernicus WorldDEM-30 free and open license](https://documentation.dataspace.copernicus.eu/APIs/SentinelHub/Data/DEM/resources/license/License-COPDEM-30.pdf)を適用する。

クレジット・配布告知・該当データのメタデータには、第6条(a)(b)に基づく出典・加工表示を保持する。

> produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights reserved.

抽出・補間・位置合わせを行ったことを併記する。
第6条(c)に基づく次の免責は、クレジットと配布時の法的告知に保持する。

> The organisations in charge of the Copernicus programme by law or by delegation do not incur any liability for any use of the Copernicus WorldDEM-30.

### 気象庁の平年値

`assets/biwa-typical-july-environment-v6.json`は、[気象庁の彦根・月平年値](https://www.data.jma.go.jp/stats/etrn/view/nml_sfc_ym.php?block_no=47761&day=29&month=12&prec_no=60&view=a3s&year=)から地上平均風速と最多風向を抽出し、湖上の風・波・空の仮定を加えたゲーム用データである。
[気象庁の利用条件](https://www.jma.go.jp/jma/kishou/info/coment.html)に従い、次の出典・加工表示を保持する。

> 出典：気象庁ホームページ。月平年値を加工し、pr-simulator-game-1が仮定に基づくゲーム用環境を作成。

原HTMLは配布しない。
本プロジェクトが作成した生成処理・仮定にはMIT Licenseを適用する。

注意: 月平均風速と最多風向は、観測された平均風ベクトルや湖上の三次元風場を表すものではない。波や空もゲーム用の仮定であり、気象庁が作成した湖上気象ではない。

### 沖の白石

島の輪郭にはOpenStreetMapを使用する。
岩の個数と高さには、高島市が公開する説明を参考として使用する。
個々の岩の高さは視覚上の推定である。
市の写真・画像は使用しない。

## フォントと同梱ソフトウェア

| ファイル・用途 | 出典・利用条件 |
| --- | --- |
| `assets/fonts/MPLUS1p-Regular.ttf` | M PLUS 1p Regular。M+ Project AuthorsとRounded M+ Project Authorsの著作物。[SIL Open Font License 1.1](assets/fonts/MPLUS1p-OFL.txt)。元のフォントを改変せず、日本語表示に使用する。 |
| `assets/test-fonts/FiraMono-subset.ttf` | Mozilla FoundationとTelefonica S.A.の著作物。[SIL Open Font License 1.1](assets/test-fonts/FiraMono-LICENSE.txt)。CPU試験専用であり、製品のフォント読込みとNative配布物には含めない。 |
| `assets/earcut.LICENSE.txt` | GeoRust earcutのMITと、派生元MapboxのISCの両告知。ポリゴンの三角形分割に使用する。 |
| `assets/ryu-js.LICENSE.txt` | ryu-jsのBoost Software License 1.0。数値の文字列変換に使用する。 |
| `assets/game-ui-validators.LICENSE.txt` | AjvとesbuildのMIT。生成したUI検査コードに含まれる告知を保持する。 |
| `assets/time.LICENSE.txt` | timeと有効な依存ライブラリのMITを選択した告知。保存日時の変換に使用する。 |

WebのThree.js・SnabbdomとNativeのBevyを含む依存ライブラリにも、各ライセンスを適用する。
Native配布物の第三者ソフトウェア一覧は、固定した`Cargo.lock`と対象構成から生成する。
上記の同梱告知を、この依存一覧で置き換えない。

## プロジェクト作成データ・画像

- `assets/biwa-launch-venue.json`は、利用者が提示した発進地点と概略寸法を基に作成した設定である。測量データではない。
- `assets/indigo-resist-decal.png`と`assets/tokushima-fin-artwork.png`は、利用者提供写真を参考に生成した描画用画像である。参考写真は再配布しない。
- これらの配布ファイルには、manifestに記載したMIT Licenseを適用する。

## 登録・再配布時の確認

新しいファイルは、配布前にmanifestへ次の事項を登録する。

- パスとSHA-256。
- 出典・元データの版・取得元の識別情報。
- ライセンス名、利用条件への参照、帰属表示。
- 加工方法と、地理データの座標・標高基準。

`web/public/`からの配布にも同じ登録規則を適用する。
CIはファイルの存在・ハッシュ・登録漏れ・重複を検査する。
利用条件への適合は、採用時の一次資料確認と表示・同梱物の検査によって確認する。

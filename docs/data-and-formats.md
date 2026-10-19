# データと形式

## 正本と版管理

外部データのcodecは`birdman-game-format`、実行環境・archiveの準備は`birdman-game-session`、filesystem・IndexedDB・ダウンロードはplatform adapterが所有する。runtimeで気象APIやGISサービスへ接続しない。

| 対象 | 現行契約 |
| --- | --- |
| 環境JSON | `EnvironmentDocument`、schema 1、最大8 MiB |
| 飛行記録JSON | `TailFlightRecordDocument`、schema 6、最大16 MiB |
| 飛行ログCSV | export version 3 |
| Personal Best | 比較方式v2、Rustによるkey再生成・選定 |
| 既定機体・scenario | aircraft model 3、catalog 5、scenario 5、controller profile 3 |

- 版の正本は実装の公開定義・登録catalogとする。
- JSON schema、物理model、機体、controller、環境、score、比較方式、build来歴は異なる概念であり、独立して保持する。
- 未知の版へ現行値を補完しない。

## assetの登録・出典・利用条件

- ゲーム固有の画像、音声、model、shader、binaryは`assets/`へ置き、`assets/manifest.toml`へpath、SHA-256、source、source version、license、license URL、attribution、加工条件を登録する。
- 入力snapshotと配布生成物のhashを区別する。
- 生データを配布物へ無条件に含めない。

再生成処理は、取得、範囲切り出し、座標・標高基準の整理、NED変換、LOD生成、量子化・圧縮、hash作成からなる。source version、tool version、パラメータ、原点を保存し、観測値・導出値・仮定・ゲーム調整を区別する。

- `web/public`は空とし、`web/src`にはコード・HTML・CSSを置く。
- Viteのmodule import、`new URL(..., import.meta.url)`、HTML resource属性、CSS `url()`、`?url`・`?raw`を登録assetと照合する。
- 静的な一般の`new URL()`も拡張子にかかわらず検査し、動的asset URLは登録IDから解決する。
- Worker/SharedWorkerへ直接渡すJS/TS source URLはmodule参照として扱う。

- HTMLの`src`・`href`・`poster`は単一URL、`srcset`・`imagesrcset`は候補ごとに検査する。
- CSS `image-set()`は対応する登録検査を実装するまで使用しない。
- data URL化する小容量assetにも元ファイルの登録を要求する。
- 依存package内のassetはそのpackageの利用条件で管理する。

- manifest検査は存在、hash、重複、登録経路を確認する。
- 利用許諾そのものの証明にはならない。
- 法的な表示文と個別条件は[第三者データ](../THIRD_PARTY_DATA.md)、配布への同梱は[nativeガイド](native-guide.md)に定める。

## 地理・景観データ

### 原点と発進台

- 発進原点はWGS84緯度35.294075°、経度136.254448°である。
- 発進台は幅12 m、奥行20 m、前縁の水面上高10 m、北西向き、北西側へ3.5°下がる長方形で近似する。
- 寸法・傾斜は利用者提供の概数であり、GISの実測値と区別する。

### 登録asset

| asset | 入力と用途 |
| --- | --- |
| `biwa-shoreline.json` | OpenStreetMapの琵琶湖relation 63499、version 111、snapshot `2025-08-14T09:30:32Z`の湖岸と島輪郭。発進地点付近2 m、遠景20 m、島8 mの許容差で簡略化する |
| `biwa-analysis-map.json` | 原点を中心とする3 km四方のAnalysis用湖岸線。遠い島によって飛行軌跡が過度に縮小しない表示範囲を持つ |
| `biwa-land-mask.json` | 同じOSM輪郭からAW3D30格子点へ生成した陸水判定。標高は含めず、OSM由来の派生データとして分離する |
| `biwa-venue-features.json` | 原点周辺3 km四方のOSM map API snapshotから抽出した砂浜、樹林、tree row、桟橋・岸壁・防波堤 |
| `biwa-terrain.json` | AW3D30を主入力とし、欠損陸域だけCopernicus DEMで補完した描画専用標高 |

- 3D用の島輪郭は沖島・多景島・竹生島・オコノ洲を含む17輪である。
- 竹生島の表示名はinner ring内のway 183556759の位置から識別する。
- 外周切出し範囲、入力hash、加工条件はasset内のmetadataに保持する。

Analysis用の発進台・湖岸・望遠camera点はOSMとAW3D30に基づく試作位置である。望遠高度80 mは地形遮蔽を避ける表示調整であり、測量値ではない。

- OSM地物は局所APIに存在しない場合やbbox端でwayが分割される場合に欠落しうる。
- タグ分類は樹種、砂質、構造物の高さ・現況を保証しない。
- 桟橋等の幅・高さは表示用の概算とする。
- DSMに含まれる樹冠へ樹林高さを再加算しない。

### 標高と描画解像度

主入力はJAXA Earth APIの`JAXA.EORC_ALOS.PRISM_AW3D30.v3.2_global`、2021-02 collection、E135-N35・E136-N35である。mask 0の有効AW3D30とmask 12のPRISM DSMを優先し、mask 4の国土地理院DEMを含む他の補完区分は採用しない。

| 範囲 | 描画格子 |
| --- | --- |
| 広域 | 300 m |
| 原点を含む12.06 km四方と主要4島 | 30 m |
| OSM woodland属性を持つ小島 | 10 mの再サンプリング |
| 対岸4峰 | 90 m |

- 30 m近景格子は外周1,800 mで広域の三角形面へ接続する。
- 格子間隔は描画頂点の間隔である。
- 10 m再サンプリングは、元の30 m DSMの精度を10 mへ高める処理ではない。

- AW3D30に値がない陸上点だけ、Copernicus DEM GLO-30 Public COGのN35-E135・N35-E136で補完する。
- 水域の欠損は補完しない。
- CopernicusはEGM2008、AW3D30はEGM96を標高基準とする。
- 重複陸上8,828標本の差の中央値−0.325 mに基づき、補完値へ+0.3 mを加える。
- これは局所的な表示位置合わせであり、測地基準変換・現地較正と区別する。

- 水面相対高の生成にはB.S.L. 0 = T.P. +84.371 mを差し引く。
- EGM96からT.P.への測地基準変換は実施していない。
- この標高は景観用であり、測量・航行・物理高度に使用しない。

- 多景島・オコノ洲では、OSM輪郭内の有効陸上DSM標本を使い、元格子の半間隔へ最大4点の逆距離加重で補間する。
- 多景島の深緑の変化はart directionであり、植生観測値ではない。
- 沖の白石4岩のうち最高14 mは[高島市の案内](https://www.city.takashima.lg.jp/kanko_bunka_sports/kanko_tokusan_rekishi_bunkazai/3/4633.html)を描画上限に用い、他の高さは輪郭面積から推定する。
- これらの補間・岩礁形状をcollisionへ用いない。

### 出典と帰属

- OSM加工データはODbL 1.0として扱い、© OpenStreetMap contributorsと[著作権・ライセンス](https://www.openstreetmap.org/copyright)を表示する。
- AW3D30は「AW3D30 (JAXA)」を表示する。[製品説明](https://www.eorc.jaxa.jp/ALOS/en/aw3d30/aw3d30v3.2_product_e_e1.2.pdf)と[JAXA研究データ利用条件](https://earth.jaxa.jp/en/data/policy/)に従い、商用利用時の事前通知を含む条件を確認する。
- Copernicusの帰属・改変通知・免責は[第三者データ](../THIRD_PARTY_DATA.md)の全文を保持する。[公式データ説明](https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM)を参照する。

## 機体形状データの区分

- 共有f64構造形状と、表示専用の材質・sampling・たわみを区別する。
- 主翼span 20.863 mは提供図面の値である。
- root chord 1.13 m、tip chord 0.38 mや輪郭の一部は図面比率からの推定であり、実測の再現値として扱わない。
- 構造形状はright/up/aft、単位mで保持し、FRDへは`(-aft, right, -up)`で変換する。

- 主翼の60%・80%位置の二段上反角、翼型・コクピット断面・尾翼節点から、表示meshと固定物理polytopeを独立に生成する。
- 物理は有限polytopeの全頂点を保持し、静水面に対する任意姿勢の支持量をその頂点から導出する。
- 連続曲面、表示たわみ、可動尾翼、細径材を含む変形collisionの精度保証へ拡張しない。

- 表示たわみのE=45 GPa、I=4×10⁻⁶ m⁴、支持質量110 kg、基準速度9.5 m/s、load factor 0.3–1.6は描画用仮定である。
- 機体・pilotの実行時質量や運動方程式へ代入しない。
- 空力・質量・慣性・制御の正本は[飛行力学](flight-dynamics.md)と[飛行制御](flight-control.md)に定める。

## 環境JSON

### 境界と風場

- schema 1は風・波・空・local frame・出典を検証する。
- JSONは最大8 MiB、風標本は65,536件、各説明はUTF-8で2,048 byte、出典・月統計・各source参照はそれぞれ16件以内とする。
- 配列は上限を超える要素をstorageへ追加する前に拒否する。
- JSON escapeを含むencode結果も入力上限内へ制限する。

- `ScenarioModelCatalog::resolve`でcatalog、scenario、aircraft、environmentのversionとWeatherを完全照合する。
- scenario IDを配列indexへ算術変換しない。
- 現在の登録環境はCalm 1、Mild 2、Challenging 4、NearLimit 5、Typical 6である。
- Typicalは登録格子、他は登録済みの一様風presetを用いる。

- `local_frame`はWGS84原点と水面基準を持つ。
- NEDのdown=0は固定水面、負のdownは水面上である。
- `wind_grid`はnorth/east/down順のorigin、正のspacing、各軸2点以上のcountを持ち、indexは`(down * east_count + east) * north_count + north`である。
- 速度は空気が向かうNED方向のm/sとする。
- 気象の「吹いてくる風向」から生成する際は水平符号を反転し、鉛直流もdown正とする。

- 構築時に有限座標・速度、件数、表現可能な上端、代表地点のcoverageを検査する。
- ownedな格子storageを一度構築し、coreのWindFieldが借用する。
- 閉区間補間と範囲外エラーを保持する。
- 実scenarioがRK4途中の機体位置と全空力評価点を覆うことは統合試験で確認する。

代表風は位置と水面上高度を明示する。一地点の値を全格子の風として表示しない。

### 根拠と描画条件

- local frame、wind grid、waves、skyの各provenanceを必須とし、`observed`・`derived`・`assumed`・`game_tuned`を排他的に区分する。
- 導出元のsource indexと仮定の説明を検証する。
- sourceにはURL、version、入力SHA-256小文字64桁、利用条件、帰属・加工表示を持たせる。
- codecはURLへ通信しない。
- 生成物hashはmanifestへ置き、JSON内へ自己参照hashを作らない。

- Typicalの観測anchorは、気象庁[彦根・1991–2020年平年値](https://www.data.jma.go.jp/stats/etrn/view/nml_sfc_ym.php?block_no=47761&day=29&month=12&prec_no=60&view=a3s&year=)の7月平均風速2.5 m/s・最多風向北西である。
- 月平均scalarと最多風向は、湖上の瞬間ベクトル・飛行高度の風・全期間の卓越風を保証しない。
- 月統計は格子の平均風と別に保存する。

- [Endoh et al.](https://www.jstage.jst.go.jp/article/rikusui1931/56/4/56_4_269/_article)と[Edagawa](https://www.jstage.jst.go.jp/article/grj1984a/65/10/65_10_735/_article/-char/en)は湖上風の非一様性・局地風の質的根拠とする。
- 要旨や別条件の事例からゲーム領域の数値勾配、Typical、NearLimitを直接復元しない。
- Calm等の強度、湖上勾配、鉛直流、高度補正、持続性は観測と区別して記録する。
- 気象庁の加工データは[利用規約](https://www.jma.go.jp/jma/kishou/info/coment.html)に基づく出典・加工表示を保持する。

- 波は履歴、fetch、detail、seedを持つ描画条件である。
- 風速は60 m/s以下、fetchは正で50 km以下、detailは正で3以下とする。
- 物理接触面は変更しない。
- 空は北0°・時計回りの方位、±90°の太陽高度、0–1の雲量、非負の雲底、正で有限の視程を持ち、飛行中は固定する。

### metadata query

- WASMのenvironment snapshotはschema 1とし、Available・Unavailable・NoSelectionを排他的に返す。
- TitleはNoSelection、Setupは選択preview、BriefingからResultは封印済み環境、Replayは対象record/archive、Attractはdemoの環境を参照する。
- 未記録の値へ現在の環境を代入しない。

- queryは名称、identity、hash、local frame、格子範囲、代表風、波・空・月統計・出典を返し、全風格子を送らない。
- registry queryのJSON入力は最大4,096 byteで、未知field、不正範囲、zero versionを拒否する。
- 未登録identityはUnavailableとし、aliasや再シミュレーションで補わない。

- 登録環境の準備成功・失敗は共有OnceLockへ保持し、metadataと物理WindFieldは同じowned storageを使う。
- 64bit seedは境界でlow/highのu32へ分割する。
- 独立したquery世代を追加せず、adapterの既存要求IDとsession照合を利用する。

- `asset_bytes`の環境JSON hashと、preset実装等の`source_fingerprint`を区別する。
- 前者は特定assetの識別、後者はbuild来歴である。
- source hashを比較方式v2の条件へ混入させない。
- 環境定義の有無とrendererでの適用成功も別の観測として扱う。

## 飛行記録とログ出力

### 所有・容量・時刻

- FlightRecordはResult、Analysis、Replay、Attractの共通の正本である。
- coreがsample追加、確定、集計、補間を所有する。
- Briefingで最大4,000 tickと初期sampleに必要な最大4,001件を予約し、予約失敗を発進前に返す。
- 記録を循環上書きしたり、容量不足でphysicsを間引いたりしない。

- 初期sampleはtick 0であり、以降は成功した各固定tickを保存する。
- input kは区間k→k+1へ適用する。
- 時刻は整数tickとfractionで保持し、f64の反復加算を正本にしない。
- 位置・速度はNED、角速度はFRD、quaternionはbody→NED、すべてf64とする。
- 機体datumと合成重心のtelemetryを区別する。

- 接触は全登録点の最早fractionを採用する。
- 位置・速度・角速度・pilot状態と最短経路quaternion補間を同じ瞬間へ揃え、record、Result、score、表示へ同じ終端sampleを渡す。
- 正のfractionでは区間中に保持した尾翼値を使い、fraction 0では既存sampleとcontrolsを保持して新inputや重複sampleを追加しない。
- 失敗tickの未来状態を露出しない。

- tail controlsには保持取付角、直前区間の任意input、pilot目標、正規化操縦要求、Manual/FBW/混合の実適用値を保存する。
- 初期inputはNoneである。
- 閲覧時にFBWや入力を再計算しない。

- finalizationはcomplete/interrupted/failed、終了理由、最後の有効tick/fraction、score、元failureを一度だけ確定する。
- 型付きfailureのsite・stage・causeを保持し、情報欠落へ架空の診断を補わない。
- 有効sampleがない初期化失敗はrecord unavailableとする。

### codecとJSON

- 現行codecはschema 6だけを受理する。
- 旧schemaのdecoder・migrationを追加しない。
- 未対応データを通知して元の保存内容を保持する。
- schema 6の破損、未知field、非有限値、非単位quaternion、時刻・header不整合を拒否する。
- 任意のidentity metadataには後述の未知raw保持規則を適用する。
- 数値のround-tripは`serde_json`の`float_roundtrip`を使用する。

- live記録は共有producerがdocumentへ変換し、保存archiveは取得したJSON原文を保持する。
- JSON exportは解決済み条件と来歴を含む完全な記録である。
- 保存されていない情報を現行buildから補完しない。

### CSV version 3

- CSVは初期・全成功tick・fractional terminalを省略せず、位置、速度、姿勢、角速度、pilot状態、入力、actuator、風、telemetry、終了情報等を出力する。
- 列名に単位を明示し、f64のround-trip表現を使う。
- 未提供値は空欄とし、ID・構造化診断は`_json`列へ格納する。

- 加速度は保存済みNED速度、FRD角速度、pilot相対速度の有限差分による派生値である。
- 内点は不等間隔3点、端点は2点の割線を用い、methodとstencil時刻を出力する。
- これはspecific force、荷重、合成重心加速度、Euler角の二階微分を表さない。
- 標本不足・不正間隔・非有限推定は状態名を出し、NaN/∞で埋めない。

- 比較metadataは状態・有界な方式番号・既知build来歴のscalar列へ投影する。
- 未知rawと完全なcontroller条件はJSONに保持し、各sampleへ反復しない。
- rustc・target・profileは最大256 byteの既知値をJSON文字列化した`_json`列とし、CSV引用処理とは分離する。
- 外部記録の先頭記号を表計算の数式として出力しない。

### scoreとtelemetry

- 対気速度は合成重心の対地速度から同地点の風を引いた3D normである。
- 対地速度も3D normとし、高度は重心の`-down`である。
- CG迎角は診断値であり、主翼局所迎角と同一視しない。

- score definition 1の飛距離は、発進datumから終端datumまでの水平変位を発進courseへ符号付き投影した距離である。
- 軌跡長、直線変位norm、最大到達距離を混同しない。
- 最大値の集計は保存sampleの最大であり、連続軌道の極値を保証しない。

- course方向は`(cos ψ₀, sin ψ₀)`、右方向は`(-sin ψ₀, cos ψ₀)`とする。
- 風のcourse成分は追風正である。
- 横変位には操縦・姿勢等も寄与するため、全横変位を風だけの結果と解釈しない。

## Personal Bestと保存

### 比較方式v2

Personal BestはcompleteなWaterContact、score、現行physics/score version、既知の解決済み条件、有効canonical keyを要求する。共有`TailPersonalBestSelection`が比較を所有し、同点は最初の既存recordを保持する。

keyは次の条件をSHA-256へ入力する。

- 明示identity、catalog・aircraft・controller・scenario・environment・physics・scoreのversion。
- seed、tick上限、Hz、解決済みInformation/HUD、Assistance、Weather。
- 実ControlModeとShared authority、二軸gain、slew、optional alpha guard、course。
- 初期datum位置・速度、quaternion、角速度、wind、尾翼取付角、pilot位置・速度。

- finite f64は負zeroをpositive zeroへ正規化してbig-endian IEEE-754で符号化する。
- 画質、presentation、build来歴は比較条件へ含めない。
- 記録自身からkeyを再生成し、保存keyとの一致を検査する。
- 現在のpresetから過去のcontrollerを再解決しない。

- schema 6の任意`identity_metadata`は、未指定・既知v2・未知rawを排他的に保持する。
- block欠落、未知方式、構造情報不足の記録は未順位で閲覧し、元key・原文を保持する。
- 既知v2の数値不正・保存key不一致は拒否する。
- 新旧方式の横断比較、key補完、既存JSONの書換えを行わない。

- build来歴は4種のsource fingerprint、Cargo.lock SHA-256、観測したrustc・target・profileを保持する。
- `BPG_NATIVE_SOURCE_SHA`が明示された場合だけsource commit宣言値を保存する。
- この宣言はlocal treeのclean状態を保証しない。
- 取得不能値は不明のまま保持する。
- 共有Result/Replayの技術情報は有界scalarを表示し、未知rawの方式表示はu32に限定する。

### 保存adapter

- WebのIndexedDBはversion 4を用い、record・metadata・PB更新を同一transactionで確定する。
- 旧DB version 1–3は内容を保持して明示的に拒否する。
- 旧DB移行、metadata backfill、互換index再構築を行わない。
- PB indexはRust由来の方式revisionを新規entryへ保存し、読取りで欠落・未知revisionを未順位へ分類する。

- Title previewは最大3件である。
- PB探索は保存された全候補をcursorで処理し、preview件数へ制限しない。
- 失敗時はtransactionをrollbackしてselectorを解放する。
- Nativeも共有producer/selectorを利用し、filesystem上の全候補を検査する。
- 同点ではpath昇順の最初の記録を保持し、新しい索引用DBを設けない。

### 意味versionの更新責務

| 変更対象 | 更新するversion |
| --- | --- |
| 運動方程式・積分・接触算法・数値結果へ影響する依存 | physics model |
| 質量・慣性・空力係数・機体の物理形状 | aircraft model |
| 操縦支援・controllerの意味 | controller profile |
| 不変scenario・発進条件・接触proxy・会場条件 | scenarioと必要なcatalog/model |
| WindField・環境の意味と数値条件 | environmentと必要なscenario |
| 終了・飛距離・採点 | score definitionと必要なphysics |
| 比較入力・hash符号化・適格性 | Personal Best comparison |

- 実装者とreviewerが更新対象を確認する。
- テスト、証明注釈、同値な整理、描画依存だけの変更は来歴として識別する。
- 意味同値の自動判定、source hash allowlist、attestation表を作らない。
- 比較方式の導入だけを理由にphysics/scoreを更新しない。

## Replayとquery時刻

- Replayは保存snapshotの補間queryであり、物理を再積分しない。
- Pause、seek、再生速度は同じRustのclockへ作用し、graph cursorと3Dが同じrecord時刻を参照する。
- Resultへ戻る際は元の確定状態を表示し、cursor状態へ置換しない。

- 記録queryのwire形式はschema 2であり、保存JSONのschema 6と区別する。
- Result・Replay・Attractの記録所有phaseだけを受理し、live Flightからの記録queryを拒否する。
- Summary、Analysis、再生poseは同じsource contextを持ち、元のfinalization・terminal stamp・確定scoreと、cursor位置・時刻の投影値を別fieldとして保持する。
- 値が存在しない角度・scoreには型付きの不在理由を返す。

- 風queryは検証したnorth/east下限・水面上高度・正のspacing・5×5寸法をechoし、North行・East列の順で25地点すべてを返す。
- 保存identityに対応する登録WindFieldを評価し、未登録identityはUnavailableとする。
- 一点でもOutsideGridになる場合はOutsideRegisteredDomainとし、部分格子やzero風を返さない。

- 同じrecordのseek・clock・再生速度の変更はdatasetを保持する。
- Retry、設定変更、archive開閉、disposeではrecord source tokenを失効し、古い非同期query結果を拒否する。
- 要求ごとの世代と、元recordの世代を区別する。

- Attractはplayerの飛行と独立した有界record・clockを使う。
- demo recordをplayerの結果・保存・exportへ混入させず、Attractからのplayer exportは拒否する。
- seedはwire境界でlow/highの32bitへ分割し、整数精度を維持する。

- tick/fraction queryは整数tickと`[0, 1)`のfractionを受け取り、`(n, 1)`と`(n+1, 0)`を同一時刻として扱う。
- 保存sampleとfinalizationの整合は保存した組の厳密一致で検査する。
- 微小fractionを絶対tickへのf64加算で消失させず、隣接sampleの局所時間差から補間率を求める。

- 位置・速度・身体状態は線型補間、quaternionは最短経路slerp、角度はwrap-awareに補間する。
- actuatorはexact sample時刻で保存値を返し、内点では終端側sampleの保持値を返す。
- fractional terminalにも同じ規則を適用する。
- 範囲外queryは拒否し、physics・原recordを変更しない。

- 秒queryは有限な`0 <= seconds <= duration_seconds`を受理する。
- 終端秒と一致した場合は保存終端を優先し、tick/fractionを正規化する。
- 微小な正durationが秒では0へ丸められる場合もseconds=0で保存終端を返す。
- 内点は表示秒に変換した隣接sampleから選び、局所秒差の比で補間する。
- 同じ秒へ丸められたsampleを個別指定する場合はtick/fraction queryを使う。
- epsilonによる時刻移動やsample削除で整合を補わない。

保存入力による再積分は別の検証機能とし、利用するmodel・controller・physics・scenarioを照合する。数値・境界・容量・round-trip・adapterの検査は[検証](verification.md)、離散的な所有・保持の保証は[形式検証](formal-verification.md)に定める。

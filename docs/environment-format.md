# 環境asset契約

## 外部境界

`birdman-game-format::EnvironmentDocument` はschema version 1のJSONを受け取り、
固定環境の風・波・空・原点・出典を検証する。JSONは最大8 MiB、風格子は最大65,536標本とする。
各説明文字列はUTF-8で最大2,048 byte、出典・月統計・各source参照一覧はそれぞれ最大16件とする。
JSONの配列decoderは要素数上限を超えた要素をstorageへ追加する前に拒否する。
標本・metadataの構造上限は、JSON escapeを含むencoderの最大出力も8 MiB未満へ制限する。
データ取得・filesystem I/Oはoffline生成toolとCLI/Web adapterに置く。

`environment_version` は既存 `ScenarioCatalogEntry` のenvironment versionに対応する。
`validate_for` が一致を検査する。scenario ID、Weather分類、選択seedは既存catalogが所有する。
この形式の追加によって独立したscenario選択やゲーム状態を作らない。
WASM adapterのprepareとFlightRecord出力は同じ`ScenarioModelCatalog::resolve`を使用し、
catalog・scenario・aircraft・environmentの各versionとWeather分類を完全照合する。
scenario IDを配列indexへ算術変換する取得経路は使用しない。

## 座標・風場

`local_frame` はWGS84の緯度・経度と、固定水面の高さ基準を記録する。
local NEDのdown=0はscenarioの固定水面、負のdownは水面上の高度である。
水位・測地基準間の変換や近似は `water_level_datum` とprovenanceへ記す。

`wind_grid` はnorth/east/down順のorigin・正のspacing・各軸2点以上のcountを保持する。
風標本のindexは `(down * east_count + east) * north_count + north` である。
各標本は空気が向かうNED方向のm/sを表す。気象の風向は吹いてくる方位であるため、
生成処理で水平方向の符号を反転する。鉛直流もdown正として記録する。

`WindGridDocument::build` は有限座標、有限速度、標本数、表現可能な上端、代表地点の範囲を検査し、
構築時だけ確保するNED標本storageを返す。`EnvironmentWindGrid::as_field` がそのstorageを借用し、
既存coreの `WindField::grid` を構築する。fieldはstorageのlifetime内で使用する。
補間・閉区間境界・範囲外エラーは既存風場契約に従う。
実行scenarioはRK4途中の機体位置と全空力評価点を含むcoverageを、統合検証で確認する。

`representative_position_ned_m` はFlight Setup等で表示する代表風の地点である。
代表値の地点・水面上高度を明示し、単一の代表値を全格子の風と扱わない。

## 観測根拠と仮定

`sources` にsource URL、version、入力snapshotのSHA-256、利用条件、帰属・加工表示を記録する。
入力hashは小文字hexadecimal 64文字とする。codecはsource URLへ通信しない。
生成物自身のhashはasset manifestへ登録し、自己参照するhashをJSON内に作らない。

`provenance` はlocal_frame・wind_grid・waves・skyそれぞれに必須である。
型は `observed`、`derived`、`assumed`、`game_tuned` を排他的に表す。
observed/derivedは有効なsource index、assumed/game_tunedは根拠・適用範囲の説明を要求する。
格子のprovenanceには空間分布・鉛直流・高度依存・フライト中の定常性を含める。

`ground_wind_normals` は地上観測点の月平均風速・最多風向、統計期間、観測条件を保持する。
平均風速はscalar平均、最多風向はfrom-directionであり、この組合せは平均風速vectorを与えない。
これらの観測統計は格子風の標本から分離して保存する。湖上・飛行高度への外挿は追加の仮定として記す。

## 描画用環境

`waves` はrender-onlyの風履歴、effective fetch、detail scale、pattern seedを保持する。
風履歴を瞬時の物理風と区別する理由はprovenanceへ記す。既存湖面rendererと同じく、
風速60 m/s以下、fetchは正かつ50 km以下、detail scaleは正かつ3以下とする。
波面は物理の固定水面接触を変更しない。

`sky` は太陽の北基準時計回りazimuth、水平面上elevation、雲量、cloud base、visibilityを保持する。
sun方位は0以上360°未満、elevationは±90°以内、雲量は0～1とする。
cloud baseは水面上の非負高度、visibilityは有限正値である。条件はフライト中に固定する。
Three.js等への座標変換とshader/GPU更新はengine adapterが担当する。

## WASM metadata snapshot

`GameSessionBridge::environment_snapshot_json` はschema version 1のJSONを一括返す。
包絡は`schema_version`、`context`、`projection`である。session contextは`kind: session`と
同時点の`phase_code`を保持する。registry queryのcontextは`kind: registry`である。
queryはsession・record・physics timeを変更せず、独立したgeneration counterを保持しない。
Web adapterは既存のrequest IDと捕捉したsessionの一致を検査し、同じprojection messageへmetadataを含める。

projectionは`available`、`unavailable`、`no_selection`を排他的に表す。
available/unavailableは完全identityとsourceを保持し、unavailableにはmetadataを含めない。
identityはcatalog/scenario/aircraft/environment/controllerのversionとscenario ID、`seed_low`/`seed_high`である。
seedは符号なし32 bitの2 wordとして転送し、JavaScript numberへの64 bit整数変換を避ける。

| Scene / phase | sourceと参照元 |
|---|---|
| Title | no_selection |
| FlightSetup | selected。同じconfiguration resolverが選ぶpreview |
| Briefing〜Result | sealed。sessionの確定identity |
| Replay | record、またはarchive。現在再生中のrecord header |
| Attract | attract。独立demoのrecord header |

metadataは名前、hash、local frame、風のquery領域、代表点・高度・coreでsampleした風、
wave/sky inputs、月統計、provenanceと出典を含む。全格子標本は転送しない。
local frameとskyは`defined`/`unavailable`の直和型であり、legacyに架空の値を補完しない。
definedは環境入力の定義を示す。rendererへの適用状況はWeb/engine adapterが管理する。
hashはenv6の`asset_bytes`とlegacyの`source_fingerprint`を区別し、各SHA-256を保持する。
legacy hashは現在のbuildのsource fingerprintであり、過去recordの実asset hashを復元する値ではない。

`environment_snapshot_for_identity_json` はJSON文字列だけを受理するregistry queryである。
JSのnull・型不一致、4,096 byte超過、object以外の包絡、JSON/field/range異常、0のversionを分類して拒否する。
registryはcatalog v1の1〜5とcatalog v2の1/2/4/5/6を完全identityで照合する。
各組合せのaircraft model v1は旧archiveの環境metadata照会用に保持し、v2を現行playable機体として登録する。
環境metadataの照会は旧機体を再積分する操作を含まない。
既知registryの存在は通常選択の公開を意味しない。現行の選択catalogはv1の5scenarioで、aircraft model v2を使用する。
未知identityはunavailableを返し、archiveの受入・記録・snapshot replayを維持する。

## 現在の実装範囲

正式な外部形式・検証・core風場への変換と、`tools/environment-build`のoffline生成を実装している。
hash固定した地上月統計と追跡可能recipeから生成し、Rust codec検証後だけ出力を保存する。
version 6のJSONをrepositoryの配布manifestへ登録し、全格子標本と生成入力hashをCIで検証する。
WASM adapterはversion 6のJSONをbundleへ組み込み、session生成前に既存codecで検証する。
`OnceLock<Result<RuntimeEnvironment, EnvironmentFormatError>>`がimmutableなmetadataと風標本を所有し、
成功・失敗を一度だけ保持する。風場はこのstorageを借用し、coreにI/Oや所有用allocationを追加しない。
build時にasset bytesのSHA-256を計算する。runtime環境moduleはsource fingerprint入力にも含める。
raw asset hashのPersonal Best content keyへの接続は、実環境scenarioを公開する後続単位で行う。
versioned metadataとstrict registry queryを実装している。現行catalogの実環境への接続、
描画用metadataの消費と通常入力coverageの受入は後続単位で行う。
現行browserのsynthetic scenarioはこの追加だけでは変更されない。

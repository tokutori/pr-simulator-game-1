# 環境assetのoffline生成

## 観測snapshotの抽出

`jma_normals.py` はhash固定したUTF-8の気象庁月平年値HTMLから、指定観測点・月の
平均風速、最多風向、統計期間を抽出する。標準ライブラリだけを使用し、通信しない。
現行の風・日照詳細tableの3段header、span、列数、資料年数を検証する。
構造変更、選択列の欠測・品質注記、重複行、入力hash不一致はエラーとする。
無関係な雲量等の参考値注記は抽出結果へ影響しない。

```sh
python3 tools/environment-build/jma_normals.py snapshot.html \
  --sha256 <raw-byte-sha256> --station 彦根 --month 7
python3 -m unittest discover -s tools/environment-build -v
```

出力は地上観測の抽出結果である。scalar平均風速と最多風向を独立して保持する。
平均風速vector、湖上の空間風場、鉛直流を観測した根拠として使用しない。
観測高度・平均化時間はこのページから確定できないため、環境asset生成時に
`measurement_scope`へ明記する。平年期間は入力から取得し、recipeで期待期間を照合する。

入力取得元は[気象庁の彦根月平年値・風日照詳細](https://www.data.jma.go.jp/stats/etrn/view/nml_sfc_ym.php?block_no=47761&day=29&month=12&prec_no=60&view=a3s&year=)である。
配布時は[気象庁の利用条件](https://www.jma.go.jp/jma/kishou/info/coment.html)に従い、
出典と加工表示をasset metadataへ記録する。入力snapshotのhashは改行変換前のbyte列に対応する。

## recipeからの生成

```sh
cargo build -p birdman-game-cli --locked
python3 tools/environment-build/build_environment.py \
  tools/environment-build/typical-july.recipe.json snapshot.html output.json \
  --validator target/debug/birdman-game-cli
```

recipeはこのdirectoryで追跡し、観測値そのものを重複記載しない。指定した期間・月・snapshot hashを照合し、
N-fastのaffine格子、独立した波の風履歴、空の設定を生成する。
風成分は1e-9 m/sの小数精度へ丸め、負のzeroを正規化する。
出力に時刻・絶対path・repository HEADを含めず、原snapshot・recipe・generator・parserのexact-byte hashを保持する。
recipeの各環境成分はassumedまたはgame_tunedへ限定する。
生成後はRust `EnvironmentDocument::decode_json`で全成分を検証し、成功した場合だけatomic replaceで保存する。
入力やvalidatorの上書きを拒否する。WindFieldのquery・物理計算をPythonへ移さない。

Typical recipeは新environment version 6を予約する。既存catalogの1～5を再利用しない。
格子のdown上端には水面下10 mの評価余裕を設ける。±2 kmの水平coverageは明示した生成範囲であり、
通常入力・RK4全stageのcoverage受入はruntime接続時に確認する。
fetch 600 mとsky設定は観測された琵琶湖条件を主張しない。

この工程の追加だけではbrowserのscenarioを変更しない。
version 6のJSONはrepositoryの配布manifestへ登録済みである。現行Web bundleはまだこのJSONを含めない。
browser配布・既存catalog接続・PB identityへのasset hash反映・統合受入を後続で行い、BPG-008を完了扱いにしない。

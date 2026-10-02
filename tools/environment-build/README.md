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

次の工程で追跡可能なrecipeから風格子・波・空を生成し、Rust codecで検証してmanifestへ登録する。
この抽出toolの追加だけではbrowserのscenarioを変更せず、BPG-008を完了扱いにしない。

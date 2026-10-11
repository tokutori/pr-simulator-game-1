# 共有水面モデル

## 基準と責務

Issue #306の数値・挙動基準はWeb版の `51b8678100da1520773d2247db52e0d830c34124` である。
`birdman-game-session::lake_water` は環境から波スペクトル、quality別投影、周期RGBAを純粋に生成する。
機体の空力荷重、飛行状態、着水面、記録、clockを変更しない。
Three.js、Bevy、texture、sampler、GLSL/WGSL、反射passの所有権はadapterへ残す。

canonical spectrumは18成分とし、quality別に6/10/18成分を選択する。
Webのfetch、JONSWAP重み、方向分散、成分順、正規化、0.52のsteepness上限を保持する。
quality投影はWebのFloat32 upload、frequency bandのenergy補償、mesh spacing、解析的微分上界とchoppiness reserveを使用する。
幾何位相と方向jitterは既存の固定hashを使用し、`patternSeed` はdetail画像のみに作用する。

detailは512×512の周期画像である。R/Gは勾配、Bは勾配variance、Aは静水面を基準とする高さを格納する。
uint32 LCG、crestletとmound、解析的勾配、各加算時のFloat32丸めとbyte量子化を移植する。
本番nearは64 m・3150 wavelets、farは288 m・5400 waveletsと900個のbroader waveletsを使用する。
生成metadataはextent、方向、正規化したseed、wavelet条件、画像寸法、モデルversionを保持する。
生成時の上限は合計65535 wavelets、broader raster radiusは512 texelsとし、範囲外を型付きerrorで拒否する。
これらは生成コストと有限中間値の境界であり、本番Web条件を変更しない。

## 数値比較

固定oracle `crates/birdman-game-session/src/lake_water/web-reference.json` は変更前のWeb関数から生成済みである。
元ソースのSHA-256、spectrum、quality投影、RGBAの統計・代表pixel・fingerprintを保持する。
生成元のTS計算はconsumer移行時に撤去し、oracleを移植後の実装で再生成しない。
Rust testsは演算スケールに応じた数値差、独立した微分上界、量子化スケールのpixel・統計差を検査する。
RGBAのSHA-256は実行fingerprintであり、合否条件へ使用しない。
ECMAScriptの数学関数は近似実装を許容するため、OSやnative/WASM間の無条件bitwise一致を契約にしない。
Math.fround、Math.imul、非負値のMath.roundの対応は[ECMAScript公式仕様](https://tc39.es/ecma262/multipage/numbers-and-dates.html)へ照合する。

全byte比較は次の順で実行する。出力directoryは既存の無関係な成果物を上書きしないpathを指定する。

```sh
cargo test -p birdman-game-session --locked lake_water
cargo run -p birdman-game-session --example lake_wave_parity --locked -- target/lake-parity
npx tsx tools/wasm-build/main.ts
npx tsx tools/wave-model/check-native-parity.ts target/lake-parity
```

現在の全byte比較は共通モデルのnative/WASM出力を照合し、1 byte以内の量子化差、差分byte数とfingerprintを出力する。
純粋モデル導入時にはWindows上の専用build directoryで、sessionの21 tests、Clippy all-targets（`-D warnings`）、tools TypeScript検査、`cargo fmt --all --check` が成功した。
4条件の512×512 RGBAを変更前のWeb実装と全byte比較し、差分0を確認した。
比較元の3ファイルは基準commitから実装base `0f9fce9` まで変更がないことを確認した。
これらは純粋モデルと生成画像の検査結果である。GPU描画、native/WASMの実画面比較、他OSでの検査は未実施である。

## Web consumer境界

WASM adapterは `LakeWaveProjectionBridge` と `LakeDetailImageBridge` を公開する。
`wasm-bindgen` がRust宣言からTypeScriptの境界型を生成し、Float32 uploadとRGBAをowned typed arrayでまとめて返す。
Webは環境・quality変更時に呼び出し、受信後にbridgeを解放する。物理tickや通常描画frameから画像生成を呼ばない。
本番detailのextent、wavelet数、seed変換は共通Rustの `production_lake_detail_configs()` を使用する。
Three.js adapterにはDataTexture、sampler、GLSL、反射pass、mesh生成とGPU resourceの交換・解放を残す。
起動時はWASMの初期化をrenderer作成より先に完了させる。

## 残作業

Web consumerのbuild・回帰と実描画の検査、およびBevy consumerの移行が残る。
Issue #306全体の完了にはWeb移植前後の実描画検査も必要である。
Issue #311のWGSL、finite wave packet、micro-normal、天空・機体反射、shadow、Fresnel、glitter、白波、haze、LODは後続である。
共通スペクトルの完成をnativeの描画品質改善の完了へ読み替えない。

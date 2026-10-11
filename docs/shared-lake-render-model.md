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

Bevy consumerは同じ24-slot uploadと6/10/18成分を直接使用する。
近景meshはWeb基準の6 km非均一格子、96/144/224分割を採用し、頂点のUV1へgrid spacingを格納する。
WGSLは有限wave packet、位相warp、visibility勾配、horizontal Jacobianとheight勾配を移植し、共有choppiness reserveを使用する。
独自JONSWAP、seeded geometry phase、4成分選択、0.35 mの独自高さ上限は撤去した。

近景・遠景の512×512 RGBAは共通Rustから生成し、線形RGBA、periodic wrapping、anisotropy 8でBevy Imageへ投影する。
mipmapはBevy 0.19.1の公式 `generate_mips_for_phase` を使用し、自作downsamplingを追加しない。
正規downsample shaderの2 entry pointが準備済みであることを確認し、active画像のAssetIdとGPU TextureIdに対してcommand記録を管理する。
未準備の画像は再queueし、command記録後は同じ画像を恒常dispatchしない。環境変更による画像交換は再queueし、retired画像は追跡対象から除去する。
この状態はordered commandの記録を表し、GPU完了の同期保証を表さない。
detail画像はadapterが所有するimmutable assetである。環境交換は新しいAssetIdを使用し、quality変更は同じ画像を再利用する。
pending中の同一AssetId hot-reloadは対応範囲に含めない。公式bind group cacheは、queueから除外された画像を退役させる。
環境更新は既存resourceを全て確認した後に画像・material・bounds・sunを一括交換する。
quality変更はnear/farへ同一の6/10/18成分投影を原子的に適用し、skyと既存detail画像を保持する。physics、record、simulation timeを変更しない。
shader未読込み・import待機はpendingとして扱う。正規downsampleの確定pipeline失敗は対象画像・shader・entry point・元原因を既存NativeEnvironmentの診断へ伝達し、対象画像の再queueを停止する。
新環境で退役した画像のGPU診断を除去する。非同期GPU失敗はCPU交換失敗と分離し、以前の表示を保持したという保証を付与しない。

Bevy consumerはcompile・Clippy、Native 100試験・共有session 21試験が合格した。Windows 11/AMD Radeon 860M/VulkanでWGSLを起動し、開始・Pilot/Chase・pause・画質変更・Result・Retryのscripted GPU検査が合格した。
Web consumerも同じWindows環境のChromium/WebGL2で起動・Flight・pause・Result・Retryを確認した。物理キー・マウスの受入、同一条件の移植前後比較、release性能計測は未完了である。
Issue #306全体の完了にはWeb移植前後の実描画検査も必要である。
Issue #311の後続phaseでは、fine/rippleを含む4層micro-normal、subpixel filtering、天空・機体反射、shadow、Fresnel、glitter、crest/compressionの白波shading、hazeをWeb基準へ揃える。
phase 1のnear/far detailとcrest/compression入力だけで描画品質の移植完了を扱わない。
共通スペクトルの完成をnativeの描画品質改善の完了へ読み替えない。

# 共有水面モデル

## 基準と責務

Issue #306の数値・挙動基準はWeb版の `51b8678100da1520773d2247db52e0d830c34124` である。
`birdman-game-session::lake_water` は環境から波スペクトル、quality別投影、周期RGBAを純粋に生成する。
`birdman-game-session::lake_sky` は同じ固定Web基準の全天sRGB RGBAを生成し、WebとNativeで共用する。
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
4条件の512×512 RGBAを変更前のWeb実装と全byte比較し、差分0を確認した。
比較元の3ファイルは基準commitから実装base `0f9fce9` まで変更がないことを確認した。
全天画像は固定Web関数をNode.js 24で直接実行した独立oracleの2条件・14画素を使用し、RGBA各channelの許容差を1 byteとする。
これらは純粋モデルと生成画像の検査である。移植前後の実画面比較と他OSの検査は未完了である。

## Web consumer境界

WASM adapterは `LakeWaveProjectionBridge`、`LakeDetailImageBridge`、`LakeSkyImageBridge` を公開する。
`wasm-bindgen` がRust宣言からTypeScriptの境界型を生成し、Float32 uploadとRGBAをowned typed arrayでまとめて返す。
Webは環境・quality変更時に呼び出し、受信後にbridgeを解放する。物理tickや通常描画frameから画像生成を呼ばない。
本番detailのextent、wavelet数、seed変換は共通Rustの `production_lake_detail_configs()` を使用する。
Three.js adapterにはDataTexture、sampler、GLSL、反射pass、mesh生成とGPU resourceの交換・解放を残す。
起動時はWASMの初期化をrenderer作成より先に完了させる。
全天画像の入力は検証済みの太陽方位・仰角、雲量、visibilityの4項目である。
Nativeは実環境の `SkyStateDocument` から変換し、Webは同じ4項目を渡す。画像に使用しないcloud baseを補完しない。
sky欠損時は `neutral()` から方位非依存の背景を取得し、太陽・雲を付与しない。
Three.jsはowned RGBAからDataTextureを構築し、bridgeを `finally` で解放する。TSのLCG、cloud配置、pixel計算は撤去した。

## Native consumer境界

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

near/far/fine/rippleの4層micro-normalと4点subpixel filteringをWeb基準で実装する。fine/rippleはnear画像を再利用し、回転・縮尺をtexture勾配と法線勾配へ一貫して適用する。
未解像varianceからroughnessを導出し、facet Fresnel、sun glitter、crest/compressionの明暗・白波、wave height tint、距離hazeを描画する。
paletteはBevyの既存color APIでlinearへ変換する。daylightとhaze距離は確定したsky metadataから導出し、sky欠損時はdaylight 0とWebと同じ既定hazeを保持する。
cloud fractionを水面shadowとして代用しない。
全天画像は固定LCG、cumulusの形状・noise、horizon gradientとsun glowを保持する。
NativeはBevyの既存color変換でlinear `Rgba16Float` へ投影し、正式対応する公式mip生成を再利用する。S方向はRepeat、T方向はClampToEdge、filterはlinear/trilinearとし、背景と水面radianceは同じ全天画像を参照する。
環境交換で3枚のowned画像を原子的に交換し、quality変更は既存画像を保持する。

機体mirror passは反射専用cameraと透明linear `Rgba16Float` targetをBevy adapterが所有する。
反射対象はairframe meshと照明のlayer 1に限定し、UI、湖面、天空、地形を描画しない。通常cameraだけを既定UI cameraに指定する。
水面基準の鏡映行列、反転したculling、oblique near clip planeは[Bevy 0.19.1の公式mirror例](https://github.com/bevyengine/bevy/blob/v0.19.1/examples/3d/mirror.rs)のAPIを使用する。
固定Web基準にはoblique clipの指定がなく、Nativeのclip planeは水面下の反射geometryを除外するadapter側の処理である。
cameraの高さが0.05 mを超えた場合だけpassとsamplingを有効にする。
targetはviewportの半解像度を基準に共通縮尺で最大1024 texelsへ制限し、各軸を1 texel以上とする。projective matrixは丸め後の実target aspectを使用する。
固定Webの鏡映forward/upによるlookAtとNativeの鏡映行列ではright基底が逆向きとなる。textureのY原点も逆向きとなるため、Webのslope offset `(0.006, 0.006)` をNativeでは `(-0.006, -0.006)` へ変換する。
独立したWebのcross-product基底・OpenGL projectionとNativeの鏡映行列を使い、rollを含む姿勢で両UVとshaderのoffset係数の対応を検査する。
alphaと `clamp(0.16 + Fresnel * 0.7, 0, 0.72)` はWeb基準を保持する。
resizeは新しいtargetをcamera・near/far materialへ一括接続してから旧targetだけを退役させる。環境・quality交換は反射targetと投影を保持し、detail・全天画像の寿命と分離する。

DirectionalLightのshadowはdaylightに同期し、Bevyの既存2048 texel cascade shadow mapと30–600 mの3段cascadeを使用する。
湖面fragmentは変位後のworld positionとgeometric normalを[公式 `fetch_directional_shadow`](https://github.com/bevyengine/bevy/blob/v0.19.1/crates/bevy_pbr/src/render/shadows.wgsl)へ渡し、Webと同じglitter・diffuse・base attenuationへshadow visibilityを適用する。
湖面自身のshadow casterとprepassは無効のまま維持する。専用shadow mapやshadow samplingの別実装は追加しない。

## 検証境界と残作業

全天画像のWeb consumer統一後、Native 105試験・共有session 28試験・WASM 42試験、fmt・Clippyが合格した。Webの型検査・lint・1,663試験・登録asset 9件の検査・production buildも合格した。
Windows 11/AMD Radeon 860M/Vulkan・800×600のscripted GPU検査（13 capture）が合格し、実Title・Chase画像で積雲と水面radianceを確認した。
Webの波・全天画像consumerは同じWindows環境のChromium 148・AMD Radeon 860Mのhardware WebGL2で起動・Flight・pause/resume・Result・Retryを確認した。実画像で積雲と水面反射を確認し、browser errorとcontext lossは発生しなかった。
同一条件のWeb移植前後・Web/Nativeの実描画比較、物理キー・マウス受入、release 80 FPSの計測は未完了である。
機体mirror passと水面shadowの追加後、fmt・Clippy・Native 114試験と同環境のscripted GPU検査13 captureが合格した。独立readonly reviewで検出した反射distortionのX符号を是正し、実shader係数とWeb/Nativeの基底対応を回帰試験で確認した。
共通モデルの数値検査と実描画品質の受入を区別する。

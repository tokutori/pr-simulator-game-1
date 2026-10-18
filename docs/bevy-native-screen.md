# Bevy native Screen確認版

## 対象と比較基準

[#254](https://github.com/tokutori/pr-simulator-game-1/issues/254)の確認用実装を`bevy`ブランチで開発し、検証後にmainへ統合した。
比較基準とbaseは`c5408914015f2f5c2679c033efca41e6b6be8404`に固定する。
既存Web版は維持し、今回のnative対象はWindows 11のScreenに限定する。
2026-10-09の指示に基づくmain統合後も、ユーザー受入と追加修正の完了を区別する。
GitHub Pagesの公開は明示的な手動操作へ分離し、releaseは今回の対象外とする。

## 起動

workspaceのルートで実行する。

Windowsでの開発・確認用ディレクトリは`C:\projects\birdman\tokutori\pr-simulator-game`である。
このディレクトリの`bevy`または統合済みmainから起動でき、追加のworktreeへの移動を起動条件にしない。

```sh
cargo run -p birdman-game-bevy --release --locked
```

表示同期による上限と処理能力を区別する場合は、明示的に`--no-vsync`を指定する。

```sh
cargo run -p birdman-game-bevy --release --locked -- --no-vsync
```

通常WindowはFifoを維持し、指定時だけAutoNoVsyncを要求する。GPU/OSの対応状況に応じてImmediate→Mailbox→Fifoを選択するため、同期解除は保証されない。
tearingやGPU負荷・消費電力の増加が生じうる。物理tick・入力割当・画質・安全停止は同じ契約を使用する。

Rustは既存の1.97.0、Bevyは`=0.19.1`を使用する。BevyのMSRVは1.95.0である。
[公式setup](https://bevy.org/learn/quick-start/getting-started/setup/)に従うWindowsのnative build環境とGPU driverが必要である。
Node.js、npm、TypeScript、ブラウザ、WASM runtimeはnative版の起動条件に含めない。
初回やRust改修後のrelease buildは最適化・LTOに時間を要する。初回buildは26分17秒、継続修正の再buildは26分07秒で完了した。
build完了後の同じ起動コマンドでは、生成済みbinaryを再利用する。

## モデルと状態の正本

既定は現行二系統hybrid・Manual・Full・Typical、seed `0x00005f98000055aa`、最大4,000 tickである。
共有Rust準備と既存GameSession、力学、空力、制御、接触、score、recordを直接利用する。
Bevy Transformはtyped snapshotの描画投影であり、physics stateを所有しない。
固定tickはcoreの`PHYSICS_HZ`を参照し、描画frame・camera・ウィンドウ寸法から分離する。
Runningの描画はtick前後の機体pose、pilot位置、両尾翼incidence、simulation時刻を同じfractionで補間する。
初回は発進状態と最初のtickを使用し、prepared・Pause・Resultは確定sampleを表示する。非Running時に描画履歴を破棄し、Retryへ旧飛行の補間を持ち越さない。
終了理由が適用範囲外の場合も元のcauseと最後の有効状態を保持する。
旧三軸モデルへ切り替えない。既定モデルの改善は公開可能なsynthetic係数に限定し、実機同定と分離する。

発進方位は[#255](https://github.com/tokutori/pr-simulator-game-1/issues/255)の修正として北西315°へ統一する。
比較元にはplatform315°と初期heading・距離評価軸0°の不整合があった。
共有`assets/biwa-launch-venue.json`からRustの初期姿勢・course axis、Web/Bevyのplatform配置を導出する。
風はworld NEDを維持し、カメラまたは景観だけを回転する補正は行わない。

## 最初に確認する操作

起動→飛行を設定→操縦支援を選択→飛行準備へ進む→発進カウントダウンを開始→Flight→Result→同じ条件で再試行を確認する。
設定・確認・発進の進行表示を設け、設定選択、主要操作、戻る操作を区別する。
設定のプリセット、Information、Assistance、Weatherは共有Rust Viewの候補と順序を使用する。
確認画面では選択値と操作方法を表示し、versionとseedは閉じた技術情報に格納する。
カウントダウンを取り消すと確認画面へ戻り、確認画面から設定変更もできる。
既定のTypical環境を維持し、気象選択は共有Rustの設定解決と準備処理へ接続する。

湖面画質は設定画面および一時停止中にLow・Medium・Highから選択する。既定値はHighである。
Screenの3D描画倍率は同じ設定経路で100%・75%・50%から選択し、既定100%を維持する。
Autoを明示的に選択すると、共通Rustの規則がGPU処理時間を評価して倍率を一段ずつ調整する。手動選択でAutoを停止し、Auto解除時は保持した手動倍率へ戻る。
NativeはBevyの標準Recorderで反射・主画面・合成UIを含むRender命令区間を計測する。GPUへの準備・upload、後段のscreenshot/readback、present待ち、CPU全体の処理時間は含まない。
GPU timestampに非対応の場合は理由を表示して適用済み倍率を保持する。通常の手動起動ではRecorderを退避し、各passのqueryとreadbackを実行しない。Auto中および既存の水面検証時は標準pass診断の費用が加わる。
遅延した測定値は元のframe・計測世代・同一時計のsource時刻で検査する。resize、画質変更、中断、device再取得は旧測定窓を失効させる。
縮小時は3Dと反射の内部bufferだけを変更し、UI・入力座標・Window・画角を原寸に保持する。0寸法では既存資源を維持し、復帰時に再適用する。
倍率による画素数削減、表示同期の要求、12.5 msの処理予算を個別に評価する。手動倍率の追加を80 FPS達成として扱わない。
波スペクトル、6/10/18成分の投影、near/far微細波画像はWebと共通のRustモデルから生成する。
GPUへの投影、mesh、WGSLはBevy adapterが所有する。
機体と地形の照明は共有`lake_sky::LAKE_SCENE_LIGHTING`のWeb基準色・irradianceを使用する。
Nativeは既存18,000 luxをadapterの換算基準とし、`Exposure = 2^(-EV100)/1.2`からDirectの基準値へ対応付ける。
Ambientは等方光の`E = πL`と同じExposureから輝度へ換算し、main/反射cameraへ同一のExposureを設定する。
Webの既定と同じNoToneMappingを使用する。これらは描画規約であり、実測の気象光量を表す値ではない。
主翼CoverはWebの非光沢Lambertへ合わせてreflectanceを0とし、藍地を灰色方向へ持ち上げるspecular寄与を抑える。共有texture・sRGB・照明は維持する。
PBRとLambertの白の明度差、縮小投影による細部の視認制約は残る。同記録の近景GPU比較と検証境界は[#417](https://github.com/tokutori/pr-simulator-game-1/issues/417)へ記録した。
水面のfinite wave packet、解析的勾配、4層micro-normal、subpixel filtering、Fresnel、glitter、crest/compressionの色調・白波とhazeをWeb基準へ移植した。波と全天画像の生成は共有Rustへ集約し、Web/BevyはGPU投影を担当する。検証境界と未完了項目は[共有モデル](shared-lake-render-model.md#検証境界と残作業)へ記載する。
現在の適用済み画質と選択状態は単一のpresentation Resourceから導出する。交換中は選択を停止し、失敗時は適用済み画質と原因を表示する。
Retry・タイトルへ戻る操作・操縦支援の変更でも画質を保持する。画質変更はGameSession、Difficulty、飛行記録、simulation時刻を変更しない。

| 入力 | 操作 |
|---|---|
| ↑ / ↓ | 現行二系統モデルのpitch入力 |
| ← / → | 現行二系統モデルの旋回入力 |
| J / L | パイロット位置の目標値を前後に移動 |
| C | FlightのPilot / Chase切替。Replayでは共有cameraボタンを使用する |
| P / Esc | 飛行の一時停止・復帰 |
| 右mouse buttonを押してdrag | 視点操作 |
| F12 | カレントディレクトリへnative screenshotを保存 |
| Tab / Shift+Tab | 共通Viewの操作順にフォーカスを移動 |
| Enter / Space | フォーカスした有効な操作を実行 |
| Range上の矢印 / Home / End | 解析・Replayの共有時刻を変更 |

独立したroll入力は現行二系統モデルに追加しない。操作機器を解放した後のpilot目標保持も既存coreの契約を使用する。

Resultは終了理由、確定距離、短い原因説明を表示し、入れ子の内部診断は「技術情報」に格納する。
HUDの文言、表示値、表示条件と配置は共有Rust `GameUiHud`を使用する。Web基準のADI、FPI、方位目盛、風向針、AoA指示をretained ECSへ投影する。ADIのpitch係数・clamp、FPI位置、方位ラベル、AoA位置は共有Rustのgeometry descriptorで導出し、物理値を再計算しない。
Customで姿勢cueを非表示にしても、飛行経路cueを選択した場合は独立したFPI領域を表示する。Replay入場時は共有UIで選択されたcameraを毎回適用し、Flight・Resultの視点切替と分離する。
一時停止中は停止理由と再開方法を優先し、未回復の再開ボタンを非活性で表示する。
メニューの幅、最大scroll高、候補列数、CTAの順序と最低高は共有Viewのlogical pixelを使用する。
600px以下のresponsive分岐も共通Rustで解決する。ウィンドウのlogical sizeとBevy `UiScale`から同じviewportを渡す。
画面外の操作は縦scrollとkeyboard focusで到達できる。scene変更時にscrollを初期化し、同じsceneの更新は既存entity・focus・scrollを保持する。
GPU撮影時は表示中のbuttonのglyphと割当領域を検査し、画面外の操作についてscroll範囲内の到達可能性を検査する。
Resultでは選択した操縦支援、記録由来のcontroller version、確定した飛行時刻も確認できる。
通常の概要と元のtyped causeを保持した技術情報を分離し、UIの折畳み操作から物理状態を変更しない。

尾翼の合成角制限に対しては[共通Rustの入力保護](flight-control.md#局所迎角差と尾翼角の合成範囲保護)を適用する。
既存body-rate FBWとこの保護は異なる責務である。現在の保護は完全な迎角・失速・姿勢保護を提供しない。

## Assetとフォント

登録済みの湖岸・地形・会場・環境JSONを再利用する。機体は現行表示の寸法に基づくprocedural meshで表示する。
出典・加工・利用条件は`assets/manifest.toml`を正本とする。native起動前にTypeScriptでassetを生成する手順は不要である。
日本語表示はWindowsにインストールされたMeiryoをローカルで読み取る。フォントファイルをrepositoryやbinaryへ同梱しない。
日本語の単語分割にはICU辞書を使用する。Bevy 0.19.1が利用するParley 0.9.0へ公式の`complex-scripts`選択処理を限定backportし、
Windows native依存だけで有効化する。出典と変更範囲は`vendor/parley/BACKPORT.md`に記録する。
ICU4X 2.3の`BidiClass::to_icu4c_value`に関する非推奨警告は、公式のconst mask処理を限定backportして解消した。
出典と23分類のbit対応・未知分類の処理は[backport記録](../vendor/parley/BACKPORT.md)を参照する。
警告の抑制や依存versionの後退は行わない。
利用と再配布の境界は[Microsoftのfont FAQ](https://learn.microsoft.com/en-us/typography/fonts/font-faq)に従う。
未導入環境では起動時に明示的なエラーを返し、利用許諾を持つフォントを`--font`で指定する。

## Three.js版との差分

native ScreenのPilot/Chase切替はpresentation機能として提供し、core configuration・physics・recordを変更しない。
水面・空・機体・湖岸/地形はBevy adapterで表示する。水面の波・反射は描画専用であり、接触判定は既存coreを使用する。
水面shaderの完全一致、全Scene装飾、複雑なAnalysis、保存一覧UI、全Replay/gamepad、VRと別OSの対応は今回の対象外である。
共有UI接続では記録由来の解析系列、seek、再生停止・再開、再生速度を接続する。
解析の軸名・単位、目盛、grid、基準線、legend、地図のscaleと北方位、plotの配置は共有`GameUiChart.presentation`から投影する。chart viewportの実測寸法を共有UIへ渡し、静的要素はview revisionの変更時だけ再投影する。seek中は同じcursor entityと共有系列色を使用する。低高さでは共有overflow契約に従ってchart内部をスクロールし、seek操作領域を保持する。文字のbaselineはBevyが使用するフォントの実測metricsから変換する。今回のchart投影に対する実GPU検査は未実施である。
Replay cameraは共通候補順序のAuto・Pilot・Chase・Orbit・Platform・Shore・Overhead・Side・Front・Telephotoを提供する。既存Webを移植基準とする共有RustのgeometryとDirectorを使用し、Attractも同じrecord時刻からshotを選択する。固定点は登録環境のoriginとwater datumが一致する場合に使用する。退出時は通常Screenの60°FOVへ復帰する。本追加の実GPU描画は未確認である。
記録一覧・保存・downloadはnativeで未提供であり、対応操作は共有capabilityから無効化する。
水面は共有wave spectrum、finite packetと解析的勾配、near/far/fine/rippleの4層micro-normalを使用する。画素footprintによるfilteringと二次momentから未解像varianceを保持し、Fresnel・sun glitterのroughnessへ反映する。
明暗と白波はWebと同じcrest/compressionから導出する。局所風速を白波の有無へ直結させず、既存の波群を保持する。色調patchのnoiseはWebと同じ`[0,1]`へ正規化する。
near/farと全画質は同じ環境snapshot、world-space位置、固定phase、simulation timeを使用する。水面は不透明であり、描画qualityはphysics・record・着水面を変更しない。
空と天空反射は共有Rustが生成したWeb基準の全天textureを共用する。生成入力は実環境の太陽方位・仰角、雲量、visibilityの4項目とし、cloud baseを補完しない。NativeはsRGB画像をBevyの既存APIでlinearへ変換し、公式mip生成と同じequirectangular samplingを使用する。機体mirror passは水面で鏡映したcameraからairframeだけを透明textureへ描画し、Webと同じprojective UV・slope offset・alpha/Fresnel合成を使用する。DirectionalLightの公式cascade shadowを水面へ接続する。追加後のfmt・Clippy・Native 114試験とWindows 11/Vulkanのscripted GPU検査13 captureが合格した。固定条件の描画品質・時間的aliasing・optic flow・80 FPS達成は実GPUで別途評価する。
Bevy 0.19.1のカラーmipmapは、初回Extract前に既存の`Rgba8Unorm`・`Rgba16Float` shaderのsampling位置を2×2平均の中心へ補正する。公式のpipeline・後続平均・同じshader handleを保持し、depthと別formatは変更しない。既知WGSL表現に適合しない依存更新は、原因を示して起動を停止する。
空・水面反射・PBRのDirectionalLightは登録環境の太陽方位・仰角を共用する。
NEDから描画座標へ変換した太陽への方向を水面・空へ渡し、DirectionalLightのforwardをその負方向へ向ける。
方向の符号、基準方位・仰角、実際のworld初期化経路の一致をnative回帰試験で検査する。
Titleと未準備のSetupは共有catalogの既定Typicalを展示する。準備後はGameSessionが確定した全identityを共有登録sourceと照合し、同じ環境の近景・遠景水面、空、DirectionalLight、変位Aabbを一括適用する。未知identityや資源欠損はtyped causeを保持し、交換前の表示を維持する。通知概要と技術情報から原因を確認できる。通常Updateは同じ失敗を反復せず、別環境への遷移または明示的な別画質への変更で再適用する。
Calm等のsynthetic preset環境には共有の登録済み波浪入力を使用する。物理風から波浪入力を再生成しない。sky metadataが欠ける場合は共通の中立背景を使用し、太陽・雲と直達照明を無効化する。現在の環境6の太陽条件を補完しない。画質変更とRetryは確定した環境を維持し、近景mesh交換でcamera・波位相のsimulation timeを変更しない。複数Weatherの時間的optic flow検査は別作業である。
near meshは全画質で6 km四方の非一様gridを使用し、cameraの水平位置を1 m格子へsnapして追従する。far ringは18万m四方を覆い、中央のnear領域を除外する。波の位相はworld-space、時刻は共有simulation timeで評価する。finite packetと解析的勾配、共有投影の非反転reserveを使用し、描画専用の水平・鉛直変位を与える。実測波浪の再現性や波浪予報精度は主張しない。

Low・Medium・Highは共有18成分のspectrumから6・10・18成分を選択し、near/farへ同じ投影を原子的に適用する。実meshの間隔に対する波長filterと非反転reserveを使用し、選択した環境の全quality共通Aabbを保持する。detail画像、4層micro-normal、環境時刻と固定phase、sky資源を再利用する。描画負荷と視覚品質の差、時間的aliasing、全画質のoptic flow、80 FPS達成は実GPUで別途確認する。
vertex shaderでworld位置を変位させ、変位量を含むAabbを使用する。sky domeは変位から除外する。水面のshadow caster・prepassは無効とし、平面のdefault prepassとの不一致を回避する。shadow receiverは変位後のworld positionとgeometric normalを公式samplingへ渡す。reflection targetはUI・水面・空・地形を含めず、resizeで旧targetだけを退役させる。環境・quality交換は同じ反射passを保持する。geometry・micro-normal・反射pass・cascade shadowの負荷を含む80 FPS目標の達成は実GPUでの計測を必要とする。
登録broad/fine地形はWebと同じ非shadow casterとし、現在のshadow receiverを保持する。不透明な機体被覆・骨格とplatformのcaster、既存cascade・水面samplingは維持する。
地形のvisibility hazeは共有near/farとsRGB色を使用する。公式PBR照明の後にWeb Screen基準のsmoothstep(view-depth)を適用し、BevyのsRGB出力用にlinearへ戻す。水面のradial hazeとreflection対象は保持する。環境交換では実terrain materialを事前検査し、水面・cloud・太陽と同じcommit境界で更新する。
遠方波は[#257](https://github.com/tokutori/pr-simulator-game-1/issues/257)の修正として、
画素footprintに応じて解像できない法線・色変調成分を減衰させる。近距離の波と既存反射式を保持する。
主翼とcockpitの形状はWeb基準の共有Rustを使用する。cockpitは白いframe、透明nose canopy、後部fairingを表示する。主翼上面の藍染textureも共有Rustで生成し、登録decalのalphaを左右へ投影する。Nativeではlinear-light mipを生成してsRGB textureへ格納する。T字骨格を表示し、全160本の主翼rod・44個のfoamが同じWingFrameの曲げへ追従する。rodのunit meshと左右同形foamのmeshを共有する。機体fidelityは未検証である。
両尾翼は登録physical geometryの外周・quarter-chord原点を共有Rustで表示へ投影し、前半の藍色被覆、後半の半透明film、薄茶ribと水色foamを両版で共用する。被覆比率・palette・rib厚さは既存主翼の表示規則であり、rib配置とfoam帯は視覚専用である。static geometryは起動時一回だけ生成し、尾翼全体は従来のphysical incidence親Transformへ従う。空力・physical寸法・操縦・100Hzを変更しない。実描画受入と80FPSは関連CPU検査から推定しない。
HUDの内容・幾何・表示条件は共有Rustを使用する。[#258](https://github.com/tokutori/pr-simulator-game-1/issues/258)の修正として、
数値と単位を不可分にし、角速度・姿勢の共有単位を見出しへ分離する。ADI、FPI、方位目盛、風向針、AoA指示はWeb版と同じ共有値・表示条件を使用する。実GPUでの視認性と各機器の操作性は検証結果を参照する。

## GPU接続の検査

OSウィンドウを生成せず、CUIから本番scene・共有UI・水面shaderを描画する場合は、次を実行する。

```sh
cargo run -p birdman-game-bevy --locked -- --verify target/bevy-offscreen --offscreen --verify-size 1280x720
```

`--offscreen`は`--verify`との併用を必須とする。WinitPluginを登録せず、ScheduleRunnerPluginで実行する。
既定runnerの最小周期は16.67 msである。`--offscreen --no-vsync`を明示すると、公式runnerの待機なしLoopを使用する。
起動logの`Loop { wait: Some(...) }`/`Loop { wait: None }`でpaced/unpacedを識別できる。Image targetのGPU描画・readbackは同じ経路を使用する。
描画接続の初回検査は既定dev profileで実行できる。性能計測時はreleaseを使用する。
オフスクリーンの検証フローは180秒、通常Windowの検証フローは従来の90秒で打ち切る。実ゲームのProcessingDelay条件は変更しない。
Window componentはviewport寸法と検査用focusの論理metadataとして保持し、OSのWindow handleを生成しない。
本番のworld、aircraft、camera、UI、Material、shaderと既存の検証手順をそのまま使用し、最終出力と縮小描画の合成先をImageへ接続する。
PNGの保存完了、float probe、shader/pipeline readiness、開始・Pilot/Chase・一時停止・Result・Retryを検査して終了する。
出力先の画像を確認し、実GPU/driverでの描画結果と物理入力・OS focus・通常表示の性能を分けて評価する。
オフスクリーンの描画成功はVR、物理keyboard/mouse操作、通常Windowでの80 FPS達成を保証しない。

実際に自然着水した同じ記録を全Replay cameraから観察する場合は、任意の検査を追加する。

```sh
cargo run -p birdman-game-bevy --locked -- --verify target/bevy-replay-cameras --offscreen --verify-replay-cameras
```

既存のfull nose-up logical input試験によるWaterContact記録を使用し、共有UIのReplay・cursor・camera切替を通して、
記録の中間時刻を停止したまま共有の全10候補を撮影する。`06-replay-*.png`へ保存し、選択名・解決したshot・記録時刻・
本番cameraのTransform/FOV・scoreを標準出力へ記録する。全sample・header・finalizationと、Resultへ戻った終端状態の保持を検査する。
通常の起動と既定`--verify`の経路は変更しない。`--verify-water`との同時指定は拒否する。
この検査は一つの記録・中間時刻・viewportの描画接続を確認する。全軌跡・全環境・VRの受入は個別に扱う。

`--verify`は本番の`WaterMaterial`と同じ19 bindings・shader asset・import・vertexを使用し、平坦・Low・Medium・High・Farの固定入力を16×16の`Rgba32Float`へ描画してreadbackする。変位は本番vertexが渡したworld positionと平面の基準UVの差、法線は本番fragmentの導出結果を検査専用ShaderDefで取り出す。色は検査分岐を使わず元のfragment最終出力を使用する。通常起動には検査Material・camera・readbackを登録せず、通常shader variantには検査用の処理・bindingを追加しない。

有限値、単位法線と上向き、共有投影から既に導出した変位上界、平坦入力の零変位、波の空間変動、昼間の黒一色・異常輝度を検査する。RGB8変換より前のfloat値を使用し、GPUの実行成功と色変換の成功を区別する。既存の資源・mip・quality交換・cleanup回帰は保持する。

通常の`--verify`のTitle captureでは、既存の展示camera `(15,14,22) → (0,8,0)`、既定Typical、横長640×480以上で、左下の水面領域（横3–25%、縦86–96%）を検査する。menu・機体・地平線を含む他領域と他Sceneへ同じ基準を適用しない。平均RGB値と変動幅による重大な黒色化・飽和・一様面の検出であり、画質や美観の合格判定を意味しない。対象外の寸法ではこのROI検査をskipし、float probeを必須のまま維持する。

```sh
cargo run -p birdman-game-bevy --locked -- --verify target/bevy-gpu-verification
```

小さいwindowまたは1080pで検査する場合は`--verify-size`を併用する。

```sh
cargo run -p birdman-game-bevy --locked -- --verify target/bevy-gpu-verification-small --verify-size 800x600
cargo run -p birdman-game-bevy --locked -- --verify target/bevy-gpu-verification-1080p --verify-size 1920x1080
```

寸法は正の整数で指定し、`--verify`との併用を必須とする。通常起動は1280×720を維持する。

`--verify`は一時停止中の同じ飛行state・camera・simulation時刻でLow→Medium→Highへ交換し、
`03b-paused-quality-low.png`、`03c-paused-quality-medium.png`、`03d-paused-quality-high.png`を追加保存する。
新near meshがGPU資源へ反映され、既存shader/pipeline readinessが成立した後に撮影する。
この経路は画質交換の起動検査であり、時間的optic flow、複数Weather、性能目標の全面的な受入とは分離する。
同じ停止状態の50%解像度から、共有Auto controllerへ20 msの明示的な人工標本を実`Time<Real>`・`FrameCount`とともに渡し、実際のshadow mapを1024→512へ交換して各画像を保存する。その後、通常のManual 100%操作で2048へ復帰する。各段階では実shadow資源と共有ACK、描画準備、停止中の記録・state・水面時刻の保持を確認する。人工標本は専用段階に限定し、復帰時の既存epoch失効を経て、後続の実GPU Auto窓と分離する。この検査は描画接続fixtureであり、実GPU負荷・Auto判断・80 FPS達成の根拠には含めない。
OS表示倍率は変更しない。指定寸法とPNGの物理pixel寸法はOS倍率により異なりうるため、保存ログの実寸も確認する。

このmodeは選択したローカルフォントで日本語の単語境界と日本語/Latin混在の折返しを検査し、
通常のsession操作とlogical inputを使って開始・飛行・Pilot/Chase・一時停止/再開・終了・Result・Retryを確認する。
Title/設定/確認/Countdown/Pilot/Chase/一時停止/Resultの画像を書き込み、保存完了後に成功を返す。
Resultから共有control IDの実操作経路を使ってMap→Altitude→Speed→Replay→Replay Analysis→Resultへ進み、各画面も撮影する。Speedでは記録中点へseekし、AnalysisとReplay退出が確定結果と記録を変更しないことを検査する。
再試行では全nose-up入力による実フライトの終端と技術情報も撮影し、元cause・score・recordの一致を確認する。
WaterContact/TimeLimitの正常終了も受理する。診断表示のために通常入力の失敗を必須としない。
途中のwindow終了とtimeoutは失敗になる。
水面/PBR/terrain haze shaderのロードと実pipelineのcompile完了を5連続frame確認してから撮影する。
撮影frameのUI layout後に、ボタンlabelのglyph・寸法・包含、viewport内の配置、本文と下部操作の非重複を検査する。
shader/assetの確定エラーは原因を保持して返す。timeoutはGPU初期化後の最初のUpdateから90秒とする。
検査中はnativeウィンドウをアクティブに維持する。通常のfocus・処理遅延による停止契約も適用される。
撮影処理によるProcessingDelayだけの停止は、正常なfocused frameと描画準備の回復後に検査操作として明示的に再開する。
通常モードの安全停止・再開条件は変更しない。
scripted logical inputの検査は、実キーボード・マウスによる操作の受入と区別する。

### 湖面の6条件GPU観測

検証専用の`--verify-water`は、Calm/TypicalとLow/Medium/Highの6条件から1条件を指定する。
各条件は独立したprocessと保存先で実行する。通常起動と通常の`--verify`は既存の経路を維持する。

```sh
cargo run -p birdman-game-bevy --release --locked -- --verify target/bevy-water-calm-low --verify-water calm-low
```

他の指定値は`calm-medium`、`calm-high`、`typical-low`、`typical-medium`、`typical-high`である。
shared preparationで実GameSessionのWeatherを確定し、既存の画質要求と環境投影を通す。
Calmも登録済み波浪を使用する。気象値・波浪・skyのuniformだけを検証用の値へ置換しない。
実飛行の初期Pilot poseを観測基点とし、静止、前進20 m（10 m/s）、横移動6 m（3 m/s）、pitch 6°、roll 10°の
各区間の開始・終了画像を保存する。移動・姿勢変更は描画cameraだけに適用し、各区間は2秒である。
開始画像のreadbackと保存が完了し、正常なRunning frameへ到達してから区間を開始する。
区間中は撮影要求を発行せず、正常Running frameの実deltaだけを進捗へ加算する。
区間内のframe数、最大delta、Pause回数を集計する。focus喪失、ProcessingDelay、過大delta、
早期終了を含む中断区間はエラーとし、回復後のResumeによって連続観測の成功へ読み替えない。
静止cameraでも波位相は共通RenderSampleのsimulation timeで進む。近景・遠景・skyのcameraと時刻を照合する。
撮影ログのtickと時刻は要求時点の観測値であり、非同期画像の露光時点を保証する値ではない。

observerはPause・Result・Retryで無効化する。Pause中のcore state・record・波時刻を保持し、
異なる画質への交換と指定画質への復帰後、Resumeによるtick・record・波時刻の進行と同条件Retryを検査する。
各撮影は既存のGPU資源・pipeline readinessを待ち、失敗・timeout・遅延callbackを成功扱いしない。
画像変換・RGB変換・PNG圧縮と保存は既存`AsyncComputeTaskPool`で実行する。
保持するsave taskは1件とし、case・stage・serialを照合した保存完了後だけ次段階へ進む。
区間外のProcessingDelayは既存の安全停止と明示的な回復経路を維持する。GPU readbackの負荷は別途計測する。
水面検証processだけに公式`FrameTimeDiagnosticsPlugin`と`RenderDiagnosticsPlugin`を追加する。
この明示検証ではManual 100%でも既存`screen_work` root spanを記録し、Auto判定や描画倍率を変更しない。
`--verify-timing root|full`は`--verify-water`との併用を必須とし、省略時は従来の`full`を維持する。
`root`はManualにも通常Autoと同じroot-only計測を許可し、RenderGraph本体では既存recorderを退避して全pass診断を停止する。
`full`はrootに加えて従来の全pass診断を収集する。通常起動のManualは計測なし、Autoはroot-onlyを維持する。
Motion logへ診断方式・選択・実適用scale・shadow map寸法を出力する。通常Manualへ診断費用を追加せず、明示検証のquery自体の費用は残る。
初期observerは既存Chase cameraの位置・姿勢を発進時に一度取得し、Static区間ではworld固定する。
各motionのEnd画像保存後、次motionは既存Abort・Retry・Countdownにより同じsealed blueprintの個別flightから開始する。
連続2秒区間内の自然終端・ProcessingDelayは失敗として保持し、coreがResultの場合は実終了理由とfailureをlogへ記録する。
機体と通常Pilot cameraは従来の追従を維持する。撮影は指定observerと実Flight cameraのTransform一致後に要求し、実poseを観測logへ記録する。
通常Manualではroot spanを実行しない。root GPU時間はRenderGraphの描画命令区間であり、準備・upload、後段のscreenshot/readback、present待機を含まない。
`frame_time`と各`elapsed_cpu`/`elapsed_gpu`の既存履歴を4096件へ拡張し、Motion区間終了時、撮影要求前に
既存stage開始時刻以後に受領した標本のcount・時間幅・mean・nearest-rank p95・maxをmsで出力する。
履歴が満杯で最古標本が開始境界以後にある場合、容量不足として分布不完全を明記する。非finite・負値を除いた
補助統計と未取得も区分し、中断区間は既存の失敗を保持する。`frame_time`はVSync・OS待機を含むReal frame周期である。
render CPU/GPUの時刻はMainへの診断受領時刻であり、同Motion内に実行された全render frameとの対応を保証しない。
各passは合算しない。timestamp featureの強制要求、通常のFifo/VSync・画質・physics・150 ms安全停止の変更は行わない。
画像の時間的aliasing・optic flow・白波OFF/ONは、native表示の直接観測と実GPU画像で評価する。
開始・終了画像だけを連続表示の受入根拠にしない。
このcamera-only観測とscene/pass計測は、物理入力応答、描画FPS非依存性、80 FPSやrelease予算の達成を証明しない。
既定`--offscreen`ではrunnerの最小周期が16.67 msであり、80 FPS throughputの測定には明示的な待機解除が必要になる。

```sh
cargo run -p birdman-game-bevy --release --locked -- --verify target/bevy-water-unpaced --offscreen --no-vsync --verify-size 1280x720 --verify-water typical-high
```

同じ5 motionをroot-only診断で観測する場合は、上記へ`--verify-timing root`を追加する。
この選択は診断方式だけを変更する。5 motion・水面画質・固定tick・150 ms安全条件は同一である。
飛行中の実Autoを観測する場合は`--verify-water-auto`を追加する。省略時は従来のManual100を維持する。
Setupの公開Auto操作を通し、最初の有効窓と資源ACKまで既存Setup内で最大10秒待つ。未対応・失敗・未達は原因を保持する。
この準備確認は最適画質への収束を意味しない。飛行中も同じ共有policyが実GPU標本を受け、倍率とshadowを適応させる。
到着時刻基準の従来集計とは別に、root GPUの同一完了batchのepoch・source-frame・実適用scale/shadowで分布を分ける。
source区間はStart画像保存frameを除き、2秒motionの完了frameまでを含む。整数msは同じReal時計から取得し、frame包含の判定に小数時刻を用いない。
Bevy 0.19.1は`Last`でFrameCountを増加させてからExtractを実行するため、Update境界を同じ描画元frameへ変換する。区間中のframe wrapは未対応として明示する。
既存End画像保存完了を受信cutoffとする。GPU query全件のdrainは保証せず、欠損・重複・metadata不一致・履歴不足とpair別の部分区間を明示する。
履歴は既存DiagnosticsStoreの4096件をmetadataにも適用し、別collectorを保持しない。GPU非対応や不足標本を0msへ置換しない。
通常Windowの80 FPS受入、Release性能、CPU時間とGPU時間の同frame合算は別の確認対象である。

この組合せはrunnerの人工的な待機だけを解除する。100 Hz物理、Real時刻、入力割当、150 ms安全停止、画質は維持する。
Real Update周期と有効root GPU標本数・時間分布を分けて確認し、offscreen throughputと通常Windowの80 FPS表示受入を区別する。
WindowのPresentModeはこのmodeの論理metadataである。画像targetには実際のpresent待機がなく、Real周期にはCPU・RenderApp同期・GPU backpressure等の影響が残る。
6条件の実行・画像評価結果は実施後に記録する。

## 検証状態

Windows 11 / AMD Radeon 860MのVulkanでnativeウィンドウの起動を確認した。
ユーザーの実機試験でも描画を確認したが、日本語分割モデル欠損のログと表示の問題が報告されている。
辞書constructorへの修正と、LUTを要求しない`Tonemapping::Reinhard`の明示を適用した。
修正後のGPU検査は日本語分割・shader/assetの確定エラーなしで成功した。
水面/PBR pipelineの準備を確認後、Title・Pilot・Chase・Resultの4枚を保存し、画像上で水面・地形・機体を確認した。
scripted logical inputで開始・飛行入力・視点切替・一時停止/再開・ManualAbort・Result・同条件Retryを検査した。
撮影負荷によるProcessingDelayを2回検出し、安全停止の回復後に検査操作として再開した。
画像は`target/bevy-gpu-verification-ready/`に保存する。実キーボード・マウス操作の受入はユーザー確認待ちである。
同じWindows 11 / AMD Radeon 860MのVulkanでrelease buildと次の実GPU検査も成功した。

```sh
cargo run -p birdman-game-bevy --release --locked -- --verify target/bevy-gpu-verification-release
```

release版のTitle・Pilot・Chase・Result画像は`target/bevy-gpu-verification-release/`に保存する。
release版でも日本語分割・shader・assetの確定エラーは0件であり、開始・飛行入力・視点切替・一時停止/再開・
ManualAbort・Result・同条件Retryを検査した。rootと独立したreadonly reviewerが4画像を確認した。
初回release画像には遠方水面のmoireとHUD単位の折返しがあった。継続修正の結果は次項に記録する。
この検査はscripted logical inputによるものであり、実キーボード・マウスの受入と時間的aliasingの評価は未実施である。
実装HEAD `c75b2cca`はnative check/Clippy・17件のnative test、共有Rust層の関連test、Webの1,416件・
WASM/Vite production buildに合格した。[CI run 37745038206](https://github.com/tokutori/pr-simulator-game-1/actions/runs/37745038206)も
Ubuntu・Windowsとも成功した。Pagesはskipである。このCIは初回実装HEADの検査であり、継続修正の検査と区別する。

### 水面・HUD・入力境界の継続修正

2026-10-08、実装HEAD `947dff5`でformat、native check/Clippy、25件のnative test、repository checkが成功した。
[#259](https://github.com/tokutori/pr-simulator-game-1/issues/259)では入力adapterを`InputSystems`後に登録する。
実`InputPlugin`とraw keyboard/mouse messageを通す試験で、当frameの押下・解放、Cの一回切替、右dragの反映とmotion解除を確認した。
これはOSからの物理入力の受入と区別する。GameSession、physics、score、recordの変更は0件である。

Windows 11 / AMD Radeon 860M / Vulkanのdebug版で、1280×720、800×600、1920×1080のGPU検査がexit 0で完了した。
各寸法でTitle/Pilot/Chase/Resultを保存し、開始・入力・視点切替・一時停止/復帰・ManualAbort・Result・Retryを検査した。
画像の実寸は指定寸法と一致した。rootと独立したreviewerが1280×720と800×600の全画像を確認し、
単位分断・文字欠落・中央前方とCTAの重複がないこと、機体・地形・水面が保持されることを確認した。
1920×1080のPilot/Chaseもrootが確認した。800×600では尾翼行と上部説明がsoft wrapするが、値と操作は到達可能である。
保存先は`target/bevy-visual-fixes-debug/`、`target/bevy-visual-fixes-800x600/`、`target/bevy-visual-fixes-1080p/`である。

水面だけの先行比較では、既存release binaryが読み込む新shaderで同じtick 25の720p画像を比較し、遠方の強いmoire低減と近景波保持を確認した。
異なるviewportの撮影tickは一致しないため、飛行結果やshaderの定量比較には使用しない。
ProcessingDelayは通常の安全停止を保持し、回復後に検査操作として明示再開した。shader・assetの確定エラーは0件である。
最新HUD・入力修正を含むrelease版も、同じWindows 11 / AMD Radeon 860M / Vulkanで次の検査に合格した。

```sh
cargo run -p birdman-game-bevy --release --locked -- --verify target/bevy-visual-fixes-release
cargo run -p birdman-game-bevy --release --locked -- --verify target/bevy-visual-fixes-release-small --verify-size 800x600
cargo run -p birdman-game-bevy --release --locked -- --verify target/bevy-visual-fixes-release-1080p --verify-size 1920x1080
```

1280×720・800×600・1920×1080の各検査はexit 0で完了し、Title/Pilot/Chase/Resultを保存した。
rootと独立したreviewerが最新版releaseの1280×720全画像を確認し、単位・数値・前方視界・CTA、機体・地形・水面の保持を確認した。
800×600のPilot/Resultもrootが確認した。各Pilotはtick 25である。異なるviewportのshader・終端scoreの定量一致は評価しない。
shader・asset・日本語分割の確定エラーは0件であり、旧releaseの結果を新実装の合格へ流用していない。
再build完了後の指定cargo runは既存binaryを再利用し、Cargoの起動確認は1.56秒で完了した。

継続修正のhead `7c469bc3`の[CI run 37764364175](https://github.com/tokutori/pr-simulator-game-1/actions/runs/37764364175)も、
Ubuntu・Windowsの全検査stepが成功した。Pagesの設定・artifact upload・deployはskipである。
実キーボード・マウス、Windows表示倍率、時間的aliasing、他GPUは未検証である。ユーザーの実操作受入とsoftware検査を区別する。

### 設定・結果・入力保護の継続修正

2026-10-09、設定・確認・発進の画面役割、操縦支援の直接選択、主要操作と戻る操作を整理した。
Resultの概要と技術情報を分離し、一時停止の理由と再開可否を同じcore条件から表示する。
実画像で検出した操作labelの0幅collapseを修正し、本文とnavigationの共通column、明示幅button、折返しを検査する。
native 45試験、共有Rust関連試験、Web 1,416試験とWASM/Vite buildが成功した。
Windows 11 / AMD Radeon 860M / Vulkanのdebug版で、800×600と1280×720の全10画面のGPU検査が成功した。
rootと独立readonly reviewerが800×600の画像を確認し、rootは1280×720の設定・確認・Pilot・技術情報も確認した。
保存先は`target/bevy-ux-fixed-800x600/`と`target/bevy-ux-fixed-1280x720-retry/`である。
1280×720の初回はfocus喪失により失敗し、通常の安全停止を維持して再実行した。
今回のsourceに対するrelease再buildと実キー・クリック・wheel・DPIの受入は未実施である。以前のrelease検査と区別する。

共通Rust tickへ尾翼合成角の有限候補保護を追加し、controller version 2を記録する。
既定環境で全nose-upを継続すると、独立した`StaticAlpha`制約により1.60秒・11.48mで終了する。
この入力系列は完全な迎角・失速保護の未達事項を示す。ユーザーの117.91mの入力列との同一再現を主張しない。

検査基準は次の通りであり、共有crateを追加した場合はその関連testも実行する。

```sh
cargo fmt --all --check
cargo check -p birdman-game-bevy --locked
cargo clippy -p birdman-game-bevy --all-targets --locked -- -D warnings
cargo test -p birdman-game-core -p birdman-game-format -p birdman-game-bevy --locked
cargo run -p birdman-game-bevy --release --locked
```

共有層の抽出によるWASM/Webの回帰、native active-target依存、NED/FRD・quaternion・cameraの変換も検査する。
Windows 11 / AMD Radeon 860Mの実起動とGPU validation・shader・asset読込みを確認し、取得可能ならnative screenshotを残す。
合格・失敗・未実施・未移植を区別し、実操作確認後にIssueへ「確認用実装完了・ユーザー確認待ち」と記録する。
ユーザーの受入結果を先取りしてIssueをcloseしない。

### 描画FPSと性能目標

WebとBevyは右上に描画FPSを表示する。750 ms窓で集計し、起動直後・長時間中断後は`FPS —`を表示する。
100 Hzの物理tickとは独立する。Bevyの値は実時間Updateの周期であり、GPU present完了を直接測定する値ではない。
同じ派生labelのText更新を省略し、表示・入力・Rustの正本は維持する。
PC Screenの目標は80 FPS相当・12.5 ms/frameである。現在の検査画面は60 Hzであり、80 FPSの実表示受入は未実施である。
`--no-vsync`は通常Windowで提示方式の要求を変更し、offscreenではrunnerの待機を解除する。既存診断のwindow.present_modeは要求値であり、GPU/OSが実際に選択した方式と区別する。
Update頻度の増加、80 Hz以上の実表示、12.5 msの処理予算達成を個別に確認する。WebのrequestAnimationFrameには影響しない。
FPSの単体試験、debug版の撮影、以前のrelease検査を、最新版の性能達成として扱わない。

### Playable model 2 / controller 3の確認

2026-10-09、Windows 11 / AMD Radeon 860M / Vulkanで最新debug版を実起動した。
組込みのlogical input harnessでTitle→Setup→Briefing→Countdown→Pilot/Chase→Pause/Resume→
ManualAbort Result→Retry→full nose-up flight→WaterContact Result→技術情報→Retryを確認した。
full nose-up列は25.13 s・169.07 mで着水し、failureはNoneである。
1280×720の10枚を取得し、水面・地形・機体・FPS・設定/結果の表示を確認した。
shader compilation、GPU validation、asset読込み、日本語分割の確定エラーは0件である。
撮影時のProcessingDelayは既存の安全停止で処理し、通常frameへの復帰後に明示Resumeした。

実キーボード・マウス・wheel・DPI、他GPU、VR、最新版releaseでの性能受入は未実施である。
共有RustのTypical環境39入力列はすべてWaterContactとなった。条件とsoft保護の限界は
[Playableモデル契約](playable-hybrid-model.md)に記載する。

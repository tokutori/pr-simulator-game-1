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
終了理由が適用範囲外の場合も元のcauseと最後の有効状態を保持する。
旧三軸モデルへ切り替えない。既定モデルの改善は公開可能なsynthetic係数に限定し、実機同定と分離する。

発進方位は[#255](https://github.com/tokutori/pr-simulator-game-1/issues/255)の修正として北西315°へ統一する。
比較元にはplatform315°と初期heading・距離評価軸0°の不整合があった。
共有`assets/biwa-launch-venue.json`からRustの初期姿勢・course axis、Web/Bevyのplatform配置を導出する。
風はworld NEDを維持し、カメラまたは景観だけを回転する補正は行わない。

## 最初に確認する操作

起動→飛行を設定→操縦支援を選択→飛行準備へ進む→発進カウントダウンを開始→Flight→Result→同じ条件で再試行を確認する。
設定・確認・発進の進行表示を設け、設定選択、主要操作、戻る操作を区別する。
設定の操縦支援はManual・Shared 50%・Automaticの3択であり、確認画面では選択値と操作方法を表示する。
カウントダウンを取り消すと確認画面へ戻り、確認画面から設定変更もできる。
既定のTypical環境を維持し、nativeで未提供の気象選択を表示しない。

| 入力 | 操作 |
|---|---|
| ↑ / ↓ | 現行二系統モデルのpitch入力 |
| ← / → | 現行二系統モデルの旋回入力 |
| J / L | パイロット位置の目標値を前後に移動 |
| C | Pilot / Chase切替 |
| P / Esc | 飛行の一時停止・復帰 |
| 右mouse buttonを押してdrag | 視点操作 |
| F12 | カレントディレクトリへnative screenshotを保存 |

独立したroll入力は現行二系統モデルに追加しない。操作機器を解放した後のpilot目標保持も既存coreの契約を使用する。

Resultは終了理由、確定距離、短い原因説明を表示し、入れ子の内部診断は「技術情報」に格納する。
Flight HUDは飛行中に限定する。一時停止中は停止理由と再開方法を優先し、未回復の再開ボタンを非活性で表示する。
本文と下部操作を同じcolumnに配置する。本文は最大65vhと残余高の範囲へ収め、操作領域を保持する。
ボタンは明示幅と最低高48pxを持ち、狭いwindowでは折り返す。本文と技術情報はwheelでスクロールできる。
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
Parley 0.9.0にはICU4X 2.3の`BidiClass::to_icu4c_value`に関する既存の非推奨警告が1件残る。
これは分割モデル欠損と別の上流API変更であり、[ICU4X #6067](https://github.com/unicode-org/icu4x/issues/6067)を参照する。
警告の抑制や依存versionの後退は行わない。
利用と再配布の境界は[Microsoftのfont FAQ](https://learn.microsoft.com/en-us/typography/fonts/font-faq)に従う。
未導入環境では起動時に明示的なエラーを返し、利用許諾を持つフォントを`--font`で指定する。

## Three.js版との差分

native ScreenのPilot/Chase切替はpresentation機能として提供し、core configuration・physics・recordを変更しない。
水面・空・機体・湖岸/地形はBevy adapterで表示する。水面の波・反射は描画専用であり、接触判定は既存coreを使用する。
水面shaderの完全一致、全Scene装飾、複雑なAnalysis、保存一覧UI、全Replay/gamepad、VRと別OSの対応は今回の対象外である。
水面は八方向の波packet、画素footprintで減衰するnormal、Fresnel・低コントラストの空反射・広い太陽反射を使用する。
波の位相と方向は同じ環境snapshotとsimulation timeから導出する。空はprocedural gradientを使用する。
空・水面反射・PBRのDirectionalLightは登録環境の太陽方位・仰角を共用する。
NEDから描画座標へ変換した太陽への方向を水面・空へ渡し、DirectionalLightのforwardをその負方向へ向ける。
方向の符号、基準方位・仰角、実際のworld初期化経路の一致をnative回帰試験で検査する。
Gerstnerのgeometry変位・高品質な反射は未移植である。遠方波は[#257](https://github.com/tokutori/pr-simulator-game-1/issues/257)の修正として、
画素footprintに応じて解像できない法線・色変調成分を減衰させる。近距離の波と既存反射式を保持する。
機体は簡易mesh、HUDは数値主体である。[#258](https://github.com/tokutori/pr-simulator-game-1/issues/258)の修正として、
数値と単位を不可分にし、角速度・姿勢の共有単位を見出しへ分離する。ADI等の詳細計器は未移植である。

## GPU接続の検査

```sh
cargo run -p birdman-game-bevy --locked -- --verify target/bevy-gpu-verification
```

小さいwindowまたは1080pで検査する場合は`--verify-size`を併用する。

```sh
cargo run -p birdman-game-bevy --locked -- --verify target/bevy-gpu-verification-small --verify-size 800x600
cargo run -p birdman-game-bevy --locked -- --verify target/bevy-gpu-verification-1080p --verify-size 1920x1080
```

寸法は正の整数で指定し、`--verify`との併用を必須とする。通常起動は1280×720を維持する。
OS表示倍率は変更しない。指定寸法とPNGの物理pixel寸法はOS倍率により異なりうるため、保存ログの実寸も確認する。

このmodeは選択したローカルフォントで日本語の単語境界と日本語/Latin混在の折返しを検査し、
通常のsession操作とlogical inputを使って開始・飛行・Pilot/Chase・一時停止/再開・終了・Result・Retryを確認する。
Title/設定/確認/Countdown/Pilot/Chase/一時停止/Resultの画像を書き込み、保存完了後に成功を返す。
再試行では全nose-up入力による実フライトの終端と技術情報も撮影し、元cause・score・recordの一致を確認する。
WaterContact/TimeLimitの正常終了も受理する。診断表示のために通常入力の失敗を必須としない。
途中のwindow終了とtimeoutは失敗になる。
水面/PBR shaderのロードと実pipelineのcompile完了を5連続frame確認してから撮影する。
撮影frameのUI layout後に、ボタンlabelのglyph・寸法・包含、viewport内の配置、本文と下部操作の非重複を検査する。
shader/assetの確定エラーは原因を保持して返す。timeoutはGPU初期化後の最初のUpdateから90秒とする。
検査中はnativeウィンドウをアクティブに維持する。通常のfocus・処理遅延による停止契約も適用される。
撮影処理によるProcessingDelayだけの停止は、正常なfocused frameと描画準備の回復後に検査操作として明示的に再開する。
通常モードの安全停止・再開条件は変更しない。
scripted logical inputの検査は、実キーボード・マウスによる操作の受入と区別する。

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

# Bevy native Screen確認版

## 対象と比較基準

[#254](https://github.com/tokutori/pr-simulator-game-1/issues/254)の確認用実装を`bevy`ブランチで開発する。
比較基準とbaseは`c5408914015f2f5c2679c033efca41e6b6be8404`に固定する。
既存Web版は維持し、今回のnative対象はWindows 11のScreenに限定する。
mainへのmerge、GitHub Pagesの公開切替、releaseは行わない。

## 起動

workspaceのルートで実行する。

Windowsでの開発・確認用ディレクトリは`C:\projects\birdman\tokutori\pr-simulator-game`である。
このディレクトリを`bevy`ブランチに配置し、追加のworktreeへの移動を起動条件にしない。

```sh
cargo run -p birdman-game-bevy --release --locked
```

Rustは既存の1.97.0、Bevyは`=0.19.1`を使用する。BevyのMSRVは1.95.0である。
[公式setup](https://bevy.org/learn/quick-start/getting-started/setup/)に従うWindowsのnative build環境とGPU driverが必要である。
Node.js、npm、TypeScript、ブラウザ、WASM runtimeはnative版の起動条件に含めない。

## モデルと状態の正本

既定は現行二系統hybrid・Manual・Full・Typical、seed `0x00005f98000055aa`、最大4,000 tickである。
共有Rust準備と既存GameSession、力学、空力、制御、接触、score、recordを直接利用する。
Bevy Transformはtyped snapshotの描画投影であり、physics stateを所有しない。
固定tickはcoreの`PHYSICS_HZ`を参照し、描画frame・camera・ウィンドウ寸法から分離する。
終了理由が適用範囲外の場合も元のcauseと最後の有効状態を保持する。
旧三軸モデルへ切り替えず、モデルの変更や飛距離tuningを行わない。

発進方位は[#255](https://github.com/tokutori/pr-simulator-game-1/issues/255)の修正として北西315°へ統一する。
比較元にはplatform315°と初期heading・距離評価軸0°の不整合があった。
共有`assets/biwa-launch-venue.json`からRustの初期姿勢・course axis、Web/Bevyのplatform配置を導出する。
風はworld NEDを維持し、カメラまたは景観だけを回転する補正は行わない。

## 最初に確認する操作

起動→開始→Flight操作→Pilot/Chase切替→一時停止・復帰→終了→Result→Retryを確認する。
画面下部のボタンで開始、操縦方式選択、飛行準備、発進、終了、再試行を実行する。

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
水面は二つの空間周波数によるnormalとFresnel・太陽反射、空はprocedural gradientを使用する。
Gerstnerのgeometry変位・高品質な反射・遠方波のfilteringは未移植であり、水平線付近にmoireが残る。
機体は簡易mesh、HUDは数値主体である。一部の単位表示に折返しがあり、ADI等の詳細計器は未移植である。

## GPU接続の検査

```sh
cargo run -p birdman-game-bevy --locked -- --verify target/bevy-gpu-verification
```

このmodeは選択したローカルフォントで日本語の単語境界と日本語/Latin混在の折返しを検査し、
通常のsession操作とlogical inputを使って開始・飛行・Pilot/Chase・一時停止/再開・終了・Result・Retryを確認する。
Title/Pilot/Chase/Resultの画像を書き込み、保存完了後に成功を返す。途中のwindow終了とtimeoutは失敗になる。
水面/PBR shaderのロードと実pipelineのcompile完了を5連続frame確認してから撮影する。
shader/assetの確定エラーは原因を保持して返す。timeoutはGPU初期化後の最初のUpdateから60秒とする。
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
今回のGPU検査はdebug buildであり、release buildでの実描画確認は区別して記録する。
修正前HEAD `259f1477`はUbuntu/WindowsのCI、共有層76件、native check/Clippyに合格した。
この結果は修正後の実描画受入と区別する。
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

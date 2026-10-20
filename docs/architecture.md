# アーキテクチャ

## 目的と設計原則

- 本プロジェクトは、短時間の無動力滑空を開始・操縦・着水・結果・再試行まで実行するシミュレーターである。
- WebとBevy nativeは同じRustの物理、セッション、記録、UIモデルを利用する。
- 描画品質と実機への適合性は独立した検証対象とする。

- 状態の正本、純粋な更新、派生値の導出、副作用境界を分離する。
- phaseに必須のデータは対応するvariantが所有し、排他的な状態を型で表現する。
- 同じ規則を複数のplatformに実装しない。adapterは外部イベント・資源・表示APIとの変換を担当する。
- 現行の二系統尾翼モデルをlive経路と記録形式の対象とする。旧モデル、旧live経路、旧schemaの互換実装を維持しない。
- 型・形式証明・数値試験・実GPU・操作性・実機fidelityの保証を区別する。

開発時の運用は[CONTRIBUTING](../CONTRIBUTING.md)、検査方法は[検証](verification.md)に定める。

## crateと依存方向

```text
Web application ── WASM adapter ─┐
CLI ────────────────────────────┼─ session ── format ── core
Bevy native adapter ────────────┘       └──────────────── core
```

adapterからcore・formatへの直接参照も許可する。逆方向の依存と循環依存は禁止する。

| 層 | 所有する責務 |
| --- | --- |
| `birdman-game-core` | 座標・数学、運動方程式、空力、風場、制御、固定tick、接触、GameSession、FlightRecord、集計・補間query |
| `birdman-game-format` | 外部設定・環境・記録のcodecと検証、解決済み設定、比較identity、Personal Best選定 |
| `birdman-game-content` | 登録機体JSONと検証済みの質量・空力storage。sectionとstrip境界から借用可能な荷重モデルを準備する |
| `birdman-game-session` | ApplicationSession、共有command実行、scenario準備・資源所有、archive生成、共有UI・camera・描画用モデル |
| `birdman-game-wasm` | JavaScript ABI、入力のdecode、共有処理の呼出し、DTOのencode |
| `birdman-game-cli` | CLI/WASIのI/O、offline検査・出力・計測入口 |
| `birdman-game-bevy` | nativeウィンドウ、ECS・入力・時刻、共有Viewの投影、GPU資源 |
| `web/src` | browser資源・権限・focus・保存、presentation、共有契約のDOM/VR/Three.jsへの投影 |
| `tools` | offline asset処理、schema生成、境界検査、再現可能な検証コマンド |

- core・format・contentは`no_std + alloc`を維持する。
- coreにfilesystem、network、OS時計、乱数取得、GIS、serde、WASM ABI、描画engineを持ち込まない。
- 乱数seed、環境、入力、時刻観測は外部から明示的に渡す。
- モデル構築と記録容量の予約は準備段階で行い、固定tickのheap allocationを発生させない設計を要求する。
- 達成状態は対応するallocation検査で確認する。

- 依存の版とfeatureは`Cargo.toml`・`Cargo.lock`を正本とする。
- coreの数値依存は`libm`と`polycool`であり、`nalgebra`はdesktopの検証用dev-dependencyに限定する。
- 証明用`vstd`とschema生成用`schemars`は明示的な非default featureとし、通常のnative/WASM実行経路へ有効化しない。
- Parleyのlocal patchはnative日本語組版に限定し、`vendor/parley`を独立したworkspace memberへ追加しない。

- 依存境界検査は設計上の禁止依存・feature・対象platformを検査する。
- manifestの内容を別の一覧へ複写することを目的にしない。
- 全targetの宣言一覧と、active targetで実際に有効になる依存は区別する。

## 状態と実行規則の正本

### GameSessionとApplicationSession

- `GameSession`はdomain phase、停止理由、封印済みconfiguration、飛行状態、終了理由、score、確定record、RetryとReplayを所有する。
- 公開snapshotはこの状態から導出する観測値である。
- Bevyの`Transform`、DOM、TypeScriptのScene名を飛行状態の正本にしない。

- `SessionLifecycle`と`ConfiguredPhase`は、設定済み・飛行中・結果・再生に必要なpayloadをvariantへ結合する。
- 独立したphaseと多数の任意fieldの組合せで不変条件を管理しない。
- Title用demoのようにphaseをまたいで存続する資源は独立した所有領域に置く。

- `ApplicationSession`はprivateなGameSessionと、難易度、seed、tick上限、準備metadata、archive原文、記録の制御identityを所有する。
- `HybridSessionPreparation`は機体・環境の共有storageを組み立て、Webとnativeへ同じconfigurationを渡す。
- 準備をWASM専有処理やUI表示処理にしない。

- 共有command executorが、設定、準備、Countdown、Pause、Resume、Retry、Replayの呼出し規則を所有する。
- UIは型付きcommandを生成し、adapterは時刻・可用性・非同期I/Oの観測を共有側へ渡す。
- platformごとの再開規則や独立したCountdownを作らない。

- 停止理由はManual、DocumentHidden、TrackingSuspended、ProcessingDelayの集合である。
- 各外部観測の解消は対応する理由だけを解除する。
- 理由の解除で自動再開しない。
- 明示ResumeはManualを除去し、残る外的理由によって認可を判定する。
- 拒否されたResumeでもManualの解除は保持する。

### 固定tickと失敗

- 物理周期はcoreの公開`PHYSICS_HZ`を参照する。
- 現在は100 Hzである。
- adapterはwall-clock、固定tickへの入力割当、描画補間、表示frameを管理する。
- 可変dtへの変更、遅延tickの黙示的破棄、独立した二重clockを導入しない。
- Pause中に物理状態とrecordを進めない。

- Countdownは経過時間を共有側へ渡して認可する。
- callback回数を秒数へ代用しない。
- attempt tokenで古い観測を拒否し、初期telemetryと記録開始が成功した後にFlightへ遷移する。

- 飛行tickは候補状態の評価とcommitを分離する。
- 失敗したtickをrecordへ追加せず、最後の有効状態と元の型付き原因を保持する。
- 失敗には操作拒否、再開条件未解消、正常終端、異常終端がある。
- 全APIに一律の「Errなら全状態不変」を要求しない。

数値契約は[飛行力学](flight-dynamics.md)と[飛行制御](flight-control.md)、記録と終端時刻は[データと形式](data-and-formats.md)に定める。

## 数値モデルと座標の境界

- 現行経路は`TailFlightScenario`、二系統の尾翼取付角、共有FBWを使用する。
- `StaticPolar`と`HybridModel`は排他的な荷重providerであり、同一荷重を二重加算しない。
- Hybridのstatic項と局所補正、風評価、失敗site・stage・causeを保持する。
- telemetryは同じ風場を参照する。

- 物理はNED位置・速度、FRD機体座標、bodyからNEDへのquaternionを使用する。
- 単位とquaternion要素順はproducerとadapterの接続試験で確認し、描画APIに必要な座標変換をadapterで行う。
- 描画原点、眼位置、機体原点、合成重心は用途を区別する。
- 表示の都合で物理状態を変更しない。

- Web/nativeの数値比較は、同じmodel・scenario・seed・解決済み設定・tick入力で行う。
- 異なるcompiler/targetに無条件のbitwise一致を要求しない。
- 物理一致と、表示補間・視点・入力割当の一致を個別に検査する。

- 共有`airframe_shape`はf64の構造形状を所有し、表示meshと固定物理polytopeを別々に導出する。
- 全polytope頂点による静水面支持と、連続曲面・変形機体への近似は別の契約である。

- 湖面の波・空・地形は描画用モデルとして共有する。
- 物理接触は固定水面を使う。

## 共有UI

### 純粋な更新とView

Rustの`game_ui`がUIの正本を所有する。

```text
GameUiState × Action → 更新後State × Effect
State × Session観測 × Platform観測 → GameUiView / GameUiFrame
GameUiView / GameUiFrame → platform renderer → DOM / VR panel / Bevy entity
```

- ラベル、主要操作、順序、section、表示条件、theme、HUD、chartの意味は共有Rustで定義する。
- platformは表示API、フォント計測、safe area、focus、scroll、入力方式を担当する。
- ボタンIDや表示文字列から意味を再推定しない。

- menuは順序付きsectionとrow/column/gridを持つ。
- section見出し、control順序、chart legendのsolid色・gradientを各rendererへ転送する。
- 動的readoutは型付きproviderを用いる。
- HUDは順序付きgroupとText・Attitude・HeadingTape・Direction・LinearGauge等のnodeを持ち、adapterはkindを網羅的に投影する。
- IDは差分追跡に用い、計器項目の個別再実装に用いない。

- chartの軸、series、segment、風、marker、cursorはRustのrecord queryから導出する。
- TypeScriptやBevy UIで飛行物理・scoreを再計算しない。
- 未提供の警報producerは利用不可として表示し、Custom設定値の保持と実装済み警報の提供を区別する。
- 致命的なcontroller失敗は通常の情報量設定にかかわらず通知する。

### 構造・動的値・外部入力

- 構造Viewと高頻度の動的Frameを分離する。
- 構造revisionと依存keyの意味は共有側が所有し、adapterは変更時だけ構造を再投影する。
- 数値更新ごとにNode・grid・背景・DOM全体を再構築しない。
- DOMはrenderer境界で反映し、focus・caret・scrollの保持を検査する。

- Titleの保存一覧は共有queryが要求する最大3件の新しいmetadataへ限定する。
- Webは逆順metadata cursorで取得し、UI JSON入力は131,072 byte、noticeは2,048 Unicode scalar、日時文字列は256文字以内とする。
- このpreview制限はPersonal Bestの全候補探索を変更しない。
- 原記録JSONをUI入力へ反復しない。

- 難易度はInformation、Assistance、Weatherの三軸である。
- presetは三軸を一括設定するshortcutであり、画質とpresentationは別の設定である。
- UI仕様は[ゲームとUI](gameplay-and-ui.md)に定める。

### schemaとABI

- Rustのserde型とSchemars schemaを正本に、readonlyなTypeScript型と接続試験用のschemaを生成する。対象はUI、操作、session snapshot、準備metadata、環境、record queryである。
- 生成型のreadonlyとtuple制約はTypeScript compilerで検査する。schema生成用featureとAjvによるschema検査は開発ツールの経路へ限定する。
- 同一buildのWASM出力は、instanceを所有するfacadeの内部でJSONをparseし、生成DTOへ直接接続する。phase、profile、環境、描画poseの正本は共有Rustに置く。
- WebはDTOを保持し、DOM・engine APIに必要な投影だけを行う。readonlyは静的な変更制約とし、時刻・score・quaternion等の数値契約はRustのproducerとcoreで検査する。
- 外部の保存JSONはformatのcodecで受理する。内部出力の受渡しと、外部原文のdecodeを独立した境界として扱う。
- Web入力はown fieldとgetterの値を一度捕捉し、prototype由来の値を送信内容から分離する。捕捉後にownerの生存を確認し、サイズ制限とRustの受理規則を適用する。
- 非同期要求、record source、Countdownのtokenはadapterの実際の待機と資源寿命を管理する。構造・数値・所有権の接続試験は[検証](verification.md)に定める。
- 生成物を手編集しない。hot pathと保存用文書の変更は、転送量・decode費用・入力意味を個別に確認する。

## Webの状態と資源寿命

- WebのAppModelはbrowserが所有するpresentation、resource観測、権限、focus、form draft、非同期要求を保持する。
- 純粋なupdateからEffectを生成し、browser APIの実行をinterpreterへ分離する。
- Rust snapshotの観測をTypeScriptの独立したゲーム状態機械へ変換しない。

- 非同期要求IDとrecord source generationを区別する。
- 古いquery結果、別recordの解析結果、破棄済みownerの通知を受理しない。
- WASM resourceは単一のowner/facadeが解放する。
- Scene退出やBFCache待避によるsuspendと、owner交換・終了によるdisposeを区別する。

- 同recordのseek・clock・再生速度はdatasetの寿命を変えない。
- Retry、設定変更、archive開閉、disposeはrecord sourceを失効させる。
- Attractのdemoはplayerと独立した有界record・clockを所有し、playerの保存・exportへ混入させない。

- Controllerのdisposeは入口でgenerationとclockを無効化し、input、session、renderer pose、HUD等の独立cleanupをすべて試行する。
- 最初の失敗で後続を中断しない。
- 単一原因を保持し、複数原因はAggregateError等で集約する。
- 再入と再実行によって同じ解放を重複しない。

- PresentationRuntimeもloop、backend、rendererのcleanupをすべて試行し、最初の完了Promiseと結果を共有する。
- 失敗は原因を保持する空でない型付き結果として通知する。
- 表示更新とresourceの寿命変更を同じ責務へ混在させない。

## 描画・camera・presentationの境界

### engine非依存の契約

- Screen、WebXR、Phone VRは表示modeである。
- Three.js、Bevyはengine/backendである。
- 公開契約へengine固有型を露出しない。

| 契約 | 内容 |
| --- | --- |
| snapshot・pose | 機体・pilot・風・終端状態とtyped telemetry |
| world asset | 登録ID、座標、mesh・属性の共通データ、利用条件 |
| camera | Pilot/Chase/Replay/Attractのpose、mount、head pose |
| ViewerFrame | 同一frameのmono/stereo眼、projection、viewport、head frame |
| UI | Action、View、Frame、anchor、panel bounds、入力hit領域 |
| capability・quality | 可用なmode、画質・倍率、適用状態、型付き失敗 |

- Three.jsの型・importは`web/src/render/engines/three`内部へ限定する。
- 具体factoryの接続はcomposition rootが行う。
- app、presentation、camera、契約層からengine adapterを参照しない。
- 型専用import、re-export、JSDoc import、`require`、dynamic import、module referenceも同じ境界と依存DAGの対象である。

- 共有cameraの低位basisはapp/runtime型に依存しない。
- session選択・snapshotの共通観測型もapp所有型への逆参照を持たない。
- 型の複写によって循環を回避しない。

### cameraとpanel

- Replay/Attractのcamera directorは共有Rustに置く。
- liveの入力・head trackingはadapterで観測し、aircraft pose、cockpit mount、pilot offset、head poseを合成する。
- head trackingで物理状態を変更しない。

- layout、hit test、backend描画は同じimmutableなViewerFrameを使う。
- panelはAbsent・Visible・Unavailableを排他的に表現する。
- 必須capabilityが失われた際はUnavailableとし、前frameのpanelを残さない。
- HUDはcamera/head基準のanchor、前方clear region、透明度、stereo可読性を明示する。

- GPU mesh、material、shader、render target、entity/DOMはadapterが所有する。
- 共有モデルはenvironmentから波スペクトル・空・cloud・地形・機体形状を生成し、adapterが成分選択・uniform・GLSL/WGSL・engine座標へ変換する。
- 変更のたびに巨大なsceneを再送しない。

- 資源交換は候補の準備成功後にcommitし、旧資源だけを解放する。
- 空のCloudLayerは型でmesh不在を表現し、頂点・indexが空のmeshをGPU資源として登録しない。
- 失敗時は有効な旧資源と元原因を保持する。

詳細な描画・空間・品質契約は[描画とplatform](rendering-and-platforms.md)に定める。

## nativeの独立性と検証境界

- native版は同じApplicationSession、共有UI、記録producer/selectorを直接利用する。
- 起動にNode.js、JavaScript、browser、WebView、WASM実行環境を要求しない。
- OS/GPU・window systemの通常の依存はnative環境の前提とする。

- native実装と配布対象はWindows/macOS/Linuxのx86_64/arm64、表示対象はScreenである。
- Webの全Scene/overlayへのScreen/VR契約を維持する。
- nativeへのXR追加や署名・notarizationの達成は別途検証する。

- build成功、ソース外のCLI smoke、offscreen GPU、実画面の操作、性能、OSごとの受入を区別する。
- CPUのadapter testだけでshader・描画を確認済みとしない。
- 形式証明は[形式検証](formal-verification.md)、asset・保存形式・出典は[データと形式](data-and-formats.md)、起動手順は[nativeガイド](native-guide.md)を参照する。

# アーキテクチャ

## 目的と設計原則

- 本プロジェクトは、短時間の無動力滑空を開始・操縦・着水・結果・再試行まで実行するシミュレーターである。
- WebとBevy nativeは同じRustの物理、セッション、記録、UIモデルを利用する。
- 描画品質と実機への適合性は独立した検証対象とする。

- 状態の正本、純粋な更新、派生値の導出、副作用境界を分離する。
- phaseに必須のデータは対応するvariantが所有し、排他的な状態を型で表現する。
- 同じ規則を複数のplatformに実装しない。adapterは外部イベント・資源・表示APIとの変換を担当する。
- live経路は現行の二系統尾翼モデルを使う。保存記録は[データと形式](data-and-formats.md)のschema別契約に従って読み出す。
- 型・形式証明・数値試験・実GPU・操作性・実機fidelityの保証を区別する。

開発時の運用は[CONTRIBUTING](../CONTRIBUTING.md)、検査方法は[検証](verification.md)に定める。

## crateと依存方向

```text
WASM ── runtime ── presentation ── app ── content ── format ── core
          └─────── scene ──────────┘
Native ── runtime
CLI ────────────────────────── app
```

adapter・runtime・presentation・scene・app・contentは責務に応じて下位のcore・format・contentを直接参照し、依存DAGを形成する。集約入出力のschemaと起動表示はruntimeのbuild用exampleが生成する。

| 層 | 所有する責務 |
| --- | --- |
| `birdman-game-core` | 座標・数学、運動方程式、空力、風場、制御、固定tick、接触、有限な曲げ係数と補間、GameSession、FlightRecord、集計・補間query |
| `birdman-game-format` | 外部設定・環境・地物・記録のcodecと検証、解決済み設定・水面品質の共通型、比較identity、Personal Best選定 |
| `birdman-game-content` | 登録機体・地物の原文JSON、検証済みの質量・空力storageと構造形状。sectionとstrip境界から荷重モデルと接触支持点を準備し、対気速度・物理尾翼角から固定順の変形点群を生成する |
| `birdman-game-app` | ApplicationSession、共有command実行、scenario準備・資源所有、実行状態・入力profile・環境観測、会場・地物の登録整合と共有storage、archive・記録cursor認可・ビルド来歴の生成 |
| `birdman-game-presentation` | 共有UI・camera・XR layout・適応画質の方針と、live・prepared・記録状態からの描画poseの直接導出 |
| `birdman-game-scene` | contentから導出する機体・地形mesh、水面・天空・画像の描画資産生成 |
| `birdman-game-runtime` | app・presentation・sceneを束ねる単一application owner、入力・frame進行・XR・記録I/O・資源準備と完了認可、最終描画データとprimitive I/O要求の出力 |
| `birdman-game-wasm` | JavaScript ABI、入力のdecode、共有処理の呼出し、DTOのencode |
| `birdman-game-cli` | CLI/WASIのI/O、offline検査・出力・計測入口 |
| `birdman-game-bevy` | nativeウィンドウ、ECS・入力・時刻、共有Viewの投影、GPU資源 |
| `web/src` | browser資源・権限・focus・保存、presentation、共有契約のDOM/VR/Three.jsへの投影 |
| `tools` | offline asset処理、schema生成、境界検査、再現可能な検証コマンド |

- core・format・contentは`no_std + alloc`を維持する。
- appとsceneは登録storageの一度限りの初期化に`std::sync::OnceLock`を使う。sceneは`image`・`earcut`による画像処理・地形三角化を所有し、app・presentation・runtimeは日時処理や資源所有に`std`を利用する。
- CLIはapp・content・format・coreのoffline検査を実行する。cameraと機体形状の整合はpresentationからsceneへのdev-dependencyで検査する。
- coreにfilesystem、network、OS時計、乱数取得、GIS、serde、WASM ABI、描画engineを持ち込まない。
- 乱数seed、環境、入力、時刻観測は外部から明示的に渡す。
- モデル構築と記録容量の予約は準備段階で行い、固定tickのheap allocationを発生させない設計を要求する。
- 達成状態は対応するallocation検査で確認する。

- 依存の版とfeatureは`Cargo.toml`・`Cargo.lock`を正本とする。
- coreの数値依存は`libm`と`polycool`である。登録機体の局所線型化試験はcontentに置き、`nalgebra`をdesktopの検証用dev-dependencyに限定する。
- 証明用`vstd`とschema生成用`schemars`は明示的な非default featureとし、通常のnative/WASM実行経路へ有効化しない。
- Parleyのlocal patchはnative日本語組版に限定し、`vendor/parley`を独立したworkspace memberへ追加しない。

- 依存境界検査は設計上の禁止依存・feature・対象platformを検査する。
- manifestの内容を別の一覧へ複写することを目的にしない。
- 全targetの宣言一覧と、active targetで実際に有効になる依存は区別する。

## 状態と実行規則の正本

### ApplicationRuntimeと描画資源

- runtimeの合成ownerは生観測・UI操作・I/O完了を受け、最終描画データとprimitive I/O要求を返す。Nativeは同じbuffer descriptorの型付きsliceを借用し、WASMは必要なbufferをLittle Endianの所有コピーで提出する。動的属性の借用は次のupdateまで有効とする。
- 空のpanorama、光源・霧、波の係数とdetail画像は登録環境からRustが準備する。画像は同じ生成条件で共有し、水面品質の変更では波の投影とmesh密度を更新する。
- GPU資源は候補作成、ownerのcommit認可、atomicなbinding完了の順に進む。ownerはcommitを直列化し、確認した適用状態から次の候補と旧資源の解放を決定する。
- 画質変更は環境資源と同じ準備・commit経路へ投入する。共有policyの要求は最初の適用確認で確定し、各commitの実適用品質・framebuffer寸法・世代を記録する。Screenの未縮小物理解像度とHMD runtimeのframebuffer寸法は別の生観測とする。
- preparationの取消は完了通知までCPU bufferを保持する。取消中に作成が完了した候補もownerが解放し、adapterは要求されたGPU操作と結果通知を実行する。
- 初期rendererが未取得の場合、runtimeは`CreateRenderer`を要求し、request IDで`RendererCreated`を認可する。空と会場のcommit完了までBoot Viewを表示し、初期化失敗の再試行と再読み込みも同じ入力・I/O要求へ統合する。取得済みrendererを渡すnative起動はその資源を採用する。
- WASM読込前の初期表示・失敗表示は、共有Rustからbuild時に生成した静的HTMLを使う。実行開始後のView・操作認可・初期化世代はruntimeが所有する。

### GameSessionとApplicationSession

- `GameSession`はdomain phase、停止理由、封印済みconfiguration、飛行状態、終了理由、score、確定record、RetryとReplayを所有する。
- appはdomainの進行と入力を所有する。presentationは確定したlive・prepared状態と記録補間から`FlightRenderPose`を直接導出し、runtimeがUI・scene・I/O要求を合成してWASM境界へ返す。
- Bevyの`Transform`、DOM、TypeScriptのScene名を飛行状態の正本にしない。

- `SessionLifecycle`と`ConfiguredPhase`は、設定済み・飛行中・結果・再生に必要なpayloadをvariantへ結合する。
- 独立したphaseと多数の任意fieldの組合せで不変条件を管理しない。
- Title用demoのようにphaseをまたいで存続する資源は独立した所有領域に置く。

- `ApplicationSession`はprivateなGameSessionと、難易度、seed、tick上限、準備metadata、archive原文、記録の制御identityを所有する。
- appは記録cursorの時間範囲を認可し、presentationはcoreの記録補間から描画poseとReplay cameraを導出する。
- 準備・Retry・Briefing再準備・archive交換・初回demo登録はapplication単位の記録source識別子を更新する。tick・seek・再生速度の変更は同じsourceを保持する。
- `HybridSessionPreparation`は機体・環境の共有storageを組み立て、Webとnativeへ同じconfigurationを渡す。
- `PreparedFlight`は登録機体と準備時に確保した固定長の接触点bufferを所有する。通常飛行とAttractは同じprovider入口で、封印済み機体・荷重・接触設定を評価し、同じ係数・尾翼角の成功投影を再利用する。
- 初期状態へ実際の合成重心対気速度から求めた主翼係数を封印し、Retryはその初期値と準備済みbufferを再利用する。
- 準備をWASM専有処理やUI表示処理にしない。

- 共有command executorが、設定、準備、Countdown、Pause、Resume、Retry、Replayの呼出し規則を所有する。
- UIは型付きcommandを生成し、adapterは時刻・可用性・非同期I/Oの観測を共有側へ渡す。
- platformごとの再開規則や独立したCountdownを作らない。

- 停止理由はManual、DocumentHidden、TrackingSuspended、ProcessingDelayの集合である。
- 外部の利用不可観測は共有applicationで入力保持を解除し、Countdownを取り消し、Flightを一時停止し、Replay/Attractの再生時計を停止する。
- 表示backendの準備状態とXRのvisibility・input focus・head pose validityを共有applicationで保持し、合成した利用可否からTrackingSuspendedを導出する。各観測の復帰は残る要因と合わせて判定し、同じframeの観測を物理更新より先に反映する。
- 各外部観測の解消は対応する理由だけを解除する。
- 理由の解除で自動再開しない。
- 明示ResumeはManualを除去し、残る外的理由によって認可を判定する。
- 拒否されたResumeでもManualの解除は保持する。

### 固定tickと失敗

- 物理周期はcoreの公開`PHYSICS_HZ`を参照する。
- 現在は100 Hzである。
- `ApplicationSession`が入力状態・frame時計・実行失敗を所有し、各tickの入力割当、100 Hzの実行予算と補間残余を導出する。adapterはOS/browserの時刻・機器・可用性を観測し、共有frame入口を一度呼ぶ。
- 発進・明示Resume直後のframeは経過0とする。150 msを超えるframe間隔はProcessingDelayを設定し、有効frameへの復帰と明示Resumeを分ける。
- presentationが最後の物理区間を共有残余で補間し、同じposeと時刻を機体・pilot・cameraへ渡す。0 tickのframeも区間内の表示を更新し、Pause・Resultは確定状態を投影する。
- Webはanimation frameのtimestamp、Nativeは単調時計の累積時刻を渡す。両adapterは同じruntimeへframe・入力・I/O完了を通知し、返された描画データとprimitive要求を実行する。
- Gamepadのdeadzone・中立取得・pilot targetの引継ぎはappが所有する。Nativeは順序付きraw eventを保持し、Entityと接続世代からidentityを作る。接続時の標準軸0はBevyの初期状態として扱い、以後のraw値で更新する。
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

- contentの`AircraftDefinition`が検証済みJSON、f64の`AircraftShape`と全構造材の固定頂点・三角形を所有する。空力sectionと表示mesh・接触polytopeは同じ断面・翼型・部材寸法から導出し、管の周方向位相と変形則を共有する。sceneは材質・座標変換・batch化を所有する。
- 共有準備層が登録機体と借用する荷重viewを一度初期化し、発進・live session・表示へ同じ所有者を渡す。
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
- 共有Viewは有効な操作からBackを導出する。adapterは`GameUiControlInput`としてcontrol IDと入力値を渡し、共有Viewが現在の種類・可用性を確認して実行Actionへ解決する。
- XRのrangeはclip前のtrackを基準に共有Rustで段階値へ丸める。表示・hit領域にはclip後の矩形を使い、スクロール時も同じ値域を保つ。
- 共有Viewの`interaction_scope`がScene・overlay・Result/Replayの表示タブを識別する。Screen・Nativeのscroll保持と空間Menuの入力世代は同じ識別子を利用する。runtimeはpresentationの`MenuSession`を所有し、adapterは実scroll位置・focus・入力を通知する。
- MenuとHead HUDの内容はRust ownerが保持するGameUiViewと現在のGameUiFrameから導出する。共有presentationは両眼fit、文字サイズ、折返し、control・chart・計器線分・実測文字の配置と視角を一回の同期呼出しで確定する。
- Menu viewportは共有Rustがcontent clip・page操作領域・scroll量を導出する。矩形とpanel/document座標は一括で投影し、描画とpointer入力が同じclipを利用する。rangeの値はclip前のtrackに対応する。
- MenuとHead HUDのフォント観測cacheは既存UI ownerの寿命に従い、最大32組を保持する。計測identityは表示内容・locale・surface・解決済み文字サイズ・font世代を含み、変更時に再計測する。Webは共通の描画命令DTOをCanvasへ反映し、Head HUDのposeとsurface寸法をmeshへ渡す。
- 文字折返しは共有Rustが明示改行・Unicode文字列・符号付きink幅から決める。フォント観測callbackは要求された文字列全体を計測し、advance・bearing・ascent/descentをメートル単位で返す。Webは同期呼出し中だけCanvas観測を提供し、共有処理が行とparagraphを確定する。
- platformは表示API、フォント計測、safe area、OS/browserのfocus・visibility・機器観測を担当する。Native UIは入力とentity寿命の管理、View/Frameのwidget投影、chart描画、HUD描画をmodule境界で分ける。XRのcursor積分・押下edge・入力源切替・dwellは共有presentationが所有する。
- 共有UIの表示modeはScreen・Hmd・PhoneVrとし、HMDの利用可否とMenuの再配置能力をadapterが観測する。WebはHmdをWebXRへ変換し、共有UIが公開した能力から切替・再配置Actionを認可する。
- Phone VRの可用性は共有Runtimeが所有する。権限要求APIのない環境では、起動準備の完了後に購読し、実購読の成功通知から1,500 ms以内に生の絶対姿勢と画面回転の有限値を確認し、購読解除後に切替を認可する。権限要求APIのある環境では利用者の切替操作から権限を要求する。起動時確認と表示中のセンサー購読は同じI/O要求と完了認可を使い、表示中の初回姿勢待ちは実購読の成功通知から3,000 msとする。終了時は解除の終端を待つ。
- ボタンIDや表示文字列から意味を再推定しない。

- menuは順序付きsectionとrow/column/gridを持つ。
- controlは意味・操作・sectionを持ち、配置は各surfaceの実測寸法から解決する。ScreenはDOM/Bevyのflow、空間Menuは共有Rustの文字計測・配置結果を利用する。
- section見出し、control順序、chart legendのsolid色・gradientを各rendererへ転送する。
- 動的readoutは型付きproviderを用いる。
- HUDは順序付きgroupとText・Attitude・HeadingTape・Direction・LinearGauge等のnodeを持ち、adapterはkindを網羅的に投影する。ADIの目盛・文字anchor・丸角clip・機体記号・飛行経路円、HeadingTape・LinearGaugeの目盛線分・文字anchor・三角pointer、Directionの円・十字・北基準文字・矢印は、共有Rustが固定logical canvasの完成幾何として確定する。Screen・Native・Head HUDは同じ幾何を各描画領域へ縦横同率で拡縮し、実fontのbaselineと描画primitiveへ転写する。
- IDは差分追跡に用い、計器項目の個別再実装に用いない。

- chartの軸、series、segment、風、marker、cursorはRustの保持記録から導出する。生成途中の記録座標はpresentation内部へ閉じ、公開View/Frameは論理pxへ投影した点・参照線・矢印線分を返す。静的geometryとシーク時のcursorは同じ投影を使う。
- Replay cameraと描画poseはcoreの同じ記録補間から状態・telemetry・物理舵角を参照する。
- TypeScriptやBevy UIで飛行物理・scoreを再計算しない。
- 未提供の警報producerは利用不可として表示し、Custom設定値の保持と実装済み警報の提供を区別する。
- 致命的なcontroller失敗は通常の情報量設定にかかわらず通知する。

### 構造・動的値・外部入力

- 構造Viewと高頻度の動的Frameを分離する。
- 構造revisionと依存keyの意味は共有側が所有し、adapterは変更時だけ構造を再投影する。
- `GameUiStructureCache`は表示側がapplication ownerと同じ寿命で所有し、構造keyと記録source識別子からrevisionを導出する。ApplicationSessionは記録所有を識別し、UIのcacheは表示側へ置く。
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

- runtimeのserde型とSchemars schemaを正本に、入力観測・描画出力・I/O要求・文字計測の集約契約を一つのreadonly TypeScript型へ生成する。
- 共通の型は同じschema内の参照で共有し、固定長配列・生JSON値・入力と出力の可用性を保持する。起動前の表示HTMLも同じRust ownerから生成する。
- 生成型のreadonlyとtuple制約はTypeScript compilerで検査し、schema featureは開発時の生成経路へ限定する。
- 同一buildのWASM出力は単一runtime bridgeから受け取り、生成DTOを描画・I/Oへ直接提出する。phase、profile、環境、最終camera・描画poseの正本は共有Rustに置く。
- WebはDTOを保持し、DOM・engine APIに必要な投影だけを行う。readonlyは静的な変更制約とし、時刻・score・quaternion等の数値契約はRustのproducerとcoreで検査する。
- Analysisのchart・cursorは共有Rust UIから投影し、Replay cameraと時計操作は同期呼出しで現在の観測を反映する。遅延tickはsession所有者とtimer世代で管理し、停止・seek・速度変更・復帰で旧callbackを退役させる。
- 共有session commandは同期で実行し、観測したRust phaseを即時採用する。保存記録の読込みは専用の待機状態で管理し、I/O応答を要求ID・session所有者・source tokenで照合する。
- ログ出力は現在の記録原文を同期で取得してdownloadを要求し、成功・失敗の通知を保持する。Blob URLはbrowser adapterが所有し、遅延解放と破棄を管理する。
- 外部の保存JSONはformatのcodecで受理する。内部出力の受渡しと、外部原文のdecodeを独立した境界として扱う。
- Webが生成する内部入力DTOはJSONへ直接encodeし、Rustが型・値・Actionの受理を検査する。browser由来の表示文字列と転送量は共有の上限へ収め、WASM数値引数はJavaScriptの整数変換前に境界を検査する。
- 非同期要求、record source、Countdownのtokenはadapterの実際の待機と資源寿命を管理する。構造・数値・所有権の接続試験は[検証](verification.md)に定める。
- 生成物を手編集しない。hot pathと保存用文書の変更は、転送量・decode費用・入力意味を個別に確認する。

## 共有ownerと資源寿命

- `ApplicationRuntime`がアプリケーション状態、進行、入力認可、表示切替、記録検証・選定、非同期要求と世代を所有する。WebとNativeは実API・transaction・GPU handle・timerと描画cacheを保持する。
- 操作は現在提示したViewのcontrol IDとrevisionで通知する。権限要求、保存、読込、資源準備、commit、解放は、実I/Oの終端を元のrequest/resource IDと原因付き結果で返す。
- 同recordのseek・clock・再生速度はdatasetの寿命を維持し、Retry、設定変更、archive開閉、disposeはrecord sourceを失効させる。Attractはplayerと独立した有界record・clockを所有する。
- 終了はRunning、Closing、Drainedで進む。Closingは新しい進行を停止し、発行済みI/Oの終端と遅着資源の解放を継続する。GPU準備・commit・解放の終端後にrendererを解放する。
- adapterは出力された同期I/O要求を実行し、Drainedを確認してWASM instanceまたはNativeプロセスを終了する。再読み込みも同じ解放順序を使う。
- BFCacheのpersisted pagehideはsuspendとして通知し、pageshowの生visibility観測で復帰する。disposeはownerの終了に用いる。

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
- sceneとruntimeは機体・会場・水面の完成mesh属性、画像、材質範囲と動的変形を生成する。Native worldとThree adapterは共通bufferをGPUへ転写し、同じ頂点色・haze契約の地形材質を共有する。
- app、presentation、camera、契約層からengine adapterを参照しない。
- 型専用import、re-export、JSDoc import、`require`、dynamic import、module referenceも同じ境界と依存DAGの対象である。

- 共有cameraの低位basisはapp/runtime型に依存しない。Rustのpresentation::cameraがPose合成・FRD変換・Pilot/Chase配置・Screenのyaw/pitch lookを所有し、XRは同じ型を用いて追跡・両眼・reference spaceを扱う。
- WebとNativeはポインター移動量と標準button maskを共有ownerへ通知し、共有ownerがsecondary押下による視点操作を認可する。各adapterは出力された最終camera・部品pose・projectionをengine型へ転写する。
- scene::lake_skyが太陽のEast/Up/negative-North方向と地平線以上のdaylight条件を導出し、天空画像・水面・照明へ共通適用する。
- coreのsession状態と記録補間結果はapp所有型への逆参照を持たない。
- 型の複写によって循環を回避しない。

### cameraとpanel

- Replay/Attractのcamera directorは共有Rustに置く。
- liveの入力・head trackingはadapterで観測する。presentationの共有Rustがaircraft pose、cockpit mount、pilot offset、head pose、reference resetの累積補正を合成し、cameraとWorld/Cockpit/Menu/Head anchorへ同じ参照系を渡す。
- head trackingで物理状態を変更しない。

- layout、hit test、backend描画は同じimmutableなViewerFrameを使う。
- panelはAbsent・Visible・Unavailableを排他的に表現する。
- 必須capabilityが失われた際はUnavailableとし、前frameのpanelを残さない。
- HUDはcamera/head基準のanchor、前方clear region、透明度、stereo可読性を明示する。
- 両眼pose・projection・clip-depthの契約、Menuの光学fit、Head HUDの探索・配置、実測文字の視角判定はpresentationの`xr`が所有する。Webは観測を生成DTOへ変換し、Rustの結果をCanvasとmeshへ反映する。

- GPU mesh、material、shader、render target、entity/DOMはadapterが所有する。
- 共有モデルはenvironmentから波スペクトル・空・cloud・地形・機体形状を生成し、adapterが成分選択・uniform・GLSL/WGSL・engine座標へ変換する。
- 変更のたびに巨大なsceneを再送しない。
- 地物・Analysis地図・固定camera点の原文はcontent、形式・座標・camera点のhash検証はformat、登録原点と完全なlocal frameの整合および一度限りの初期化はappのVenueCatalogが所有する。地図・Replay camera・地形・構造物は同じcatalogを借用し、presentationは描画用の派生geometryをcacheする。
- appの`EnvironmentSelection`が表示環境のsourceと完全identityを選ぶ。Titleは既定Typical・展示seed、Setupは保持中の気象・seed、飛行とResultは封印済み条件、ReplayとAttractは各recordの条件を使う。
- 両adapterは同じ選択keyで環境交換を判定し、共有projectionの波・天空・会場可用性を反映する。Webの初期描画も同じ展示projectionから準備する。
- archiveの会場原点は保存された完全identityに対応する登録環境から得る。登録原点と水面datumが一致するときに会場を表示する。
- 尾翼の表示用arm metadataはrender_poseが登録機体の物理pivotから導出し、liveとReplayへ共用する。
- 水面品質はformatのWaterQualityを正本とする。波成分・mesh分割数はlake_waterの単一profile、Autoの探索順は画質policyが所有する。

- 資源交換は候補の準備成功後にcommitし、旧資源だけを解放する。
- 上空の雲は共有天空画像へ合成し、背景と水面反射で同じWorld方向のradianceを参照する。
- 失敗時は有効な旧資源と元原因を保持する。

詳細な描画・空間・品質契約は[描画とplatform](rendering-and-platforms.md)に定める。

## nativeの独立性と検証境界

- native版は同じApplicationRuntimeを介してApplicationSession、共有UI、記録producer/selectorを利用する。
- 起動にNode.js、JavaScript、browser、WebView、WASM実行環境を要求しない。
- OS/GPU・window systemの通常の依存はnative環境の前提とする。

- native実装と配布対象はWindows/macOS/Linuxのx86_64/arm64、表示対象はScreenである。
- Webの全Scene/overlayへのScreen/VR契約を維持する。
- Native OpenXRの接続範囲は[描画とplatform](rendering-and-platforms.md)、ソフト接続と実HMDの受入は[検証](verification.md)に定める。
- 署名・notarizationは配布条件として別途検証する。

- build成功、ソース外のCLI smoke、offscreen GPU、実画面の操作、性能、OSごとの受入を区別する。
- CPUのadapter testだけでshader・描画を確認済みとしない。
- 形式証明は[形式検証](formal-verification.md)、asset・保存形式・出典は[データと形式](data-and-formats.md)、起動手順は[nativeガイド](native-guide.md)を参照する。

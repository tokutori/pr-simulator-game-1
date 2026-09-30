# Flight record・telemetry契約

## 責務と不変性

Result、Analysis、Replayは同一の確定済みFlightRecordを参照する。scoreとterminal dispositionもfinalization metadataに保持する。
record型、tick sample追記、終端確定、domain validation、集計値はRust coreが所有する。
Replay時刻・再生速度・再生状態もRust coreが所有する。Webは操作intentと経過wall-clock時間を送り、確定clock stateを表示へ投影する。
秒単位のsample queryはRust coreでrecord時刻へ変換する。TypeScriptはsecondsをtick/fractionへ分解しない。
recordはrendererのframe数に依存せず、成功したphysics tickに対応する値を保存する。
coreはBriefing時に最大4,000 tick（4,001 state sample）の`Vec` capacityを予約し、simulation step中はallocationなしでappendする。現行layoutの`FlightRecordSample`はx86_64と`wasm32-unknown-unknown`で各424 byteであり、最大sample payloadは1,696,424 byte（約1.62 MiB）となる。一括WASM転送はsampleごとに51個の`f64`を別bufferへ展開し、最大payloadは1,632,408 byte（約1.56 MiB）である。全recordの転送時、両bufferの論理payload合計は3,328,832 byte（約3.18 MiB）となる。allocator overhead、`FlightRecord`本体、wasm-bindgen境界のcopy、TypeScript解析配列、JSON encode/decode用memoryは含まない。実allocatorが要求capacityを超える領域を確保する可能性もある。layout変更時は回帰試験と本値を更新する。予約失敗は型付きerrorとしてReady遷移を拒否する。
上限不足・capacity不整合は型付きerrorを返し、recordの部分更新を公開しない。
`birdman-game-format`は外部schemaのversion・encode/decode・入力検証を担当し、保存I/OはCLI/Webが担当する。
WASMはrecord append/finalizeをsimulation operationと一括処理し、snapshot・metrics・analysis queryを返す。Rust coreは固定tick時刻とfractionから保存済みsampleを補間し、summary metricsを生成する。WASM bridgeは`flight_record_sample_at`・`flight_record_summary`・bulk sample exportと各packed layoutを公開する。bulk transfer bufferはfallibleに予約し、確保失敗をadapter errorとして返す。Result遷移時、Webは一度のbulk transferからRust由来summaryを表示する。validated JSON exportはWASMから行い、WebはResult確定時にIndexedDBへ原recordを保存する。保存JSONはRustのbounded decoderで検証し、`GameSessionBridge`がRust coreのquery APIへ復元する。Titleは保存済みrecordの最新3件を表示し、Personal Best記録を識別する。選択recordをRust Replayとして開く。Analysis graphと共通cursorを実装済みである。IndexedDB version 1/2のmetadata migrationと実ブラウザー操作は未検証である。version 4では初回一覧時に旧DB recordからcanonical Personal Best indexを再構築する。
WASMは秒単位の`flight_record_sample_at_seconds` queryも公開し、record時刻からtick/fractionへの変換をRust coreへ委譲する。
recordからRenderSnapshotへの変換を1か所へ集約し、graph・cameraからphysicsを呼ばない。

## Header

外部保存形式は`birdman-game-format::FlightRecordDocument`のJSON schemaを用いる。現行versionは5であり、
physics model versionを含まないversion 1〜3 recordも読み込み対象とする。version判定とschema検証は
`birdman-game-format`が担当し、Web保存adapterはschema versionを解釈しない。
physics/model versionはschema versionから独立させる。decoderは16 MiBを超える入力、未知schema version、未知field、壊れたJSONを拒否する。
`serde_json`は`float_roundtrip`を有効化し、f64 sampleのencode/decodeで値を完全一致させる。

schema version 1と2はscenario/model/environment/controller version、resolved presetと三軸、seed、tick上限、全sample、input、telemetry、finalization、scoreを保存する。version 2はCustom HUD profileを追加する。version 3はscore定義versionを追加する。version 4はphysics model versionを追加する。version 5はeligibleな記録にcanonical Personal Best keyを追加する。version 1〜3はphysics model versionが不明であり、version 1〜4はcanonical keyを持たないため、Personal Best比較対象から除外する。
physics build・scenario・aircraft・environmentのsource hashはcanonical Personal Best keyへ集約する。個別hash、presentation policy、初期環境位相は記録しないため、個別検証や再構成が必要になった段階でschema拡張を検討する。

- record schema、座標・単位契約、physics build/modelのversionとhash
- AircraftModel、scenario/world asset、controller設定とversion、seed、機体・身体の初期状態と身体移動モデル
- 解決済み三軸設定、score定義version、固定dt
- 波の初期位相とsimulation時刻の基準、環境の再表示に必要なmetadata
- 最大flight tick数、sample layout、追加diagnosticの有無
- backend・端末情報等の任意metadata、presentation event列のschema

同じIDでもhashが異なるassetを同一データとして扱わない。
保存schemaとphysics versionの互換性を分離する。記録済み値の表示と再積分による検証は別操作である。
元のphysics実行系が利用できなくても、schemaと必要assetに互換性があればsnapshot再生は可能とする。
未知schema、破損、欠落、未対応追加項目の必須性は検証結果として通知する。

## Samplingと容量

初期formatは100 Hzの全成功tickを保存する。tick kのstateは時刻k×dtを表す。
tick kの入力はstate k→k+1に適用する。tick 0の初期状態も保存する。
時刻は整数tickを正本とし、浮動小数時刻の反復加算を避ける。
接触がtick間にある場合はtickとfractionを持つ終端event/sampleを追加し、二重sampleを排除する。
終端sampleは接触時刻に対応する位置、速度、短経路quaternion slerpによる姿勢、角速度、actuator、
身体位置・速度を同じ補間規則で確定する。接触fractionは接触geometryの各点について補間経路上で探索し、
最早eventを選択する。
接触後のtick $k+1$ 状態は保存しない。Resultの終了点、graph終端、Replay最終poseは同じsampleを参照する。

最大flight tick数はscenario/game policyの明示的な上限である。
Briefingで上限分のsample・入力・終端event領域を確保し、確保失敗時はReadyへ進めない。
上限到達はTimeLimitとして確定し、bufferの上書きやsample間引きを行わない。
Distance score v1はWaterContact時にfractional terminal datum、TimeLimit時に最後の有効integer-tick datumを用いる。
記録上限は4,000 tickとし、最大4,001 sampleを保持する。実測したsample領域とWASM転送payloadの容量は「責務と不変性」に記載する。
端末負荷によって保存周期を変更しない。描画用downsampleは原recordを保持して別途生成する。

## Sampleと入力列

f64の物理値を保存する。圧縮・量子化は後続format versionで誤差契約とともに導入する。

| 項目 | 定義 |
|---|---|
| tick / terminal fraction | simulation時刻 |
| position_ned_m | 機体構造datum $O$ の対地位置 |
| composite_cg_position_ned_m | 合成重心 $G$ のNED位置。map軌跡の正本 |
| velocity_ned_mps | datum $O$ の対地速度 |
| attitude_body_to_ned | 単位quaternion。Euler角はderived |
| angular_velocity_body_rad_s | body角速度 |
| wind_at_cg_ned_mps | 同じ位置・時刻の重心風sample |
| actuator_state | 実舵角等の表示・検証に必要な状態 |
| pilot_motion_state | パイロットの実前後位置・相対速度・相対加速度 |
| combined_cg_offset_body_m | datum $O$ から導出した合成重心offset |
| optional element diagnostics | 要素ID、局所風・対気速度・荷重等 |

入力列にはtickごとの機器非依存な舵指令、身体目標位置、FBW舵出力、混合後舵commandを保存し、
actuator stateと実身体位置を区別する。keyboardの押下やgamepadの生軸値を再現用入力の正本としない。
各diagnosticの評価位置・stageを明記する。RK4内部stageの値を次tickの確定値として流用しない。
重心風sampleはgrid範囲等を検証して保存し、取得不能をゼロ風へ置換しない。
AoA等の未定義値はOption等で表し、NaNを欠損値として使用しない。
追加diagnosticはschemaで有無を明示し、初期基本recordだけでmap・高度・速度とPilot再生が成立する構成とする。

## Derived telemetryと集計

Summary metrics、record由来のtelemetry series、sample/cursor queryはRust coreの純粋関数で生成する。
Webは返された値を選択・描画し、scoreや集計値を独自に再計算しない。表示用downsampleはWebで行えるが、
終端値・summary・Personal Bestの正本へ逆流させない。

```math
V_{air,G}^N=v_G^N-W(p_G^N),\qquad
V_{air}=\lVert V_{air,G}^N\rVert,\qquad
V_{ground}=\lVert v_G^N\rVert,\qquad h_G=-D_G
```

基本表示の速度は3D normである。水平速度を表示する場合は別名で明示する。
AoAは合成重心のair-relative vectorをbodyへ変換して求める診断値とし、各翼の局所AoAと区別する。
Distance score definition v1はstart datumからterminal datumまでの発進course axisに対する符号付き水平projectionとする。
cross-trackとnet horizontal displacementを別metricとして保持する。trajectory lengthからscoreを算出しない。
最大AoAは重心AoAの定義可能なsampleに対する最大値、最大rollは標準Euler分解の絶対値最大とする。
特異姿勢で角度が定義できない場合はinvalidを保持する。
最大速度・角度は保存された100 Hz sampleと有効な終端sampleから集計し、連続時間の厳密最大とは表記しない。
graphの表示用間引きからSummaryを再集計しない。
着水時の重心高度は接触点のoffsetにより0とは限らない。

発進方向の水平単位ベクトルを $e_{\parallel}=(\cos\psi_0,\sin\psi_0)$、
右向きを $e_{\perp}=(-\sin\psi_0,\cos\psi_0)$ とする（成分順N,E）。

```math
s_{\parallel}=\Delta p_{NE}\cdot e_{\parallel},\qquad
d_{cross}=\Delta p_{NE}\cdot e_{\perp},\qquad
W_{\parallel}=W_{NE}\cdot e_{\parallel},\qquad
W_{\perp}=W_{NE}\cdot e_{\perp}
```

Cross-track displacementは発進軸からの符号付き横偏位であり、操舵等の影響も含む。
因果関係を検証せず全偏位を風だけによるdriftと断定しない。
W_parallelは正が追い風方向、負が向かい風方向である。基準を瞬間headingへ変更する場合は別系列名とする。
scoreのDistanceはversioned score定義に従い、水平累積経路長・直線距離・投影距離を混同しない。

## FinalizationとReplay

complete / interrupted / failedと終了理由、最後の有効tick、終端位置を保存する。
finalizationは一度のみ実行し、その後はimmutableとする。
失敗tickの状態は保存しない。初期化失敗で有効sampleがない場合はrecord unavailableとする。
不完全recordも有効区間の解析に使用できるが、通常のPersonal Bestへ登録しない。
初期Personal Best候補は、finalize済みの完全なWaterContact recordでscoreを持つものに限る。
Rust coreの`personal_best_candidate_score()`は完了・WaterContact・scoreの適格性を判定する。formatの`personal_best_candidate_score()`は、さらに現行score definition versionとphysics model versionを要求する。Rust coreは同じcanonical keyを持つ適格scoreを比較し、formatは解決済みconfiguration、初期状態、course axis、各content hashからkeyを生成する。`PersonalBestSelection`は保存済みrecordを逐次評価し、tieでは既存recordを保持する。WASMの`PersonalBestSelectionBridge`はRustの選択状態を保持し、ブラウザーはIndexedDB transaction内で保存済みrecordを照会する。新規recordの保存とPersonal Best index更新を同じtransactionで確定する。IndexedDB version 4では初回一覧時に既存record全体からcanonical key別indexを一度再構築し、以後はindex先recordとのみ比較する。

Canonical key v1ではpreset labelを除外し、Information cue、ControllerProfileのmode・authority・version・gain・command limit、scenario identity・seed、aircraft/scenario/environment/physics content hash、course axis、physics・score version、tick契約、launch stateをSHA-256へ入力する。浮動小数点値は有限値に限定し、負のzeroをpositive zeroへ正規化してbig-endian IEEE-754 bit patternを符号化する。表示品質とpresentation backendはkeyに含めない。

初期Replayはsnapshot再生とする。並進値を補間し、姿勢はquaternionの最短経路で補間する。
pause、seek、速度変更、逆方向操作はplayback clockだけへ作用し、物理を再積分しない。
graph cursorと再生位置は同じrecord時刻を参照する。
保存済み入力からのnative/WASM再積分は検証機能として分離し、Replay表示と同一視しない。

BPG-021の現行実装はRustのResult/Replay phase往復、記録時刻scrub、Result Analysis cursor同期、
Rust補間sampleからのrender pose適用、連続playback clock、pause、0.5×/1×/2×速度選択、ScreenでのPilot/Chase選択までを含む。
その他のReplay rigとブラウザー／VR受入は未実装であり、Scene受入完了条件として残す。

## 検証

BPG-019でtick/FPS独立性、初期・終端sample、欠損/非有限値/重複tick、範囲外seek、
capacity境界、allocation、schema round-trip、手計算可能な集計値、失敗finalizationを検証する。
record validationは単調時刻、単位quaternion、有限値、有効な要素IDとheader整合を確認する。
playback queryは整数tickと`[0, 1)`のfractionを受け取り、最短経路quaternion slerp、線形状態補間、
角度のwrap-aware補間を行う。queryは記録範囲外を拒否し、物理状態を変更しない。

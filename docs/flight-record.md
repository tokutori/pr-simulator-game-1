# Flight record・telemetry契約

## 責務と不変性

Result、Analysis、Replayは同一の確定済みFlightRecordを参照する。scoreとterminal dispositionもfinalization metadataに保持する。
record型、tick sample追記、終端確定、domain validation、集計値はRust coreが所有する。
Replay時刻・再生速度・再生状態もRust coreが所有する。Webは操作intentと経過wall-clock時間を送り、確定clock stateを表示へ投影する。
秒単位のsample queryはRust coreでrecord時刻へ変換する。TypeScriptはsecondsをtick/fractionへ分解しない。
recordはrendererのframe数に依存せず、成功したphysics tickに対応する値を保存する。
coreはBriefing時に最大4,000 tick（4,001 state sample）の`Vec` capacityを予約し、simulation step中はallocationなしでappendする。現行layoutの`FlightRecordSample`はx86_64と`wasm32-unknown-unknown`で各424 byteであり、最大sample payloadは1,696,424 byte（約1.62 MiB）となる。legacy packed ABIの一括転送はsampleごとに51個の`f64`を別bufferへ展開し、最大payloadは1,632,408 byte（約1.56 MiB）である。このlegacy転送時、両bufferの論理payload合計は3,328,832 byte（約3.18 MiB）となる。named二系統JSON queryの容量へこの転送値を適用しない。allocator overhead、`FlightRecord`本体、wasm-bindgen境界のcopy、TypeScript解析配列、JSON encode/decode用memoryは含まない。実allocatorが要求capacityを超える領域を確保する可能性もある。layout変更時は回帰試験と本値を更新する。予約失敗は型付きerrorとしてReady遷移を拒否する。
上限不足・capacity不整合は型付きerrorを返し、recordの部分更新を公開しない。
公開fieldから構築した`FlightRecordHeader`も、`FlightRecord::try_new`のbuffer予約前に再検証する。
最大tick数は1〜4,000、physics frequencyは`PHYSICS_HZ`と一致し、catalog・scenario・aircraft・environment・controllerの各versionは非0を必須とする。
`FlightRecordHeader::try_new`とarchive復元は同じheader条件を用いる。scenario IDとseedの数値範囲は追加で制限せず、catalog解決は呼出し側の責務とする。
不正headerは構築時に`InvalidHeader`、archive復元時に`InvalidArchive`となる。正当な容量の予約失敗は`AllocationFailed`とし、domain errorと区別する。
`birdman-game-format`は外部schemaのversion・encode/decode・入力検証を担当し、保存I/OはCLI/Webが担当する。
WASMはrecord append/finalizeをsimulation operationと一括処理し、snapshot・metrics・analysis queryを返す。Rust coreは固定tick時刻とfractionから保存済みsampleを補間し、summary metricsを生成する。既定の`HybridGameSessionBridge`はnamed sample・Summary・風queryと、保存v1〜5/v6の復元を同じcore recordへ接続する。旧`GameSessionBridge`のpacked queryはlegacy互換入口として保持する。validated JSON exportはWASMから行い、WebはResult確定時にIndexedDBへ原recordを保存する。Analysis datasetは同recordのcontextを照合して取得し、通常のReplay clock更新ではcursorだけをqueryする。毎frameの全record変換・Summary再集計を行わない。Titleは保存済みrecordの最新3件を表示し、Personal Best記録を識別する。選択recordをRust Replayとして開く。IndexedDB version 1〜3からのupgrade、metadata移行、version 4のindex revisionによる再構築はfake-indexeddbで検証する。限定Screen受入では保存・一覧・schema 5 archiveのReplay、CSV1と元JSON全文一致を確認した。条件は[検証契約](verification.md)に記載する。PB indexへの登録対象はschema version 5または6の、有効なcanonical keyを持つeligible recordに限る。version 1〜4のrecordは一覧・閲覧できるが、Personal Best比較対象にはならない。
WASMは秒単位の`flight_record_sample_at_seconds` queryも公開し、record時刻からtick/fractionへの変換をRust coreへ委譲する。
recordからRenderSnapshotへの変換を1か所へ集約し、graph・cameraからphysicsを呼ばない。

## Header

外部保存形式は三軸の`FlightRecordDocument`（schema v1〜5）と二系統の`TailFlightRecordDocument`（schema v6）を用いる。
physics model versionを含まないversion 1〜3 recordも読み込み対象とする。schema検証は
`birdman-game-format`が担当する。Web保存repositoryはJSONを保持し、PB factoryはschema versionから対応するRust selectorを選択する。
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
schema 4以降のphysics model versionは既知の1から現行versionまでを読み込み対象とし、0、欠落、
現行より新しいversionを拒否する。過去versionのJSON encode/decodeはversionと保存sampleを維持する。
Personal Best候補は現行physics model versionとの一致も要求し、過去versionのcanonical keyが
保存されていても比較対象にしない。
physics model version 1は初期モデル、2は回復可能な身体移動状態を維持する目標制御、
3はtick内保持値と終端actuatorの一致を表す。versionの更新は保存schemaを変更しない。
元のphysics実行系が利用できなくても、schemaと必要assetに互換性があればsnapshot再生は可能とする。
未知schema、破損、欠落、未対応追加項目の必須性は検証結果として通知する。

## Samplingと容量

初期formatは100 Hzの全成功tickを保存する。tick kのstateは時刻k×dtを表す。
tick kの入力はstate k→k+1に適用する。tick 0の初期状態も保存する。
時刻は整数tickを正本とし、浮動小数時刻の反復加算を避ける。
接触がtick間にある場合はtickとfractionを持つ終端event/sampleを追加し、二重sampleを排除する。
終端sampleは接触時刻に対応する位置、速度、短経路quaternion slerpによる姿勢、角速度、
身体位置・速度を確定する。actuatorは入力 $k$ による更新後の値を区間 $(k,k+1]$ で保持し、
正の接触fractionでも荷重に用いた同じ値を保存する。fractionが0なら既存のtick $k$ sampleを保持し、
actuator更新後の値や重複sampleを追加しない。接触fractionは接触geometryの各点について補間経路上で探索し、
最早eventを選択する。
接触後のtick $k+1$ 状態は保存しない。Resultの終了点、graph終端、Replay最終poseは同じsampleを参照する。

最大flight tick数はscenario/game policyの明示的な上限である。
Briefingで上限分のsample・入力・終端event領域を確保し、確保失敗時はReadyへ進めない。
上限到達はTimeLimitとして確定し、bufferの上書きやsample間引きを行わない。
Distance score v1はWaterContact時にfractional terminal datum、TimeLimit時に最後の有効integer-tick datumを用いる。
記録上限は4,000 tickとし、最大4,001 sampleを保持する。実測したsample領域とWASM転送payloadの容量は「責務と不変性」に記載する。
端末負荷によって保存周期を変更しない。描画用downsampleは原recordを保持して別途生成する。

## Sampleと入力列

BPG-042のcore制御契約は`FlightRecordControls`で旧三軸と二系統尾翼を排他的に表す。
各variantは同方式のphysical actuatorとinterval inputを一組として保持する。
二系統inputは正規化nose-up/right intent、body q/r target、身体Hold/Set指令、解決済身体目標、
coreで一度評価したmanual/FBW/mixed incidenceを区別する。記録層でFBWや身体mappingを再計算しない。
正の接触fractionはtick reportの保持incidenceを保存し、fraction=0は新しいinput/sampleを追加しない。
`FlightRecordSample.controls`とplaybackの`actuators`は同じ判別型を使い、既存の時刻query・summaryを共用する。
`begin_tail`と`append_tail_report`は同じ予約済bufferへ記録し、record内の制御方式混在を拒否する。
schema v1–5と旧WASM packed layoutはlegacy値を維持し、二系統値を受けると型付き非互換を返す。
既定のhybrid flightはv6 codecとnamed WASM queryへ接続し、legacy三軸と二系統を同じsample/query正本で扱う。

二系統の確定済archiveは`TailFlightRecordDocument`のschema v6で保存する。
`control_identity`はaircraft configurationとcontroller profileのIDを保持し、各versionはheaderを正本とする。
control payloadは`layout: tail_incidence`で判別し、水平尾翼・垂直尾翼の実効incidence、
normalized manual intent、body q/r target、身体Hold/Set、manual/FBW/mixed commandを名前付きで保持する。
`FlightRecordArchiveDocument`はv1–5とv6をversion別にdecodeし、同じcore queryへ保存snapshotを復元する。
未知schema、未知field、余剰roll軸、破損値は型付きerrorで拒否する。旧recordのidentityと保存値は保持する。
新hybrid再積分の互換性はsnapshot閲覧と独立に検査し、旧三軸またはmodel/controller/scenario/physicsの不一致を拒否する。
archiveのPersonal Best比較はv6の適格な同identity・同canonical keyだけに限定し、旧三軸recordを混在させない。
二系統canonical keyは専用domainで明示identity、各version、resolved difficulty、二軸profileのgain/slew、
course、content hash、初期physical incidence・姿勢・身体状態を保持する。三軸feedbackは使用しない。
`TailPersonalBestSelection`はv6同identity・同keyの完全なWaterContactだけを比較し、tieは最初の既存recordを維持する。
layout別archive/PB factoryはschemaを明示してRust decoder/selectorへ委譲し、未知schemaを拒否する。

coreの`finalize_with_failure`は共通`SessionSimulationFailure`を保持し、causeの分類と終了理由、
三軸・二系統のcontrol layoutを照合する。最新の成功sampleと一致するstampだけを確定し、
失敗intervalの部分状態を追加しない。cause未対応のcodecは型付き非互換を返し、原因を欠落させない。
v6 finalizationはtyped failureを明示し、元のcontrol/contact/dynamics/load原因と
Hybridのstage/site/limit/causeを個別に保存する。OutOfValidEnvelopeには元causeを必須とし、
原因と終了理由の不一致・未知tag・余剰fieldを拒否する。保存queryは最後の成功sampleを利用する。
Hybrid診断のlimitはcauseとsiteへ照合し、Control incidenceはTailIncidenceのsiteと
stage未指定、NonFiniteまたは水平・垂直尾翼のincidence限界へ限定する。

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
| actuator_state | 初期sample `(0, 0)` は初期舵角、後続sampleはそのsampleへ進めた区間の保持舵角 |
| pilot_motion_state | パイロットの実前後位置・相対速度 |
| additional diagnostics | 合成重心offset・要素別荷重・実加速度等は現行schemaに保存しない |

入力列にはtickごとの機器非依存な舵指令、身体目標位置、FBW舵出力、混合後舵commandを保存し、
actuator stateと実身体位置を区別する。keyboardの押下やgamepadの生軸値を再現用入力の正本としない。
各diagnosticの評価位置・stageを明記する。RK4内部stageの値を次tickの確定値として流用しない。
重心風sampleはgrid範囲等を検証して保存し、取得不能をゼロ風へ置換しない。
AoA等の未定義値はOption等で表し、NaNを欠損値として使用しない。
追加diagnosticはschemaで有無を明示し、初期基本recordだけでmap・高度・速度とPilot再生が成立する構成とする。

## 数値ログのdownload

ResultとReplayでは同じ現在recordからCSVと元record JSONを取得する。
`GameSessionBridge`と`HybridGameSessionBridge`の`export_flight_log_csv`・`export_current_flight_record_json`は
Result/Replay限定のqueryであり、physics、record、Replay clockを更新しない。
通常recordのJSONは現行encoder、保存archiveのJSONはopen成功時に保持した検証済み原文を返す。
Legacyの通常recordはschema 5、Tailはschema 6を使用する。保存schema 1〜6、未知環境、欠損metadataを
現行機体・環境で再構成しない。未保存のsource hashを現在のbuildから補完しない。

Legacy CSV export version 1は従来の三軸舵列を維持し、Tailのversion 2は`control_layout=tail_incidence`と
水平・垂直尾翼のphysical incidence、nose-up/right-turn intent、$q,r$ target、pilot Hold/Set、
resolved target、manual/FBW/mixed incidenceを保存値から出力する。尾翼を旧三軸舵へ変換しない。
両versionはUTF-8/LF、header付きの全標本を出力し、CSVの区切り・引用符・改行をescapeする。
全保存state、telemetry、区間input、metadataとfinalizationを列へ写し、unitsとNED/body座標を列名へ明記する。
f64はroundtrip可能な十進文字列、欠損値は空欄とavailabilityで表す。難易度・終了理由は検証済みenum名を用いる。
Tailの任意IDは`aircraft_configuration_id_json`・`controller_profile_id_json`、元failureは
`terminal_failure_available`・`terminal_failure_json`で保持し、JSON文字列をCSVとしてescapeする。
`tick_index`と`fraction`を独立して保持し、初期・fractional terminalを含めて標本を間引かない。

両layoutは同じfinite-difference estimatorを使用し、加速度を保存値との差を明示した`estimated_*`列に出力する。
datumのNED速度、body角速度$p,q,r$、
body前方軸に対するパイロット相対速度から有限差分を導出する。NED加速度は重力を含む運動学的変化率であり、
specific force、合成重心加速度、荷重・momentの再評価は行わない。Quaternion/Euler角の差分は使用しない。
内部標本は不等時間間隔の3点公式、両端は片側2点secantを用い、methodとstencil時刻を保存する。
単一標本は`insufficient_samples`、非正・丸めで識別不能な時間間隔は`invalid_interval`、
推定値のoverflowは`non_finite_estimate`として空欄にする。終端の極小fractionをNaNや無限大へ置換しない。
TypeScriptは返されたtextをBlob/download adapterへ渡し、数値や推定値を再計算しない。

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
Rust coreの`personal_best_candidate_score()`は完了・WaterContact・scoreの適格性を判定する。formatの`personal_best_candidate_score()`は、さらに現行score definition versionとphysics model versionを要求する。Rust coreは同じcanonical keyを持つ適格scoreを比較し、formatは解決済みconfiguration、初期状態、course axis、各content hashからkeyを生成する。`PersonalBestSelection`は保存済みrecordを逐次評価し、tieでは既存recordを保持する。WASMの`PersonalBestSelectionBridge`はRustの選択状態を保持し、ブラウザーはIndexedDB transaction内で保存済みrecordを照会する。Repositoryとpersistence portにはRust selection factoryを必須で供給する。新規recordの保存とPersonal Best index更新を同じtransactionで確定する。

IndexedDB version 4のPB index修復revisionは`first-winner-layouts-v1-physics-v3`とする。Rust canonical keyのlayout・適格性契約が変わる場合は修復revisionも更新する。
この完了markerがない場合、初回一覧・初回保存のどちらからも既存record全体をID昇順で再構築する。従来の`canonical-v1`、`first-winner-v1`、`first-winner-physics-v3`や過去physics versionの完了markerがあっても再構築し、schema v1〜5/v6を対応するRust selectorで検証する。各layoutの同identity・同canonical keyだけを比較し、同点は最初のwinnerを保持する。過去physics versionや不適格recordはPB indexから除外し、保存snapshotの閲覧を維持する。score比較と適格性判定をWebへ複製しない。record JSON・ID・保存日時は変更しない。

再構築・PB index・修復markerと、保存時のrecord・metadata追加は同一readwrite transactionで確定する。不適格candidateの保存も修復を先に完了する。失敗時は全変更をrollbackし、生成したselectionを解放する。以後の保存はindex先recordとのみ比較し、一覧取得は再構築を繰り返さない。

Canonical key v1ではpreset labelを除外し、Information cue、ControllerProfileのmode・authority・version・gain・command limit、scenario identity・seed、aircraft/scenario/environment/physics content hash、course axis、physics・score version、tick契約、launch stateをSHA-256へ入力する。浮動小数点値は有限値に限定し、負のzeroをpositive zeroへ正規化してbig-endian IEEE-754 bit patternを符号化する。表示品質とpresentation backendはkeyに含めない。

初期Replayはsnapshot再生とする。並進値を補間し、姿勢はquaternionの最短経路で補間する。
pause、seek、速度変更、逆方向操作はplayback clockだけへ作用し、物理を再積分しない。
graph cursorと再生位置は同じrecord時刻を参照する。
保存済み入力からのnative/WASM再積分は検証機能として分離し、Replay表示と同一視しない。

BPG-021の現行実装はRustのResult/Replay phase往復、記録時刻scrub、Result Analysis cursor同期、
Rust補間sampleからのrender pose適用、連続playback clock、pause、0.5×/1×/2×速度選択、ScreenでのPilot/Chase選択までを含む。
`1093d4cb`の限定Screen受入でReplay・Analysis操作を確認した。全Replay rigの網羅、VR・実端末の表示と操作はScene受入の残条件として保持する。

## 検証

BPG-019でtick/FPS独立性、初期・終端sample、欠損/非有限値/重複tick、範囲外seek、
capacity境界、allocation、schema round-trip、手計算可能な集計値、失敗finalizationを検証する。
record validationは単調時刻、単位quaternion、有限値、有効な要素IDとheader整合を確認する。
playback queryは整数tickと`[0, 1)`のfractionを受け取り、最短経路quaternion slerp、位置・速度・
身体状態の線形補間、telemetryと角度のwrap-aware補間を行う。actuatorはexact sample時刻でその保存値を返し、
sample間の内点では終端側sampleの保持値を返す。fractional terminal区間にも同じ規則を適用する。
queryは記録範囲外を拒否し、物理状態を変更しない。
復元した旧recordにも同じactuator query規則を適用し、保存sampleと厳密な終端値は変更しない。
旧recordの線形補間済みterminal actuatorから当時の荷重に用いた保持値を再構成することはできないため、
その値の補正や再積分は行わない。

recordの順序・範囲判定は整数tickとfractionの組で行い、`(n, 1)`と`(n + 1, 0)`を同一時刻として扱う。
保存sampleとfinalization metadataの一致検査には保存したtick/fractionの厳密一致を用いる。
微小な終端fractionを絶対tickへ浮動小数加算して消失させず、補間率は隣接sampleからの局所時間差で計算する。
表示用secondsへの変換で同じ値になるsampleもすべて保持し、core・format・Web境界は真の重複時刻を拒否する。

秒単位queryは有限かつ`0 <= seconds <= duration_seconds`を受け付ける。
`seconds == duration_seconds`では保存終端の状態・telemetryを優先し、返却時刻は上記の同値規則で正規化する。
正の微小durationが秒への変換で0へ丸められる場合も、seconds=0は保存終端を返す。
秒の内点は表示秒へ変換した隣接sampleの区間を選び、その区間内の秒差の比から補間する。
これにより秒から絶対tickへの再乗算で生じる丸めを範囲判定へ持ち込まない。
同じsecondsへ丸められた時刻を個別指定する場合はtick/fraction queryを用いる。
durationの直外を含む範囲外queryは拒否し、epsilonによる時刻の移動やsample削除は行わない。

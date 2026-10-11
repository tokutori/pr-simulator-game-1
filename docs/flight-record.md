# Flight record・telemetry契約

## 責務と不変性

Result、Analysis、Replayは同じ確定済みFlightRecordを参照する。
Rust coreが型、tick sample追記、終端確定、domain validation、集計、Replay clockを所有する。
Webとnativeは保存I/O、操作intent、表示を担当し、score・FBW・物理値を再計算しない。
成功したphysics tickだけを保存し、描画frame数・画質・backendから独立する。
Briefingで最大4,000 tickと初期状態に対応するsample capacityを予約する。予約失敗はReady遷移を拒否する。
上限不足・不正header・capacity不整合は型付きerrorとし、部分recordを公開しない。
headerは最大tick 1〜4,000、公開PHYSICS_HZ、非0のcatalog/scenario/aircraft/environment/controller versionを要求する。
scenario IDとseedのcatalog解決は呼出側が担当する。

HybridGameSessionBridgeは同じcore recordからnamed sample・Summary・風query・schema 6 exportを返す。
秒単位queryのtick/fraction変換・補間はRustで行う。通常のseekで全Analysis datasetを再生成しない。
query変更とrecord source変更は別の世代で管理し、古い非同期結果を破棄する。

## Headerと外部schema

外部保存形式はTailFlightRecordDocumentのschema 6に限定する。
旧live実行系、旧packed ABI、schema 1〜5 decoder、schema migration、旧Personal Best selectorを提供しない。
未対応schemaはUnsupportedSchemaVersionを返す。既存保存データの削除・変換・架空metadataの補完は行わない。
decoderは16 MiBを超える入力、未知field、壊れたJSON、非有限値、単位quaternionと時系列の不整合を拒否する。
serde_jsonのfloat_roundtripでf64のencode/decode値を保持する。
score_definition_versionとphysics_model_versionは必須値であり、schema versionと独立して検証する。
control_identityはaircraft configurationとcontroller profileのIDを持ち、versionはheaderに保持する。
Personal Best keyは任意の保存fieldとし、適格性・比較条件をRustで判定する。
未知保存環境はUnavailableとして扱い、現在の環境・地図へ置換しない。

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

FlightRecordControlsはTailIncidenceと任意のinput_from_previousを一組として保持する。
初期sampleはinterval inputを持たず、後続sampleはnormalized nose-up/right intent、
body q/r target、身体Hold/Set、resolved target、manual/FBW/mixed incidenceを記録する。
記録層でFBWや身体mappingを再評価しない。
begin_tailとappend_tail_reportは予約済みbufferを使用し、FlightRecordPlaybackSample.actuatorsはTailIncidenceを返す。
正のcontact fractionは適用reportの保持incidenceを保存し、fraction=0では新input/sampleを追加しない。
finalize_with_failureはTailFlightTickErrorを保持し、最新成功sampleと同じstampだけを確定する。
v6 codecはcontrol/contact/dynamics/load原因とHybrid stage/site/limit/causeを保存する。
原因・site・stage・終了理由の不一致、未知tag、余剰fieldを拒否する。
失敗intervalの部分状態を追加しない。causeの欠落を仮定して補完しない。

f64のstate、telemetry、controlsを保存する。圧縮・量子化は誤差契約を伴う後続schemaで扱う。
datum位置・速度はNED、body角速度はFRD、姿勢はbody-to-NED quaternionである。
合成重心の位置・高度・速度とdatumを区別し、保存telemetryを表示正本にする。

## 数値ログのdownload

ResultとReplayでは同じ現在recordからCSVと元record JSONを取得する。
`HybridGameSessionBridge`の`export_flight_log_csv`・`export_current_flight_record_json`は
Result/Replay限定のqueryであり、physics、record、Replay clockを更新しない。
通常recordのJSONは現行encoder、保存archiveのJSONはopen成功時に保持した検証済み原文を返す。
記録はschema 6を使用し、未知環境へ現在の環境metadataを流用しない。未保存のsource hashを現在のbuildから補完しない。

CSV export version 2は`control_layout=tail_incidence`と
水平・垂直尾翼のphysical incidence、nose-up/right-turn intent、$q,r$ target、pilot Hold/Set、
resolved target、manual/FBW/mixed incidenceを保存値から出力する。尾翼を旧三軸舵へ変換しない。
CSVはUTF-8/LF、header付きの全標本を出力し、CSVの区切り・引用符・改行をescapeする。
全保存state、telemetry、区間input、metadataとfinalizationを列へ写し、unitsとNED/body座標を列名へ明記する。
f64はroundtrip可能な十進文字列、欠損値は空欄とavailabilityで表す。難易度・終了理由は検証済みenum名を用いる。
Tailの任意IDは`aircraft_configuration_id_json`・`controller_profile_id_json`、元failureは
`terminal_failure_available`・`terminal_failure_json`で保持し、JSON文字列をCSVとしてescapeする。
`tick_index`と`fraction`を独立して保持し、初期・fractional terminalを含めて標本を間引かない。

finite-difference estimatorを使用し、加速度を保存値との差を明示した`estimated_*`列に出力する。
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

## Finalization、Personal Best、保存

complete / interrupted / failed、終了理由、最後の有効tick/fraction、score、元failureを一度だけ確定する。
失敗tickは保存せず、初期化失敗で有効sampleがない場合はrecord unavailableとする。
Personal Bestは完全なWaterContact、score、現行score/physics version、有効canonical keyを要求する。
TailPersonalBestSelectionとTailPersonalBestSelectionBridgeが同identity・同keyの比較を所有し、同点は最初の既存recordを保持する。
未対応schemaは比較対象から除外する。schema 6の破損は元errorを保持して拒否する。
WebのIndexedDB名とversion 4は維持し、record追加・metadata・Personal Best更新を同じtransactionで確定する。
新規DBはversion 4で生成する。version 1〜3の既存DBは明示的に拒否し、保存内容を保持する。
旧DB移行、metadata backfill、互換revisionに基づくindex再構築を実行しない。
既存record JSON・ID・保存日時は変更せず、旧record選択時には未対応schemaを通知する。
失敗時はtransactionをrollbackし、Rust selectorを解放する。

Canonical keyは明示identity、model/controller/scenario/physics/score version、
difficulty、profile gain/slew/optional alpha guard、course、content hash、初期状態をSHA-256へ入力する。
負zeroはpositive zeroへ正規化し、finite f64をbig-endian IEEE-754で符号化する。
画質とpresentation backendはkeyへ含めず、保存keyを現在のbuildから再生成しない。

## Replay

snapshot再生はRust補間queryからstateを取得し、物理を再積分しない。
pause、seek、速度変更は同じReplay clockへ作用し、graph cursorと3D描画が同record時刻を参照する。
Resultへ戻る場合は元の確定state・finalizationを表示し、cursor stateへ置換しない。
保存入力による再積分は検証機能として分離し、利用する現行model/controller/physics/scenarioと明示的に照合する。

## 検証

BPG-019でtick/FPS独立性、初期・終端sample、欠損/非有限値/重複tick、範囲外seek、
capacity境界、allocation、schema round-trip、手計算可能な集計値、失敗finalizationを検証する。
record validationは単調時刻、単位quaternion、有限値、有効な要素IDとheader整合を確認する。
playback queryは整数tickと`[0, 1)`のfractionを受け取り、最短経路quaternion slerp、位置・速度・
身体状態の線形補間、telemetryと角度のwrap-aware補間を行う。actuatorはexact sample時刻でその保存値を返し、
sample間の内点では終端側sampleの保持値を返す。fractional terminal区間にも同じ規則を適用する。
queryは記録範囲外を拒否し、物理状態を変更しない。

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

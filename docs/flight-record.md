# Flight record・telemetry契約

## 責務と不変性

Result、Analysis、Replayは同一の確定済みFlightRecordを参照する。
record型、tick sample追記、終端確定、domain validation、集計値はRust coreが所有する。
recordはrendererのframe数に依存せず、成功したphysics tickに対応する値を保存する。
coreはBriefing時に最大tick数分のcapacityを準備し、simulation step中はallocationなしでappendする。
上限不足・capacity不整合は型付きerrorを返し、recordの部分更新を公開しない。
`birdman-game-format`は外部schemaのversion・encode/decode・入力検証を担当し、保存I/OはCLI/Webが担当する。
WASMはrecord append/finalizeをsimulation operationと一括処理し、snapshot・metrics・analysis queryを返す。
recordからRenderSnapshotへの変換を1か所へ集約し、graph・cameraからphysicsを呼ばない。

## Header

- record schema、座標・単位契約、physics build/modelのversionとhash
- AircraftModel、scenario/world asset、controller設定とversion、seed、機体・身体の初期状態と身体移動モデル
- 解決済み三軸設定、許可camera・安定化、score定義version、固定dt
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
終端sampleは接触時刻に対応する位置、姿勢、actuator、身体位置・速度を同じ補間規則で確定する。
接触後のtick $k+1$ 状態は保存しない。Resultの終了点、graph終端、Replay最終poseは同じsampleを参照する。

最大flight tick数はscenario/game policyの明示的な上限である。
Briefingで上限分のsample・入力・終端event領域を確保し、確保失敗時はReadyへ進めない。
上限到達はTimeLimitとして確定し、bufferの上書きやsample間引きを行わない。
具体的な上限・1 sampleのbyte数・総容量はBPG-019で計測して登録する。
端末負荷によって保存周期を変更しない。描画用downsampleは原recordを保持して別途生成する。

## Sampleと入力列

f64の物理値を保存する。圧縮・量子化は後続format versionで誤差契約とともに導入する。

| 項目 | 定義 |
|---|---|
| tick / terminal fraction | simulation時刻 |
| position_ned_m | 機体構造datum $O$ の対地位置 |
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

初期Replayはsnapshot再生とする。並進値を補間し、姿勢はquaternionの最短経路で補間する。
pause、seek、速度変更、逆方向操作はplayback clockだけへ作用し、物理を再積分しない。
graph cursorと再生位置は同じrecord時刻を参照する。
保存済み入力からのnative/WASM再積分は検証機能として分離し、Replay表示と同一視しない。

## 検証

BPG-019でtick/FPS独立性、初期・終端sample、欠損/非有限値/重複tick、範囲外seek、
capacity境界、allocation、schema round-trip、手計算可能な集計値、失敗finalizationを検証する。
record validationは単調時刻、単位quaternion、有限値、有効な要素IDとheader整合を確認する。

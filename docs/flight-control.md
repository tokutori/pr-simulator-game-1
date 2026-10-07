# 舵指令・FBW・actuator契約

## 所有境界

制御・authority混合・actuator状態はRust coreの決定的なdomain stateとする。入力機器はTypeScript adapterで
機器非依存のpilot commandとdesired body rateへ変換し、tick単位でcoreへ渡す。DOMとGamepad APIはこの境界へ含めない。
observed body rateはRustの直前tick stateから取得し、FBW commandをcore内で生成する。

FBW authorityは舵指令だけに適用する。pilot body targetは別経路であり、authority mixerに含めない。
Manualはpilot指令を選択し、Automaticはcontroller指令を選択し、Sharedはauthority $a$ による線形混合を行う。

```math
u=(1-a)u_{pilot}+a u_{FBW},\qquad 0\le a\le1
```

## 型と単位

roll、pitch、yawの各論理指令・actuator出力は、対応する機体軸まわりの角度をradianで表す。
authorityは有限な$[0,1]$値だけを保持するvalidated型とする。制御modeはManual、Shared(authority)、Automaticの
直和型で表し、modeとauthorityの矛盾を許さない。

`BodyRateFeedbackConfig`はroll・pitch・yaw順の有限な非負gain（秒）と正のcommand limit（radian）を保持する。
`body_rate_feedback_commands`は明示されたtarget/observed body angular rate（rad/s）から
$u_i=K_i(\omega_{target,i}-\omega_{observed,i})$を計算し、axisごとにcommand limitで飽和する。
feedback-controlled tickはtargetを入力とし、observed rateは直前のRust core stateから取得してFBW commandを導出する。
CLI/WASM adapterはFBW commandを事前生成しない。これはcontroller primitiveであり、標準gainや公開機体へのtuningを定義しない。機体固有controllerは、空力微係数・actuator・scenarioと
合わせた検証後に構成する。

各actuatorは正の最大舵角$radian$と最大舵角速度$radian/second$を持つ。
入力targetは最大舵角でsaturateし、現在状態から1 stepで移動できる角度を最大舵角速度とtimestepで制限する。
整数tick $k$ のsnapshotは直前区間の保持値 $\delta_k$ を保存する。入力 $k$ から求めた更新値
$\delta_{k+1}$ は区間 $(k,k+1]$ で保持し、全RK4 stageの空力評価へ同じ値を渡す。
RK4の開始stageは入力更新後の右側値を評価する。rate limitは隣接tickの更新量を制限し、
tick内の連続したactuator軌跡を定義しない。
接触fractionが正なら終端physical actuatorも $\delta_{k+1}$ とする。fractionが0なら正の飛行時間を
経過していないため、既存snapshotの $\delta_k$ を終端値として保持する。
制御・actuator更新周期はphysics tickと同じ100 Hzとする。actuator stepは正の有限timestepのみ受理する。
pilot target policyは保持加速度によるtick内軌跡と終端stateの有限停止証明を検査し、目標近傍では2 tickで位置目標へ停止する運動学に基づく要求を生成する。
停止証明は積分器と同じ`f64`運動を最大4096 step検査する。物理的な連続停止不能とpolicyの数値適用範囲外を別の型付きerrorにする。詳細は `pilot-motion.md` を正本とする。
そのtimestepは100 Hz tick以下の有限値とし、描画frame数から値を生成しない。
`advance_surface_control`はpilot/FBWのauthority混合、rate limit・saturation適用、更新後stateを一つの
決定的な操作として返す。混合後commandもrecord可能な値として返却する。
`advance_flight_tick_with_contact`は統合tickの次状態をwater-contact detectorへ渡し、次のinteger-tick stateまたはterminal contactを
返す。Contact時にはfractional sampleのみを公開し、接触後のinteger-tick stateを呼出し側へ返さない。
FlightRecordとReplayのactuatorもこの保持規則を使用する。exact sample時刻は保存値、
sample間の内点はその区間の終端sampleに保存された保持値を返す。

このactuator modelは静的舵角限界とrate limitを表す。独立した遅延・一次lagを追加する場合は、
遅延bufferとその初期状態をFlightRecordへ含める契約および統合収束試験を同時に定義する。

## エラーと検証

BPG-040の専用二系統境界は`TailPilotIntent`、`TailRateTarget`、`TailControlProfile`とする。
manual nose-up/right intentは各[-1,1]で、水平・垂直尾翼のphysical incidenceへ各-0.2 rad倍で写像する。
body q/r targetは各±0.2 rad/sである。`tail_rate_feedback_incidence`は
gain×(observed-target)を各±0.2 radで飽和する。observed roll rateを操作へ使用しない。
software profileの初期設定はq/r gain各0.2 s、slew各1 rad/sとし、airframe/polar parameterから分離する。
`advance_tail_control`は既存ControlModeのauthority混合、physical saturation、slewを純粋に評価し、
混合targetと次区間の保持incidenceを返す。pilot位置指令はこのmixerの対象外である。
既存generic三軸APIを保持する。この単位は二系統制御primitiveとHybrid荷重の符号を検証する。
新mock機体とWASM・input・record・既定モデル切替は後続単位で実施する。

`TailPilotPositionCommand`は新しいnormalized inputと`Hold`を排他的に表す。
`TailPilotPositionMapping`は[-1,0,1]を[-0.4,trim,0.4] mへ区分線形で写像し、出力を±0.4 mに制限する。
新flightの保持targetはtrimで初期化する。中立inputはtrimへ写像し、input欠損・機器切断の`Hold`は直前targetを維持する。
この位置commandはControlMode・舵authorityから独立する。物理travel・速度・加速度制約は既存pilot policyが検証する。

`TailFlightTickState`はbody/pilot state、二系統incidence、保持pilot target、整数tickの単一正本である。
`advance_tail_flight_tick`は直前成功stateのq/rから制御を一度だけ評価し、更新incidenceを全RK4 stageへ保持する。
Hybrid専用load adapterはphysical incidenceを直接評価する。旧roll枠・legacy actuator stateへ写像しない。
pilot target policy・moving mass・積分器・contact検索/slerpは既存処理を共有する。
`advance_tail_flight_tick_with_contact`は着水時にfractional terminal sampleだけを返す。
fraction 0では直前incidence/target、正fractionでは新incidence/targetを保持し、body/pilot stateは同時刻へ補間する。
制御・pilot policy・荷重stage・contactの失敗時は部分stateをcommitしない。
`advance_tail_flight_tick_with_contact_report`は同一評価の`TailControlCommands`と入力を成功outcomeへ付属させる。
normalized manual intent、body q/r target、manual/FBW/mixed incidence targetを型とaccessorで区別する。
physical incidenceと保持pilot targetはoutcomeのstate/sampleを参照し、record側で制御を再計算しない。
fraction 0のreportは未適用の新controlを保持せず、正fractionと整数tickはその区間の適用controlを返す。
`TailFlightScenarioParameters`は共通のComposite-CG launch/contact検証から二系統のtick-zero stateを生成する。
`TailFlightScenario`はborrowed Hybrid load/wind、pilot trim、contact、courseを固定し、software profileを機体から分離する。
telemetry式とdistance scoreは旧scenarioと共有する。`run`はTimeLimit/WaterContactで同時刻のstateとscoreを返す。
失敗時は直前成功の二系統stateと元のcause/stageを返し、そのtickをcommitしない。
新mock機体の公開WASM/input/record/default切替はBPG-042の後続範囲である。

GameSessionは既存lifecycleを共有し、scenarioと同型のactive stateをlegacy/tailの排他的engineへ保持する。
hybrid tickの成功reportをFlightRecordへ渡し、記録・telemetryの成功後に整数tickを公開する。
失敗stageのstate・incidence・pilot target・inputを公開せず、直前成功stateをResult/recordへ確定する。
Resultは元のtick errorを保持する。空力envelopeと有限windのOutsideGridはOutOfValidEnvelope、
非有限値・算術・policy等の失敗はFatalSimulationErrorとして区別する。
FlightRecordのfinalizationにも同じ型付きerrorを保存し、失敗tickのsampleを追加しない。
旧三軸snapshot ABIはtail payloadを型付きerrorで拒否し、余剰rollを生成しない。
新既定モデル・新ABI・record schemaの公開切替はBPG-042の同一PRで結合する。
WASMのadditive Rust入口`HybridSessionPreparation`はmock定義とsurfaceを固定owned cacheへ保持し、
既存owned環境6のwindを借用したscenarioをGameSessionへ渡す。自己参照とleaked storageを使用しない。
trimのair-relative速度へCG位置のwindを一度加算してlaunch ground速度とし、同windをtelemetryへ使用する。
model/controllerの文字列identityはRust定義からrecordへ渡し、UI側で生成しない。旧JS factoryの既定モデルは保持する。

明示的な`HybridGameSessionBridge`はschema 2のJSON境界を提供する。`control_layout=tail_incidence`を必須とし、
入力は`nose_up`/`turn_right`、body-positiveの`desired_pitch_rate_rad_s`/`desired_yaw_rate_rad_s`、
`pilot_position_command`の`hold`/`set`を受け取る。余剰axis・未知field・異なるschemaを拒否する。
snapshotの`frame`は`menu`/`flight`/`result`の排他型である。physical incidence、身体状態、CG telemetryと
terminal finalization/causeを同じRust stateから投影する。seedはlow/highの32bit値で正確に受け渡す。
旧factory・33値ABI・公開既定モデルは維持し、Replay/Attract・公開切替は後続のBPG-042統合で接続する。
hybridのSetupは既存`DifficultySettings`とcatalog 2を使用する。Calm/Mild/Challenging/NearLimitは
登録済みuniform provider 1/2/4/5、Typicalはoffline asset 6のgridを使用し、環境metadataと物理のproviderを一致させる。
Informationは表示だけに作用し、Assistanceは既存Strong/Assisted/Light/Manualをauthority 1/0.5/0.2/0へ解決する。
Briefing開始後は選択を固定し、18値のconfiguration metadataとmodel/controller identityをRustから供給する。
snapshotはTitle/FlightSetupで両identityを`null`、BriefingPreparing/BriefingFailed/BriefingReady/Countdown以降で両identityを必須とする。
Setupの候補は別environment projectionの`selected`で示し、準備失敗・再準備・Countdown取消でもsealed identityを保持する。
record/PBには同じ成功したpreparationのprofile・course・difficultyを渡す。旧三軸feedback profileを生成しない。
`control_profile_json()`はsealed controller ID/versionとpitch/yawのrate上限・feedback gain・slewを供給する。
device adapterは型付き上限へnormalized demandを写像し、TSに物理定数を定義しない。explicit q/r入力は同一ABIを維持する。
live stateの`pilot_position_target_normalized`は、同じsealed mappingで現在のheld physical targetを逆写像した値である。
trimを0、両端を±1とし、trimと端点が一致する場合は同じphysical targetを0へ正規化する。
初期化・Resume・入力機器の再取得はこの値を参照し、現在のphysical positionや初期positionから目標を推定しない。
`Hold`は直前目標を保持する。旧保存queryへnormalized targetを追加せず、記録値とlive入力状態を区別する。
Flight/Pausedの`frame.progress_m`は`course_parallel_m`・`cross_track_m`・`net_horizontal_m`をRustから供給する。
sealed initial datumと最新成功datumの変位へdistance score v1の幾何計算を適用し、CG移動・累積経路長と区別する。
進行値は未確定の診断値であり、Resultは同時刻の既存`finalization.score_m`を保持する。保存queryへcursor scoreを追加しない。
Result/Replayは`flight_analysis_samples_json()`と`flight_record_sample_at_seconds()`を同じRust保存recordへ接続する。
queryのschema 2は保存physics/telemetryと`controls.layout`による`legacy_three_axis`/`tail_incidence`の排他値を返す。
Replayの時刻・再生速度・再生可否は既存`GameSession`のclockを使用し、グラフと描画が同じsecondsをqueryする。
`playback_context_json()`はnamed phase `replay`/`attract`、保存scenario/control identity、difficulty、layout別finalization/causeを返す。
live `snapshot_json()`はReplayをtyped拒否し、Replay表示は専用context/queryを使用する。保存v1–5を二系統へ読み替えない。
`export_flight_record_json()`は同じsealed metadataからschema 6とcanonical PB keyを生成する。archiveはsnapshot閲覧へ限定する。
`TailPersonalBestSelectionBridge`は既存Rust比較を使用し、旧layoutやmodel/controller/key不一致を比較対象から除外する。
`enter_attract()`はTitleから独立した有界demo recordを一度生成・保持し、再入時は同recordのclockだけを初期化する。
demoは登録Calm環境・Automatic制御の架空hybridモデルを使用し、playerのdifficulty・seed・recordへ書き込まない。
Attractの描画・分析は保存queryを使用し、live snapshot・player record exportを拒否する。`leave_attract()`はTitleへ戻る。
`flight_record_summary_json()`はResult/Replay/Attractの同recordからRust集計・physics Hz・保存contextを返す。
最大迎角と確定scoreは理由付き`available`/`unavailable`へ写像し、cursor進行値へ置換しない。
`flight_wind_grid_json(north_min_m, east_min_m, altitude_m, spacing_m)`は登録済み環境を5×5点でqueryする。
未知identityと登録領域外は理由付き`unavailable`とし、部分gridや無風への代替値を生成しない。

非有限command、authority範囲外、無効なactuator limit、無効timestep、travel範囲外のstateは型付きerrorとする。
途中まで進めたactuator stateを公開しない。混合結果と更新結果の決定性を保証する。

検証ではManual/Automaticの端点、Sharedの各axis混合、飽和、rate limit、境界値、拒否された入力後の
入力state不変を確認する。body-rate feedbackはaxis符号、飽和、極端な有限rate、および合成roll momentを用いた
閉ループ減衰で検証する。閉ループcontrollerの安定性・通常操縦での飛行成立性は、
合成係数を使った100 Hz/200 Hzのstep-halving連成試験で数値収束を確認する。
この試験は、実機の安定性・通常操縦の成立性を保証しない。aircraft-specific controllerと
aerodynamic derivativesのfidelity検証はBPG-035でM6完了後に扱い、M3〜M6をblockしない。

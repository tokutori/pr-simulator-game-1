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

TailPilotIntentはnormalized nose-up/right-turn、TailRateTargetはbody q/r、TailIncidenceは水平・垂直尾翼のphysical angleを保持する。
ControlModeはManual、Shared(authority)、Automaticの直和型とし、authorityは有限な$[0,1]$へ限定する。
slew更新はcoreの固定tickに従う。整数tick $k$ のstateは前区間の保持値 $\delta_k$、
入力 $k$ による更新値 $\delta_{k+1}$ は区間 $(k,k+1]$ の全RK4 stageへ保持する。
正のcontact fractionは更新後incidenceを保存し、fraction=0は直前incidenceを保存する。
Replayのexact sampleは保存値、区間内点は終端側sampleの保持incidenceを返す。
pilot target policyは身体の独立した有限停止条件を検証する。詳細は[pilot-motion](pilot-motion.md)へ従う。

## エラーと検証

BPG-040の専用二系統境界は`TailPilotIntent`、`TailRateTarget`、`TailControlProfile`とする。
manual nose-up/right intentは各[-1,1]で、水平・垂直尾翼のphysical incidenceへ各-0.2 rad倍で写像する。
body q/r targetは各±0.2 rad/sである。`tail_rate_feedback_incidence`は
gain×(observed-target)を各±0.2 radで飽和する。observed roll rateを操作へ使用しない。
software profileの初期設定はq/r gain各0.2 s、slew各1 rad/sとし、airframe/polar parameterから分離する。
`advance_tail_control`は既存ControlModeのauthority混合、physical saturation、slewを純粋に評価し、
混合targetと次区間の保持incidenceを返す。pilot位置指令はこのmixerの対象外である。
二系統制御primitiveとHybrid荷重の符号を検証する。
新mockのWASM・input・record・既定モデルはBPG-042のアプリ統合で同じ二系統契約へ接続する。

### 局所迎角差と尾翼角の合成範囲保護

[#262](https://github.com/tokutori/pr-simulator-game-1/issues/262)の保護は二系統の共通Rust tickへ配置し、
Manual・Shared・Automaticへ同じ規則を適用する。独立した尾翼角飽和は局所迎角差との合計を保証しないため、
既存hybrid荷重が定義する各proxyの`ControlledAlphaDifference`を参照する。

```math
I(s)=[-0.2,0.2]\cap\bigcap_i[-0.2-\Delta\alpha_i,0.2-\Delta\alpha_i]
```

初めに従来のauthority・slewで生成した保持incidenceを、同じ固定tickの全RK4 stageと公開stateで検査する。
公開stateは非接触時のweighted endpoint、接触時の同時刻terminal stateである。
成功した状態はそのまま採用する。尾翼の合成角制限が失敗した場合は、現在stateの許容区間と
actuatorの`previous ± maximum_slew × PHYSICS_DT_SECONDS`の可到達区間を交差する。
各軸の投影要求・前tick incidenceの投影値・中央・下限・上限による最大25組を追加検査し、最初の成功結果だけを公開する。
controller、pilot目標、pilot加速度は一度だけ導出し、候補間で変更しない。
候補ごとにincidenceを全stageへ保持し、時間刻み、物理式、空力係数、適用範囲を変更しない。

これは有限候補による一tickの検査である。連続区間の網羅、将来の継続安全、失速保護、姿勢・迎角の保持を保証しない。
`StaticAlpha`、局所速度、風の領域外など、尾翼合成角から独立した失敗は元の型付きerrorを保持する。
候補が得られない場合は名目指令の元cause/site/stageで終了し、最後の有効stateとrecordを維持する。
endpointの直接評価はstageを`None`とし、第四stageと混同しない。

要求intent・manual/feedback/mixed targetは従来のcontrol reportへ保存する。
保護後の実incidenceはphysical sampleへ保存する。現行controller version 3にこの保護を含め、Personal Bestの比較keyもversionを保持する。

`TailPilotPositionCommand`は新しいnormalized inputと`Hold`を排他的に表す。
`TailPilotPositionMapping`は[-1,0,1]を[-0.4,trim,0.4] mへ区分線形で写像し、出力を±0.4 mに制限する。
新flightの保持targetはtrimで初期化する。中立inputはtrimへ写像し、input欠損・機器切断の`Hold`は直前targetを維持する。
この位置commandはControlMode・舵authorityから独立する。物理travel・速度・加速度制約は既存pilot policyが検証する。

`TailFlightTickState`はbody/pilot state、二系統incidence、保持pilot target、整数tickの単一正本である。
`advance_tail_flight_tick`は直前成功stateのq/rから制御を一度だけ評価し、更新incidenceを全RK4 stageへ保持する。
Hybrid専用load adapterはphysical incidenceを直接評価する。TailIncidenceをそのまま使用する。
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
telemetry式とdistance scoreは共有coreから導出する。`run`はTimeLimit/WaterContactで同時刻のstateとscoreを返す。
失敗時は直前成功の二系統stateと元のcause/stageを返し、そのtickをcommitしない。
新mock機体のWASM/input/record/defaultはBPG-042で一体として接続する。

GameSessionは既存lifecycleを共有し、TailFlightScenarioと同型のTailFlightTickStateをactive stateへ保持する。
hybrid tickの成功reportをFlightRecordへ渡し、記録・telemetryの成功後に整数tickを公開する。
失敗stageのstate・incidence・pilot target・inputを公開せず、直前成功stateをResult/recordへ確定する。
Resultは元のtick errorを保持する。空力envelopeと有限windのOutsideGridはOutOfValidEnvelope、
非有限値・算術・policy等の失敗はFatalSimulationErrorとして区別する。
FlightRecordのfinalizationにも同じ型付きerrorを保存し、失敗tickのsampleを追加しない。
新既定モデル・新ABI・record schemaの公開切替はBPG-042の同一PRで結合する。
WASMのadditive Rust入口`HybridSessionPreparation`はmock定義とsurfaceを固定owned cacheへ保持し、
既存owned環境6のwindを借用したscenarioをGameSessionへ渡す。自己参照とleaked storageを使用しない。
trimのair-relative速度へCG位置のwindを一度加算してlaunch ground速度とし、同windをtelemetryへ使用する。
model/controllerの文字列identityはRust定義からrecordへ渡し、UI側で生成しない。
アプリ既定は`bpg041-playable-hybrid-mock`/version 2と`bpg040-tail-rate-feedback`/version 3である。
modelのPWL列・geometry・trimと、全modeのsoft alpha/身体target保護は[playableモデル契約](playable-hybrid-model.md)に従う。

明示的な`HybridGameSessionBridge`はschema 2のJSON境界を提供する。`control_layout=tail_incidence`を必須とし、
入力は`nose_up`/`turn_right`、body-positiveの`desired_pitch_rate_rad_s`/`desired_yaw_rate_rad_s`、
`pilot_position_command`の`hold`/`set`を受け取る。余剰axis・未知field・異なるschemaを拒否する。
snapshotの`frame`は`menu`/`flight`/`result`の排他型である。physical incidence、身体状態、CG telemetryと
terminal finalization/causeを同じRust stateから投影する。seedはlow/highの32bit値で正確に受け渡す。
既定アプリのReplay/Attractは同じbridgeのnamed保存queryを使用する。
hybridのSetupは既存`DifficultySettings`とcatalog 3を使用する。Calm/Mild/Challenging/NearLimitは
登録済みuniform provider 1/2/4/5、Typicalはoffline asset 6のgridを使用し、環境metadataと物理のproviderを一致させる。
Informationは表示だけに作用し、Assistanceは既存Strong/Assisted/Light/Manualをauthority 1/0.5/0.2/0へ解決する。
Briefing開始後は選択を固定し、18値のconfiguration metadataとmodel/controller identityをRustから供給する。
snapshotはTitle/FlightSetupで両identityを`null`、BriefingPreparing/BriefingFailed/BriefingReady/Countdown以降で両identityを必須とする。
Setupの候補は別environment projectionの`selected`で示し、準備失敗・再準備・Countdown取消でもsealed identityを保持する。
record/PBには同じ成功したpreparationのprofile・course・difficultyを渡す。同じ二系統profileを使用する。
`control_profile_json()`はsealed controller ID/versionとpitch/yawのrate上限・feedback gain・slewを供給する。
device adapterは型付き上限へnormalized demandを写像し、TSに物理定数を定義しない。explicit q/r入力は同一ABIを維持する。
live stateの`pilot_position_target_normalized`は、同じsealed mappingで現在のheld physical targetを逆写像した値である。
trimを0、両端を±1とし、trimと端点が一致する場合は同じphysical targetを0へ正規化する。
初期化・Resume・入力機器の再取得はこの値を参照し、現在のphysical positionや初期positionから目標を推定しない。
`Hold`は直前effective targetを保持した名目要求であり、controller 3のalpha保護は危険側biasをさらにtrim側へ絞り得る。
元Hold/Set要求とresolved effective targetを区別する。保存記録値とlive入力状態を区別する。
Flight/Pausedの`frame.progress_m`は`course_parallel_m`・`cross_track_m`・`net_horizontal_m`をRustから供給する。
sealed initial datumと最新成功datumの変位へdistance score v1の幾何計算を適用し、CG移動・累積経路長と区別する。
進行値は未確定の診断値であり、Resultは同時刻の既存`finalization.score_m`を保持する。保存queryへcursor scoreを追加しない。
Result/Replayは`flight_analysis_samples_json()`と`flight_record_sample_at_seconds()`を同じRust保存recordへ接続する。
queryのschema 2は保存physics/telemetryと`controls.layout`による`tail_incidence`のphysical値を返す。
Replayの時刻・再生速度・再生可否は既存`GameSession`のclockを使用し、グラフと描画が同じsecondsをqueryする。
`playback_context_json()`はnamed phase `replay`/`attract`、保存scenario/control identity、difficulty、二系統finalization/causeを返す。
live `snapshot_json()`はReplayをtyped拒否し、Replay表示は専用context/queryを使用する。未対応schemaを拒否する。
`export_flight_record_json()`は同じsealed metadataからschema 6とcanonical PB keyを生成する。archiveはsnapshot閲覧へ限定する。
`TailPersonalBestSelectionBridge`は既存Rust比較を使用し、未対応schemaとmodel/controller/key不一致を比較対象から除外する。
`enter_attract()`はTitleから独立した有界demo recordを一度生成・保持し、再入時は同recordのclockだけを初期化する。
demoは登録Calm環境・Automatic制御の架空hybridモデルを使用し、playerのdifficulty・seed・recordへ書き込まない。
Attractの描画・分析は保存queryを使用し、live snapshot・player record exportを拒否する。`leave_attract()`はTitleへ戻る。
`flight_record_summary_json()`はResult/Replay/Attractの同recordからRust集計・physics Hz・保存contextを返す。
最大迎角と確定scoreは理由付き`available`/`unavailable`へ写像し、cursor進行値へ置換しない。
`flight_wind_grid_json(north_min_m, east_min_m, altitude_m, spacing_m)`は登録済み環境を5×5点でqueryする。
未知identityと登録領域外は理由付き`unavailable`とし、部分gridや無風への代替値を生成しない。

Webの`tail-session-codec`はschema 2と`tail_incidence`を検査し、phaseとframeを結合したimmutableな直和型へ変換する。
physical incidenceは水平・垂直尾翼の二値、body角速度はroll/pitch/yawの三値として保持する。
terminalのfraction・stamp・typed causeとRust由来identityを保存し、このcodecから共通表示snapshotを導出する。
二系統device adapterはArrowUp/Downをnose-up/down、ArrowRight/Leftをright/left turn、J/Lをnormalized身体指令に対応付ける。
Gamepadは明示したnose-up・right-turn・身体軸だけを読む。キーボード解放はHold、Gamepadの身体軸はSetとする。
身体の物理target・FBW出力・gain・slewはRustに保持し、Webは呼出し側から受け取ったexplicit q/r demandを変更せず渡す。

`parseTailControlProfile`はRustのsealed controller ID/version・pitch/yaw rate limit・gain・slew metadataをstrictに検査する。
`tailInputFromControlProfile`は同じsealed scenario/aircraft/controller/seedのsnapshotと照合し、normalized demandをRust所有rate limitへ写像する。
gainとslewはmetadataとして保持する。FBWの評価とsoftware actuator更新はRust coreが所有し、Web側には追加のcontrol loopを設けない。

`BrowserTailPilotInput`は毎tickのRust held targetを使用し、Gamepad再取得時はneutralと身体軸のpickupを確認する。
身体軸の再取得前とキーボード解放時は`Hold`を送り、現在のphysical positionや中立値で目標を置換しない。
`TailSessionPort`と`TailFlightController`はnamed JSONとsealed profileを介して入力・fixed tick・共通表示snapshotを接続する。
発進原点・発進台寸法・方位は`assets/biwa-launch-venue.json`をRust session、Web、nativeで共用する。
hybrid scenario version 3は北から時計回り315°を初期headingとcourse axisへ適用する。
NEDの環境風は回転せず、同じCG地点の風をtrimの空気相対速度へ一度加算する。
reset・snapshot同期・描画の失敗時は入力とclockを停止し、再同期の成功前にtickを再開しない。
既定アプリはこの二系統port/controllerを使用し、入力・HUD・Resultへ同じsnapshotを渡す。
`TailAppSessionFacade`は単一ownerとしてWASM resourceを排他的に所有し、命令・snapshot・保存queryを既存境界へ委譲する。
facade内にdomain phaseや物理状態の独立した正本を保持しない。TailFlightControllerも同じresource ownerのportを使用する。
非同期queryは観測開始時のopaque owner/query generation tokenを保持し、取得途中の変更後に届いた結果を拒否する。
Analysis datasetのproofは別のrecord source generationを保持し、Retry・設定変更・archive開閉・disposeで失効する。
同recordのseek・再生速度・再生可否・clock更新ではdatasetを維持し、cursor queryだけを実行する。
scenario/seedが同一でもresource世代が異なる結果は受理しない。
`session-factory`はlayoutを明示した構成からWASM resourceを生成し、単一のfacade ownerへ渡す。
新factoryはraw bridgeを公開せず、初回projectionに失敗したresourceを解放する。
`archived-personal-best`はschema 6をRust Tail selectorへ接続する。
未対応schemaの保存bytesは変更せず、比較から除外する。schema 6の破損とkey・適格性の検証はRust selectorへ委譲する。
既定アプリのmain・入力・HUD・Result・保存・Replayは同じtail facadeを使用する。
Scene退出とBFCache退避ではcontrollerを停止し、非復帰teardownとowner置換でresourceを最終解放する。
AppModelのFlight/Pausedはcontrol layout・Rust phase・共通snapshotを相関した直和型で保持する。
Resultの保存snapshotはfinalizationと同じ終端stampだけを受理し、Replay/Attractのcursorとは分離する。
record未取得は理由付きavailabilityで保持する。
adapter停止ではcontroller世代とlayoutを照合し、Pause同期前の最後の有効snapshotも保持する。Rust domain phaseは変更しない。
Attractは専用named contextとRust-owned record/clockを通じてfacadeへ接続する。
Titleのidle/attract表示値はRust phaseと同demo contextから導出し、Attractのlive snapshotやplayer record exportへ迂回しない。
enter/leaveはrecord sourceの世代を更新し、同demoへ再入した場合も前回の非同期結果を拒否する。
再生操作はquery generationを更新し、同recordのdataset proofを維持する。
共通表示の`progressMeters`はFlight/PausedでRustのcourse/cross-track/netを保持し、Resultの確定scoreと区別する。
保存queryの不足理由は`unavailable`へ保持し、保存cursorの距離を再計算しない。

`named-record-query`はschema 2の保存query・Analysis batch・Replay/Attract contextをimmutableな型へ変換する。
schema 6の二系統尾翼をphysical controlsとして保持する。
同sessionのRust terminal projectionとphysics frequencyを使用し、layout・stamp・初期/終端・時系列・clockの整合を検査する。
queryは保存snapshotとRust-owned Replay clockを参照し、再simulation・物理/score/controllerの再評価を行わない。
queryにrecord identityが含まれないため、非同期の古い応答は呼出し側のsession generation/request IDで拒否する。
`named-record-analysis`はRust集計のSummaryと固定5×5 Wind queryをfacadeへ接続する。
Result/Replay/Attractのphase・保存identity・元finalizationを照合し、最大迎角とscoreの不足理由をavailability tagとして保持する。
風は同recordの登録providerだけを参照し、全25点のfinite値・北row/東column順序・request echo・sourceを検査する。
未知環境と登録領域外は個別の`unavailable`を保持し、欠損風速や集計値を数値で補填しない。
Web側で物理・集計・scoreを再評価せず、確定Summaryと保存cursorを分離する。

非有限command、authority範囲外、無効なactuator limit、無効timestep、travel範囲外のstateは型付きerrorとする。
途中まで進めたactuator stateを公開しない。混合結果と更新結果の決定性を保証する。

検証ではManual/Automaticの端点、Sharedの各axis混合、飽和、rate limit、境界値、拒否された入力後の
入力state不変を確認する。body-rate feedbackはaxis符号、飽和、極端な有限rate、および合成roll momentを用いた
閉ループ減衰で検証する。数値積分と離散controlの刻み依存は、[量別step-halving](verification.md)で
physics-onlyと連成の100/200/400 Hz比較へ分離する。局所増大・減衰は[離散線形化](hybrid-numerical-validation.md)で
Jacobian/固有値と小摂動時系列を照合する。過去のmodel 1の局所増大観測は履歴値として区別する。
現行Playableの明示的な入力列の受入と局所線形化は個別に検証し、全状態・任意入力や実機の安定性を保証しない。aircraft-specific controllerと
aerodynamic derivativesのfidelity検証はBPG-035でM6完了後に扱い、M3〜M6をblockしない。

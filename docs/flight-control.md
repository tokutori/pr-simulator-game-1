# 飛行制御とパイロットの運動

本書は二系統尾翼の指令、FBW、actuator、パイロットの移動質量、身体目標、入力機器の境界を定義する。
座標系・積分・空力・接触は[飛行力学](flight-dynamics.md)、保存・queryは[データ形式](data-and-formats.md)、UIと利用者向け操作は[ゲームとUI](gameplay-and-ui.md)に従う。

## 状態の所有と二系統入力

- 制御、authority混合、actuator、身体の位置・速度・保持目標は共有Rust coreの決定的な状態とする。
- WebとNativeの入力adapterは機器の値を機器非依存のintentへ変換し、整数tickへ割り当てる。
- FBWは直前成功tickのbody角速度を観測する。
- adapterに追加の制御ループや物理定数を置かない。
- DOM、Gamepad API、OSイベント、描画はこのcore境界に含めない。

| 型 | 意味と範囲 |
|---|---|
| `TailPilotIntent` | normalized nose-up/right-turn。各閉区間[-1,1] |
| `TailRateTarget` | body正方向のpitch q / yaw r目標。各閉区間±0.2 rad/s |
| `TailIncidence` | 水平・垂直尾翼のphysical effective incidence。各閉区間±0.2 rad |
| `TailPilotPositionCommand` | 身体位置の`Set`要求または`Hold` |
| `ControlMode` | Manual、Shared（authority）、Automaticの直和型 |
| `TailFlightTickState` | body/pilot state、二系統incidence、保持pilot target、整数tickの単一正本 |

- 独立したroll入力は持たない。
- manualの正nose-up/right-turnは、それぞれ水平・垂直尾翼のphysical incidenceへ-0.2 rad倍で写像する。
- observed roll rateを操作量に使わない。

FBW authorityは舵指令だけに適用する。Manualはpilot指令、Automaticはcontroller指令を選び、Sharedは有限な $a\in[0,1]$ で線型混合する。

```math
u=(1-a)u_{pilot}+a u_{FBW}
```

- Informationは表示だけへ作用する。
- AssistanceのStrong / Assisted / Light / Manualはauthority 1 / 0.5 / 0.2 / 0へ解決する。
- 既定の支援はLightである。
- 実際に解決した`ControlMode::Shared`のauthorityを保持し、保存後にAssistanceから再解決しない。
- 身体の位置目標は舵authorityと独立であり、Automaticでも入力できる。
- 未入力時は保持中の目標を使用する。

## FBWとactuator

- `tail_rate_feedback_incidence`は各軸の`gain × (observed - target)`を±0.2 radへ飽和する。
- `TailControlProfile`のq/r gainは各0.2 s、最大slewは各1 rad/sとし、機体の空力係数から分離する。
- `advance_tail_control`はauthority混合、physical saturation、slewを純粋に評価し、混合要求と次区間の保持incidenceを返す。

- 整数tick $k$ のstateは前区間の保持値 $\delta_k$ を持つ。
- 入力 $k$ による更新値 $\delta_{k+1}$ は区間 $(k,k+1]$ の全RK4 stageへ保持する。
- 正の接触fractionでは更新incidenceを保存し、fraction 0では直前incidenceを保持する。
- Replayのexact sampleは保存値、区間内点は終端側sampleの保持incidenceを返す。
- 入力から連続したactuator軌跡を推測しない。

### 局所迎角差と尾翼角の合成範囲

Manual・Shared・Automaticの全てへ、同じRust tick内で保護を適用する。
hybridの各proxyが持つ局所迎角差 $\Delta\alpha_i$ と尾翼角の合計を検査する。

```math
I(s)=[-0.2,0.2]\cap\bigcap_i[-0.2-\Delta\alpha_i,0.2-\Delta\alpha_i]
```

- 最初に通常のauthority・slewで生成した保持incidenceを、全RK4 stageと公開stateで評価する。
- 公開stateは非接触時のweighted endpoint、接触時の同時刻terminal stateである。
- 成功結果は変更せず採用する。

- 合成角制限に失敗した場合は、現在状態の許容区間と`previous ± maximum_slew × PHYSICS_DT_SECONDS`の可到達区間を交差する。
- 各軸の投影要求、前tick incidenceの投影値、中央、下限、上限から最大25組を追加検査し、最初に成功した結果だけを公開する。
- controller、身体目標、身体加速度は一度だけ導出して候補間で固定する。
- incidenceを候補ごとの全stageへ保持し、刻み・物理式・係数・適用範囲を変更しない。

- この探索は有限候補による1 tickの検査である。
- 連続区間の網羅、将来の継続安全、失速保護、姿勢・迎角の保持は保証しない。
- `StaticAlpha`、局所速度、風の領域外等の独立した失敗は元の型付きエラーを保持する。
- 候補が得られない場合は名目指令の元cause/site/stageで終了し、最後の有効stateとrecordを維持する。
- endpointの直接評価はstageを`None`とし、Fourth stageと区別する。

### 迎角と身体目標のsoft保護

- `TailAngleOfAttackGuard`は検証済みprofileに保持するoptional policyである。
- 公開Playableの設定はdatum alpha運用帯[-0.09,+0.09] rad、preview 1.0 s、pitch gain 1.6 sとし、基準alphaはtrimのdatum alphaである。
- Manual・Shared・Automaticに同じpolicyを適用し、元のauthority、q/r gain、slewを維持する。
- これらは合成controllerの設計値であり、hybridのhard envelopeを拡大しない。

- guardはdatum $O$ で同じ風場を評価し、対地速度から風を引いてbodyへ変換した速度の`atan2(down, forward)`を使う。
- HUD・record telemetryが示す合成重心 $G$ のalphaとは区別する。
- 風・数学評価が不能、またはdatum alphaが未定義の場合は予測を適用せず、実RK荷重の元の型付きエラーを保持する。

混合要求を一度だけ評価し、現在のdatum alphaから許容q帯を求める。

```math
q_{lo}=\frac{-0.09-\alpha_O}{1.0},\qquad q_{hi}=\frac{0.09-\alpha_O}{1.0}
```

physical elevator要求を、このq帯に対応する`gain × (observed q - allowed q)`のincidence区間へ投影して既存slewを適用する。
rudder要求はこのalpha policyで変更しない。

- 身体目標は初期trim位置を基準とし、危険側のbiasだけを連続的に絞る。
- 後方biasはnose-up・alpha増加側、前方biasはnose-down・alpha減少側である。
- 上側riskは`alpha + preview × max(q, 0)`、下側riskは`alpha + preview × min(q, 0)`とする。
- trim alphaから各運用端までのheadroomの後半で、残すbias係数を1から0へ下げる。
- 運用端ではtrim targetへ戻す。
- 反対側の身体目標は変更せず、固定normalized capを追加しない。

- effective targetへの移動は身体の加速度policyとRK4で行い、身体stateを直接変更しない。
- tick stateはguard投影前の要求位置と、投影後のeffective targetを別々に保持する。
- 次tickの解決は保持した要求位置を基準とし、guard候補の射影値を新しい進行量の上限として使う。
- `Set`の元normalized要求、`Hold`、manual intent、desired rate、manual/FBW/mixed要求をrecordに保存する。
- 実stateと`resolved_pilot_position_target_m`は採用したeffective targetを保持する。
- `Hold`は保持した要求位置と、認可済みの保護停止を継続する。
- 保護による進行上限が実速度の停止可能位置より手前になる場合は、有限停止certificateが返す停止位置まで制動して保持する。
- 停止保持は同じ長押しのSetやHoldで継続し、入力ownerが認可した新しい操作で再評価する。J/Lはfresh press、gamepadは既neutral・pickupで取得した操作を認可する。保護停止後のgamepadは停止位置のpickupを経て再取得する。
- 逆入力は現在の制動を完了してから新しい方向へ進む。位置・速度は保持加速度の運動で更新する。
- 飛行中の目標表示は保持需要とし、保護中は「保護制動」「保護停止」を示す。Replayは記録された適用目標を表示する。

- previewはalpha変化をqから近似する。
- 空間風・flight-pathの連成、独立したyaw域、任意の将来入力に対する安全は保証しない。
- 保護後も全RK4 stageと公開endpointの元の荷重検査を適用し、失敗時はlast valid stateと元のsite/cause/limit/stageを保持する。

## 移動質量の力学

- パイロットの身体移動は機体前後方向の1自由度とする。
- パイロットを除く機体の固定重心をdatum $O$ とし、主翼・尾翼・接触点・眼位置の基準をdatumから保持する。
- 機体は剛体、パイロットは機体に対して回転しない質点である。
- 相対位置 $x_p$、速度 $\dot x_p$、加速度 $\ddot x_p$ を扱い、質量・固定高さ・範囲・最大速度・最大加速度・初期状態は検証済み`AircraftModel`へ含める。

機体質量 $m_a$、パイロット質量 $m_p$、固定高さ $z_p$、位置 $r_p^B=(x_p,0,z_p)$ から合成重心を導出する。

```math
M=m_a+m_p,\qquad r_G^B=\frac{m_p r_p^B}{M}
```

datumまわりの機体固定慣性を $I_a$、$d_a=-r_G^B$、$d_p=r_p^B-r_G^B$ とし、平行軸の定理で合成慣性を求める。

```math
I_G=I_a+m_a\left(\lVert d_a\rVert^2 I_3-d_a d_a^T\right)
       +m_p\left(\lVert d_p\rVert^2 I_3-d_p d_p^T\right)
```

固定点 $i$ は $r_{Oi}^B$ を保持する。積分状態はdatumの位置・対地速度、姿勢・角速度、$x_p,\dot x_p$ とする。

```math
p_i^N=p_O^N+R_{NB}r_{Oi}^B,\qquad p_G^N=p_O^N+R_{NB}r_G^B
```

```math
v_i^N=v_O^N+R_{NB}(\omega^B\times r_{Oi}^B),\qquad
v_G^N=v_O^N+R_{NB}(\omega^B\times r_G^B+\dot r_G^B)
```

- これらは点の運動学的関係であり、移動後の重心へ固定形状の原点を追従させない。
- 合成重心位置・対地速度を指定する発進条件は、身体状態・姿勢・角速度からdatumへ逆変換し、身体範囲を検証する。
- 対地速度の入力に風を暗黙加算しない。

### 運動量収支

$v_O^B=R_{NB}^Tv_O^N$、$\dot r_p^B=(\dot x_p,0,0)$ とし、内部質量の相対運動を運動量へ含める。

```math
P^B=Mv_O^B+m_p(\omega^B\times r_p^B+\dot r_p^B)
```

```math
H_O^B=I_a\omega^B+m_p r_p^B\times(v_O^B+\omega^B\times r_p^B+\dot r_p^B)
```

外力 $F^B$ とdatumまわりの外部モーメント $M_O^B$ は、重力を含む全質量要素・作用点から評価する。
移動原点の角運動量収支には原点輸送項を含める。

```math
\dot P^B+\omega^B\times P^B=F^B,\qquad
\dot H_O^B+\omega^B\times H_O^B=M_O^B-v_O^B\times P^B
```

- 各RK4 stageで $x_p,\dot x_p,\ddot x_p$ を評価し、収支を連成して並進・角加速度を解く。
- 内部駆動力を外力へ重複計上せず、重心と慣性だけを更新する準静的近似を使用しない。
- 各stageは質量・慣性・位置・速度・加速度の有限性と、連成行列の数値的可逆性を検査する。
- 失敗時は型付きエラーだけを返し、部分更新を公開しない。
- 内部駆動がエネルギーを投入できるため、身体移動中の機械的エネルギー保存は要求しない。

$m_p=0$、身体静止、無外力時の全運動量・角運動量、一般3次元姿勢の連成を検査する。
[縦運動の参照式](https://github.com/tokutori/simulator-prototype-1/issues/1)は平面運動の資料であり、一般6DoFへの適用は上記の収支で個別に検証する。

## 身体の目標位置と停止policy

- `TailPilotPositionCommand`は新しいnormalized inputの`Set`と、前tickの保持要求を使う`Hold`を排他的に表す。
- `TailPilotPositionMapping`は[-1,0,1]を[-0.4,trim,0.4] mへ区分線型に写像し、出力を±0.4 mへ制限する。
- 新しいflightの保持targetはtrim、中立inputもtrimとする。
- 欠損・切断の`Hold`は保持中の要求位置を名目要求とする。
- 速度・加速度・移動範囲は身体policyが検証し、位置の瞬間的な変更を禁止する。
- 公開モデルの最大速度0.3 m/s・最大加速度0.8 m/s²と機体値は[Playable仕様](flight-dynamics.md#公開playableモデル)に従う。

目標との距離を $d=|x_t-x|$、最大加速度を $A=a_{max}$、最大速度を $v_{max}$、刻みを $h$ とする。

```math
v_t=\operatorname{sgn}(x_t-x)\min\left(v_{max},\sqrt{2Ad}\right)
```

- 要求加速度は $(v_t-\dot x)/h$ を $[-A,A]$ へ制限する。
- 目標方向の停止位置が目標へ到達している場合は最大制動を要求する。
- 目標方向へ運動中は、1 tick後の移動量と終端速度の標本化停止距離（次節の $S_h$）の和が残距離と一致する加速度を二分で求め、要求する。
- 停止多様体へ乗った身体は制動証明の系列により目標で正確に静止し、到達時の超過と速度反転を生じない。
- 最大制動でも多様体を超える状態では最大制動を要求し、最大加速でも多様体へ届かない状態では停止曲線速度へ追従する。
- 目標誤差と現在の連続停止距離がともに $Ah^2/2$ 以下なら、近傍の要求終端速度を $w=(x_t-x)/h-\dot x/2$ とする。
- 次tickに $-w/h$ を適用すれば目標で静止する2 tickの運動学から得られる。
- この要求も速度・加速度上限と安全検査を通す。
- 1 tick幅を位置のdead zoneにせず、policyの刻みを100 Hz tick以下とする。

各tickの加速度は全RK4 stageへ保持する。パイロットの位置・速度はstage時刻 $0,h/2,h/2,h$ と終端 $h$ に対して、同じ一定加速度の解析解を使用する。

```math
x(t)=x+\dot x t+\frac{at^2}{2},\qquad v(t)=\dot x+at
```

- 他の6DoF状態は通常のRK4で積分する。
- pilotのstage位置をEuler予測し、境界内で停止する軌跡を範囲外にする処理は使用しない。
- 終端位置・速度に加え、$0<-\dot x/a<h$ の場合はtick内極値を検査する。
- 受理候補の終端stateには、次節の有限停止証明を要求する。
- 証明できない要求加速度は、現在stateの証明済み制動加速度へ二分して近づける。
- 二分列が有限な`f64`上で端点に一致した時点で終了する。
- 位置・速度のclampや範囲判定の許容差を追加しない。

### 保持加速度による有限停止証明

速度の絶対値を $v$、$r=v\bmod Ah$ とする。
速度符号を反転せずに最大制動を繰り返し、最後のtickを $-\dot x/h$ で停止する系列の距離は次式となる。

```math
S_h(v)=\frac{v^2}{2A}+\frac{r(Ah-r)}{2A}
```

- この距離を確保できない場合、最大制動後の残速度 $r$ から終端速度 $-w$ へ反転する1 tickと、次の停止tickを検討する。
- 元の進行方向を正とすると、反転tickの加速度は $-(r+w)/h$、最大前進量は $r^2h/[2(r+w)]$、停止までの変位は $rh/2-wh$ である。
- $r+w\le Ah$、反転後の速度上限、反対側境界、tick内極値を同時に検査する。
- 最大反転速度が反対側境界を超える場合は、極値が安全な最小制動加速度を二分探索する。

- 閉形式は制動候補の生成に使用する。
- 停止証明では積分器と同じ`f64`解析運動を反復し、全held stepの範囲・速度を検査して速度0への到達を確認する。
- 実数上で等価な停止距離でも演算順序で丸めが異なるため、閉形式だけで可回復性を保証しない。
- 制動時は近傍の有限な転回点を基準として位置を評価し、微小加速度で遠方の転回点から桁落ちする場合は通常の二次式を使用する。

- 証明上限は4096 held stepである。
- 100 Hzでは40.96秒に相当し、substepでも同じstep数を上限とする。
- 計算費用を有限にする数値policyであり、機体の調整項目を追加しない。
- `f64`上で加速度が速度を減少させない場合も適用範囲外とする。
- 受理指令の終端は同じ有限停止証明を持ち、証明済み制動の次stateは検査済み系列のsuffixを持つ。
- 同じ機体モデル・刻みの下で後続policyも受理可能なstateを保つ、という帰納関係を使用する。

- 初期の連続停止位置 $x+\operatorname{sgn}(\dot x)\dot x^2/(2A)$ が移動限界を超える場合は`PilotMotionUnrecoverable`とする。
- 連続停止位置が範囲内でも、保持加速度・反対側境界・丸め・4096 step以内の系列を得られない場合は`PilotMotionOutsidePolicyDomain`とする。
- 停止距離の非有限演算も数値適用範囲外である。
- 後者はこのpolicyの範囲を表し、あらゆる保持加速度制御で停止不能だと判定するものではない。
- 狭い移動範囲の静止状態からの目標追従にも同じ検査を使用する。

## tickの実行と記録

- `advance_tail_flight_tick`は直前成功stateのq/rから制御を一度評価し、更新incidenceを全stageへ保持する。
- hybrid荷重はphysical incidenceを直接使い、身体policy・移動質量・積分器・接触探索を共有する。
- `advance_tail_flight_tick_with_contact`は接触時に同時刻のfractional terminal sampleだけを返す。
- fraction 0では直前incidence/target、正fractionでは更新値を保持する。

- `advance_tail_flight_tick_with_contact_report`は同一評価の`TailControlCommands`と入力を成功outcomeへ付ける。
- normalized intent、body q/r target、manual/FBW/mixed要求、実incidence、保持身体目標を区別し、record側で制御を再計算しない。
- fraction 0のreportへ未適用の新controlを保存しない。
- 正fractionと整数tickは当該区間の適用controlを返す。
- 制御・身体policy・荷重・接触の失敗で部分stateをcommitしない。

- `TailFlightScenarioParameters`はComposite-CG launch/contact検証からtick-zero stateを作る。
- `TailFlightScenario`は借用したhybrid荷重・風、trim、接触点、courseを固定し、software profileを機体定義から分離する。
- `GameSession`は同じtick stateを保持し、記録・telemetryの成功後だけ整数tickを公開する。
- 失敗stageのstate・incidence・身体target・入力を公開せず、直前成功stateをResultとrecordへ確定する。
- 空力envelopeと有限風のOutsideGridはOutOfValidEnvelope、非有限値・算術・policy等はFatalSimulationErrorとして区別し、元のcauseをfinalizationへ保存する。
- 失敗tickのsampleは追加しない。

- 共有`HybridSessionPreparation`は検証済み機体・surfaceを所有し、同じ登録風場を使うscenarioと解決済みprofile・course・difficultyを構築する。
- 自己参照やleaked storageを使用しない。
- Web/WASMとNativeは同じ準備・実行処理へ接続する。
- Briefing以後の設定を固定し、controllerのID/version・rate上限・gain・slew・迎角guardと、実際のauthorityを保持する。
- 初期化・Resume・入力機器の再取得は、同じsealed mappingで保持目標を逆写像した`pilot_position_target_normalized`を使う。
- trimを0、両端を±1とし、trimと端点が同じ場合は同じphysical targetを0へ正規化する。
- 現在の身体位置から目標を推測せず、元の`Hold`/`Set`要求とresolved effective targetを区別する。

- seedはlow/highの32 bit値で正確に渡す。
- WASMのframe入口はキー遷移と生Gamepad軸・接続世代を受け取り、appの共有規則がsealed rate上限への写像と各tick直前の確定targetへの積分を行う。offlineのWeb/Native比較も同じframe観測を使用する。
- snapshotのphase/frame、physical incidence、身体状態、CG telemetry、terminal stamp/causeの整合は[データ形式](data-and-formats.md)に従う。
- 保存値を再シミュレーション・再制御して表示しない。

- Webの`TailSessionPort`と`TailFlightController`は単一のfacade ownerのresourceを使う。
- 入力・reset・snapshot同期・描画の失敗は共有driverへ通知し、最後の物理状態を保持する。新しい飛行の準備と表示bindingの再構築で実行を再開する。
- Scene退出とBFCache退避ではcontrollerを停止し、最終teardownとowner置換でresourceを解放する。
- 古い非同期結果はowner/query generationで拒否し、同じscenario/seedでも異なるresource世代を混同しない。
- 同じrecordのclock・seek・再生速度更新と、Retry・設定変更・archive開閉・disposeによるrecord source変更を区別する。
- 所有・effect・queryの契約は[アーキテクチャ](architecture.md)、保存形式・Summary・Replayは[データ形式](data-and-formats.md)へ集約する。

## 入力機器と引継ぎ

- 身体の論理指令はdatum上の目標前後位置とする。
- キーボードは保持時間を固定tickで積算し、解放時に目標を保持する。
- キーリピート回数や描画frame数を使わない。
- 飛行準備のキーボード操縦設定で、パイロット目標位置の移動速度を0.1・0.2・0.4・0.8 m/sから選択する。既定値は0.4 m/sとする。
- 舵要求は100 Hzの物理tickで中立から最大へ進む。所要時間は0.1・0.25・0.5・1・2秒（既定0.5秒）、応答曲線は線形またはsmoothstep、要求上限は25・50・75・100%（既定100%）を選択する。
- キー解放時は同じ速度で中立へ戻り、逆方向への入力は中立を通過する。両方向同時押下は中立要求とする。Pause・focus喪失・入力無効化では要求を即時退役し、新たな押下を要求する。
- 設定は飛行準備時に固定し、同じ条件での再試行でも保持する。記録には適用済み要求を保存し、Replayでランプを再適用しない。機器設定そのものの記録・復元は提供しない。
- ゲームパッドの校正済み身体軸は範囲内の絶対目標位置へ写像し、中立は校正済みの中立位置を指す。

- キーボードの応答設定と、標準ゲームパッドの固定binding・軸方向・dead zoneを区別する。個別binding・校正の変更操作は提供しない。
- 論理舵指令と身体目標は別fieldへ保持し、舵角・姿勢目標・角速度目標を曖昧なpitch値へ混在させない。
- Web・Nativeの標準入力はArrowUp / Wが機首上げ、ArrowDown / Sが機首下げ、ArrowLeft / Aが左旋回、ArrowRight / Dが右旋回、J / Lがパイロットの前後移動である。物理キーごとに押下・受理を保持し、同じ方向のキーは一つでも受理中なら要求を維持する。相反する方向の同時保持は中立要求とする。
- ゲームパッドは割り当てたnose-up・right-turn・身体軸だけを読む。
- キー解放は`Hold`、取得済みゲームパッドの身体軸は`Set`とする。

- 同時に有効な操縦機器は1つとし、切替・切断・非表示・overlay移行時に保留中の押下・軸値を破棄する。
- 身体目標は直前の有効値へ保持し、再接続時は中立確認と明示的な再取得を要求する。
- 絶対位置軸への切替では目標との不連続を防ぐpickupを行う。
- Menuイベントを同じtickのFlight指令へ転用しない。
- 標準mappingと未対応機器の個別bindingを区別する。

### ゲームパッドの接続とpickup

- 選択接続は`{ slot_low, slot_high, generation }`の32 bit整数で識別する。adapterはOS/browserの機器識別子と接続・切断世代をこの型へ写像する。
- 選択中の接続を維持し、小さいindexの追加で切り替えない。
- 未選択時と選択接続の消失時だけ、取得配列の先頭から候補を選ぶ。
- 同じindexの再利用も中立待ちへ戻す。
- 製品ID、入力timestamp、pollごとのobject同一性を接続識別へ使わない。
- これは[Gamepad API](https://www.w3.org/TR/gamepad/)のindex・イベント契約に基づく。

- 取得状態は未選択、中立待ち、舵有効・身体pickup待ち、全軸有効の直和型とする。
- 全割当軸の中立確認前は舵0と保持身体目標を返す。
- 中立後は舵mappingを受理し、身体軸だけを保持する。
- 身体軸の絶対mappingが保持目標に一致するか、前後sampleで目標を交差するとpickup成立とする。
- 成立sampleは保持目標を返し、次sampleから通常mappingへ移る。
- 保持目標0では中立sampleでpickupも成立する。
- 許容幅、offset、追加rate limitは設けない。
- 操作上の妥当性は実機で個別に検証する。

- キーボードの操縦キーを優先し、押下時・押下中はゲームパッドを中立待ちへ戻す。
- blur、suspend、resume、resetも再armするが、同じ接続がある間は選択indexを維持する。
- resetだけは新Flightのsnapshotから身体目標を初期化する。
- 停止中も接続世代を記録し、古い中立確認を流用しない。
- disposeはkeyboard・blur・接続イベントの全listenerと世代情報を解放する。
- Menu入力の機器選択・button履歴は別adapterの責務である。

### Phone VR Menuのゲームパッド

- Menu portは必須の`connection: { index, generation }`を返し、有効な選択接続を維持する。
- 同indexの再接続は新しい接続とし、ID・timestamp・poll objectを識別に使わない。
- 接続イベントを受信できるactiveな文書を前提とする。
- 非active時に通知されなかった再接続の識別は保証しない。

- portの`start`/`stop`はPhone VR tracking attemptが所有する。
- factory生成時はlistenerを登録せず、開始時に登録し、停止・取消・開始失敗・追跡失敗時に解除する。
- 部分開始失敗や解除例外でも全解除を試行し、旧callbackが新attemptを変更しないようにする。
- backendの終端と外部資源の解除成功を区別する。

- selectorは接続ごとにbuttonとframe時刻の履歴を持つ。
- 接続交代・全切断・resetでcursorを中央へ戻し、focusと旧履歴を破棄する。
- 新接続の初回sampleは経過時間0とし、保持中のprimary/backを履歴へ取り込む。
- 保持継続では操作せず、解放後の再押下から1回だけ操作する。
- 内部状態を外部UiAction dispatch前に確定し、dispatch例外で旧履歴を再使用しない。
- 操縦用の中立確認・身体pickupとは別の入力契約として扱う。

## 検証範囲

- 非有限command、範囲外authority、不正actuator limit・timestep、travel外stateを型付きエラーとし、途中のactuator状態を公開しない。
- Manual/Automaticの端点、Sharedの各軸混合、飽和、slew、境界値、拒否後の入力状態不変、決定性を検査する。
- feedbackの軸符号、極端な有限rate、合成roll momentを含む閉ループ減衰も確認する。

- 移動質量は無外力時の運動量・角運動量、非対称姿勢の慣性連成、移動限界、急変、数値収束、100 Hzの誤差を検査する。
- 初期身体位置・速度、目標列、実位置・速度・加速度、合成重心offsetと質量特性を記録する。
- 同じ初期状態・機体・環境・舵入力列・身体目標列・刻みの決定性と、身体指令だけを変更した場合の軌道差を確認する。

- 数値積分と離散制御の刻み依存は[量別step-halving](verification.md)でphysics-onlyと連成の100/200/400 Hzへ分ける。
- 局所増大・減衰は離散線型化のJacobian/固有値と小摂動時系列を照合する。
- 明示したPlayable入力列、局所線型化、任意入力の安定性、実機fidelityを区別する。
- 機器切断・切替・再割当・tick割当・UI競合、身体状態と終端時刻の一致はadapterと保存queryを通して検査する。

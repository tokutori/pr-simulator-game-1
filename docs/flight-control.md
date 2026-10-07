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
新playable tick/pilot targetへの接続、WASM・input・record・既定モデル切替は後続単位で実施する。

`TailPilotPositionCommand`は新しいnormalized inputと`Hold`を排他的に表す。
`TailPilotPositionMapping`は[-1,0,1]を[-0.4,trim,0.4] mへ区分線形で写像し、出力を±0.4 mに制限する。
新flightの保持targetはtrimで初期化する。中立inputはtrimへ写像し、input欠損・機器切断の`Hold`は直前targetを維持する。
この位置commandはControlMode・舵authorityから独立する。物理travel・速度・加速度制約は既存pilot policyが検証する。

非有限command、authority範囲外、無効なactuator limit、無効timestep、travel範囲外のstateは型付きerrorとする。
途中まで進めたactuator stateを公開しない。混合結果と更新結果の決定性を保証する。

検証ではManual/Automaticの端点、Sharedの各axis混合、飽和、rate limit、境界値、拒否された入力後の
入力state不変を確認する。body-rate feedbackはaxis符号、飽和、極端な有限rate、および合成roll momentを用いた
閉ループ減衰で検証する。閉ループcontrollerの安定性・通常操縦での飛行成立性は、
合成係数を使った100 Hz/200 Hzのstep-halving連成試験で数値収束を確認する。
この試験は、実機の安定性・通常操縦の成立性を保証しない。aircraft-specific controllerと
aerodynamic derivativesのfidelity検証はBPG-035でM6完了後に扱い、M3〜M6をblockしない。

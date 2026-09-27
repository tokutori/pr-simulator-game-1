# 物理契約

## 状態と入力

状態は機体構造datum $O$ の対地位置・速度（NED）、body-to-NED単位quaternion、body角速度、
パイロットの前後位置・相対速度を含む。合成重心の位置・速度は `pilot-motion.md` の式から導出する。
body表現の対地速度 $(u,v,w)$ は $R_{NB}^{T}v^N$ として導出する。
対気速度とは明確に区別する。移動質量を含む運動量収支は `pilot-motion.md` を正本とする。
質量・慣性・機体空力モデル・環境・操縦入力・積分刻みを明示的に渡す。

## 固定質量時の基準方程式

以下はパイロット相対位置が固定され、合成重心基準の状態へ変換した場合の基準式である。
移動中の一般6DoF式として流用しない。移動質量の連成は `pilot-motion.md` に従う。

地球回転と局所NEDの曲率を省略する。対象は約500 m、数十秒のフライトである。
空力合力と重心まわりのmomentをbody表現で受け取る。

```math
\dot p^N=v^N,\qquad
\dot v^N=\frac{R_{NB}F^B}{m}+\begin{bmatrix}0\\0\\g\end{bmatrix}
```

```math
\dot\omega^B=I_B^{-1}\left(M^B-\omega^B\times(I_B\omega^B)\right),\qquad
\dot q_{NB}=\frac12 q_{NB}\otimes(0,\omega^B)
```

慣性テンソルは重心基準の対称正定値行列として検証する。
対称機での積慣性表記は次の通りとする。

```math
I_B=\begin{bmatrix}I_{xx}&0&-I_{xz}\\0&I_{yy}&0\\-I_{xz}&0&I_{zz}\end{bmatrix}
```

$I_{xz}$ を行列成分として入力するか積慣性として入力するかを外部formatで曖昧にしない。
RK4の各stageで位置・速度・姿勢・局所風・空力を再評価する。
姿勢の正規化規則と誤差はBPG-002で収束試験により検証する。
操縦指令とactuator出力の更新周期・保持規則はBPG-006で固定する。
authority混合・actuator stateの型と更新規則は `flight-control.md` を正本とする。
tick単位の統合処理は `advance_flight_tick` が固定100 Hzで実行し、成功時だけ整数tickと全physical stateを更新する。

## BPG-002のcore実装契約

`FlightState`はdatum $O$ のNED位置・対地速度、body-to-NED quaternion、body角速度、
パイロットの前後位置・相対速度を保持する。`AircraftModel`はパイロットを除く機体質量と
$O$まわりの固定慣性、パイロット質量・固定高さ・移動限界を保持する。慣性は有限・対称・正定値、
機体質量は正、パイロット質量は非負でなければならない。

運動量収支を解く未知量は、body成分で表したdatum速度の時間微分
$d v_O^B/dt$ と角加速度である。積分stateの速度はNED成分なので、その時間微分へ変換する際に
$R_{NB}(d v_O^B/dt+\omega^B\times v_O^B)$ を用いる。座標成分の微分と慣性座標で表した物理加速度を
同一視しない。

`advance`は正の有限timestep、有限の一定パイロット加速度、重力、機体モデル、現在state、
外力providerを受け取る。providerが返すbody-frame wrenchは重力を含まない。重力合力はcoreが
全質量へ作用させ、パイロット重力によるdatum $O$まわりmomentも計上する。外力providerは4つの
RK4 stageごとのstateから評価する。姿勢は各中間stateと出力stateで単位長へ射影する。
一回のstepは入力stateを変更せず、いずれかのstageが失敗した場合は型付きerrorのみを返す。
力学step内にheap allocationを行わない。

この段階のpilot accelerationは1 step中一定とする。位置目標から停止距離・移動限界・速度・加速度を満たす
指令を生成するRust coreのpolicyは `pilot-motion.md` とBPG-026で定義する。

## BPG-003の空力接続

`AerodynamicModel`は左右主翼・水平尾翼・垂直尾翼・胴体を各1要素保持する。各要素は評価点と
荷重作用点、取付姿勢、参照面積・span・chord、6係数law、alpha/beta/dynamic-pressure envelopeを持つ。
`UniformAerodynamicLoad`は位置一様な風と密度から各要素の局所流を評価し、forceとdatum $O$ まわりの
momentを合成する。位置依存風場はBPG-004/005で別境界から接続する。

評価点の速度には機体datum速度と $\omega\times r_i$ を用いる。身体の相対速度は空力点速度へ加えない。
係数lawは参照値・alpha derivative・beta derivativeによる一次式であり、rate derivativeは持たない。
適用範囲外はclampせずerrorとし、外力providerの失敗としてstep全体へ伝播する。

## 時間と処理落ち

```math
\Delta t=0.01\ \mathrm{s},\qquad f_{physics}=100\ \mathrm{Hz}
```

rendererは前後の有効snapshotを補間する。画質設定は物理tickを変更しない。
遅延が発生してもsimulation tickを飛ばさず、1 frameの処理量を制限しながらbacklogを処理する。
復帰不能な遅延では明示的にpauseする。非表示tabはpauseし、復帰時にwall-clock差を積分しない。
入力は整数tickに割り当て、replay用に記録する。異なるFPSで同一入力列を検証する。
同一の物理結果を保証する対象はtick入力列であり、人間のwall-clock操作タイミングではない。

## 発進と着水

launchはdatum $O$ のground velocityと姿勢、構造datum高度、身体初期位置・速度から初期化する。
合成重心位置・ground velocityを指定する入力形式は、身体状態・姿勢・角速度からmass-ratio offsetを用いてdatum position・velocityへ変換する。Rust coreは`flight_state_from_composite_cg_launch`でこの変換を提供する。風をground velocityへ自動加算しない。
trimから生成する場合はair-relative条件を明示し、ground条件によるlaunchと別の生成方法にする。
静水面はNEDのD=0とする。初期版の接触判定は登録した機体接触点のいずれかがD>=0となる条件を用いる。
接触点は構造datumからの固定offsetでモデルに記録する。接触検出は隣接する成功tick間のstateを補間し、
各接触点の水面到達fractionを探索して最早eventを選択する。位置・速度・角速度・身体状態・actuatorは線形補間し、
姿勢は短経路quaternion slerpを用いる。接触探索では補間経路を16区間で調べ、接触を含む区間のfractionを
同じ補間state上で二分探索により確定する。
終端位置・姿勢・actuator・身体状態は同一の接触時刻で確定する。
接触後のtick状態を通常sampleとして保存しない。Result、graph、Replayは同じ終端sampleを参照する。
`advance_flight_tick_with_contact`はContact時にfractional terminal sampleだけを返し、接触後のinteger-tick stateを公開しない。
`run_flight`は固定100 Hzの機器非依存input列を順に適用し、最初のWaterContactまたはTimeLimitで終了する。score v1はWaterContactならfractional terminal sample、TimeLimitなら最後の有効stateから算出し、終了後のtickを処理しない。
水面波は初期版では描画のみであり、波頂による接触時刻の変動は計算しない。
platform上の走行・拘束解除・複雑な陸地衝突は初期版の対象外である。
Distance score v1は発進course axisに対するdatumの符号付き水平変位projectionとする。右向きcross-track変位と直線水平変位長は独立metricとして保持し、trajectory lengthとは区別する。公式競技計測との同一性を仮定しない。

## 数値エラーと検証範囲

不正値、非正定値慣性、空力適用範囲外は型付きエラーとして返す。
失速域を使用するゲームでは、その範囲を含む係数モデルを別途検証する。
適用範囲外を無条件clampして飛行継続しない。
native/WASM比較はBPG-007でscenarioと許容差を固定する。許容差を失敗結果へ事後適合させない。
力学検証の合格は実機の飛距離予測精度を保証しない。

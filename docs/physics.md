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
合成重心高度を指定する入力形式は、身体初期位置からdatum位置へ変換する。風をground velocityへ自動加算しない。
trimから生成する場合はair-relative条件を明示し、ground条件によるlaunchと別の生成方法にする。
静水面はNEDのD=0とする。初期版の接触判定は登録した機体接触点のいずれかがD>=0となる条件を用いる。
接触点は構造datumからの固定offsetでモデルに記録する。stepを跨ぐ接触は位置の交差を検出し、
score用の接触位置を補間する。終端位置・姿勢・actuator・身体状態は同一の接触時刻で確定する。
接触後のtick状態を通常sampleとして保存しない。Result、graph、Replayは同じ終端sampleを参照する。
水面波は初期版では描画のみであり、波頂による接触時刻の変動は計算しない。
platform上の走行・拘束解除・複雑な陸地衝突は初期版の対象外である。
飛距離の定義（直線水平距離または規定方向への投影）はBPG-006で明示し、公式競技計測との同一性を仮定しない。

## 数値エラーと検証範囲

不正値、非正定値慣性、空力適用範囲外は型付きエラーとして返す。
失速域を使用するゲームでは、その範囲を含む係数モデルを別途検証する。
適用範囲外を無条件clampして飛行継続しない。
native/WASM比較はBPG-007でscenarioと許容差を固定する。許容差を失敗結果へ事後適合させない。
力学検証の合格は実機の飛距離予測精度を保証しない。

# Hybrid風条件と適用限界近傍の刻み幅検査

## 範囲

BPG-043の風条件比較は、架空の標準矩形mockと既存の公開core方程式を用いる。
`tail_simulation/numerical_tests.rs`のdriverを共用し、Manual、Shared（FBW authority 0.5）、
Automaticについて100、200、400 Hzを比較する。機体係数、controller係数、製品APIを変更しない。
旧playable fixtureの飛距離条件や実機性能をこの検査へ適用しない。

physics-only比較は制御とpilot policyを100 Hzで評価し、その結果を物理substep間で保持する。
coupled比較は制御とpilot policyも物理刻みで評価する。同じ物理時刻の入力を両比較に与え、
100 Hzの結果は`advance_tail_flight_tick`と全状態で完全一致させる。

## 実行前に固定する条件

密度1.225 kg/m³、重力9.80665 m/s²、trim基準速度9.7 m/s、開始CG位置NED `(0, 0, -50)` m、
heading 0、controllerのq/r gain各0.2 s、slew 1 rad/sを共通とする。

| 条件 | 風と初期状態 | 共通比較区間 | 入力 |
| --- | --- | --- | --- |
| 一様風 | NED `(0.3, 0.1, -0.04)` m/s。既存driverの初期角速度 `(0.02, -0.04, 0.03)` rad/s | 0.5 s、50区間 | 既存SmoothChanged |
| 空間shear | 基準位置NED `(0, 0, -50)` m、基準風は一様風と同じ。下記gradientを位置差へ適用する。初期角速度は一様風と同じ | 0.5 s、50区間 | 既存SmoothChanged |
| static alpha上限近傍 | 零風、角速度0、pilotはtrim位置で静止。公開polarの上限から0.002 rad内側、標準mockではalpha 0.118 rad。body相対風の大きさ9.7 m/s | 0.05 s、5区間 | neutralとHold |

shearのgradientは、列がN/E/D位置差、行がN/E/D風速を表す。単位はs⁻¹である。

```text
[[0,     0,      0.015],
 [0.012, 0,      0    ],
 [0,    -0.003,  0    ]]
```

SmoothChangedは0.10–0.20 sでnose-up/right intent `(0.01, 0.005)`、
q/r target `(0.01, -0.005)` rad/s、0.20–0.30 sで符号を反転する。
0.20 sの1区間はneutralとし、0.30 sでpilotの正規化目標0.2をSetする。
それ以外はneutralまたはHoldであり、200/400 Hzでも10 ms区間内の入力を保持する。

## 量別予算と域内条件

既存零風検査と同じ予算、丸めfloor、縮小率を用いる。結果に合わせて値を変更しない。
最大差は共通の10 ms時刻で測定し、100/400 Hz差を次の予算以下とする。

| 量 | physics-only | coupled |
| --- | ---: | ---: |
| datum位置 m | 1e-6 | 1e-3 |
| datum速度 m/s | 1e-6 | 1e-3 |
| quaternion姿勢距離 rad | 1e-7 | 1e-4 |
| body角速度 rad/s | 1e-6 | 1e-3 |
| pilot位置 m | 1e-10 | 1e-10 |
| pilot速度 m/s | 1e-10 | 1e-10 |
| 二系統physical incidence rad | 1e-7 | 5e-4 |

上表の順で丸めfloorは`4096 * f64::EPSILON * [50, 10, 1, 0.1, 0.1, 0.3, 0.01]`とする。
200/400 Hz差は100/200 Hz差の0.125倍（physics-only）または0.75倍（coupled）とfloorの和以下とする。
本単位はSmoothChangedとneutralのtarget即時到達branchを検査する。
既存SlewReversalの非滑らかなbranchと独立の絶対予算はそのまま保持する。

各RK stageで、datum速度からその地点の実風を差し引いたbody相対風のalphaを補助観測する。
風2条件ではalphaを開区間 `(0, 0.06)` rad、上限近傍では公開polarの最終PWL区間の内部に保つ。
0.06 radは標準mockの内部PWL節点であり、0.12 radの適用域上限とは異なる。
このdatum観測は選んだ補間segmentの補助チェックである。
適用域の判定には毎stageの実`HybridAerodynamicLoad::evaluate_hybrid`を使い、
static polar、global beta、全proxyの速度比・前向き流れ・局所角・controlled角を検査する。
proxy速度比の閉区間0.8–1.2は参照速度に対する比であり、絶対速度上限を捏造しない。
datum downを-40 m未満に保ち、既存mock接触点より十分高い接触前区間だけを比較する。

## 境界外と未検証範囲

公開polar上限に1e-6 radを加えた初期状態は、全modeの最初のRK stageで
`StaticPolar / OutsideEnvelope / StaticAlpha / First`として拒否され、入力状態は不変となる。
成功trajectoryやscoreを生成せず、外挿やerrorの吸収を行わない。
既存`tail_tick_load_failure_preserves_stage_cause_and_every_previous_state_field`と
`tail_scenario_runner_returns_previous_success_state_and_original_failed_stage`は、
後段stage失敗と直前成功状態の保持を別途検査する。

本単位は局所的な風条件・適用域近傍の数値比較であり、全風領域、接触時刻、長時間安定性、
実機fidelity、WASM実行、実ブラウザー/GPU、HMDの検証を含まない。
実行結果は検査後に報告する。

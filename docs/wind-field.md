# 風場契約

外部assetのschema、local NED原点、出典・仮定、波・空との対応は
[`environment-format.md`](environment-format.md) に従う。

## 定常空間場

1フライトの基本環境は時間不変とし、NED位置に対する空気のNED速度を返す。

```math
W=W(x,y,z),\qquad V_{air}=V_{ground}-W
```

公開APIはUniform、Analytic linear gradient、Gridのvalidated constructorを提供する。
APIは概念的に `velocity_at(Point3<NedFrame>) -> Result<Vector3<NedFrame>, WindError>` とする。
coreでclockを参照しない。対地速度を積分stateに使用するため、風勾配に起因する別の加速度項を重複加算しない。
`WindField`はno_std coreにあり、linear gradientは基準NED位置・風速と、行を風速成分、列を位置成分とする$3\times3$勾配で定義する。
Gridのvelocity sampleはimmutableなsliceを借用し、coreで確保・複製しない。
`SyntheticPlayableFlight::try_new_with_wind_field`は検証済み`WindField`を受け取り、
fixtureと`into_parts`で返す`FlightScenario`へ借用期間を伝播する。
呼出側がgrid sampleを所有し、無風・一様風の既存constructorと同じ機体・発進・controller構成を使用する。
grid範囲の確保は呼出側の責務であり、空力要素の評価時の範囲外errorには元の`WindError`と要素roleを保持する。
`WindFieldAerodynamicLoad`は各RK4評価stageの全空力要素位置でfieldをqueryする。
空力要素ごとの位置・回転速度は `aerodynamics.md` に従う。

Gridは原点、N/E/D各軸の正の間隔、各軸2以上の点数、NED速度列を持つ。
配列順はNが最速、次いでE、Dとし、indexは `((d * count_e) + e) * count_n + n` とする。
三線形補間、閉区間の境界を採用する。範囲外、非有限値、overflow、不正な点数はエラーとする。
閉区間はNED座標の `origin .. origin + spacing * (count - 1)` で判定する。
区間内の位置をindexへ換算した際の上端丸め誤差を補正し、1 ULPでも区間外の位置は拒否する。
暗黙の外挿・clampは行わない。全機の評価点を含む飛行領域をofflineで確保する。

## 一貫した作用経路

```text
WindField → local air-relative velocity → V/alpha/beta/dynamic pressure
          → coefficients → forces/moments → 6DoF → FBW observation
```

headwind/tailwind、crosswind、上昇流、wind shear、左右非対称流を同じ経路で扱う。
独立したcrosswind forceや位置の移流補正を追加しない。
一様定常風では同じair-relative初期条件に対する姿勢・対気運動が一致する。
ground velocityを固定したlaunchでは対気条件が変化するため、この不変性とは試験を分ける。

```math
\frac{dW}{dt}=(V_{ground}\cdot\nabla)W,\qquad \frac{\partial W}{\partial t}=0
```

定常場でも移動する機体が経験する風は変化する。
multi-point modelは要素間の速度差によるmomentを表現するが、連続流体の完全なvorticity応答を保証しない。
時間依存gustやstochastic turbulenceは後続拡張とし、明示的simulation timeと状態のAPI設計を先行する。

## Scenario

Weather分類は `difficulty.md` のCalm / Mild / Typical / Challenging / NearLimitを正本とし、
Customは利用者によるscenario選択を表す。具体的なscenario ID・versionと分類を区別する。
0–5 m/sはゲーム設計上の主要範囲であり、公式運用限界として扱わない。
Typicalの風向・風速・鉛直風・空間相関は観測と研究により確定する。
地上観測から湖上の3D fieldを一意に復元できるとは仮定しない。
合成した場には観測値、補間、仮定、適用範囲、seedを記録する。

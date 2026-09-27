# 舵指令・FBW・actuator契約

## 所有境界

制御・authority混合・actuator状態はRust coreの決定的なdomain stateとする。入力機器はTypeScript adapterで
機器非依存の舵指令へ変換し、tick単位でcoreへ渡す。DOM、Gamepad API、FBW出力の生成に必要な観測値の取得は
この境界へ含めない。coreには観測snapshot、pilot command、controller command、actuator model、timestepを明示して渡す。

FBW authorityは舵指令だけに適用する。pilot body targetは別経路であり、authority mixerに含めない。
Manualはpilot指令を選択し、Automaticはcontroller指令を選択し、Sharedはauthority $a$ による線形混合を行う。

```math
u=(1-a)u_{pilot}+a u_{FBW},\qquad 0\le a\le1
```

## 型と単位

roll、pitch、yawの各論理指令・actuator出力は、対応する機体軸まわりの角度をradianで表す。
authorityは有限な$[0,1]$値だけを保持するvalidated型とする。制御modeはManual、Shared(authority)、Automaticの
直和型で表し、modeとauthorityの矛盾を許さない。

各actuatorは正の最大舵角$radian$と最大舵角速度$radian/second$を持つ。
入力targetは最大舵角でsaturateし、現在状態から1 stepで移動できる角度を最大舵角速度とtimestepで制限する。
step中の出力は更新済みactuator stateとして保持し、次のtickまで同じ値を空力評価へ渡す。
制御・actuator更新周期はphysics tickと同じ100 Hzとする。stepは正の有限timestepのみ受理する。
`advance_surface_control`はpilot/FBWのauthority混合、rate limit・saturation適用、更新後stateを一つの
決定的な操作として返す。混合後commandもrecord可能な値として返却する。

このactuator modelは静的舵角限界とrate limitを表す。独立した遅延・一次lagを追加する場合は、
遅延bufferとその初期状態をFlightRecordへ含める契約および統合収束試験を同時に定義する。

## エラーと検証

非有限command、authority範囲外、無効なactuator limit、無効timestep、travel範囲外のstateは型付きerrorとする。
途中まで進めたactuator stateを公開しない。混合結果と更新結果の決定性を保証する。

検証ではManual/Automaticの端点、Sharedの各axis混合、飽和、rate limit、境界値、拒否された入力後の
入力state不変を確認する。閉ループcontrollerの安定性・通常操縦での飛行成立性は、
aircraft-specific controllerとaerodynamic derivativesを組み合わせたBPG-006統合検証で扱う。

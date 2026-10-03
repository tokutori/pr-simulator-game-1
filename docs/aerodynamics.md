# 空力契約

## 要素と局所流

初期から左右主翼・水平尾翼・垂直尾翼・胴体の5要素を使用する。
`AerodynamicModel`は5役割を各1個要求する。要素位置は構造datumからの固定body offsetである。
要素ごとに評価点、荷重作用点、面積、span、chord、取付姿勢、係数、適用範囲を設定する。
評価点と荷重作用点が異なる場合は両方を明記する。

```math
p_i^N=p_O^N+R_{NB}r_{Oi}^B
```

```math
V_{air,i}^N=v_O^N+R_{NB}(\omega^B\times r_{Oi}^B)-W(p_i^N),\qquad
V_{air,i}^B=R_{NB}^{T}V_{air,i}^N
```

パイロット移動は $O$ まわりの運動量収支を通じて機体運動へ作用する。固定空力点の速度に
パイロット自身の相対速度を直接加算しない。速度式の $W$ は評価点の風速である。BPG-003では
空間一様な風を入力し、位置依存の風場はBPG-004/005で接続する。詳細は `pilot-motion.md` に従う。
要素取付姿勢で局所座標へ変換し、局所 $(u_a,v_a,w_a)$ から以下を求める。

```math
V=\sqrt{u_a^2+v_a^2+w_a^2},\quad
\alpha=\operatorname{atan2}(w_a,u_a),\quad
\beta=\operatorname{atan2}(v_a,\sqrt{u_a^2+w_a^2}),\quad
q_\infty=\frac12\rho V^2
```

ゼロ対気速度で方向を定義しない。動圧0・荷重0とし、角度はundefinedを表す型で扱う。
局所速度が非ゼロでも $u_a=w_a=0$ ならalphaを定義できず、型付きerrorを返す。
後流・downwashの使用有無、低速・逆流・失速域の適用範囲は機体モデルごとに明記する。
`FlowAngles`は`Zero`またはalpha/betaを同時に保持する`Defined`で表す。
`AerodynamicEvaluation`は構築時に検証した5 roleを必ず1個ずつ返し、role lookupはtotalである。
評価失敗は`AerodynamicEvaluationError`でcauseと要素roleを保持し、全機合成時の失敗はAggregateとして区別する。
`ExternalLoadProvider`からdynamicsへ渡す場合も、load error内に元の空力causeとroleを保持する。

角度計算にはno_std対応`libm::atan2`を使用する。独自近似誤差で閉区間の適用範囲判定を反転させない。
envelope境界は包含し、内側・境界・外側を個別に検証する。速度方向の正規化は有限な非ゼロ速度成分を
速度量で直接除算し、速度の逆数を先に計算しない。動圧のunderflowを伴う微小速度でも方向と荷重は有限である。

## BPG-003係数契約

各要素は6つのdimensionless coefficient lawを持つ。揚力 $C_L$、抗力 $C_D$、側力 $C_Y$、
roll $C_l$、pitch $C_m$、yaw $C_n$ は、参照値とalpha/beta微係数で定義する。
角度はradであり、係数は次の一次式で評価する。

```math
C=C_0+C_\alpha\alpha+C_\beta\beta
```

角速度rate derivative、Mach依存、失速後モデル、downwashは含めない。
局所速度による回転減衰は評価点速度にのみ反映する。`ElementEnvelope`はalpha、beta、
dynamic pressureとroll・pitch・yaw舵角の閉区間を定め、外側の値をclampせず拒否する。
`ControlEnvelope`は各軸について有限かつneutralを含む上下限を要求する。非対称範囲を許可し、
上下限とも0の軸はneutral固定を表す。actuatorの最大舵角・最大舵角速度とは別の係数lawの適用範囲である。
全要素へ同じglobal舵角を渡すため、微係数0の軸にも適用範囲を明示する。

舵角微係数を使用する場合の6係数は次式で評価する。

```math
C=C_0+C_\alpha\alpha+C_\beta\beta+
C_{\delta_r}\delta_r+C_{\delta_p}\delta_p+C_{\delta_y}\delta_y
```

要素構築時にalpha・beta・3舵角の直積領域の32頂点を、実行時と同じ係数評価で検査する。
一次lawの領域全体で6係数が有限、$C_D$が非負であることを要求する。係数や負抗力をclampしない。
零速でも舵角領域を先に検査し、逸脱はroleを保持した`OutsideEnvelope`となる。
`FlightScenario::try_new`は全5要素・全3軸についてactuatorの全travel範囲が舵角領域に含まれることを
検査し、不一致は`ControlEnvelope` error内にroleと`IncompatibleControlEnvelope`を保持する。
この検査は設定の整合性を保証する。飛行中のflow領域逸脱や、参照寸法・動圧を乗じた荷重の算術overflowは
実行時の型付きerrorとして引き続き検査する。

揚抗力・側力のwind-axis基底を要素local axesで定義する。$H=\sqrt{u_a^2+w_a^2}$、
$V=\sqrt{u_a^2+v_a^2+w_a^2}$ とすると、速度軸は $(u_a,v_a,w_a)/V$、揚力方向は
$(w_a/H,0,-u_a/H)$、側力方向は $(-u_av_a/(HV),H/V,-w_av_a/(HV))$ である。
これらから得た力を要素姿勢でbodyへ回転する。揚力の正係数はalpha 0でbody-upの $-z$ 方向となり、
側力の正係数はbeta 0でbody-rightの $+y$ 方向となる。

```math
F_i^a=q_i S_i\left(-C_D\hat e_V+C_Y\hat e_Y+C_L\hat e_L\right),\qquad
M_i^a=q_iS_i\begin{bmatrix}b_iC_l\\c_iC_m\\b_iC_n\end{bmatrix}
```

要素固有momentは荷重作用点のlocal axes基準であり、その姿勢でbodyへ回転する。係数momentと
作用点momentは同じ荷重作用点を基準とし、datum $O$ へ移す際に $r_{Oi}\times F_i$ を一度だけ加える。
合成重心 $G$ まわりのmomentが必要なら、datum momentと合力から変換する。

係数値は機体ごとの出典・同定範囲・要素寄与を持つデータ層で供給する。この実装は標準係数や
実機同定値を仮定しない。

## 荷重合成

揚抗力はwind axes、momentは要素基準axesで定義し、bodyへ変換して合成する。
抗力は対気速度に逆向き。揚力は正迎角付近で上向きとする。
各係数の参照面積とmoment基準長を外部formatへ保存する。

```math
L_i=q_i S_i C_{L,i},\quad D_i=q_i S_i C_{D,i},\quad
F^B=\sum_i F_i^B,\quad
M_O^B=\sum_i\left(M_i^B+r_{Oi}^B\times F_i^B\right),\qquad
M_G^B=M_O^B-r_G^B\times F^B
```

回転に伴う局所速度が生む減衰と、係数表のrate derivativeを二重計上しない。
全機係数を各要素に複製しない。係数の同定範囲と、要素モデルへ割り当て済みの寄与を区別する。
`UniformAerodynamicLoad`と`WindFieldAerodynamicLoad`はBPG-002のRK4荷重境界へ接続する。
`WindFieldAerodynamicLoad`は各RK4 stageで各要素位置の風をsampleし、相対速度から荷重を評価する。
左右翼・尾翼の異なるsampleは各要素のforceとdatum momentを通じてのみ作用し、独立した風力や勾配加速度を追加しない。

## 符号確認

右翼の $r_y>0$、上向き揚力の $F_z<0$ より、右翼揚力の増分は負roll momentを生じる。
左右の作用点を $r_y=\pm a$、上向き揚力の大きさを $L_R,L_L$ とすると、

```math
M_x=a(L_L-L_R)
```

となる。作用点間隔2aを全翼幅と同一視しない。
尾翼は通常 $r_x<0$ である。上向き尾翼荷重の増分は負pitch momentを生じる。
右向き尾翼荷重の増分は負yaw momentを生じる。
試験では具体的なoffsetとforceから外積を計算して期待符号を固定する。

## Playable合成機体のyaw軸

`SyntheticPlayableFlight::AIRCRAFT_MODEL_VERSION` は2である。ブラウザーのscenarioとdemoは
この版を記録する。v2の垂直尾翼は評価点・荷重作用点をbody `(-1.8,0,-0.1)` mとし、
取付姿勢IDENTITYの側力へ $C_{Y,\delta_{yaw}}=-0.2$ rad$^{-1}$ を割り当てる。
正のyaw指令は左向き尾翼力を生み、後方の作用点から正のyaw momentを生成する。
要素固有yaw momentは0であり、作用点の外積でmomentを計算する。
この腕長と微係数は合成fixtureの仮定として固定する。実機同定値・性能予測の根拠には使用しない。

正負yaw操舵の側力・momentを解析値と比較し、Manual・Shared・Automaticの軸応答、authority、
neutral、決定性を検証する。yawのみのfeedbackを使って追加減衰の寄与を分離し、
全軸feedbackでも初期yaw rateからの減衰を確認する。roll/pitchの符号と既存neutral glideも検査する。

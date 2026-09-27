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
dynamic pressureの閉区間を定め、外側の値をclampせず拒否する。$C_D$は宣言範囲全体で非負でなければならない。

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
BPG-003の一様風providerはBPG-002のRK4荷重境界へ接続する。空間風fieldとの結合は後続BPGの責務である。

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

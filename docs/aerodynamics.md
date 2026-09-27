# 空力契約

## 要素と局所流

初期から複数要素を扱い、左右主翼・水平尾翼・垂直尾翼・胴体の5要素を基本構成とする。
要素位置は構造datumからの固定body offsetである。要素ごとに面積、基準長、取付姿勢、係数基準を記録する。
評価点と荷重作用点が異なる場合は両方を明記する。

```math
p_i^N=p_O^N+R_{NB}r_{Oi}^B
```

```math
V_{air,i}^N=v_O^N+R_{NB}(\omega^B\times r_{Oi}^B)-W(p_i^N),\qquad
V_{air,i}^B=R_{NB}^{T}V_{air,i}^N
```

パイロット移動は $O$ まわりの運動量収支を通じて機体運動へ作用する。固定空力点の速度に
パイロット自身の相対速度を直接加算しない。詳細は `pilot-motion.md` に従う。
要素取付姿勢で局所座標へ変換し、局所 $(u_a,v_a,w_a)$ から以下を求める。

```math
V=\sqrt{u_a^2+v_a^2+w_a^2},\quad
\alpha=\operatorname{atan2}(w_a,u_a),\quad
\beta=\operatorname{atan2}(v_a,\sqrt{u_a^2+w_a^2}),\quad
q_\infty=\frac12\rho V^2
```

ゼロ対気速度で方向を定義しない。動圧0・荷重0とし、角度はundefinedを表す型で扱う。
後流・downwashの使用有無、低速・逆流・失速域の適用範囲は機体モデルごとに明記する。

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
BPG-003で力・moment変換、符号、面積・基準長、残余のrate derivativeの扱いを検証する。

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

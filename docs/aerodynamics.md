# 空力契約

## 全機static polar（BPG-038）

`StaticPolar`は一つの解析方式・離散設定・参照量を持つ全機の非線形static表である。
行は`StaticPolarRow`、独立列は`StaticPolarCoefficients`で表す。迎角`alpha_rad`と
揚力`lift`（$C_L$）、誘導抗力`induced_drag`（$C_{Di}$）、profile抗力`profile_drag`（$C_{Dv}$）、
側力`side_force`（$C_Y$）、`roll_moment`（$C_l$）、`pitch_moment`（$C_m$）、`yaw_moment`（$C_n$）を区別する。
全機profile dragはこの表へ集約する。`drag()`は次式の計算値であり、独立入力列を持たない。

```math
C_D=C_{Di}+C_{Dv}
```

構築時に2点以上、全値有限、迎角の厳密単調増加、$C_{Di},C_{Dv}\ge0$、
正の有限な共通面積$S$・span $b$・MAC $c$を検証する。負の揚力やmomentも有効である。
型の内部値は非公開とし、検証済みのconstructorだけで組み立てる。各列は区分線形補間する。
table knotでは元の行を保持し、両端を含む閉区間で評価する。外挿・端点clamp・失速後曲線の追加を行わない。
有限値の計算で生じるoverflowも型付きerrorとする。

`StaticPolarMetadata`は解析方式、離散設定ID、非ゼロmodel versionを保持する。
設定IDは舵角として解釈しない。方式・対象・設定・参照量が異なる表を数値だけ継ぎ足さない。
LLTの主翼表は全機VLM表へ加算しない。Re依存や設定間の連続補間はこの契約に含めない。
行・設定IDは借用し、構築と荷重評価はheap allocation・I/Oを要しない。

### datumの流れとstatic force

bodyはFRD、worldはNEDとし、角度rad、長さm、力N、moment N mで扱う。
$R_{NB}$はbodyからNEDへの回転である。全機staticは各RK stageのdatum $O$で風をsampleする。

```math
v_O^B=R_{NB}^{T}(v_O^N-W_O^N),\qquad
V=\lVert v_O^B\rVert,\qquad q_\infty=\frac12\rho V^2
```

```math
\alpha=\operatorname{atan2}(w,u),\qquad
\beta=\operatorname{atan2}(v,\sqrt{u^2+w^2})
```

```math
e_V=(\cos\alpha\cos\beta,\sin\beta,\sin\alpha\cos\beta),\quad
e_L=(\sin\alpha,0,-\cos\alpha),\quad
e_Y=(-\cos\alpha\sin\beta,\cos\beta,-\sin\alpha\sin\beta)
```

```math
F_{static}^B=q_\infty S(-C_D e_V+C_Y e_Y+C_L e_L)
```

beta=0の表をcurrent betaの力基底で表す処理はkinematic continuationの設計仮定である。
横滑りの非線形係数を取得したという意味は持たない。実装は上式と等価な速度成分の比で基底を計算し、
先に$1/V$を計算することによる微小速度のoverflowを避ける。minimum airspeedは追加しない。

### momentの軸と参照点

`moment_point_from_datum`はbody固定点$P$の$r_{OP}$であり、移動する重心$G$と区別する。
`PolarMomentAxes`は`BodyFrd`または`WindAtBetaZero`を明示する。
roll・yawは$b$、pitchは$c$で最初に次元化する。

```math
\widetilde M_P=q_\infty S(bC_l,cC_m,bC_n)
```

`WindAtBetaZero`では、表のbeta=0に対応する次の列ベクトルでbodyへ回転する。
current betaでは回転しない。$b\ne c$のため、係数ベクトルを先に回してから各軸をscaleする処理は禁止する。

```math
e_x^0=(\cos\alpha,0,\sin\alpha),\quad e_y^0=(0,1,0),\quad
e_z^0=(-\sin\alpha,0,\cos\alpha),\qquad
M_P^B=[e_x^0\ e_y^0\ e_z^0]\widetilde M_P
```

```math
M_{O,static}^B=M_P^B+r_{OP}^B\times F_{static}^B
```

`BodyFrd`は次元化したmomentをそのままbody成分とする。原点移送は一度だけ行い、
XCP由来のmomentを追加しない。$P$の点速度や風でstatic表を再評価しない。
同じwrenchを異なる$P$で表す試験は、移送後のdatum荷重と既存moving pilot方程式の応答を比較する。

### providerと後続実装の境界

`StaticPolarLoad`は全機staticだけを返し、errorを`AerodynamicEvaluationError::StaticPolar`として保持する。
静的providerはneutral以外の既存三系統操舵を`UnsupportedControl`で拒否する。
`AerodynamicLoadProvider`は借用した`ElementOnly`または`StaticPolar`を排他的に選択する。
全機staticと旧5要素の全荷重を加算する経路は持たない。

このstatic部品はalphaが未定義のゼロ流・純横方向流を`UndefinedFlowAngle`で拒否する。
hybridのゼロ速度特例はBPG-039で全proxy actual流と構成・state・controlを検証して実装する。
datumだけを見た荷重0の早期returnを追加しない。小さな正の前進流は有限な角度を持ち、動圧と荷重は0へ近づく。
BPG-039で局所normal-forceのcurrent-reference差分とstage単位の小擾乱適用範囲を追加する。
局所drag・固有Cm、独立rate derivative・操舵torque、wake・動的失速・Re依存・地面効果は追加しない。
水面接触は既存の幾何判定で扱い、地面近傍の空力精度は保証しない。

BPG-041 / [#220](https://github.com/tokutori/pr-simulator-game-1/issues/220)で公開用架空mockを定義・検証する。
公開アプリの既定hybrid切替はBPG-042 / [#221](https://github.com/tokutori/pr-simulator-game-1/issues/221)で、
二系統入力の公開型・model/controller identity・record versionの更新と同時に行う。

### 根拠と来歴

非公開xlsxの監査は設計者が完了しており、この実装に実係数・実機の質量・慣性・形状数値・元解析ファイルを導入しない。
software試験は架空値だけを使用する。Excel importerは対象外である。
実データ導入時には来歴・座標・参照量・版・単位を再確認する。
誤った部分集計や派生値を採用せず、非公開xlsxはrepo外へ保持する。

[開発方針](https://zenn.dev/bem130/articles/1b352797de94e7)に従い、機体空力を難易度や到達距離へ合わせて変更しない。
[XFLR5著者のmoment軸説明](https://sourceforge.net/p/xflr5/discussion/679398/thread/9fc00666c9/)は
T1/T2/T4のwind axesを参照する根拠であり、`WindAtBetaZero`の選択とは個別表の解析条件を対応付ける。
[著者の解析上の制限](https://flow5.tech/xflr5/docs/Part%20IV:%20Limitations.pdf)に示される
LLT/VLM、粘性・剥離・抗力評価の制約を、実機妥当性の検証と区別する。
本実装のPWL補間・kinematic continuation・provider構成は今回の設計判断である。

## 要素と局所流

既存のelement-onlyモデルは左右主翼・水平尾翼・垂直尾翼・胴体の5要素を使用する。
以下はその汎用API・独立software fixtureの契約である。新しいplayableは後続BPGでhybridを選択する。
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

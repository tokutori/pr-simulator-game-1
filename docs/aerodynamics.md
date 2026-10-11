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
current betaでは回転しない。公開契約の評価順序は次元化後の軸回転とする。
alphaだけを用いるこのbody y軸回転は、共通のspan $b$を用いるrollとyawだけを混合するため、
$\operatorname{diag}(b,c,b)$と可換である。
pitchとroll/yawを混合する一般回転では、$b\ne c$の場合に次元化と回転の順序が結果へ影響する。

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
`AerodynamicLoadProvider`は借用した`StaticPolar`または`Hybrid`を排他的に選択する。全機staticとhybrid差分の評価を共用し、重複荷重を生成しない。

このstatic部品はalphaが未定義のゼロ流・純横方向流を`UndefinedFlowAngle`で拒否する。
hybridのゼロ速度特例はBPG-039で全proxy actual流と構成・state・controlを検証する。
datumだけを見た荷重0の早期returnを追加しない。小さな正の前進流は有限な角度を持ち、動圧と荷重は0へ近づく。
BPG-039の局所normal-force差分とstage単位の小擾乱適用範囲は次節に定める。
局所drag・固有Cm、独立rate derivative・操舵torque、wake・動的失速・Re依存・地面効果は追加しない。
水面接触は既存の幾何判定で扱い、地面近傍の空力精度は保証しない。

BPG-041 / [#220](https://github.com/tokutori/pr-simulator-game-1/issues/220)で公開用架空mockを定義・検証する。
公開アプリの既定hybrid切替はBPG-042 / [#221](https://github.com/tokutori/pr-simulator-game-1/issues/221)で、
二系統入力の公開型・model/controller identity・record versionの更新と同時に行う。

### 公開架空mock定義

`HybridMockDefinition`は公開可能な架空値を所有し、geometry・strip・anchor・全機polarを検証する。
構築・借用viewはheapとI/Oを使用しない。公開モデルは
`bpg041-playable-hybrid-mock` / model version 2に統一する。
定義の構築、trim荷重・moving pilot連成残差、動的応答・controller・公開経路を個別に検証する。

| 面 | 投影面積 / span / MAC [SI] | strip | body quarter-chordとframe |
|---|---|---|---|
| 主翼 | 18 / 18 / 1 | 左右各8 | root=O、$z=-|y|\tan5^\circ$、left $R_x(+5^\circ)$、right $R_x(-5^\circ)$ |
| 水平尾翼 | 2.5 / 3.4 / 2.5/3.4 | 8 | $(-3.6,y,0.1)$、body frame |
| 垂直尾翼 | 0.5 / 0.7 / 0.5/0.7 | 4 | $(-1.8,0,z)$、$z\in[-0.45,0.25]$、$R_x(\pi/2)$ |

作用点は各矩形stripのmidpointである。主翼の実面積weightは投影weightを$\cos5^\circ$で除した値とする。
面全体の$AR=b^2/S$から$a=2\pi/(1+2/AR)$を求める。anchorはbody alpha=0・正の前進流で
各frameへ変換した幾何alphaを保持し、主翼CL=0.70、水平尾翼CL=-0.225、垂直尾翼CL=0とする。

全機参照はS=18 m²・b=18 m・c=1 m・P=O・`WindAtBetaZero`である。
9節点の全機polar、適用範囲、controller、trimの計算規則は
[合成playableモデル契約](playable-hybrid-model.md)を正本とする。
neutralではproxy静荷重を再加算せず、実行時は7列を個別にPWL補間する。
理想e=1、profile drag、主翼固有Cm=-0.02はsoftware仮定である。

質量はairframe24 kg・pilot70 kg、datum慣性はdiag(900,1000,980) kg m²である。
pilotのy=z=0、前後範囲±0.4 m、最大速度0.3 m/s、最大加速度0.8 m/s²を維持する。
launchは指定された会場の合成重心位置・方位を使用し、yawとtrim pitchを合成する。
trimのground velocityをCG→datum変換へ渡し、風を暗黙加算せず、platform傾斜をpitchへ混入しない。
pilot速度は0とし、`TailPilotPositionMapping`のneutralとHoldはtrim位置を保持する。

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

## Current-reference hybrid（BPG-039）

`HybridModel`は一つの全機static polarと、重複しない主翼・水平尾翼・垂直尾翼の
`HybridSurface`を借用する。面を独立評価する解析fixtureでは一部の面だけを構成できる。
対応する尾翼を欠く非zero incidenceは`UnsupportedControl`となる。
`HybridAerodynamicLoad`は正の密度と一つの定常風場を保持し、構築・評価にheapとI/Oを要しない。

### 面形状・frame・anchor

`HybridSection`はbody FRDのquarter-chord点と非負chordを持つ。zero-chord tipを許可し、
全区間の面積は正とする。投影span座標は主翼・水平尾翼でbody y、垂直尾翼でbody zであり、
厳密な昇順とする。隣接sectionのchordとquarter-chord点を線形補間する。
投影幅$\Delta u$とy-z面内の実幅$\Delta s$を区別する。sweepのx成分は作用点へ反映し、
span面積へ加えない。実面積は片面のplanform面積であり、上下両面のwetted areaではない。

```math
S_{projected}=\sum_j\Delta u_j\frac{c_j+c_{j+1}}2,\qquad
S_{surface}=\sum_j\Delta s_j\frac{c_j+c_{j+1}}2
```

```math
\int_j c(u)^2du=\frac{\Delta u_j}3(c_j^2+c_jc_{j+1}+c_{j+1}^2),\qquad
MAC_{projected}=\frac{\int c(u)^2du}{S_{projected}},\qquad
AR_s=\frac{b_s^2}{S_{projected,s}}
```

surface MACは同じ積分の$\Delta u$を$\Delta s$へ置き換える。slopeには面全体の投影ARを使い、
strip幅のARを使わない。主翼が存在する場合、全機polarの$S,b,c$を主翼の投影面積・全幅・MACと照合する。
`HybridProxy`は一つの直線section区間内を覆い、正の実面積weightと面積重心のquarter-chord作用点$r_i^B$を持つ。
区間の隙間・重複・別geometryへの流用を拒否し、weight和を面積と照合する。
`MirrorSpan`はsection・proxy・anchor・frameの反射対称性を要求する。
構造照合の相対許容差$10^{-10}$は幾何検査だけに用い、flow境界を緩和しない。

local→bodyの正規直交frame $Q_i$はlocal spanをsectionのy-z方向へ整合させ、twist・dihedralを含む。
固定normalは$n_i^B=Q_i(0,0,1)$である。既に投影したspanへdihedralのcosを再び乗じない。
`HybridAnchor`は有限の$C_{L,anchor,i}$と幾何迎角$\alpha_{anchor,geom,i}$を持つ。
anchorと有限翼slopeに含まれる誘導効果へXFLRのAiを再適用しない。
proxy作用点は全機staticの固定moment参照点$P$と独立である。

### 現在の流れとの差分

各RK stageでdatumの現在の$V,\alpha$からreferenceを作る。
referenceのbeta・rate・局所wind差・tail incidenceは0とする。固定の元解析迎角・速度へ戻さない。
datum対気速度のbody y成分が厳密に0なら$v_{ref}^B=v_O^B$をそのまま使用し、
三角関数の再構成誤差や微小残差のclampを追加しない。

```math
v_{ref}^B=V(\cos\alpha,0,\sin\alpha),\qquad
p_i^N=p_O^N+R_{NB}r_i^B,\qquad
v_i^B=v_O^B+\omega^B\times r_i^B-R_{NB}^{T}(W_i^N-W_O^N)
```

actual/referenceのlocal速度は$Q_i^T v_i^B$、$Q_i^T v_{ref}^B$であり、
幾何迎角はforward/downからatan2で得る。理想有限翼の小擾乱slopeを次式で固定する。
これは今回の近似であり、機体ごとの同定値ではない。小さい正ARにも正のslopeを保持する同値式で評価する。

```math
a_s=\frac{2\pi}{1+2/AR_s},\qquad
C_{L,i}=C_{L,anchor,i}+a_s(\alpha_{geom,i}-\alpha_{anchor,geom,i}+\delta_i),\qquad
q_i=\frac12\rho\lVert v_i^B\rVert^2,\qquad q_{ref}=\frac12\rho V^2
```

`TailIncidence`はphysical effective incidenceの$\delta_e,\delta_r$だけを保持し、主翼の$\delta_i$は0とする。
旧操縦intentやbody-axis正指令と同じ符号を仮定しない。後方の水平尾翼で正$\delta_e$は負pitch momentを生む。
垂直尾翼はlocal spanがbody +z、normalがbody -yのframeを使用すると、正$\delta_r$で正body側力・負yaw momentとなる。
同じframeで正betaは負側力・正yawの復元moment、正yaw rateは負yawの減衰momentを生む。

```math
\Delta F_i^B=-S_i\{q_i C_{L,i}(actual,\delta_i)-q_{ref}C_{L,i}(reference,0)\}n_i^B
```

```math
F_O^B=F_{static}^B+\sum_i\Delta F_i^B,\qquad
M_O^B=M_{O,static}^B+\sum_i r_i^B\times\Delta F_i^B
```

全機staticを一度評価し、固定normalの差分とその外積だけを追加する。
旧5要素の全荷重、proxy drag・固有Cm、独立rate derivative、操舵momentを加算しない。
controlで力方向を回転せず、全機staticの風向基底変化を二重計上しない。
増分の符号を保ち、非負clamp・fallback・人工dragを導入しない。

### 閉境界・零速・失敗

次の小擾乱範囲はsoftware上の方針であり、実機の測定限界を意味しない。

| 条件 | 受理範囲 |
|---|---|
| static alpha | polarの両端を含む閉区間 |
| global beta、$\delta_e,\delta_r$ | それぞれ絶対値$\le0.2$ rad |
| 全proxyのlocal alpha差 | $|\alpha_{actual}-\alpha_{reference}|\le0.2$ rad |
| control込みlocal差 | $|\alpha_{actual}-\alpha_{reference}+\delta_i|\le0.2$ rad |
| local span angle、actual/reference両方 | $|\operatorname{atan2}(v,\sqrt{u^2+w^2})|\le0.2$ rad |
| 全proxy actual速度norm | $0.8V\le\lVert v_i^B\rVert\le1.2V$ |
| local forward、actual/reference両方 | 厳密に正 |

密度・参照量・係数・stateの有限性・正値・姿勢等の既存条件も維持する。
速度比を$V$で除算せず、minimum airspeedやruntime reduced-rateの$1/V$を追加しない。
小さい正速度でも全境界を検査し、動圧のunderflowと角度未定義を区別する。
$V=0$では構成・state・controlを検証した後、全proxyの風とactual流を走査する。
Oと全proxyのactual速度が全て0の場合だけ空力0とする。局所回転流・差動windが存在すれば
`UndefinedReference`の範囲外となり、後続点のwind failureも検査する。

`AerodynamicEvaluationError::Hybrid`は元の`AeroError`、`HybridSite`、適用範囲の`HybridLimit`を保持する。
siteはDatum・StaticPolar・Surface・surface内Proxy index・TailIncidence・Aggregateを区別する。
RK4境界は`AerodynamicStage`のFirst〜Fourthを付与し、StaticPolar/Hybridの元原因を保持する。
任意stageで失敗したtickは非commitとなる。非有限計算・wind failure・unsupported controlは
`OutsideEnvelope`へまとめず、元のfatal causeを維持する。

### 現行Scenario

TailFlightScenarioParametersはlaunch、tick-zero state/incidence、contactを検証する。
TailFlightScenarioは同じHybridAerodynamicLoadを荷重・telemetry・wind queryへ使用する。
制御値はTailIncidenceとして直接評価し、三軸actuatorへ変換する経路を持たない。
flowとcontrolの動的範囲は各RK4 stageで検査し、設定整合性と飛行時の範囲保証を区別する。

## 局所流と荷重合成

Hybridの各strip/proxyは評価点で位置依存windをsampleし、datum対地速度とbody回転速度から局所対気速度を導出する。
pilotの機体内相対速度を固定翼の速度へ加算しない。
各surfaceの固定normal、geometry、anchor、physical tail incidenceからnormal-force差分を評価する。
全機static荷重は一度だけ加え、datumまわりの作用点momentも一度だけ加える。
翼左右の非対称局所流はroll moment、後方水平尾翼の上向き荷重増分はnegative pitch moment、
後方垂直尾翼のrightward荷重増分はnegative yaw momentを生成する。
各符号はFRDとNEDの公開座標契約へ従い、具体的offsetとforceの外積で検査する。

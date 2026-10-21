# 飛行力学

- 本書は、共有Rust coreの座標系、6DoF、風場、空力、発進、着水判定を定義する。
- パイロットの移動質量と操縦保護は[飛行制御](flight-control.md)、外部データと保存形式は[データ形式](data-and-formats.md)、数値検証は[検証](verification.md)に従う。
- 公開機体は合成モデルである。
- 数値計算の正しさ、明示した入力列での飛行、実機の予測精度を分けて評価する。

## 座標系と単位

単位はSIとし、距離m、速度m/s、角度rad、角速度rad/s、質量kg、力N、モーメントN mを使用する。
内部数値は`f64`を基本とし、描画境界で必要な`f32`等へ変換する。

| 座標系 | x | y | z | 原点 |
|---|---|---|---|---|
| Navigation（NED） | 北 | 東 | 下 | 発進地点の水平位置と基準湖面の交点 |
| Body（FRD） | 前 | 右 | 下 | パイロットを除く機体の固定重心 $O$ |
| 描画用world | 東 | 上 | 南 | NEDと同じ原点 |

- 彦根会場の水平原点はWGS84緯度35.294075°、経度136.254448°とし、局所NEDの $N=E=0$ に対応させる。
- 鉛直原点は描画用の平均水面とする。
- 合成重心高度 $h$ のNED下向き成分は $-h$ である。
- 発進台前縁は水面上約10 mにあり、台の幾何形状と機体の発進高度を個別に管理する。
- 楕円体高・標高・geoid補正・湖面基準・基準epochはオフライン処理で明記する。物理高度は局所NEDの湖面基準から求める。

- `Point`と`Vector`、`BodyFrame`と`NedFrame`を型で区別する。
- `Point − Point`は`Vector`、`Point + Vector`は`Point`であり、異なるframeの暗黙加算を禁止する。
- 機体構造の固定点はdatum $O$ からのoffsetで表す。
- パイロットの移動後も、形状の原点はdatumに保持する。

姿勢quaternionはHamilton積、scalar-first $(w,x,y,z)$、単位長、body-to-NEDの能動回転とする。
$q_{NB}$ と $R_{NB}$ はbodyのベクトルをNEDへ変換する。

```math
v^N=R_{NB}v^B,\qquad v^B=R_{NB}^{T}v^N
```

- 角速度 $(p,q,r)$ はbody表現の右手系であり、正rollは右翼下げ、正pitchは機首上げ、正yawは右旋回方向である。
- Euler角は表示時にquaternionから導出する。
- quaternion補間は $q$ と $-q$ の同一姿勢を考慮して最短経路を選ぶ。

描画座標への変換は次式とする。

```math
\begin{bmatrix}X\\Y\\Z\end{bmatrix}
=C\begin{bmatrix}N\\E\\D\end{bmatrix},\qquad
C=\begin{bmatrix}0&1&0\\0&0&-1\\-1&0&0\end{bmatrix}
```

- モデルのローカル軸も $C$ で変換済みなら、描画姿勢は $C R_{NB} C^T$ である。
- asset固有の前方軸が異なる場合は取り込み時の固定変換を記録する。
- Three.jsのquaternion引数順 $(x,y,z,w)$ への変換は描画adapterで行う。

## 状態と6DoF積分

- `FlightState`はdatumのNED位置・対地速度、body-to-NED quaternion、body角速度、パイロットの前後位置・相対速度を保持する。
- 合成重心の位置・速度・慣性は[移動質量の力学](flight-control.md#移動質量の力学)から導出する。
- body成分の対地速度は $R_{NB}^{T}v^N$ であり、対気速度と区別する。
- 質量・慣性・空力モデル・環境・操縦入力・積分刻みを明示的に渡す。

### 固定質量時の基準方程式

- 以下はパイロット相対位置が固定され、合成重心基準の状態へ変換した場合の基準式である。
- 移動中は[移動質量の運動量収支](flight-control.md#移動質量の力学)を使用する。
- 約500 m・数十秒のフライトを対象とし、地球回転と局所NEDの曲率を省略する。
- 空力合力と重心まわりのモーメントをbody成分で受け取る。

```math
\dot p^N=v^N,\qquad
\dot v^N=\frac{R_{NB}F^B}{m}+\begin{bmatrix}0\\0\\g\end{bmatrix}
```

```math
\dot\omega^B=I_B^{-1}\left(M^B-\omega^B\times(I_B\omega^B)\right),\qquad
\dot q_{NB}=\frac12 q_{NB}\otimes(0,\omega^B)
```

対称機の積慣性は次の符号で表す。外部形式では $I_{xz}$ が行列成分か積慣性かを明記する。

```math
I_B=\begin{bmatrix}I_{xx}&0&-I_{xz}\\0&I_{yy}&0\\-I_{xz}&0&I_{zz}\end{bmatrix}
```

### 機体と慣性の検証

- `AircraftModel`はパイロットを除く機体質量、datumまわりの固定慣性、パイロット質量・固定高さ・移動限界を保持する。
- 機体質量は正、パイロット質量は非負とし、全値の有限性を検証する。
- `InertiaTensor`は対称正定値性に加え、次の半正定値性を検査する。

```math
\Sigma=\tfrac12\operatorname{tr}(I)\operatorname{Id}-I
```

- 主慣性の三角不等式は等号を許容する。
- $\Sigma$ の全7主小行列式を非負、$I$ の先頭主小行列式を正とする。
- 根拠は[Wensing et al., Eq. 9・13](https://arxiv.org/pdf/1701.04395)である。

- 入力binary64を厳密な二進数として扱い、指定点まわりの慣性を検査する。
- $2\Sigma$ の各項をmantissaとexponentへ分解し、次数3以下の積の符号を固定長整数で求める。
- 各項は $2^{-3222}$ の整数倍であり、最大162積の絶対和は6305 bit未満となる。
- 100個の64 bit limbへ収容し、heap・追加依存・traceや積のoverflow/underflowを回避する。
- 厳密に表現された回転後の境界テンソルは受理し、外部計算で丸めた成分も最終binary64値で再検査する。
- 正定値性の失敗は`NonPositiveDefiniteTensor`、物理整合条件の失敗は`NonPhysicalInertiaTensor`とする。
- 機体生成時に検証し、移動質量の連成行列と各RK4 stageでは保存済みテンソルを使用する。

### RK4と原子的更新

- `advance`は正の有限timestep、有限の一定パイロット加速度、重力、機体モデル、現在状態、外力providerを受け取る。
- providerは空力などの非重力外力をbody-frame wrenchとして返す。
- coreが全質量への重力合力と、パイロット重力のdatumまわりモーメントを加える。
- 運動量収支の未知量はbody成分の $d v_O^B/dt$ と角加速度である。
- 積分状態のNED速度へ変換する際は次式を使い、座標成分の微分と物理加速度を区別する。

```math
\dot v_O^N=R_{NB}\left(\frac{d v_O^B}{dt}+\omega^B\times v_O^B\right)
```

- 全4 stageで位置・速度・姿勢・局所風・荷重を再評価し、中間状態と出力状態のquaternionを単位長へ射影する。
- 一定加速度を保持するパイロットの位置・速度は各stage時刻と出力時刻で解析解を与える。
- 連成力はstageごとに再評価する。
- stage間のパイロット位置の極値も検査し、移動限界を超える加速度を拒否する。
- 成功した出力状態を返し、stageの失敗は元の型付きエラーとして返す。
- 力学stepは固定サイズの値と借用データで計算する。

## 定常風場

- `WindField`はNED位置に対する空気のNED速度、すなわち空気の移動先を返す。
- 気象資料の「吹いてくる方向」は出典の定義を確認してオフラインで変換する。
- 上昇流の $W_D$ は負である。
- 1フライトの基本環境は時間不変とする。

```math
W=W(x,y,z),\qquad V_{air}=V_{ground}-W
```

- 公開APIはUniform、Analytic linear gradient、Gridの検証済みconstructorを提供する。
- queryは`velocity_at(Point3<NedFrame>) -> Result<Vector3<NedFrame>, WindError>`とし、位置から風速を求める。
- linear gradientは基準NED位置・風速と、行を速度成分、列を位置成分とする $3\times3$ 勾配で定義する。
- Gridは不変の速度sliceを借用する。
- 共有`HybridSessionPreparation`が同じ風場を使う機体・発進・controllerを準備する。

- Gridの原点、N/E/D各軸の正の間隔、各軸2以上の点数、NED速度列を検証する。
- 配列はNが最速、次いでE、Dとし、添字は`((d * count_e) + e) * count_n + n`である。
- 三線型補間を用い、各軸の閉区間`origin .. origin + spacing * (count - 1)`を受理する。
- 上端は有限かつ原点より大きい必要がある。
- overflowまたは丸めによる区間の退化は`WindError::InvalidGridDomain`とする。
- 上端の添字換算に伴う丸めを補正する一方、1 ULPでも領域外の位置は拒否する。
- 全機の評価点を含む領域の確保は呼出側の責務であり、外部形式も同じcore検証を使って原因を`EnvironmentFormatError::Wind`へ保持する。

- 風は局所対気速度、$V/\alpha/\beta$/動圧、係数、力・モーメント、6DoF、FBW観測へ一貫して作用する。
- 向かい風・追い風・横風・上昇流・wind shear・左右非対称流を、各評価点の対気速度から計算する。位置の積分には対地速度を使う。

```math
\frac{dW}{dt}=(V_{ground}\cdot\nabla)W,\qquad \frac{\partial W}{\partial t}=0
```

- 定常場でも移動中に経験する風は変化する。
- 回転流の応答は、離散要素間の相対風速と荷重差から求める。
- 一様定常風では同じ対気初期条件の姿勢・対気運動を比較し、対地速度を固定した発進の試験と区別する。
- 時間依存gustとstochastic turbulenceは未対応であり、追加時には明示的なsimulation timeと状態のAPIを定義する。
- 環境データのschema、出典・仮定、波・空との対応は[データ形式](data-and-formats.md)に従う。

## 全機静的空力表

`StaticPolar`は一つの解析方式・離散設定・参照量を持つ、全機の非線型な静的空力表である。
`StaticPolarRow`は迎角`alpha_rad`と、`StaticPolarCoefficients`の独立した7列を持つ。

| 列 | 係数 |
|---|---|
| `lift` | $C_L$ |
| `induced_drag` | $C_{Di}$ |
| `profile_drag` | $C_{Dv}$ |
| `side_force` | $C_Y$ |
| `roll_moment` | $C_l$ |
| `pitch_moment` | $C_m$ |
| `yaw_moment` | $C_n$ |

- 全機のprofile dragをこの表へ集約し、総抗力 $C_D=C_{Di}+C_{Dv}$ は計算値とする。
- 2行以上、全値有限、迎角の厳密単調増加、$C_{Di},C_{Dv}\ge0$、共通面積 $S$・span $b$・MAC $c$ の正値・有限性を構築時に検証する。
- 負の揚力・モーメントは有効である。
- 内部値を非公開とし、検証済みconstructorから構築する。
- 各列を区分線型補間し、節点では元の行、両端では閉区間の値を返す。
- 外挿・端点clamp・失速後曲線の追加は行わず、有限入力のoverflowも型付きエラーとする。

- `StaticPolarMetadata`は解析方式、離散設定ID、非ゼロmodel versionを持つ。
- 設定IDは離散設定を識別し、解析方式・対象・参照量を揃えた一つの表を選択する。
- 行と設定IDを借用し、迎角に対して補間する。Re依存と設定間の補間を追加する場合は、表の軸と参照条件を定義する。

### 力とモーメント

全機静的荷重は各RK stageのdatum $O$ で風を評価する。

```math
v_O^B=R_{NB}^{T}(v_O^N-W_O^N),\qquad V=\lVert v_O^B\rVert,\qquad q_\infty=\frac12\rho V^2
```

```math
\alpha=\operatorname{atan2}(w,u),\qquad \beta=\operatorname{atan2}(v,\sqrt{u^2+w^2})
```

```math
e_V=(\cos\alpha\cos\beta,\sin\beta,\sin\alpha\cos\beta),\quad
e_L=(\sin\alpha,0,-\cos\alpha),\quad
e_Y=(-\cos\alpha\sin\beta,\cos\beta,-\sin\alpha\sin\beta)
```

```math
F_{static}^B=q_\infty S(-C_D e_V+C_Y e_Y+C_L e_L)
```

- $\beta=0$ の表を現在の $\beta$ の力基底で表すkinematic continuationは設計仮定であり、横滑りの非線型係数を同定したものとして扱わない。
- 実装は速度成分の比を使う同値式により、先に $1/V$ を求める微小速度のoverflowを避ける。

`moment_point_from_datum`はbody固定点 $P$ の $r_{OP}$ であり、移動する合成重心 $G$ と区別する。
`PolarMomentAxes`は`BodyFrd`または`WindAtBetaZero`を明示し、roll・yawは $b$、pitchは $c$ で次元化する。

```math
\widetilde M_P=q_\infty S(bC_l,cC_m,bC_n)
```

`WindAtBetaZero`は表の $\beta=0$ に対応する次の列ベクトルでbodyへ回転し、現在の $\beta$ では回転しない。

```math
e_x^0=(\cos\alpha,0,\sin\alpha),\quad e_y^0=(0,1,0),\quad
e_z^0=(-\sin\alpha,0,\cos\alpha),\qquad M_P^B=[e_x^0\ e_y^0\ e_z^0]\widetilde M_P
```

```math
M_{O,static}^B=M_P^B+r_{OP}^B\times F_{static}^B
```

- 評価順序は次元化、軸回転、datumへの移送とする。
- 上記のbody y軸回転は同じ $b$ を使うroll/yawだけを混合し、$\operatorname{diag}(b,c,b)$ と可換である。
- pitchとroll/yawを混合する一般回転では、$b\ne c$ の場合に順序が結果へ影響する。
- `BodyFrd`は次元化した値をそのままbody成分とする。
- 原点移送は一度だけ行い、XCP由来の追加モーメントを加えない。
- $P$ の点速度や風で表を再評価しない。
- 参照点を変える試験は、移送後のdatum荷重と移動質量系の応答を比較する。

- `StaticPolarLoad`は静的荷重だけを返す。
- `AerodynamicLoadProvider`はStaticPolarとHybridを排他的に選び、全機荷重を重複生成しない。
- 静的空力表単独では迎角未定義のゼロ流・純横方向流を`UndefinedFlowAngle`として拒否する。

## 現在の流れを基準とするハイブリッド空力

- `HybridModel`は全機静的空力表と、重複しない主翼・水平尾翼・垂直尾翼の`HybridSurface`を借用する。
- 独立した面の評価用fixtureは一部の面だけを持てるが、欠けた尾翼への非ゼロincidenceは`UnsupportedControl`となる。
- `HybridAerodynamicLoad`は正の密度と一つの定常風場を保持し、構築・評価にheapとI/Oを要しない。

### 面形状と局所frame

- `HybridSection`はbody FRDのquarter-chord点と非負chordを持ち、zero-chord tipを許容する。
- 全区間の面積は正とする。
- 投影span座標は主翼・水平尾翼でbody y、垂直尾翼でbody zとし、厳密昇順である。
- 隣接sectionのchordとquarter-chord点を線型補間する。
- 投影幅 $\Delta u$ とy-z面内の実幅 $\Delta s$ を区別し、sweepのx成分は作用点へ反映する。
- span面積には加えない。
- 実面積は片面のplanform面積とする。

```math
S_{projected}=\sum_j\Delta u_j\frac{c_j+c_{j+1}}2,\qquad
S_{surface}=\sum_j\Delta s_j\frac{c_j+c_{j+1}}2
```

```math
\int_j c(u)^2du=\frac{\Delta u_j}3(c_j^2+c_jc_{j+1}+c_{j+1}^2),\qquad
MAC_{projected}=\frac{\int c(u)^2du}{S_{projected}},\qquad AR_s=\frac{b_s^2}{S_{projected,s}}
```

- surface MACは同じ積分の $\Delta u$ を $\Delta s$ へ置き換える。
- slopeには面全体の投影ARを使い、strip幅のARを使わない。
- 主翼がある場合は全機表の $S,b,c$ を主翼の投影面積・全幅・MACと照合する。
- `HybridProxy`は一つの直線section区間を覆い、正の実面積weightと面積重心のquarter-chord作用点 $r_i^B$ を持つ。
- 区間の隙間・重複・異なるgeometryへの流用を拒否し、weight和と面積を照合する。
- `MirrorSpan`はsection・proxy・anchor・frameの反射対称性を要求する。
- 幾何照合の相対許容差は $10^{-10}$、flowの受理範囲は閉境界表に従う。

- local-to-bodyの正規直交frame $Q_i$ はlocal spanをsectionのy-z方向へ整合させ、twistとdihedralを含む。
- 固定normalは $n_i^B=Q_i(0,0,1)$ とし、投影spanへdihedralのcosを重ねて乗じない。
- `HybridAnchor`は有限の $C_{L,anchor,i}$ と幾何迎角 $\alpha_{anchor,geom,i}$ を持つ。
- anchorと有限翼slopeに含まれる誘導効果へXFLRのAiを再適用しない。
- proxy作用点と全機の固定モーメント参照点は独立である。

### 局所流と荷重差分

- 各RK stageの現在のdatum $V,\alpha$ からreferenceを作り、referenceのbeta・rate・局所風差・尾翼角を0とする。
- 固定の元解析迎角・速度へ戻さない。
- body y成分が厳密に0なら $v_{ref}^B=v_O^B$ を直接使い、再構成誤差や微小残差のclampを追加しない。

```math
v_{ref}^B=V(\cos\alpha,0,\sin\alpha),\qquad
p_i^N=p_O^N+R_{NB}r_i^B,\qquad
v_i^B=v_O^B+\omega^B\times r_i^B-R_{NB}^{T}(W_i^N-W_O^N)
```

- datumと全proxyの位置で同じ風場を評価する。
- 固定翼の点速度へパイロットの機体内相対速度を加えない。
- actual/referenceのlocal速度は $Q_i^Tv_i^B$ と $Q_i^Tv_{ref}^B$ であり、forward/downから幾何迎角を求める。
- 理想有限翼の小擾乱slopeを次式とし、小さい正ARでも正のslopeを保つ同値式で評価する。

```math
a_s=\frac{2\pi}{1+2/AR_s},\qquad
C_{L,i}=C_{L,anchor,i}+a_s(\alpha_{geom,i}-\alpha_{anchor,geom,i}+\delta_i),\qquad
q_i=\frac12\rho\lVert v_i^B\rVert^2,\qquad q_{ref}=\frac12\rho V^2
```

- `TailIncidence`は水平・垂直尾翼のphysical effective incidence $\delta_e,\delta_r$ を持ち、主翼の $\delta_i$ は0とする。
- 操縦intentやbody正方向の指令と符号を区別する。
- 後方水平尾翼の正 $\delta_e$ は負pitch momentを生む。
- 垂直尾翼のlocal spanをbody +z、normalをbody -yとするframeでは、正 $\delta_r$ は正body側力・負yaw momentを生む。
- 同じframeで正betaは負側力・正yawの復元moment、正yaw rateは負yawの減衰momentを生む。

```math
\Delta F_i^B=-S_i\{q_i C_{L,i}(actual,\delta_i)-q_{ref}C_{L,i}(reference,0)\}n_i^B
```

```math
F_O^B=F_{static}^B+\sum_i\Delta F_i^B,\qquad
M_O^B=M_{O,static}^B+\sum_i r_i^B\times\Delta F_i^B
```

- 全機静的荷重を一度だけ評価し、固定normalの差分とその外積を加える。
- 左右非対称な流れはこの差分からroll momentを生む。
- proxyの寄与は固定normalに沿う符号付き荷重差と、その作用点によるモーメントである。全機静的表が基準のdrag・固有モーメントを持つ。
- 近似範囲は定常・固定Reの表と離散要素の局所荷重差であり、wake・動的失速・地面効果のモデル化は別途必要である。

### 閉境界と失敗条件

以下はソフトウェアモデルの受理範囲である。

| 条件 | 受理範囲 |
|---|---|
| static alpha | polar両端を含む閉区間 |
| global beta、$\delta_e,\delta_r$ | 各絶対値 $\le0.2$ rad |
| 全proxyのlocal alpha差 | $\lvert\alpha_{actual}-\alpha_{reference}\rvert\le0.2$ rad |
| control込みlocal差 | $\lvert\alpha_{actual}-\alpha_{reference}+\delta_i\rvert\le0.2$ rad |
| actual/referenceのlocal span angle | 各 $\lvert\operatorname{atan2}(v,\sqrt{u^2+w^2})\rvert\le0.2$ rad |
| 全proxyのactual速度norm | $0.8V\le\lVert v_i^B\rVert\le1.2V$ |
| actual/referenceのlocal forward | 各厳密に正 |

- 密度・参照量・係数・状態の有限性、正値、姿勢等の条件も検査する。
- 速度範囲は $0.8V$ と $1.2V$ の積で比較する。
- 小さい正速度でも全境界を検査し、動圧のunderflowと角度未定義を区別する。
- $V=0$ では構成・状態・controlを検証した後、全proxyの風とactual流を調べる。
- datumと全proxyのactual速度が全て0の場合に空力0を返す。
- 局所回転流・差動風があれば`UndefinedReference`として拒否し、後続proxyの風の失敗も検査する。

- `AerodynamicEvaluationError::Hybrid`は元の`AeroError`、`HybridSite`、`HybridLimit`を保持する。
- siteはDatum・StaticPolar・Surface・surface内Proxy index・TailIncidence・Aggregateを区別する。
- RK4境界は`AerodynamicStage`のFirst〜Fourthを付与し、元の原因を保持する。
- 非有限計算、風の失敗、unsupported controlは、それぞれ固有の型付きエラーとして保持する。
- 全stageと公開終端の検査に失敗したtickはcommitせず、直前のactuator・パイロット・tickを含む状態を保持する。

## 公開Playableモデル

- 公開既定は`bpg041-playable-hybrid-mock`のmodel 4、catalog 6 / scenario 6、`bpg040-tail-rate-feedback`のcontroller 3である。
- `assets/aircraft/playable-glider-v4.json`が質量・慣性・section・strip・anchor・全機polar・構造外形・視点mountを定義する。contentの`AircraftDefinition`が検証済みdocumentと借用viewのstorageを所有する。
- 定常滑空は汎用の`SteadyGlideTrim`が、質量モデル・全機polar・`SteadyGlideConditions`から求める。指定した迎角区間で力の釣合を解き、合成重心まわりのモーメントを満たすパイロット位置を求める。
- 現行catalog・scenario・controllerの組合せを登録する。保存snapshotは記録時のデータから閲覧する。

| 面 | 投影面積 / span / MAC [SI] | strip | body quarter-chordとframe |
|---|---|---|---|
| 主翼 | 約17.5551 / 20.863 / 0.90170 | 全sectionとstrip境界で分割 | 69断面のテーパー翼・二段折れ上反角 |
| 水平尾翼 | 2.5 / 3.4 / 2.5/3.4 | 8 | $(-3.6,y,0.1)$、body frame |
| 垂直尾翼 | 0.5 / 0.7 / 0.5/0.7 | 4 | $(-1.8,0,z)$、$z\in[-0.45,0.25]$、$R_x(\pi/2)$ |

- 作用点は各区間の面積重心とし、線型に変化する翼弦・sweep・上反角から求める。
- 投影面積、実面積、MAC、局所frameを同じsectionから導出する。
- body alpha=0・正の前進流を各frameへ変換した幾何alphaをanchorへ保持する。
- anchor CLは主翼0.70、水平尾翼-0.225、垂直尾翼0とし、面全体の $AR=b^2/S$ から有限翼slopeを求める。
- 全機参照の $S,b,c$ は主翼の投影面積・span・MACから導出し、参照点は $P=O$、moment軸は`WindAtBetaZero`とする。

- 機体40 kg、パイロット60 kg、datum慣性 $\operatorname{diag}(900,1000,980)$ kg m²を合成入力とする。
- パイロットの $y=z=0$、前後範囲±0.4 m、最大速度0.3 m/s、最大加速度0.8 m/s²を使用する。

- datum alphaの閉区間は[-0.18,+0.18] rad、約±10.31°とする。
- finの参照span角、各proxyのactual local flow、control込み迎角差を、汎用hybridの閉境界に照らして個別に検査する。

- 全機の9節点・7係数列をJSONに保持し、迎角順序、有限値、抗力等の物理境界を検証する。
- 実行時は保存された全機係数のPWL補間と局所荷重差を評価する。範囲外の入力は元の型付きエラーを返す。

- trimは生成済みPWL列に対して求める。
- $V=9.7$ m/s、$\rho=1.225$ kg/m³、$g=9.80665$ m/s²、alpha探索区間[0,0.06] radを使用する。
- datum荷重と合成重心まわりモーメントの釣合いからパイロット位置を計算する。
- 中立・`Hold`の初期目標はtrim位置とし、身体mappingの両端は±0.4 mである。
- controllerのsoft迎角保護は[飛行制御](flight-control.md#迎角と身体目標のsoft保護)に従う。

- 実データ導入時には来歴・座標・参照量・版・単位を再確認する。
- [XFLR5著者のmoment軸説明](https://sourceforge.net/p/xflr5/discussion/679398/thread/9fc00666c9/)と[解析上の制限](https://flow5.tech/xflr5/docs/Part%20IV:%20Limitations.pdf)を参照し、個別表の解析条件と`WindAtBetaZero`の対応を明示する。
- LLT/VLM、粘性・剥離・抗力評価の制約と、PWL補間・kinematic continuationという実装上の設計判断を区別する。

## 固定tickと発進

```math
\Delta t=0.01\ \mathrm{s},\qquad f_{physics}=100\ \mathrm{Hz}
```

- 固定tickの周期はcoreの公開定義を参照する。
- 成功時だけ整数tickと全physical stateを更新する。
- 描画は前後の有効snapshotを補間する。物理tickは共有coreの固定周期に従う。
- 遅延時もtickを飛ばさず、frame当たりの処理量を制限する。
- 回復不能な遅延と非表示状態では明示的に停止し、復帰後は保持した物理時刻から再開する。
- 入力を整数tickへ割り当てて保存する。
- 再現条件は、同じ初期条件と整数tickに割り当てた入力列である。

- 発進条件はdatumの対地位置・速度、姿勢、身体初期位置・速度を明示する。
- 合成重心の位置・対地速度を指定する形式は、`flight_state_from_composite_cg_launch`が身体状態・姿勢・角速度からmass-ratio offsetを使ってdatumへ変換する。
- trimからの生成では対気条件を明示し、CG地点の風を一度加算して対地速度を得る。
- 会場の位置・方位は共有の`assets/biwa-launch-venue.json`に従い、北から時計回り315°をheadingとcourse axisへ使用する。
- NEDの風を機体方位とともに回転せず、yawとtrim pitchを合成する。

- WeatherはCalm / Mild / Typical / Challenging / NearLimitを分類とし、具体的なscenario ID/versionと区別する。
- 登録環境は順に1/2/6/4/5であり、Typicalは共有のオフラインGrid環境6、その他は登録済みUniform環境を使用する。
- 0–5 m/sはゲーム設計上の主要範囲であり、公式運用限界として扱わない。
- 地上観測から湖上の3D場を一意に復元する仮定は置かず、観測・補間・仮定・適用範囲・seedを記録する。
- 環境のproviderは荷重・CG telemetry・位置queryで共用し、欠損した風を無風へ置換しない。

## 静水面への接触と採点

静水面はNEDの $D=0$ とし、登録した接触点のいずれかが $D\ge0$ へ到達した最初の時刻を使用する。
固定接触外形は共有`f64`構造形状から作る有限polytopeであり、次の頂点を一度構築してbody-fixed proxyへ登録する。

- cockpitは10断面の24角輪郭を使用する。
- 主翼と尾翼はJSONの全sectionで閉翼型輪郭を使用する。全chordを共通のcosine分布32区間で分割する。
- 桁はJSONの両端と半径から3D軸・直交frameを導出し、各端面を24角の輪郭として登録する。

- 各部分の凸包の全候補頂点を、物理用の共有`f64`構造形状から導出する。
- cockpit frameは同じshell内へ収める。
- 現在の登録contactは中立外形の固定支持点を用い、cockpitのframeとrimは半径を含めて外皮へ内包させる。
- 主翼の曲げ変形はcontentの共通producerが所有する。JSONの全質量・重力・EI・左右の半spanから等分布荷重の片持ち梁解を求め、基準対気速度に対する二乗荷重倍率を0.3〜1.6へ制限する。機体準備時に最大倍率の両翼端変位の有限性を検証し、表示mesh・桁・ribへ同じFRD変位を投影する。
- coreの`WingBendingFactor`は有限な符号付きの基準変位倍率と、閉区間内の線型補間を提供する。速度からの導出則と運用範囲はcontentが所有する。
- `AircraftDefinition::write_deformed_contact_points_with_factor`は準備済みの点順と可動群を保持し、明示係数と物理尾翼角からcaller所有のsliceへFRD点群を書く。速度入口は係数を一度導出して同じ処理へ委譲する。
- 水平尾翼はbody +Y、垂直尾翼はbody +Zを回転軸とし、両端の四分の一翼弦点の中央を回転中心とする。共有表示も同じ中心を使う。
- 同じ群の連続同一点を準備時に整理し、異なる可動群の境界点は別々に保持する。呼出側は成功した点群を採用し、演算失敗時の出力を再利用buffer内へ留める。
- 主翼表示は同値係数でframeを再利用する。明示係数からmesh・桁・ribを更新し、全bufferの更新成功後に係数を確定する。演算失敗後は次の更新で全bufferを再構築する。

- polytope内の点と回転後の鉛直座標は頂点の凸結合である。
- したがって全姿勢の静水面に対する支持量は全頂点の最大値と一致する。
- この有限形状に対する頂点省略誤差は0であり、`f64`の丸めと連続翼型・表示たわみの形状近似を別に扱う。
- 有限姿勢gridの表示頂点比較を、連続外形の全姿勢誤差上限として扱わない。
- 形状・荷重・trimの変更をaircraft modelとcatalog/scenarioの意味versionへ反映し、保存済みのidentityとJSON原文を保持する。

- 登録機体の接触点はdatum基準の固定順structural point群とする。区間開始の合成重心対気速度から準静的主翼係数を求め、尾翼の候補incidenceとともに点群を投影する。
- 主翼係数は100 Hz区間内で零次保持する。準備時にも同じ実対気速度から初期係数を求め、通常飛行とAttractはcaller所有の準備済みbufferを使用する。
- 隣接する成功tick間の状態を補間し、各点の到達fractionから最早eventを選ぶ。
- 位置・速度・角速度・身体状態は線型補間し、姿勢は短経路slerp、近角は既存nlerpを使う。
- 通常slerpでは線型項とsin/cosからdown座標の内部極値を隔離する。
- nlerpでは正の二次分母を払った三次式の導関数から区間を隔離する。
- 固定節点に一致しない接線と短い侵入・離脱も調べる。線型移動する対応点を含む隔離と丸めの契約は[検証仕様](verification.md#回転接触の隔離と丸め)に従う。
- 最初の到達区間を同じ補間状態上で二分探索して確定する。

- 終端の位置・姿勢・actuator・身体状態は同一時刻へ揃える。
- 区間開始時の既存形状を最初に調べ、接触済みなら直前snapshotの状態・incidence・pilot target・係数で終了する。入力採用直後の同時刻接触では更新後の値を終端境界eventへ保存し、正のfractionでは全RK4 stageへ保持した更新incidenceを使う。
- 内部接触検出はbody stateを補間し、tail tickが同時刻のphysical incidenceとpilot targetを付ける。
- `advance_tail_flight_tick_with_contact`はfractional terminal sampleだけを返し、接触後のinteger-tick stateを公開・保存しない。
- Result・graph・Replayは同じ終端sampleを参照する。

- `TailFlightScenario::run`は100 Hzの機器非依存入力列を順に適用し、最初のWaterContactまたはTimeLimitで終了する。
- Distance score v1は、発進course axisに対するdatumの符号付き水平変位projectionである。
- 右向きcross-track変位と直線水平変位長を独立metricとし、軌跡長・CG移動・公式競技計測と区別する。
- WaterContactではfractional terminal sample、TimeLimitでは最後の有効状態から採点して飛行を確定する。
- 接触面は静止した基準湖面、接触外形は採用した変形・尾翼角を反映した有限点群とする。

## 検証範囲

- 不正値、非正定値慣性、空力適用範囲外、非有限計算は型付きエラーとして返す。
- 失速域を使用する場合は、その範囲を含む係数モデルを別途検証する。
- native/WASM比較はscenario・tick入力列・許容差を事前に固定し、失敗結果へ許容差を事後適合させない。

- CLIの`probe_hybrid_playability` exampleとapp回帰はTypical環境・既定seed・最大4000tickを使用する。
- 通常列はNeutral、小pitch±0.05保持、100..109tickのfull nose-up pulse、pilot normalized±0.1への10tick移動とする。
- Manual・Shared（authority 0.5）・Automaticの18列でWaterContact/TimeLimitによる自然終端を検査する。
- 追加列はfull pitch±1保持、pilot±1への100tick移動、1秒full nose-up pulseとする。
- pilot移動後の`Hold`と、200tick以降も毎tick`Set(±1)`する列を区別し、合計39列でtyped failure、datum/CG alpha、q、速度、incidence、実身体位置、effective targetを観測する。
- 連続`Set`は保護解除後の元target再適用、振動、早期適用範囲超過を検査する。

- 全成功tickは全RK4 stageと公開endpointの荷重検査を通す。
- 保存sampleのactual local load再評価、要求report、両pitch符号、独立yaw、slew、guard無効時の一致、未定義alpha、First stageの風エラー、`Hold`/`Set`、未登録identityを回帰検査する。
- 独立polar/trim/oracleと離散線型化は、明示入力列の受入と区別する。
- 検査結果は、明記したソフトウェア条件と数値予算に対して評価する。

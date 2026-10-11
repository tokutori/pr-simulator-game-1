# Versioned playable hybrid model

[#264](https://github.com/tokutori/pr-simulator-game-1/issues/264)は、通常操作で空力適用範囲の終端へ到達しやすい公開mockを、
versionを分離した合成モデルとRust側の入力保護で改善する。実機の空力係数、失速、操縦性能の同定ではない。
`StaticPolar`の閉区間、既存6DoF式、moving pilot、100 Hz RK4、風、接触、scoreの契約を維持する。
迎角・姿勢・速度・身体位置の物理stateを補正しない。

## Identity

公開既定は bpg041-playable-hybrid-mock / model 2、catalog 3 / scenario 3、
bpg040-tail-rate-feedback / controller 3である。
Weatherのenvironment対応は1/2/4/5/6、Typicalは共有offline環境6を使用する。
旧catalog・scenario・controllerの登録と保存schemaの互換処理は提供しない。
snapshot保存値のqueryと現行モデルの再積分を区別する。

## Playableのgeometryとpolar

主翼は投影面積18 m²・span18 m・dihedral5°、水平尾翼は面積2.5 m²・span3.4 m、
垂直尾翼は面積0.5 m²・span0.7 mとする。airframe24 kg・pilot70 kg、datum慣性diag(900,1000,980) kg m²、
身体前後範囲±0.4 m、最大速度0.3 m/s、最大加速度0.8 m/s²である。
水平尾翼のbody quarter-chordはforward座標-3.6 m、垂直尾翼は-1.8 mとする。
尾翼の静的moment生成にも同じ3.6 mの腕を用いる。local flowのq由来寄与を含む通常hybrid式を変更せず、
尾翼の静的復元と動的減衰の釣合いを合成geometryで改善する。

全機参照はS=18 m²、b=18 m、c=1 m、datum O、`WindAtBetaZero`である。
datum alphaの閉区間を[-0.18, +0.18] rad、約±10.31°とする。
finの参照span角±0.2 radの内側に置くが、actual local flowの余裕をこの差だけで保証しない。
汎用hybridのbeta、local alpha/span/speed、controlled alpha differenceの制約を拡大しない。

| datum alpha [rad] | 主翼CL |
|---:|---:|
| -0.18 | -0.20 |
| -0.15 | -0.05 |
| -0.12 | 0.10 |
| -0.06 | 0.36 |
| 0 | 0.70 |
| 0.06 | 1.00 |
| 0.12 | 1.18 |
| 0.15 | 1.24 |
| 0.18 | 1.26 |

この節点CLは合成入力であり、正側の増加を緩やかにする。各節点で次の係数を一度生成する。
水平尾翼の面積は2.5 m²、ARは4.624、揚力傾斜は`2π/(1+2/AR)` rad⁻¹である。

```math
C_{L,t}=-0.225+a_t\alpha,\quad
C_{D i,t}=\frac{C_{L,t}^2}{\pi AR_t},\quad
C_L=C_{L,w}+\frac{2.5}{18}C_{L,t}
```

```math
C_{D i}=\frac{C_{L,w}^2}{18\pi}+\frac{2.5}{18}C_{D i,t},\quad
C_{D v}=0.03+1.5\max(|\alpha|-0.06,0)^2
```

profile drag増加係数1.5の単位はrad⁻²である。生成した全節点のdragは正となる。
尾翼のforce係数を面積で重み付けし、datum pitch momentへ同じ尾翼腕を適用する。

```math
F_{t,x}^{*}=2.5(-C_{D i,t}\cos\alpha+C_{L,t}\sin\alpha),\quad
F_{t,z}^{*}=2.5(-C_{D i,t}\sin\alpha-C_{L,t}\cos\alpha)
```

```math
C_m=-0.02+\frac{0.1F_{t,x}^{*}+3.6F_{t,z}^{*}}{18\cdot1}-2.0(\alpha-0.04)
```

最後の項は基準alpha 0.04 radに対する合成復元項であり、係数2.0の単位はrad⁻¹である。
CY・Cl・Cnは0とする。実行時は7列を個別にPWL補間し、上記の非線形生成式を再評価しない。
これは実機失速の外挿モデルではなく、定義域外は元のtyped failureで拒否する。

trimは生成済みPWL列から従来のsolverで解く。V=9.7 m/s、rho=1.225 kg/m³、g=9.80665 m/s²、
alpha探索区間[0,0.06] radを維持し、新Cmとdatum荷重からGまわりmomentが0となる身体位置を再計算する。
neutral身体指令は新trim位置であり、身体mappingの両端は±0.4 mのままである。
trimの成立は全状態の安定性や飛距離を保証しない。

## Controller 3のsoft保護

`TailAngleOfAttackGuard`はvalidated profileに保持するoptional policyである。
Playableの設定はalpha運用帯[-0.09,+0.09] rad、preview 1.0 s、pitch gain 1.6 s、
基準alphaは新modelのtrim datum alphaとする。これらは合成controller設計値である。
Manual・Shared・Automaticに同じpolicyを適用し、元のauthorityとq/r feedback gain 0.2 sを保持する。

guardが使うalphaは、datum Oで同じWindFieldを標本化し、ground速度から風を引いてbodyへ変換した
速度の`atan2(down,forward)`である。HUD・record telemetryの合成重心Gにおけるalphaとは区別する。
風・数学評価が不能、またはdatum alphaが未定義なら予測を適用せず、実RK荷重の元typed failureを保持する。

混合要求は一度だけ評価する。現在datum alphaから許容q帯を求め、物理elevator要求をその帯に対応する
`gain*(observed q - allowed q)`のincidence区間へ投影して、既存の1 rad/s slewを適用する。
rudder要求はこのalpha policyで変更しない。

```math
q_{lo}=\frac{-0.09-\alpha_O}{1.0},\quad
q_{hi}=\frac{0.09-\alpha_O}{1.0}
```

身体targetは、初期trim位置を基準とした危険側のbiasだけを連続的に絞る。
後方biasはnose-up・増alpha側、前方biasはnose-down・減alpha側である。
上側riskは`alpha + preview*max(q,0)`、下側riskは`alpha + preview*min(q,0)`とする。
trim alphaから各運用端までのheadroomの後半で残すbias係数を1から0へ下げ、運用端でtrim targetへ戻す。
反対側の身体targetは変更せず、固定normalized capを設けない。
effective targetへ身体を動かすのは従来のbounded acceleration policyとRK4であり、身体stateを直接変更しない。

`Set`の元normalized要求、`Hold`、manual intent、desired rate、manual/FBW/mixed要求はrecordに保持する。
stateと`resolved_pilot_position_target_m`は採用したeffective targetを保持する。
`Hold`は前tickのeffective targetを名目要求とし、作動中の保護はさらにtrim側へ絞り得る。
安全域への復帰だけで、過去の未適用targetを自動再開する状態を追加しない。

[#262](https://github.com/tokutori/pr-simulator-game-1/issues/262)のbounded controlled-tail探索と、
既存slew・全RK4段・公開weighted/fractional endpointの荷重検査を維持する。
内側alpha帯はsoft target policyであり、探索候補のhard constraintや全状態・将来の安全保証ではない。
特にpreviewでalpha変化をqから近似すること、spatial wind・flight-path連成・yaw域が独立することを限界とする。
不成功時にはlast valid stateと元のsite/cause/limit/stageを維持する。

## 受入検査

`probe_hybrid_playability`とsession回帰は共有Typical環境・既定seed・最大4000tickを用いる。
通常列はNeutral、小pitch±0.05保持、100..109tickのfull nose-up pulse、pilot normalized±0.1への10tick移動である。
Manual・Shared(authority 0.5)・Automaticの18列を旧v1と比較し、WaterContact/TimeLimitによる自然終端を要件とする。
追加列はfull pitch±1保持、pilot±1への100tick移動、1秒full nose-up pulseである。
pilot端点は移動後にHoldする列と、200tick以降も毎tick Set(±1)を反復する列を区別する。
後者はgamepadの連続要求による、clip解除後の元target再適用と振動・早期適用範囲超過を検査する。
これら39列でtyped failureの発生、datum/CG alpha、q、速度、incidence、実身体位置、effective targetを観測する。
受入はこの明示列と単体境界のsoftware検査に限定し、任意入力列や全weatherの安全性とは扱わない。

全成功tickは製品経路で全RK4段と公開endpointの荷重検査を通す。
回帰は保存sampleのactual local load再評価、要求report、両pitch符号、独立yaw、slew、
guard無効時の一致、未定義alpha、First stageの風error、Hold/Set、未登録identityを確認する。
独立polar/trim/oracle・離散線形化試験を、公開Playableの受入と区別する。

### 2026-10-09の実行結果

rootによる`cargo run -p birdman-game-session --example probe_hybrid_playability --locked`の
`target/hybrid-playability-v3-continuous.txt`では、連続Set列を含む39列すべてがWaterContact・failureなしで終了した。
Manualの代表値は次のとおりである。距離は同じ終端時刻のcourse-parallel scoreである。

| 入力列 | 終端時刻 [s] | 距離 [m] | datum alphaの記録範囲 [rad] |
|---|---:|---:|---|
| Neutral | 21.513 | 154.650 | [0.038982, 0.039015] |
| 小nose-up +0.05保持 | 21.298 | 149.443 | [0.026195, 0.075072] |
| pilot +0.1 | 19.158 | 139.848 | [0.018062, 0.055966] |
| pilot -0.1 | 21.816 | 153.880 | [0.025963, 0.066390] |
| full nose-up保持 | 25.128 | 169.069 | [0.039001, 0.078081] |
| pilot前方端点 | 18.437 | 135.200 | [-0.015139, 0.161664] |
| pilot後方端点 | 15.484 | 111.219 | [-0.009280, 0.096588] |
| 1秒full nose-up pulse | 20.880 | 148.139 | [0.007860, 0.102330] |

pilot端点を毎tick連続要求した6列も自然終端となった。Manualの前方要求は13.034 s・108.124 m、
後方要求は25.194 s・165.737 mである。SharedとAutomaticの対応する4列もWaterContactとなった。

旧v1の同じ小nose-up保持・pilot±0.1はStaticAlphaで終了したが、新candidateでは自然終端を得た。
前方端点の記録alphaは+0.09 radを超えており、運用帯がsoftであることも観測結果と整合する。
この結果はTypicalの明示39列に限る。任意入力、全weather、全RK内部stateの運用帯内保持、
全状態の安定性、実機性能、完全な適用範囲保護を証明するものではない。

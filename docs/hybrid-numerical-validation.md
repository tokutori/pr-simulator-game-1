# 公開hybrid mockの局所離散線形化

BPG-043（[#211](https://github.com/tokutori/pr-simulator-game-1/issues/211)）の第二検証単位として、
`tests/hybrid_linearization.rs`は公開100 Hz tick mapの局所Jacobian、離散固有値、微小擾乱の時系列を比較する。
物理・空力・controllerの製品実装を変更せず、実機の安定性やIssue全体の完了を示すものではない。
刻み幅検証、native/WASM/browserの横断比較、contact時刻の精度は別の検証単位である。

## 基準と状態座標

`HybridMockDefinition::try_new(Standard)`と`HybridMockTrim::try_new`から基準を構築する。
この基準はVersionOne/model 1であり、新公開既定のPlayable/model 2・controller 3へ置き換えない。
新モデルの入力列と保護の受入は[playableモデル契約](playable-hybrid-model.md)へ分離する。
密度は1.225 kg/m³、重力は9.80665 m/s²、対気速度は9.7 m/s、風は一様な0 NED vectorである。
headingは0、初期複合CGのNED位置は(0, 0, -100) mとする。trim値と右辺を試験へ転記しない。

ManualとAutomaticを個別に評価する。後者は公開`TailControlProfile`のq/r gain 0.2 s、slew 1 rad/sを使用する。
全tickのmanual intentとdesired q/rは0、pilot commandはHoldである。
Manualの「無制御」は尾翼FBWのauthorityが0という意味であり、pilot位置policyは動作を継続する。
pilotの位置・速度・held targetがtrim位置・0・trim targetから変化しないことを全RK stageとtickで確認する。
pilot移動やSet commandを摂動する系全体の微分は、この不変部分空間の検査に含めない。

線形化する11座標と正規化scaleを次の順序に固定する。

| 座標 | 物理単位 | 正規化scale |
|---|---|---|
| datum ground velocityのN/E/D差 | m/s | 各9.7 m/s |
| 基準bodyに対する局所回転vectorのx/y/z | rad | 各1 rad |
| body rate p/q/r差 | rad/s | 各1 rad/s |
| 水平尾翼・垂直尾翼のphysical incidence差 | rad | 各0.2 rad |

姿勢は入力を$q_{\mathrm{ref}}\operatorname{Exp}(\delta\theta)$、出力を
$\operatorname{Log}(q_{\mathrm{ref}}^{-1}q)$とする。同じbody-local chartを使用し、quaternionの冗長4成分は線形化しない。
NED位置3成分はzero-windの並進対称性として除外する。公開tickへ位置だけを平行移動した状態を与え、
残る11座標と位置の平行移動量が一致することを別回帰で検査する。この省略をgrid風へ適用しない。

基準trajectoryも公開tickで同時刻まで進める。基準の11座標変化を1e-9以下、datum位置と等速並進の差を1e-8 m以下とし、
同じ基準trajectoryとの差を擾乱応答とする。50tick後の局所Jacobianも初期Jacobianと比較する。

## 微分と時系列の判定

正規化座標ごとのcentral difference幅を$2^{-16}, 2^{-17}, 2^{-18}$に固定する。
速度の物理幅は各幅の9.7倍、incidenceは0.2倍、回転とrateは各幅と同じ数値になる。

```math
J_{:,j}(h)=\frac{F(x+h e_j)-F(x-h e_j)}{2h}
```

出力は次tickの基準に対する正規化座標である。幅間の最大成分差は3e-7以下、
細幅間の差は粗幅間の差に丸め誤差床2e-8を加えた値以下とする。
固有値絶対値を昇順に照合し、隣接する幅の差を1e-5以下とする。

擾乱幅は$2^{-12},2^{-13}$、時系列は最大50tick（0.5 s）、比較時刻は1/10/25/50tickに固定する。
全11基底の正負の非線形応答を同tickの基準trajectoryと比較する。
片側応答を幅で割った値と$J^n e_j$の最大成分差は3e-3以下とする。
幅半減後の最大誤差は元の0.7倍に丸め誤差床2e-7を加えた値以下とする。
正負平均によるflowのcentral derivativeと$J^n$の最大成分差は3e-5以下とする。
これは飛行性能の許容値ではなく、固定scaleでの局所一次近似の数値許容値である。

## 固有値と成立範囲

固有値kernelはhost dev-dependencyの固定`nalgebra 0.35.0`を使用する。
[`Schur::try_new`](https://docs.rs/nalgebra/0.35.0/nalgebra/linalg/struct.Schur.html)のepsを
64×f64 EPSILON、反復上限を2000に固定する。無制限反復を指定しない。
Schur再構成と直交性の最大成分残差を各2e-10以下とし、`complex_eigenvalues`を用いる。
結果は通常のtest出力へ複素固有値、絶対値、erased/neutral/decaying/growingを表示する。

これらは1tickの離散固有値である。絶対値が1より大きいmodeを増大、1より小さいmodeを減衰と分類し、
1の周り1e-6はneutral、絶対値1e-6未満はerasedとして別扱いする。
尾翼のtarget即時到達枝は直前incidenceへの依存を消すため、対応する2列と2固有値が0となることを検査する。
並進対称性に対応する3つのunit modeは座標から除外済みであり、残るheading対称性のunit modeを不安定と扱わない。
連続時間poleへ変換せず、50tickの非線形flow微分の固有値絶対値と$|\lambda|^{50}$を比較する。
許容差は各予測値に対し5e-4×(1+予測値)とする。不安定modeの観測を失敗回避のため隠さない。

各公開tickは成功したAdvancedだけを受理し、envelope逸脱・水面接触は検証区間の失敗とする。
reportが保持する同incidenceを使い、公開`advance`と元`HybridAerodynamicLoad`を委譲するobserverで再評価する。
公開tickとFlightStateの完全一致、RK4の4評価、全stageのstatic alphaが(0.03, 0.05) radであることを確認する。
全機polarの0/0.06 rad節点を跨がず、slewの到達枝とsaturation marginを各tickで確認する。
datum高度には90 m超の余裕を保つ。contact fixtureはdatumの1点であり、実機接触geometryの検査ではない。
pilot policyの移動・到達・制限、舵のslew/飽和、PWL節点、envelope終端を跨ぐ微分は扱わない。

## 依存境界と実行

数値kernelはWindows/Linux/macOSだけのdev-dependencyで、default featuresを無効にしてstdだけを指定する。
core製品依存は引き続きlibmのみとし、no_std/WASMの製品buildへhost数学kernelを追加しない。
repository checkerはCargo metadataのcrate/name/kind/target/version/featuresを限定照合し、
normal/build/untargeted dev/WASM targetや他crateへの同依存を拒否する。
target-specific dev依存の意味は[Cargo公式](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#development-dependencies)に従う。

```text
cargo test -p birdman-game-core --test hybrid_linearization --locked -- --nocapture
```

実行結果が出る前の条件を本書とtest定数で固定する。現時点のsource定義を検査成功や実機安定性の根拠にしない。

## Windows nativeでの初回観測

2026-10-08の対象2試験は、上記の幅・予算・入力・係数を変更せず成功した。
固有値数は複素共役の各根を個別に数え、neutral/erasedの判定には上記1e-6の帯を使用した。

| Mode | growing | decaying | neutral | erased | 最大固有値絶対値 |
|---|---:|---:|---:|---:|---:|
| Manual | 3 | 5 | 1 | 2 | 1.001404217661 |
| Automatic | 3 | 5 | 1 | 2 | 1.000864425616 |

| Mode | 粗幅間/細幅間の最大Jacobian差 | 幅2^-12/2^-13の最大片側応答誤差 |
|---|---|---|
| Manual | 7.673e-12 / 2.400e-11 | 1.079e-4 / 5.396e-5 |
| Automatic | 7.673e-12 / 2.400e-11 | 1.081e-4 / 5.405e-5 |

両modeに局所的な増大が残る。Automaticを安定化済みとは扱わない。
ここで照合したものは零風・stationary pilot・接触前0.5 sの局所離散mapである。
native/WASM一致、非線形大擾乱、長時間安定性、実browser/GPU/実機はこの観測の範囲に含まれない。

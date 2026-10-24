# 検証契約

- 本書は現在の検証項目、実行コマンド、数値許容差、失敗条件を定める。
- 個々の実行結果・環境・対象版はIssue、PR、実行ログへ記録する。
- 形式証明の対象と信頼境界は[形式検証](formal-verification.md)を参照する。

## 1. 検証の階層

| 階層 | 確認する内容 |
|---|---|
| 型・静的検査 | 不正状態の排除、依存方向、生成readonly型・schema、資産と出典の整合 |
| 単体・数値検査 | 解析解、不変量、独立した基準値、誤差予算、失敗時の保持 |
| 接続検査 | 本番core・WASM・DOM・ECS・保存I/Oの接続と資源寿命 |
| CUI描画 | 本番GPU pipelineの実行、画像、画面遷移、計器・操作の到達可能性 |
| 実端末受入 | 物理入力、ブラウザー、HMD・スマートフォン、通常表示の性能 |
| fidelity | モデルの出典・適用範囲・実対象との一致 |

- 各階層の結果は個別に判定する。
- ビルド成功を描画・実端末・実機精度の合格へ置き換えない。
- 期待値には解析解、不変量、独立した基準caseを用いる。
- 共有関数の出力を使う接続試験は、転送・投影を検査するものとして位置付ける。

## 2. 標準検査と実行コマンド

Rust toolchain、Node.js、依存ライブラリとVerusはリポジトリの固定設定を使用する。
通常CIはUbuntuとWindows、および独立したWindows形式検証ジョブで実行する。

~~~sh
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
cargo build -p birdman-game-core --target wasm32v1-none --locked
cargo build -p birdman-game-wasm --target wasm32-unknown-unknown --locked
cargo build -p birdman-game-cli --target wasm32-wasip2 --locked
cargo run -p birdman-game-cli --locked -- verify-hybrid-flight all
npm ci
npm run verify
~~~

- cross-target接続は`npm run test:cross-target -- target/cross-target-check`で検査する。Nativeとfresh WASMの同じApplicationRuntimeへ生観測とI/O完了を渡し、集約出力・保存原文・終了分類を照合する。入力接続世代・不規則なframe時刻・遅延回復は共有ownerの代表接続で確認する。
- Nativeの`--verify`は本番updateが受けた観測・文字計測結果・I/O要求を`application-trace.jsonl`へ記録する。描画はcapture境界だけを比較点とし、WASM再生へ同じ文字計測の成功・失敗を返す。
- 新規保存のJSONは実build来歴のtarget/profileを個別に記録し、それ以外の内容を比較する。公開Title操作からNativeの保存原文を読み戻し、同原文のJSON/CSV出力をbyte一致で検査する。
- cross-targetの浮動小数点値は絶対許容差 $10^{-9}$ と相対許容差 $10^{-10}$ の和で比較する。identity・比較key・source fingerprint・終了分類は完全一致、保存原文とそのCSV再出力はbyte一致とする。実行結果にはsourceとWASMのhashを記録する。
- Rustdocでは `RUSTDOCFLAGS=-D warnings` を設定する。
- Windows CIは共有・Web側のworkspace試験とNative単独試験を分離し、Native実行ファイルもビルドする。
- WASIはビルドと実行の結果を分ける。
- `npm run verify` はWASM生成を一度行い、型検査・lint・試験・資産/依存境界・本番ビルドを実行する。

追加の既存検査は次のとおりである。

~~~sh
python tools/allocation-check/main.py
python -m unittest discover -s tools/environment-build -v
python -m unittest discover -s tools/native-release -v
~~~

- 依存検査はcoreの `no_std`・I/O分離、crateのDAG、engine型の境界、TypeScriptの型依存を含む循環を検査する。
- CLI通常経路は`cargo tree -p birdman-game-cli --edges normal --locked`でapp・content・format・coreへの依存を確認する。集約契約の生成は`birdman-game-runtime`の`contracts` exampleと専用schema featureで実行する。
- 資産検査はmanifestの存在・hash・重複・登録漏れ、利用条件とViteの取込経路を検査する。
- 生成契約はRustの集約入出力からのfreshness、compilerによるreadonly型とtuple制約、実WASMと描画・I/Oアダプタの代表接続を確認する。
- 表示テストは必要な操作・相対順・到達可能性・非重複・表示先の意味を検査する。説明文や無関係なsectionの全列挙を仕様として固定しない。

## 3. 物理・空力・制御の基礎検査

### 解析解と不変量

`birdman-game-core/src/tests.rs` は、運動方程式の結果を解析解と運動量収支へ照合する。

| 検査 | 条件 | 許容差・判定 |
|---|---|---|
| 座標回転往復 | 90°のdown軸回転とforward単位vector | 各成分誤差 ≤ $10^{-15}$ |
| 自由落下 | $g=9.81$ m/s²、0.1 s | 位置・速度誤差 ≤ $10^{-13}$ |
| 定力並進 | 4 kg、12 N、0.2 s | 位置・速度誤差 ≤ $10^{-13}$ |
| 主軸回転 | $I_{xx}=2$ kg m²、2 N m、0.01 s | quaternion成分誤差 ≤ $10^{-13}$ |
| 縦方向の内部質量 | 機体4 kg・pilot 1 kg、加速度0.3 m/s²、0.1 s | 位置・速度と合成線運動量の誤差 ≤ $10^{-14}$ |
| 一般3Dの内部質量 | 非対角慣性、外力なし、pilot加速 | NED線運動量 ≤ $2\times10^{-10}$ kg m/s、角運動量 ≤ $2\times10^{-9}$ kg m²/s |
| 回転中の並進 | 外力なし、初速とyaw rate | NED速度誤差 ≤ $2\times10^{-12}$ m/s |
| RK4収束 | 非対称剛体の刻み0.02→0.01 s | 誤差比12–20。四次法の理論比16と照合 |
| 異常入力 | 非単位quaternion、無効慣性、範囲外pilot、非有限・overflow・失敗load | 元の型付き原因を保持し、途中状態を公開しない |

慣性は正定値と物理的に実現可能な主慣性の条件を検査する。
quaternionの正規化、最短補間枝と端点、subnormalから大きい有限値までのnormを検査する。

### パイロット・尾翼・風・採点

- pilot policyは加速・制動・移動範囲・速度上限・固定目標への収束・同条件決定性を検査する。
  tick内極値と全RK stageの一定加速度解析運動を照合し、4096 stepを超える停止系列や制動速度が数値上減少しない条件を型付きに拒否する。
- 二系統尾翼はauthority両端・Shared混合・飽和・slew・feedback符号・保持incidenceを検査する。
  全RK stageへ同じ確定操作を渡し、失敗時のpilot・actuator・tickの非commitを確認する。
- 迎角保護は本番の候補選択・適用域・slew/authority条件を通す。保護の成立と任意入力の全域安定性を分ける。
- 風は無風・一様風・headwind/crosswind/鉛直流・解析shear・grid trilinear補間を検査する。
  閉境界と直外、位置依存windの各RK stage評価、元のOutsideGrid/fatal原因を確認する。
- 発進変換はCGとdatumの位置・速度を一般姿勢、角速度、pilot移動を含めて復元する。
- scoreはcourse/cross-track/net horizontalの解析値、逆行、高度不変性、軸正規化とoverflowを検査する。

### Static polar

`aerodynamics::polar::tests` は7独立係数列と6成分wrenchを検査する。

- constructorは非有限値、点数不足、重複・逆順、負のCDi/CDv、非正参照量を拒否する。負の揚力・momentは有効である。
- PWLは全列のknot・補間・異なるslope・両端を照合し、直外を拒否する。係数誤差は概ね $10^{-15}$ とする。
- $(u,v,w)=(4,12,3)$ m/s、$S=2,b=4,c=0.5,\rho=1$ の独立wrenchで軸と次元化を確認する。
  演算scaleに対応する約32 epsilonを目安とし、力 $8\times10^{-13}$ N、moment $2\times10^{-12}$ N mを判定する。
- 固定参照点の変更でdatum wrenchと運動応答が一致すること、同じdatum流を使うことを確認する。
  100 tickの参照点回帰は位置・速度・角速度・quaternionの成分差 ≤ $10^{-12}$ とする。
- 借用期間はcompile-fail doctest、allocationなしの対象経路は独立計測、`no_std` は専用targetのbuildで検査する。

### Current-reference hybrid

本番geometry/providerを、公開可能な架空fixtureと独立した極限式へ照合する。
proxy増分の微係数はstatic基底の回転から分離し、矩形・無ねじり・中立状態を基準とする。

```math
\widehat p=\frac{pb}{2V},\quad
\widehat q=\frac{qc}{2V},\quad
\widehat r=\frac{rb}{2V},\qquad
C_{l_{\widehat p}}=-\frac{a_W}{6},\quad
C_{m_{\widehat q}}=-2a_T\frac{S_t}{S}\left(\frac{l_t}{c}\right)^2
```

```math
C_{Y_\beta}=-a_F\frac{S_f}{S},\quad
C_{n_\beta}=a_F\frac{S_f}{S}\frac{l_f}{b},\quad
C_{n_{\widehat r}}=-2a_F\frac{S_f}{S}\left(\frac{l_f}{b}\right)^2
```

- $l_t,l_f$ は後方を正とするarmである。
- beta差分では速度を一定にする。
- 複数central-difference幅を用い、矩形翼N stripの期待値 $-a_W(1-1/N^2)/6$ と連続極限を個別に検査する。

| 対象 | 判定 |
|---|---|
| geometry | 線型chordの厳密c²積分、quarter-chord、MAC、面積・AR、zero-chord tip、左右対称性 |
| slope | AR=2で $\pi$、AR=4で $4\pi/3$、大ARで $2\pi$、小ARで $\pi AR$ |
| 中立増分 | 複数alpha/速度で増分0・static一致。許容差は動圧等の演算scaleに対応 |
| 符号・風 | p/q/r、fin beta、tail incidence、dihedral、片翼gust、左右相殺、Galilean invariance |
| 適用域 | global beta、tail角、raw/controlled角差、actual/reference span角と速度比の閉境界・直外 |
| 零速・異常 | 全点静止の零荷重、datum静止と局所流の未定義参照、微小正速度、wind/density/overflowの原因 |
| 原子的更新 | 全4 RK stageの保持操作・失敗site/limit/stageと直前成功状態の保持 |

- actual/reference forwardは厳密に正である。
- proxy速度比0.8–1.2は参照速度に対する比である。
- 静的適用域、失速、wake、Re依存、実機精度はそれぞれのモデル契約に従う。

### 動的割当の計測

- `tools/allocation-check/main.py` はnative debug専用harnessを実行する。登録形状の経路では係数の導出・補間と固定順点群の更新をまとめて計測する。
- 本番provider評価・RK4・実tickの成功とGlobalBetaの型付き失敗を各10,000回計測し、 `alloc`・`alloc_zeroed`・`realloc` の各計数0を要求する。
- 同梱機体の準備済み点群をcaller所有sliceへ変形する経路を100回計測する。速度0・基準近傍・高荷重と正負の尾翼角を切り替え、同じ3種の割当計数0を要求する。
- 実`ApplicationSession`の準備を終えてから計測を開始し、発進後の全tickから自然着水まで、採用係数を持つ記録と3種の割当計数0を検査する。
- 校正では各1回を検出し、warmup・assert・出力は計測窓の外に置く。
- failureではdatum site・元cause・失敗stage・非commitも確認する。

test専用allocatorはSystemへ委譲し、内部でI/O・lock・panicを行わない。
[`GlobalAlloc`](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html)と
[`black_box`](https://doc.rust-lang.org/std/hint/fn.black_box.html)の最適化条件に従い、
実行環境での計測と、全最適化設定に対する証明を区別する。

## 4. Hybridの刻み半減と風境界

### 共通条件と入力

- `tail_simulation::numerical_tests` は現行Playableを100/200/400 Hzのtest-only driverで比較する。
- 製品は100 Hzを維持する。
- 100 Hz driverは同じ設定の公開tickと全状態が一致することを要求する。

- f64、密度1.225 kg/m³、重力9.80665 m/s²、対気速度9.7 m/s、CG高度50 mを基準とする。
- 初期body rateは $(0.02,-0.04,0.03)$ rad/s、controllerのq/r gainは0.2 s、slewは1 rad/sとする。
  初期incidenceを初期commandへ合わせ、開始時のslew過渡を分離する。
- 0.5 sの共通100 Hz時刻51標本を比較する。modeはManual / Shared(0.5) / Automaticを個別に扱う。
- Neutral、SmoothChanged、SlewReversalを分ける。
  変更系列は0.10–0.20 sにmanual intent $(0.01,0.005)$ とq/r demand $(0.01,-0.005)$ rad/s、
  0.20–0.30 sに逆符号、0.30 sにpilot normalized Set(0.2)、その他はneutral/Holdとする。
  SmoothChangedは0.20–0.21 sにneutralを挟む。
- Manualの4 mrad反転は400 Hzのslew上限2.5 mradを超える。100/200 Hzの非作動と400 Hzの作動を確認する。

- physics-onlyはFBW・actuator・pilot policyを100 Hzで評価し、各10 msの確定操作を1/2/4 RK4区間へ保持する。
- coupledはこれらも100/200/400 Hzで評価する。
- 全driverへ同じ物理時刻の入力を与える。
- 各load評価で実hybrid validatorを通し、接触前・選択したPWL区間内であることを確認する。

### 誤差予算

- 共通時刻での成分別最大絶対差を取り、100/400 Hz差を以下の上限で判定する。
- 姿勢距離には最短符号のquaternionで $4\operatorname{atan2}(\lVert q_1-q_2\rVert,\lVert q_1+q_2\rVert)$ を用いる。
- 同じ姿勢・逆符号・$10^{-10}$ radの小回転を別に検査する。

| 量 | physics-only | coupled | 丸めscale $s$ |
|---|---:|---:|---:|
| datum位置 [m] | $10^{-6}$ | $10^{-3}$ | 50 |
| datum速度 [m/s] | $10^{-6}$ | $10^{-3}$ | 10 |
| 姿勢距離 [rad] | $10^{-7}$ | $10^{-4}$ | 1 |
| body rate [rad/s] | $10^{-6}$ | $10^{-3}$ | 0.1 |
| pilot位置 [m] | $10^{-10}$ | $10^{-10}$ | 0.1 |
| pilot速度 [m/s] | $10^{-10}$ | $10^{-10}$ | 0.3 |
| physical incidence [rad] | $10^{-7}$ | $5\times10^{-4}$ | 0.01 |

```math
D_{200,400}\le cD_{100,200}+4096\epsilon s,\qquad
c=\begin{cases}1/8&\text{physics-only}\\3/4&\text{coupled}\end{cases}
```

- $\epsilon$ はf64 epsilonである。
- 丸めfloorは事前の演算余裕であり、厳密な丸め誤差証明ではない。
- coupled SlewReversalは刻み別のslew過渡が異なるため収縮率を要求せず、絶対予算・域内・製品一致を保持する。
- PWL knot、policy切替、飽和を跨ぐ一般条件へ同じ収縮率を適用しない。
- 400 Hzは細分参照であり、解析的厳密解や実機に対する絶対誤差保証とは区別する。

### 風・適用域上限近傍

一様風はNED $(0.3,0.1,-0.04)$ m/s、shearは同じ基準風とCG位置 $(0,0,-50)$ mを用いる。
gradientの行は風N/E/D、列は位置N/E/D、単位はs⁻¹である。

~~~text
[[0,     0,      0.015],
 [0.012, 0,      0    ],
 [0,    -0.003,  0    ]]
~~~

- 風2条件はSmoothChanged・0.5 sを比較し、datum alphaを $(0,0.06)$ radへ保つ。
- 上限近傍は公開polar上限から0.002 rad内側、零風・初期rate 0・静止pilot・neutral/Hold・0.05 sとする。
- この数値fixtureだけpitch gain 0.1 sを使い、公開controllerの0.2 sと分離する。
- 全条件で前節の量別予算・丸めfloor・収縮率を用い、datum downを−40 m未満に保つ。
- 上限近傍のalphaは最後のPWL区間内に置く。
- 区間内確認は補助観測であり、適用域は本番providerで判定する。

- 公開polar上限の $10^{-6}$ rad外側は全modeのFirst stageで `StaticPolar / OutsideEnvelope / StaticAlpha / First` として拒否され、入力状態を保持する。
- 後段stageの失敗保持も個別に検査する。
- 上限近傍の結果は選択したtest-local gainに限定する。

## 5. 局所離散線型化

`crates/birdman-game-content/tests/hybrid_linearization.rs` は登録機体を使って公開100 Hz tick mapを検査する。

~~~sh
cargo test -p birdman-game-content --test hybrid_linearization --locked -- --nocapture
~~~

### 基準と座標

- 登録JSONから準備した`AircraftDefinition`のpolar・trim・接触点を使い、trimを含む区分線型区間を全RK stageで保持する。
- 密度・重力・基準対気速度は登録JSONのtrim条件から取得する。零風、heading 0、CG位置 $(0,0,-100)$ mとする。
- ManualとAutomaticを分け、後者はq/r gain 0.2 s・slew 1 rad/sを使う。
- 入力はneutral/Holdである。
- 静止pilotの位置・速度・target保持を全stageで確認する。

| 11座標 | 正規化scale |
|---|---|
| datum ground velocityのN/E/D差 | 各trim対気速度 |
| 基準bodyに対する局所回転vector x/y/z | 各1 rad |
| body rate p/q/r差 | 各1 rad/s |
| 水平・垂直尾翼incidence差 | 各0.2 rad |

- 入力姿勢は $q_\mathrm{ref}\operatorname{Exp}(\delta\theta)$、 出力は $\operatorname{Log}(q_\mathrm{ref}^{-1}q)$ とする。
- 零風の並進対称性でNED位置3成分を除外し、位置だけを平行移動する別回帰で確認する。
- この省略を位置依存windへ適用しない。
- 基準trajectoryの11座標変化を $10^{-9}$ 以下、等速位置との差を $10^{-8}$ m以下とする。
- 初期と50 tick後のJacobianも比較する。

### 微分・擾乱・固有値の予算

```math
J_{:,j}(h)=\frac{F(x+he_j)-F(x-he_j)}{2h},
\qquad h\in\{2^{-16},2^{-17},2^{-18}\}
```

- 幅間の最大Jacobian成分差は $3\times10^{-7}$ 以下。細幅差は粗幅差 $+2\times10^{-8}$ 以下とする。
- 固有値絶対値を昇順に照合し、隣接幅の差を $10^{-5}$ 以下とする。
- 擾乱幅 $2^{-12},2^{-13}$、全11基底の正負、1/10/25/50 tickを比較する。
  片側応答の一次近似誤差は $3\times10^{-3}$ 以下、幅半減後は元の0.7倍 $+2\times10^{-7}$ 以下とする。
  正負平均のcentral derivativeと $J^n$ の差は $3\times10^{-5}$ 以下とする。
- host dev依存の `nalgebra::Schur::try_new` はepsを64 epsilon、反復上限2000とする。
  Schur再構成・直交性の最大成分残差は各 $2\times10^{-10}$ 以下とする。
- 固有値は絶対値1の周囲 $10^{-6}$ をneutral、絶対値 $10^{-6}$ 未満をerasedとして分離する。
  尾翼即時到達枝の2列と2固有値0を検査する。
  非線型50 tick flowの固有値絶対値を $|\lambda|^{50}$ と比較し、許容差は $5\times10^{-4}(1+\text{予測値})$ とする。

- 全tickはAdvancedを要求し、公開tickとの全状態一致・4 load評価・alpha $(0.03,0.05)$ rad・slew到達枝・飽和余裕を確認する。
- 接触点はdatum1点、datum高度は90 m超を保つ。
- pilot移動、PWL節点、slew/飽和切替、適用域終端を跨ぐ微分や長時間安定性は別の検証対象である。
- 増大modeは結果に記録する。
- 固有値の検査成功をAutomaticの安定化完了と扱わない。

## 6. 接触・終端時刻の数値検証

### Ballistic基準と事前上限

- `contact::tests::numerical` は外部空力なし・静止pilot・定姿勢を使う。
- NED水平速度 $(6,8)$ m/s、down速度 $w_0=2$ m/s、重力 $g=9.81$ m/s²、 接触点はdatumとbody down offset 0.4 mの2点とする。
- offsetを除いたclearanceを $h$ とする。

参照時刻は $t_c=0.4+(0.125,0.375,0.625,0.875)\times0.01$ sから初期高さを定め、
桁落ちを避けた $t_c=2h/(w_0+\sqrt{w_0^2+2gh})$ で計算する。
f64・100/200/400 Hz・最大1 sで、解析endpointの接触補間と実RK4 endpointの追加誤差を分離する。

```math
L(t)-z(t)=\frac{g(t-a)(b-t)}2\le\frac{g\,dt^2}8,
\qquad
0\le t_c-\widehat t_c\le\frac{g\,dt^2}{8w_0}
```

- 内部候補で区間を隔離した後、最大48回の二分幅 $dt/2^{48}$ を加える。
- 丸め余裕は $128(N+1)\epsilon S$ とし、解析endpointでは $N=0$、RK4では最大step数を使う。
- 位置scaleは高さ・offset・1秒並進と重力変位、速度scaleは初速と1秒重力増分、姿勢・rateのscaleは1とする。
- 時刻には位置余裕/$w_0$、二分幅、$8\epsilon\max(t_c,dt)$ を加える。

- course/cross/netは $(6,8,10)t_c$ mで、時刻誤差と対応速度から各誤差上限を作る。
- 刻み間差は双方の上限の和で判定する。
- off-grid phaseが刻みごとに異なるため、単調改善や固定収縮率を要求しない。
- 水平位置・鉛直速度・姿勢・pilot・接触点を同じfractionへ対応させ、実tail tickの保持incidence・record終端も接続検査する。
- 風格子のSecond/Third/Fourth stage失敗前の正fraction接触を、同じ実荷重による128分割の参照積分と50 ns以内で照合する。接触なしの場合の元診断、First stage拒否、接触solver失敗の伝播、部分区間fractionの換算を検査する。
- 200/400 Hzの区間indexを100 Hz保存recordへ変換しない。

### 回転接触の隔離と丸め

- 登録機体の採用形状: 速度5 / 9.7 / 13 m/sと左右roll・前後pitchを組み合わせた9条件で、1 mmと1 mの水面離隔から実providerを評価する。独立したquaternion回転・最短経路slerp・二分探索を参照とし、正の区間内接触は時刻差2 ns、支持高さ残差2 nm、全構造材・外皮を含むFloat32全機meshの支持高さ差2 µm以内とする。
- 同時刻接触: 左右rollの既存形状・入力採用後の計4条件でbody・採用係数・尾翼角と記録queryを照合する。整数sampleを保持し、終端境界eventの採用値を同時刻のcanonical queryへ投影する。
- 上記の数値予算は登録有限形状の指定条件に対する参照一致を検査する。連続翼面と実機の精度は、それぞれの形状近似・適合性検証で評価する。
- 接触APIは固定点と、同じ順序の対応点をbody座標で線型移動させる区間を受け取る。scenarioの固定proxyもこのAPIを使う。
- 最短符号のquaternion差を $\delta=\lVert q_1-q_0\rVert_1$、点の最大L1半径を $r$ とすると、全区間のdownを $\max(d_0,d_1)+\min(2,2\delta)r$ で上から抑える。丸め予算を加えた上界が負の点は、端点と差分の検証後に空中と判定する。
- geometry構築時に両端点のbody AABBを準備し、各8角の最大downと最大L1半径から全点の上界を導く。全体の上界が負なら一定量の評価で空中と判定し、水面付近では各対応点の探索へ移る。
- 早期判定は全state成分と点座標が $\mathrm{MAX}_{f64}/2^{20}$ 以下の数値域へ限定する。大きい入力は区間内の全座標・state評価を行い、途中のoverflow原因を保持する。
- nlerpは正の二次normで分母を払うと接触関数が高々三次となり、導関数（二次式）の根で符号区間を隔離する。
- slerpは最終正規化の正のnormで分母を払い、$F(t)=a+bt+(c+dt)\cos(\omega t)+(e+ft)\sin(\omega t)$ の符号を調べる。最短物理角は $0<\omega\le\pi$ である。
- $F''$ の線型cos/sin係数が作るphaseの単調区間から、$F''$、$F'$、$F$ の順に根を隔離する。phaseの極値は係数vectorの最近接点から求め、近接した2極値を保持する。
- determinantはFMAで積の丸めを補償する。精度の判別域・積のunderflow域・候補容量の超過は `UnresolvedMotion` とする。
- 本番mathの最短符号・枝・最終正規化を使い、二次根は固定の `polycool` で求める。

- 内部候補の接線分類に、距離予算 $E=128\epsilon S+\lVert r\rVert_1G+128\eta$ を用いる。$\eta$ は最小の正のbinary64 subnormalである。
- $\lVert r\rVert_1=\max(\lVert r_0\rVert_1,\lVert r_1\rVert_1)$、$S=\max(|d_0|,|d_1|)+\lVert r\rVert_1$ とする。通常slerpのbasisを $a,b$ として $G=|a\cdot a-1|+|b\cdot b-1|+|a\cdot b|$ とする。
- nlerpでは $G=0$ である。
- 距離予算は数値回帰の演算余裕であり、根隔離から高さまでの厳密な誤差伝播は形式検証の境界として管理する。
- 実際の補間でdownを再評価し、down ≥ 0なら先頭の交差へ精密化する。負側の内部候補が距離予算内にある場合は接線と分類する。

- 幾何scale $10^{-310}$〜$10^{150}$、近接phase極値、浅い正の侵入、接線と負側near missを回帰に含める。
- 単純根は距離予算/局所傾き、接線付近は曲率下界 $\kappa$ による $\sqrt{2E/\kappa}$ を時刻の許容へ対応付ける。解析接線のfractionは高さ残差と併せて検査する。
- 各点は最大19内部候補を24要素の固定配列へ保持し、交差を最大48回の二分で精密化する。追加allocationは0とする。
- 非有限の差分・係数・実評価はMath原因として返し、候補取得後も全点の失敗を検査する。

## 7. セッション・記録・境界の接続

| 対象 | 検査内容 |
|---|---|
| Countdown | elapsed時間、800 msでは3秒/1秒では2秒、3秒で発進1回。重複時刻・不規則差分・overflow・取消後token・stale owner |
| GameSession | 準備・発進・Pause/Resume・終端・Retryの許可と拒否、停止理由の残存、最後の有効状態、tick/record保持 |
| input/frame | キー・Gamepad観測の生成型とRust受理、接続世代・入力解放、frame時刻と停止・補間、実Native/WASMの同一観測・記録・Replay・CSVの一致 |
| 集約入出力 | 実Rust/WASM ownerからの描画・I/O要求、同観測の受理と失効、共有時刻・quaternion・終端/score契約 |
| FlightRecord | 全保存sample・finalization・capacity・終端fraction・元cause、CSV/JSON出力、保存key検証 |
| PB | 比較方式v3、有効HUD表示の同値・表示差・来歴差、旧v2を含むその他方式の未順位、元JSON/key保持、Web/Nativeの共有selector |
| Replay/Attract | Rust clock・seek・補間・最短quaternion、確定record不変、独立demoとplayer記録の分離 |
| 所有権/I/O | query世代とrecord-source世代、stale結果破棄、独立cleanup全試行、IndexedDB transaction、ファイル非上書き |
| UI | Setup/Briefing、typed HUD/availability、共通Action、group順・凡例色、全表示先、focus/scroll/拒否後の回復 |

- Web入力: 生成DTOのJSON送信とRustでの受理を接続する。不正なActionの拒否、循環参照の送信エラー、シリアライズ中に破棄されたownerへの呼出し抑止を既存試験で確認する。
- 内部出力: 所有WASM instanceの出力とfacadeが返す生成DTOの一致を確認する。Analysis列のmetadata非重複と、live/prepared/Replayに接続する共有描画poseも検査する。
- 構造変形: 保存時刻・区間内部・同時刻終端で採用曲げ係数と尾翼角を照合する。全259部材の共有頂点・三角形・管周方向位相を接触点へ照合し、Web/Nativeの11 batchが同じ頂点と物理pivotを消費することを検査する。Replayのパイロット目標位置は同じcanonical queryへ照合する。
- 描画可用性: 未記録係数と未知機体を区別し、対応する主翼・尾翼の非表示を検査する。極端な有限係数はGPU公開前に座標・Transform・法線を検査し、有効係数への復帰で表示と保持bufferの更新を確認する。
- 数値・保存契約: 時刻、stamp、quaternion、終端、score、request echoの整合はRustのproducer/core/formatで検査し、Webはその出力を転送する。
- 資源寿命: 実非同期のquery・record-source・Countdown世代とcleanupを検査し、表示DTOの静的readonly制約と分ける。

Countdownはcore、実WASM/facade/Web frame列とNative Updateを通す。
非表示化・blur・frame停止・VR中断で安全取消し、record未開始と発進tick0を確認する。

- 表示不変回帰は既存controller fixtureと同じ実WASMを使い、Fullを基準にInformation 5種、Manual/Shared/Automatic、30/60/120 FPSの代表組合せを比較する。
- 生キー観測列は発進後にpilot増加を押し、500 msのframe直後に解放する。appの共有driverが各物理tickの確定targetへ積分する。
- 100 tickのManualAbortまでの保存sample・controls・telemetry・finalization・最終poseを比較する。記録queryのABI接続はWASM DTO試験、複数tickのframe、0 tickの補間更新、Pause/Resumeとfractional終端は直接Rust・実WASM・本番controllerで検査する。
- 同じWASM演算列には追加許容差を設けない。
- 表示/PB metadataは物理列から分離する。
- backendの接続・切替・単一描画loopと資源解放はpresentation runtimeの試験、頭部・controllerによる操作は実XR端末で確認する。

## 8. 環境・地形・視認性

- 登録環境は観測値、湖上外挿、風の空間勾配、鉛直流、波、天空の仮定を分ける。
- offline生成の欠測、exact-byte hash、方位変換、全N-fast格子・閉境界、 Rust codecとWeb/Nativeの同じ環境identityへの接続を検査する。
- Title展示と保持中Setupの気象・seedを分離し、初回の波・天空・太陽・会場が同じ共有projectionに対応することを確認する。archiveは登録origin・水面datum・未登録identityの会場可用性を検査する。
- 未知の保存環境や原点未記録のarchiveへ、現在の地図・風を補完しない。

### 湖岸・島・視差

- 近傍12 km角・30 m DEM、40岸線、主要4島、沖の白石4岩礁のmesh接続を検査する。
  沖側underlap・陸側探索・水域mask・面の向き・open端・欠測帯・panel中心の三角形包含を確認する。
- 12 km patch内の岸線遷移幅は600 m以下を要求する。patch外の広い遷移帯と別に扱う。
- `lake-venue-parallax.test.ts` は本番の姿勢変換とPilot eyeを通す。
  heading310–320°、pitch/roll各±5°、高度10–30 mで北西稜線がNDC各±0.45・clip内へ入ることを確認する。
  北西500 m移動時の多景島の画面変位は、遠方稜線の3倍超を要求する。
- platform・shore・telephoto等のcamera pointと実飛行開始位置から、島と稜線の投影を検査する。
  幾何投影とrasterization・遮蔽・画素コントラストの受入を分ける。

- AW3D30最高点周辺90 m・9点と国土地理院1 mレーザDEMの独立照合は、 中心標高を差し引いた相対profileでRMSE 2.56〜6.82 mの参考範囲である。
- DSM/DEM・解像度・表面・時期の差を含み、稜線全体の誤差上限ではない。
- AW3D30の上書きやruntime API依存へ転用しない。
- 出典と処理条件は[データ仕様](data-and-formats.md)に集約する。

- 水面・地平線・機体基準・近距離固定物を同時に表示し、静止・前進・横移動・pitch/roll変化で 速度・姿勢・高度の手掛かりを検査する。
- 岸線や遠景は全方位・複数高度で確認し、 静止画像だけから連続時のoptic flowや時間的aliasingを合格としない。
- 描画の波と物理の静水面接触が独立していることを確認する。

## 9. WebのCUI描画・操作検査

- 既存の `playwright-core` と対応Chromiumを使用する。
- 初回のみ公式CLI `npx playwright-core install chromium` でbrowserを準備する。
- 一致するcacheを再利用し、[公式手順](https://playwright.dev/docs/browsers)に従う。

~~~sh
npm ci
npm run dev -- --host 127.0.0.1 --port 4202 --strictPort
~~~

別の端末から実行する。

~~~sh
npm run test:web-ui -- http://127.0.0.1:4202/pr-simulator-game-1/ target/web-ui
npm run measure:web-auto -- http://127.0.0.1:4202/pr-simulator-game-1/ target/web-auto
~~~

### 画面遷移と実download

- browserはrAFの生timestampとコールバック受信時の`performance.now()`を別々に通知する。共有ownerはScreen/Phoneのframe時刻、HMDの受信時刻を進行時計に使い、予測表示時刻を安全停止の経過時間と区別する。
- `test:web-ui`は専用BrowserContextから本番ページ・集約Rust/WASM owner・実DOM/Canvas/Three/WebGL2へ接続する。Title、Calm Setup、Briefing、Flight、Pause、Result、Analysis3図、Replayを実操作し、HD1280×720・FHD1920×1080の画像を保存する。
- 水面品質Low/Medium/Highを公開UIで切り替え、準備完了後の描画を確認する。`--without-gpu-timer`は実拡張取得を非対応へ置き換えるI/O条件である。Autoの採否と準備認可は共有Rustが所有する。
- downloadは実browserからJSON原文とCSVを保存し、Replayからの再出力とbyte一致を確認する。RetryからBriefing、SetupからTitleへの操作到達性を検査する。
- `--recovery`はWASM要求・初回WebGL2取得・初回renderer観測・framebuffer設定・Titleからの保存記録readonly transactionへ各1回の実I/O障害を注入し、公開再読み込み・Boot再試行・同じ記録の再選択から回復する。部分GPU texture・framebufferの解放、候補再試行でのcanvas再利用、日本語案内、実GPU表示、Replay/分析、原JSON/CSVのbyte一致を確認し、期待した障害診断はphase・原文・件数を照合して記録する。
- 実`KHR_parallel_shader_compile`対応環境では初回compileの完了配送を保留し、公開resizeによる取消と遅着終端・候補資源の解放を確認し、非対応時は追加観測を`unavailable`（未検証）として記録して他の回復受入を継続する。
- 画面のWebGL2画素、context loss、console、全HTTP失敗URL、配信WASMのHTTP200とSHA-256を記録する。視覚的な品質は保存画像の確認と合わせて受け入れる。
- `--allow-dirty`は基準HEAD・repository全体の`source.patch`・差分SHA-256を保存する。新規sourceはindexへ認識させ、検査前後のHEAD・差分・untracked集合・WASM hashを照合する。出力先はrepository外またはignored directoryを使用する。
- 共有波・天空・機体・座標・clip・実測文字の数値契約はRustの担当moduleで検査する。WebXRのengine境界は実WebXRManagerと集約ownerの代表結合試験で検査し、本番の内部ownerや描画producerへCUI専用操作を追加しない。
- ScreenはUI倍率適用後1280×720以上を検査対象とし、1920×1080以上を推奨する。HUDは下端の透明面として表示し、前方中央の空きとVRの両眼可読性を各担当の数値・実機受入で確認する。
- ADIの線分・pitch/roll・飛行経路円は共有完成幾何の数値代表と各adapterへの転写で検査する。Nativeの文字glyphは矩形clipを使うため、Web SVGの丸角clipとの境界付近の表示差は実描画受入で確認する。

### Autoのsource窓と性能

- `measure:web-auto`は1280×720・DPR1の実FlightでTypical/Manualを準備し、Pause中にHigh/Autoを選んでResume後の2秒窓を観測する。停止・終端を含む窓は不完全として保持し、Resumeでつなぎ直さない。
- 観測器は本番`ApplicationRuntimeBridge.update`の引数・this・戻り値を保持し、生成契約の生入力と返却値、実rAF、GPU完了値を記録する。共有ownerのAuto・epoch・ACK・stale判定をTSへ複写しない。
- 履歴上限は4096件、受信drainは500msとし、source時刻が窓内のGPU完了値を抽出する。GPU標本、owner診断、返却された資源identity、framebuffer寸法、更新CPU時間、rAF間隔と観測器費用を保存する。
- 統計はcount・非有限件数・mean・nearest-rank p95・maxで示す。取得標本0はunavailableとして扱い、raw完了値の統計とownerが受理した値を区別する。撮影要求は観測窓の外で行う。
- sourceと配信WASMの来歴照合は画面CUIと同じ条件を使用する。

Vite開発serverとproduction bundle、headlessと通常表示の性能は個別に計測する。
observer費用、headlessの表示上限、CPU/GPU競合を記録し、補正で差し引かない。

## 10. NativeのCUIオフスクリーン検査

利用者向けの起動・キー操作は[Nativeガイド](native-guide.md)へ分離する。

検証専用コードは `verify-harness` featureで有効にする。

- driver: シナリオの進行と操作を担当する。
- capture: 撮影要求の識別・非同期保存・完了通知とReplay中の記録保持検査を所有する。
- metrics: frameと計測区間の対応、統計量と計測履歴の完全性を検査する。
- render readiness / scene probe: GPU資源の準備状態と、実際のcamera・水面・描画寸法を観測する。
- UI validation: 実測文字・配置・clip・操作への到達性を検査する。

~~~sh
cargo run -p birdman-game-bevy --features verify-harness --locked -- --verify target/native-offscreen --offscreen --verify-size 1280x720 --export-dir target/native-test-logs
~~~

| 検証オプション | 条件と役割 |
|---|---|
| `--verify DIR` | 本番scene・logical input・UI・GPU画像保存を検査する |
| `--offscreen` | `--verify` 必須。OS windowを作らずImageへ描画する |
| `--verify-size WIDTHxHEIGHT` | `--verify` 必須。正の整数で検査viewportを指定する |
| `--verify-replay-cameras` | 自然着水recordの中間時刻を共有UIの全10cameraで撮影する |
| `--verify-water CASE` | Calm/Typical × Low/Medium/Highの1条件を観測する |
| `--verify-timing root` / `--verify-timing full` | `--verify-water` 必須。rootはroot-only、fullは全pass診断。既定full |
| `--verify-water-auto` | `--verify-water` 必須。実Auto policyで5 motionを観測する |

- Replay cameraとwater検査の同時指定は拒否する。
- offscreenはWinitを登録せず、ScheduleRunnerと本番Image target/readbackを使用する。
- 既定runnerの最小周期は16.67 ms、`--no-vsync` 併用で待機なしloopを要求する。
- devは接続確認、releaseは性能計測に用いる。
- 標準検証の期限はoffscreen・通常windowともに240秒である。
- 物理100 Hz・Real時計・150 ms安全停止は維持する。

### 標準フロー・資源寿命・shader

- Title/SetupのAuto表示、同一processのCalm↔Typical往復、背景・水面の天空画像共用、旧画像の退役を検査する。
- Manual比較の前提を公開UI操作で確定し、Pause中のLow/Medium/High・描画倍率・shadow交換と復元を検査する。
  適用ACKと実GPU resourceを照合し、pause中のstate・record・波時刻保持を確認する。
- 開始→Pilot/Chase→Pause/Resume→Result→Map/Altitude/Speed→Replay/Analysis→Result→Retryを通す。
  全nose-up logical inputの自然終端も実coreで確認する。
- shader/pipeline readinessを連続5 frame確認し、日本語glyph、折返し、control包含・配置・非重複とPNG保存完了を検査する。
- Shadow ACKやAuto窓のtimeoutをfindingとして残して後段を調べる場合も、最終exit codeは1とする。
  GPU/asset/state異常は失敗として保持する。

- 本番WaterMaterialと同じ19 bindings・shader・vertexを使う18 probeを16×16のRgba32Floatへ描画する。
- world position差の変位、本番導出の法線、最終fragment色をreadbackし、有限性・単位/上向き法線・共有変位上界、 平坦入力の零変位・波の空間変動・黒一色・異常輝度を検査する。
- 通常variantへ検査専用bindingや処理を追加しない。

- Titleの横長640×480以上では、既定Typical・展示cameraの水面ROI（横3–25%、縦86–96%）を検査する。
- 対象外寸法はROIだけskipし、float probeを維持する。
- これは重大な黒色化・飽和・一様面の検出であり、美観の合格基準と分ける。

Replay検査は同じrecordの中間時刻で全cameraを撮影し、選択shot・時刻・Transform/FOV・scoreを記録する。
全sample/header/finalizationとResult復帰後の終端状態を保持することを確認する。

### 湖面の6条件とAuto

~~~sh
cargo run -p birdman-game-bevy --features verify-harness --release --locked -- --verify target/native-water --offscreen --no-vsync --verify-size 1280x720 --verify-water typical-high
~~~

CASEは `calm-low` / `calm-medium` / `calm-high` / `typical-low` / `typical-medium` / `typical-high` である。
各条件を別process・出力先で実行し、shared preparation・既存画質要求・登録環境を使用する。

- 静止、前進20 m（10 m/s）、横移動6 m（3 m/s）、pitch6°、roll10°の各2秒を観測する。
  移動はcameraだけへ与え、physicsを変更しない。
- 初期observerは発進時のChase poseを一度取得し、静止区間はworld固定とする。
  各区間後はAbort/Retry/Countdownで同じsealed条件の個別flightから開始する。
- Start画像の保存・正常Running後に開始し、区間内は撮影せず、正常frameの実deltaだけを加算する。
  focus喪失・ProcessingDelay・過大delta・自然終端を含む中断窓は失敗とする。
- 近景/遠景/skyのcameraと共通simulation timeを照合する。撮影要求時刻は非同期画像の露光時刻保証ではない。
- save taskは1件とし、case/stage/serialと保存完了を確認して次へ進む。
  Pause/Result/Retryはobserverを無効化し、指定画質への復元とResume後の進行を確認する。

- Auto観測は公開Setup操作から最大10秒待ち、成熟窓・受領率90%以上・資源ACK・pipeline Ready・ 500 ms以内のGPU sourceを確認する。
- 外部context失効・適用失敗で完了事実を破棄する。
- Measuringへの戻りと初期完了を区別し、倍率低下だけを成熟窓の根拠にしない。
- 5 motion後はAuto解除で手動scale/shadow/水面の3値へ戻す。

- rootはRenderGraph描画命令区間だけを計測し、準備・upload・readback・present待機を含まない。
- fullは追加pass診断を収集する。
- GPU診断需要: Nativeのroot/fullだけが明示し、Manualでも共有ownerの同じquery・epoch・sourceを取得する。通常ManualのWeb/Nativeは診断を要求せず、Manual診断は選択画質を保持する。
- GPU値の有効期間: sourceの鮮度と測定区間に同じ500 msを適用する。非有限・負値・期間超過は原値を保持して計測窓を失効し、描画資源を保持する。ブラウザ時計の量子化を含むため、callback到着までの時間との差を物理的な厳密上限とは扱わない。
- 履歴4096件からcount/span/mean/nearest-rank p95/maxを出力し、容量不足・非有限・未取得を分ける。
- pass時間を合算しない。
- 受領時刻とGPU source時刻、epoch・frame・適用scale/shadow/water品質を区分する。
- Start保存frameを除きmotion終了frameまでをsource区間とし、End保存完了を受信cutoffとする。
- 全queryのdrainは保証せず、frame wrap・欠損・重複・metadata不一致・pair別部分窓を明示する。
- offscreen throughput、通常windowの80 FPS表示、CPUとGPUの同frame合算は個別の確認対象である。

## 11. VR・性能・配布の受入

- Native OpenXRの残る受入は、runtime・frame提出・swapchainの共有ownerへのソフトウェア接続と、実HMDでの描画・操作・停止復帰に分け、独立した証拠を記録する。
- XRの光学計算はpresentationのRust数値試験で非対称・canted眼、clip-depth 2規約とreversed Z、near/far、Information・Custom mask、前方clear region、実測glyphの視角を検査する。Webの結合試験は実WASM・フォント観測・両眼描画への接続を確認する。
- `tools/presentation-check/three-webxr-mount.test.ts`は実集約WASM ownerと実WebXRManagerを接続し、非対称眼のengine提出行列、float32揺れとreference reset後のMenu保持、描画cursorとtriggerのAction到達を確認する。
- Head HUDとMenuは同ownerの実font観測を介した最終paintを使い、共有Rustで実測ink・clip・可読性を検査する。Canvas adapterは最終命令の描画、WebXR adapterは最終pose・projection・入力面の提出を担当する。
- GPU準備取消とcleanup失敗、Commit rollback不能、終了時のlate I/Oは既存の代表adapter試験へ集約し、ownerがDrainedとなった後のfree一回を確認する。
- 全Sceneとoverlayでanchor、左右眼、視線/controllerのhit、focus・scroll・退出・中断復帰を確認する。
  WebXRの実pose/projection/IPD/reference reset、Phone VRの重力基準pitch/roll・yaw-only recenter・権限/不正sample・非表示時input破棄を含める。
- 実機確認はHTTPS、HMD/browser、スマートフォン/viewerの組合せを明記する。模擬sensorや合成backendと分ける。
- WebとNativeの80 FPS目標は12.5 ms/frameを基準とし、release/production・viewport・GPU/driver・VSync・画質・Auto状態・負荷条件を固定する。
  frame-time分布、品質振動、CPU/GPU費用、download量を測定する。
- 配布はsubpath、WASM MIME、cache、公開データの帰属、実入力、browser起動を確認する。
  Nativeは実アーカイブをソースツリー外へ展開し、元checkout内のassetへアクセスできない条件で
  GPU/shader/asset読込みと開始→Flight→Result→Retryを検査する。
- 通常起動・CUI検証・配布物・各OS/CPUの結果を分け、未確認の組合せは明記する。

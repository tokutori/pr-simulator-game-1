# 検証契約

## BPG-001の必須検査

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `RUSTDOCFLAGS=-D warnings` を設定した `cargo doc --workspace --no-deps --locked`
- coreの `wasm32v1-none` build（stdを提供しないtarget）
- WASM adapterの `wasm32-unknown-unknown` build
- CLIのnativeと `wasm32-wasip2` build
- Web/Node別のTypeScript typecheck、strict lint、tool unit tests、Web production build
- dependency boundary、asset manifest、出典・license・hash、Viteのasset取込経路、Markdown数式区切りの検査

CIはLinuxとWindowsで実施する。WASI artifactはbuildまでとし、実行を報告に混同しない。
PauseReasonsの共有sourceを検証する独立Windows jobと正・負検証の固定ツール、TCB、未証明範囲は[形式検証契約](formal-verification.md)に記載する。
browser上の物理実行はBPG-007以降、Pages配信はBPG-013で検証する。
BPG-007では旧playable synthetic flightのkeyboard/gamepad入力からWASM tick・snapshot・Screen描画までを検証する。決定性検証用`SyntheticFlight`と旧browser用`SyntheticPlayableFlight`は分離する。legacy playable fixtureは無風・neutral入力で200–300 mを15–35秒で飛行するRust core受入試験を持つ。WASM browser integrationはManual modeで100 msのpilot-position keyboard入力後に180–230 mで着水することも検証する。これらの距離・時間・係数は旧fixtureだけへ適用し、新hybrid mockの合否基準や実機性能に使用しない。
正式なGameSession遷移と実機受入は、それぞれBPG-017、BPG-015/016で検証する。

### CLI検証flight

`cargo run -p birdman-game-cli --locked -- verify-flight all` はRust coreの`FlightScenario::run_feedback`を使い、Manual・Shared(0.5)・AutomaticでTimeLimitとWaterContactの両終端を再現する。各modeは同じ固定tick pilot intent列を二度実行し、同一終端を確認する。FBWは各tickでcoreが直前stateからbody-rate feedbackを生成し、mode別の状態遷移へ適用する。空力は各element位置で固定空間wind gradientをsampleする。個別modeは `manual`、`shared`、`automatic` を指定する。scenarioの空力係数・wind gradient・feedback gainは統合経路のsoftware fixtureであり、実機同定値・公開機体のtuning・通常操縦でのゲーム成立を示さない。この再現可能なsynthetic flightがBPG-006のCLI受入条件であり、機体固有modelのsource調査・fidelity検証はBPG-035でM6完了後に行う。BPG-035はM3〜M6のsynthetic game開発をblockしない。

## 後続の物理検証

### BPG-042 native hybrid smoke

`cargo run -p birdman-game-cli --locked -- verify-hybrid-flight all`は公開coreの二系統intentを
Manual/Shared/Automaticから同じ`GameSession`のhybrid tick・記録・終端へ接続する。
各modeは短い固定入力列を二度実行し、v6 recordとnamed terminal JSONの決定性・保存controls・finalizationを照合する。
保存queryには既存held-incidence則を適用し、身体targetのSet/Holdは舵authorityから独立させる。
低初速の独立回帰は適用範囲終了時の元causeと直前成功recordだけを保持し、失敗stageのstateを出力しない。
fixtureは架空Standard modelのtrim・明示zero windとnative-smoke controller identityを使用する。
browser Typical環境、旧`verify-flight`、距離目標、長時間安定性、収束・実機検証を個別の検証範囲として維持する。
旧CLIコマンド・公開default・WASM ABIの切替はこの検査単位に含めない。

### BPG-042 アプリ統合

既定の二系統flightはnamed schema 2 snapshot、保存schema 6、同じRust GameSessionの入力・記録・終了へ接続する。
保存v1〜5は元のlegacy snapshotとして閲覧し、v6との排他layout・元identity・cause・stampを検査する。
旧recordの新mock再積分、tailへのdummy roll、TSによるFBW・score再計算を拒否する。
単一facade owner、query/record source世代、同datasetのcursor query、Rust Replay clock、BFCache停止と最終解放を検証する。
Setup/Briefing、Screen/VRの共通UI、HUDのavailability、未知保存環境のmap/風拒否、混在archive/PB routingを対象とする。
CPU上の型・unit/integration・buildの合格と、実ブラウザー/GPU・スマートフォン・HMDの表示/操作受入を区別する。
2026-10-08、`1093d4cb`（PR #235）をChromium 148・Playwright 1.60・AMD Radeon 860MのWebGLで確認した。
ScreenのBoot→Setup→Briefing→Flight keyboard→Pause/Resume→Result→CSV2/JSON6→Analysis→Replay→Retryが成功した。
Analysisは3chart×3viewport、ReplayのJSON全文一致、旧schema 5 archiveのCSV1と元JSON全文一致を確認した。
自然終端の観測は12.33 sの`OutOfValidEnvelope`であり、WaterContact・旧飛距離条件の検査と分離する。
Phone VRは合成sensor境界とStereo Title/Menu/Setup/Briefingまでを確認した。
Flight Head・実センサー・実スマートフォン・実HMDは未検証であり、[#15](https://github.com/tokutori/pr-simulator-game-1/issues/15)・[#16](https://github.com/tokutori/pr-simulator-game-1/issues/16)・[#161](https://github.com/tokutori/pr-simulator-game-1/issues/161)の受入を保持する。
新mockの全mode安定性・長時間安定性・実機性能は、この限定受入の範囲に含めない。

### BPG-043 hybrid量別step-halving

`tail_simulation::numerical_tests`はStandard架空mockの同一初期条件・入力列を、
100/200/400 Hzのtest-only driverで比較する。製品tick・機体値・公開APIは100 Hzのまま維持する。
既存のtrim参照値・決定性・解析解試験は再実装しない。

- 演算はf64、無風、密度1.225 kg/m³、重力9.80665 m/s²、初速9.7 m/sとする。
  seed・乱数・browser clockは使用しない。Standard trimの合成CG高度50 mから開始し、
  body rateだけを$(p,q,r)=(0.02,-0.04,0.03)$ rad/sへ変更する。
  controllerはq/r gain各0.2 s・slew 1 rad/s、modeはManual/Shared(0.5)/Automaticを別に検査する。
  初期incidenceは各modeの初期rateに対応するcommandへ事前設定し、開始時のslew過渡を分離する。
- 比較区間は0.5 s、観測は共通100 Hzの51標本とする。Neutral/SmoothChanged/SlewReversalを別に検査する。
  変更列は0.10–0.20 sにmanual intent$(0.01,0.005)$・q/r demand$(0.01,-0.005)$ rad/s、
  0.20–0.30 sに両者の符号反転、それ以外は0とする。0.30 sに身体targetへnormalized Set(0.2)を送信し、
  それ以外はHoldを使用する。すべてのdriverが同じ100 Hzの物理時刻入力を保持する。
  SmoothChangedだけは0.20–0.21 sにneutralを挟む。Neutral/SmoothChangedでは全control stepの
  updated incidenceとmixed targetが一致することを検査し、slew非作動を確認する。
  SlewReversalは元の直接反転を保持する。Manualの4 mrad反転は400 Hzのslew上限2.5 mradを超えるため、
  100/200 Hzでは非作動・400 Hzでは作動することも検査する。
- **physics-only**はFBW観測・authority・incidence更新・pilot policyを100 Hzに固定する。
  各0.01 s区間で一度解決したincidence・pilot加速度を、1/2/4個のRK4区間へ同じ値で保持する。
  **連成**は同じ入力を保持したまま、control・actuator・pilot policyも100/200/400 Hzへ変更する。
  両者の結果・許容差を個別に判定する。100 Hz driverの全標本は既存製品tickと完全一致させる。
- 全RK4 load評価に既存hybrid validatorを適用し、datum alphaがPWLの開区間$(0,0.06)$ radにあり、
  datum down座標が−40 mより小さいことも検査する。この高度は半径10 m以内のmock geometryを
  静水面から分離する。接触前の区間だけを比較し、範囲外の外挿・contact time/scoreの生成は行わない。

誤差は共通観測時刻での各量の成分別最大絶対差を取り、さらに区間内の最大値を判定する。
姿勢は最短quaternion符号を選び、差・和のchord長から$4\operatorname{atan2}(\|q_1-q_2\|,\|q_1+q_2\|)$を使用する。
異なる単位を一つのmaxへ混在させず、同一姿勢・逆符号・$10^{-10}$ radの小回転を独立に検査する。

以下は実行前に定める量別誤差予算であり、100/400 Hz差のCI gateである。
400 Hzは同じ方程式の細分参照であり、厳密解に対する絶対誤差上限・実機精度の証明とは区別する。
step-halvingで得る100/200 Hz差と200/400 Hz差の収縮も同時に要求する。
連成SlewReversalは刻み別のslew過渡が異なるため、固定収縮率の対象から分離し、量別予算・域内・製品一致を保持する。
physics-onlyはすべての入力系列で100 Hzのslew評価を保持し、元の収縮条件を適用する。
滑らかな固定保持RK4の理論収縮率1/16に対してphysics-onlyは1/8を上限とする。
連成の区間保持・離散feedbackはRK4四次の対象から分離し、一次の理論率1/2に対して3/4を上限とする。
PWL knot・policy切替・飽和を跨ぐ一般条件へ、この収縮率を適用しない。

| 量 | physics-only予算 | 連成予算 | 丸めscale |
|---|---:|---:|---:|
| datum位置 [m] | $10^{-6}$ | $10^{-3}$ | 50 |
| datum速度 [m/s] | $10^{-6}$ | $10^{-3}$ | 10 |
| 姿勢距離 [rad] | $10^{-7}$ | $10^{-4}$ | 1 |
| body rate [rad/s] | $10^{-6}$ | $10^{-3}$ | 0.1 |
| pilot位置 [m] | $10^{-10}$ | $10^{-10}$ | 0.1 |
| pilot速度 [m/s] | $10^{-10}$ | $10^{-10}$ | 0.3 |
| physical incidence [rad] | $10^{-7}$ | $5\times10^{-4}$ | 0.01 |

収縮判定は$D_{200,400}\le cD_{100,200}+4096\epsilon s$とし、$s$は表の量別scale、
$\epsilon$はf64 epsilonとする。4096は400 Hz系列の200 step・RK4 stage等の累積丸めへの
事前余裕であり、厳密な丸め誤差上限とは区別する。床以下の量は刻み差の検出限界だけを示す。
静止pilotや解析的な保持加速度の系列で、収縮の次数を実証したと扱わない。
許容差を観測結果へ事後fitせず、超過は試験条件・支配誤差の調査対象とする。
この単位の検査結果は実行時のcommit・toolchain・環境とともに報告する。
Jacobian/eigen・小摂動は[局所離散線形化](hybrid-numerical-validation.md)、event精度は[contact検証](contact-numerical-validation.md)、
wind・適用限界近傍は[風とdomain検証](hybrid-wind-domain-validation.md)に条件と量別上限を記載する。

### BPG-043の契約対応

[#211](https://github.com/tokutori/pr-simulator-game-1/issues/211)の9.1–9.4を次の8群で追跡する。
許容差・演算条件は各契約文書と試験定数を正本とし、以下のnative検査はWindows/Rust 1.97.0である。
CIは各PRのexact headで判定する。windの最終CI確認待ちを検査成功へ含めない。

| 指示・契約群 | 既存試験群と条件 | PR・commit | 結果・範囲 |
|---|---|---|---|
| 9.1 A/B/D 静的契約 | `polar::tests`のconstructor/PWL/axis/reference、`hybrid::tests`と`hybrid_mock::tests`のgeometry/neutral oracle | [#222](https://github.com/tokutori/pr-simulator-game-1/pull/222)、[#226](https://github.com/tokutori/pr-simulator-game-1/pull/226)、[#228](https://github.com/tokutori/pr-simulator-game-1/pull/228) | merge済み。演算scaleの許容差と独立oracleを使用する |
| 9.2 B 動的増分 | `hybrid::tests`の微係数・差分幅・strip収束、gust/dihedral/fin/rate/control符号、Galilean invariance | [#226](https://github.com/tokutori/pr-simulator-game-1/pull/226) / `1cb9b8f` | merge済み。staticとproxy増分の二重計上を検査する |
| 9.3 D/E trim・決定性 | `hybrid_mock::trim::tests`の独立参照と実wrench/core釣合い、`verify-hybrid-flight all`の固定入力二度実行 | [#228](https://github.com/tokutori/pr-simulator-game-1/pull/228) / `8cf2829`、[#235](https://github.com/tokutori/pr-simulator-game-1/pull/235) / `1093d4cb` | angle/x差≤1e-7、力/moment残差≤1e-5。同identity/tick入力を照合し、架空fixtureに限定する |
| 9.3 F 刻み・量別精度 | `tail_simulation::numerical_tests`のphysics-only/連成、3mode、Neutral/SmoothChanged/SlewReversal | [#249](https://github.com/tokutori/pr-simulator-game-1/pull/249) / `cfab952` | core273＋doc4、両OS CI成功。量別予算・丸めfloor・slew例外を分離する |
| 9.3 F 局所応答 | `hybrid_linearization`のcentral Jacobian/固有値・小摂動、零風・静止pilot・接触前0.5 s | [#250](https://github.com/tokutori/pr-simulator-game-1/pull/250) / `9f87046` | 2試験/checker24、両OS CI成功。Manual/Automaticとも局所growingが3個残る |
| 9.3 F event | `contact::tests::numerical`のballistic解析endpointと実RK4、4phase×100/200/400 Hz | [#251](https://github.com/tokutori/pr-simulator-game-1/pull/251) / `e406801` | 2試験・両OS CI成功。O(dt²)補間上限と積分丸めを分離し、hybrid着水へ転用しない |
| 9.3–9.4 F 風・domain | `numerical_tests::wind_tests`の一様風/shear/上限近傍、全mode量別gate・境界外原子性 | `945ed389` | 固定・独立review済み、最終CI確認待ち。全風領域・長時間安定性は対象外 |
| 9.4 B/C/E 範囲外・公開 | `hybrid::tests::envelope`の全stage/零速、`tail_control::tests`/`tail_tick`、`session-facade`/`named-record`/`main-tail-integration` | [#226](https://github.com/tokutori/pr-simulator-game-1/pull/226)、[#227](https://github.com/tokutori/pr-simulator-game-1/pull/227)、[#235](https://github.com/tokutori/pr-simulator-game-1/pull/235) / `1093d4cb` | software・上記限定Screen受入成功。旧archiveを保存layoutで閲覧し、VR・実端末の未検証範囲を保持する |

### BPG-002 core検証

`birdman-game-core`のunit testsは解析解または運動量不変量を期待値に使用する。
2026-09-27時点のBPG-002完了時は17件すべて成功した。BPG-003で空力unit test 19件を追加した。

| ケース | 初期条件・比較対象 | 受入許容差・結果 |
|---|---|---|
| Body/NED回転往復 | 90°のdown軸回転とforward単位vector | 各成分誤差 $\le 10^{-15}$、成功 |
| 重力自由落下 | $g=9.81$ m/s²、0.1 s、解析解 | 位置・速度誤差 $\le 10^{-13}$、成功 |
| 定力並進 | 4 kg、12 N、0.2 s、解析解 | 位置・速度誤差 $\le 10^{-13}$、成功 |
| 主軸moment | $I_{xx}=2$ kg m²、2 N m、0.01 s、解析回転 | quaternion成分誤差 $\le 10^{-13}$、成功 |
| 縦方向の内部質量 | 4 kg機体+1 kg pilot、$\ddot{x}_p=0.3$ m/s²、0.1 s、線運動量閉形式 | pilot・datum位置/速度誤差 $\le 10^{-14}$、合成線運動量誤差 $\le 10^{-14}$ kg m/s、成功 |
| 内部質量運動量 | 一般3D姿勢、非対角慣性、pilot加速、外力なし | NED線運動量誤差 $\le 2\times10^{-10}$ kg m/s、角運動量誤差 $\le 2\times10^{-9}$ kg m²/s、成功 |
| 回転中の並進 | 外力なし、初速・body yaw rateあり | NED速度誤差 $\le 2\times10^{-12}$ m/s、成功 |
| 姿勢積分収束 | torque-free非対称剛体、$0.02$ sから$0.01$ sへstepを半減 | 誤差比が12–20の範囲、成功。4次法の理論値16を含む |
| pilot境界の保持加速度 | 正負の境界、$x=0.49989999$ mの静止、境界直前の転回、低加速度の制動系列 | tick内極値・速度/加速度上限と次policyの受理、固定目標への収束を検査 |
| pilot stage解析運動 | $x=0.4999997$ m、$v=0.001$ m/s、$a=-2$ m/s²、0.01 s | 全4 stageを一定加速度解析解と比較、範囲内の転回を受理 |
| pilot policy閉包 | 4組のrange/速度/加速度上限、seed固定の目標反転列、狭travelの静止追従 | 同一入力の決定性、反復後の移動限界と停止証明を検査 |
| pilot数値適用域 | 4096 stepを超える停止系列、`f64`上で制動速度が減少しない値 | `PilotMotionOutsidePolicyDomain`、入力state不変、連続停止不能errorと区別 |
| 入力・モデル異常 | 非単位quaternion、非正定値慣性、pilot範囲超過、算術overflow、失敗load、無効step | 型付きerrorを返し、pilot状態をclampせず、部分stateを公開しない |

これらは数値積分器と運動量収支の検証であり、実機飛距離の予測精度を保証しない。
100 Hzの統合系収束と独立reference caseの追加比較は、空力・風場・control接続後にも継続する。

### BPG-003 空力core検証

2026-09-27時点で追加した19件の空力unit testはすべて成功した。

| ケース | 比較対象 | 受入許容差・結果 |
|---|---|---|
| 動圧・揚力・抗力 | $\rho=2$ kg/m³、$V=10$ m/s、$S=2$ m²、$C_L=0.5$、$C_D=0.1 | $q=100$ Pa、$F_x=-20$ N、$F_z=-100$ N、誤差 $\le10^{-12}$、成功 |
| alpha/beta係数law | rad単位の基準係数と角度微係数 | 解析力との差 $\le10^{-10}$、成功 |
| 一様windと回転局所速度 | $v_O=(10,0,0)$ m/s、$\omega_z=1$ rad/s、$r=(1,0,0)$ m | $V_i=\sqrt{101}$ m/s、pilot相対速度の変更でflow不変、誤差 $\le10^{-12}$、成功 |
| 左右対称な主翼 | $y=\pm1$ m、各要素の上向き揚力50 N | 合力 $F_z=-100$ N、roll moment 0、誤差 $\le10^{-12}$、成功 |
| 作用点と合成重心 | 実pilot position $x_p=0.3$ m、mass ratioから算出した $r_G$ | $M_O-r_G\times F$ の解析値と一致、誤差 $\le10^{-12}$、成功 |
| 係数moment | 参照span/chordと $C_l,C_m,C_n$ | 各body軸momentの解析値と一致、誤差 $\le10^{-12}$、成功 |
| 異常入力・適用範囲 | 零速、pure lateral、範囲外角度・動圧、負drag、非有限値、算術overflow | 仕様どおり零荷重または型付きerror、成功 |
| RK4接続 | 一様空力providerを通した0.01 sの積分step | 抗力による速度低下を確認、成功 |

### BPG-038 全機static polar

`aerodynamics::polar::tests`は実機値を含まない独立fixtureで次を検証する。
7独立係数列から6成分wrenchを評価し、抗力はCDi+CDvの計算値とする。

| ケース | 独立比較対象・受入条件 |
|---|---|
| 構築時検証 | 各列・alphaの非有限値、2点未満、重複・逆順、負CDi/CDv、非正参照量・密度、無効metadataを拒否。負lift/momentは有効 |
| PWL | 全7列の独立補間、異なる区間slope、元のknot値、両端点、直外の拒否。係数誤差はおおむね $10^{-15}$ |
| 数値境界 | 極端な有限alpha幅・符号の異なる係数、最小正subnormal速度、計算overflowを区別。外挿やclampは行わない |
| 力・moment軸 | $(u,v,w)=(4,12,3)$ m/s、$S=2,b=4,c=0.5,\rho=1$、非ゼロCl/Cnの解析6成分。momentの解析値、span/MACの軸対応、同じalpha・Vでbetaを変えたmomentの一致を確認。alpha-only回転と次元化は可換である。許容差は32 epsilon程度の演算scaleを目安とし、力 $8\times10^{-13}$ N、moment $2\times10^{-12}$ N m |
| 参照点 | 固定PとOの表現で同じdatum wrench、general tensor・moving pilot方程式の応答が一致。さらにalpha=beta=0を維持する軸方向drag+roll fixtureで二つの静的providerを100 tick実行し、位置・速度・角速度・quaternion各成分差 $\le10^{-12}$ |
| datum flow | Pにoffsetがあり、rate・空間wind gradientがあってもOだけでstaticを評価。XCP/点速度の二重適用がない |
| frame・風 | NEDからbodyへの逆回転、一様world速度と風の同量加算による荷重不変性 |
| provider・error | 排他的選択で既存element-onlyの結果を保持。未対応の非neutral操舵を拒否。各RK stageの位置でdatum風をsampleし、stage 2のgrid外失敗で途中stateを返さずstatic causeを保持 |
| 借用・no_std | row借用のcompile-fail doctest、coreの`wasm32v1-none` build、外部allocation probeで構築・評価・RK stepのallocationを計測 |

static部品のゼロ流はalpha未定義errorであり、hybridの全局所点静止特例は次節で検証する。
mock trim、公開終端・旧record扱いはBPG-041〜043の試験とする。
単体polarの合格を、実機精度・wake・失速・Re依存・地面効果の検証として扱わない。
公開mockはBPG-041で定義・検証し、既定切替はBPG-042の公開型・identity・記録version更新と同時に行う。
旧BPG-007の距離・時間は既存fixtureの履歴・回帰条件として扱い、新hybridの空力調整targetにしない。

2026-10-05、Windows / Rust 1.97.0の独立したworktreeで上記12件と借用期間のdoctestが成功した。
repo外の計測用Rust programで`System` allocatorのalloc・alloc_zeroed・reallocを計数し、
polar・provider・モデル構築のallocation 0、static評価10,000回と既存RK4 step 10,000回のallocation 0を確認した。
測定は非ゼロ流、P offset、空間wind gradient、moving pilotを含むdebug buildで行った。
coreのunsafe禁止と依存は維持し、計測用allocatorをcoreやproduct runtimeへ追加しない。

### BPG-039 current-reference hybrid

`aerodynamics::hybrid::tests`は公開可能な架空geometry・polarだけを使用する。
`python tools/allocation-check/main.py`はnative debug build専用の独立計測harnessを実行する。
この永続probeは上記BPG-038のrepo外計測原本と区別し、core・Cargo依存・product runtimeを変更しない。
架空の3surface・空間wind gradient・非zero rateとmoving pilotを構築し、warmup・assert・printを窓外に置く。
Systemへ転送するtest専用allocatorのalloc・alloc_zeroed・reallocを個別に計数し、校正で各1回を検出する。
provider評価・RK4・実tickの成功とGlobalBetaによる型付き範囲外を各10,000回計測し、各割当数0を要求する。
範囲外ではdatum site・元cause・直接評価のstageなしとRK4/tickのFirst stage・tick非commitを検査する。
入力・出力にblack_boxを用い、最適化による評価除去を抑制する。測定は実行したnative debug環境に限定し、
全最適化設定・全端末・実時間性能の証明とは扱わない。
[GlobalAllocの最適化・再入条件](https://doc.rust-lang.org/core/alloc/trait.GlobalAlloc.html)と
[black_boxのbest-effort条件](https://doc.rust-lang.org/std/hint/fn.black_box.html)に従い、
allocator内ではI/O・lock・panicを使わず、計数のassertを通常の安全なassertとして窓外で行う。
2026-10-06、Windows / Rust 1.97.0で校正と上記6窓の各10,000回が成功し、割当3種すべて0を確認した。
旧BPG-038原本の再実行、新Hybridの構築時実測、他の環境・最適化設定の合格をこの結果に含めない。
次の微係数はproxy増分だけを正規化し、staticの力基底変化と分離する。
flat rectangular、no-twist、alpha=0、neutralを基準とし、tailのz offset等は個別oracleの条件に固定する。

```math
\widehat p=\frac{pb}{2V},\quad\widehat q=\frac{qc}{2V},\quad\widehat r=\frac{rb}{2V},\qquad
C_{l_{\widehat p}}=-\frac{a_W}6,\qquad
C_{m_{\widehat q}}=-2a_T\frac{S_t}{S}\left(\frac{l_t}{c}\right)^2
```

```math
C_{Y_\beta}=-a_F\frac{S_f}{S},\qquad
C_{n_\beta}=a_F\frac{S_f}{S}\frac{l_f}{b},\qquad
C_{n_{\widehat r}}=-2a_F\frac{S_f}{S}\left(\frac{l_f}{b}\right)^2
```

$l_t,l_f$はdatumから後方への正armとする。beta差分ではVを一定に保つ。
finite rateの動圧差を含むため、複数central-difference幅で小擾乱極限への収束を検査する。
矩形主翼の等幅N strip midpointには$-a_W(1-1/N^2)/6$を独立期待値とし、
strip数の増加による連続翼$-a_W/6$へのquadrature収束を別に確認する。

| ケース | 独立比較対象・受入条件 |
|---|---|
| geometry・anchor | 線形chordの厳密c²積分、面積重心quarter-chord、投影/実面積・MAC、全体AR、zero-chord tip、coverage・左右対称性・別geometry拒否 |
| slope数値 | AR=2で$\pi$、AR=4で$4\pi/3$、大ARで$2\pi$、受理可能な微小正ARで$\pi AR$の解析極限と切替点 |
| current reference | 複数knot/内部alphaと複数V、極小正速度でneutral増分0・static一致。許容差は動圧等の演算scaleに対応させる |
| 微係数・符号 | 上記p/q/rとfin betaの独立極限、正tail incidenceの負pitch/yaw、左右対称上昇流のroll相殺、片翼grid gust解析力/moment、tip-up dihedralのbeta復元 |
| frame・風 | 非zero姿勢でworld機体速度とwindの同量加算による全static・increment・totalの不変性 |
| 閉境界 | global beta、tail incidence、raw/control込みalpha差、actual/reference span角、速度0.8V/1.2Vは境界を包含し直外を拒否。actual/reference forwardは厳密に正 |
| 零速・fatal | 全点静止だけ0、O静止+回転流/差動windはUndefinedReference、後続wind errorも検査。微小正速度、算術overflow、非有限wind、密度errorの元causeを区別 |
| RK/tick原子性 | actual providerを通す全4 stageでheld controlを観測し、各stageへenvelope/fatal/wind failureを注入。元cause/site/limit・失敗stageを保持し、actuator/pilot/tickの直前stateが不変 |
| Scenario互換 | 同providerのwind正本をtelemetryとloadで使用。Hybrid初期roll・欠落tail・tail travel直外を拒否し、runtime rollも非commit。Static neutral-only、旧Element全3軸travel/検査順を保持 |

ここで検証する対象はcore geometry/provider/Scenarioとload→dynamics/tick境界である。
新playableのauthority・slew・FBW、公開terminal/Result/record/schema/default、実ブラウザー/HMD・実機精度は別gateである。
fmt、warning拒否Clippy、workspace test/rustdoc、native/no_std/WASM/WASIとWeb verifyはPRでexact sourceごとに結果を記録する。
ソフトウェアoracleの合格をwake・失速・Re依存・完全なエネルギー散逸や実機性能の証明として扱わない。

### 空力舵角domainの検証（#209）

要素構築時にalpha・beta・3舵角の組合せで生じる負抗力と非有限係数を拒否する。
舵角の各正負境界・直外を零速と通常流で検査し、role付きerrorの伝達を確認する。
scenario構築ではactuatorの全travelがdomainに収まる場合を受理し、各軸の正負側の不足を拒否する。
既存のneutral荷重・force/moment scaling・aggregate overflowの回帰も維持する。

### BPG-024 空力境界・error契約

| ケース | 比較対象 | 受入条件 |
|---|---|---|
| 角度関数と迎角envelope | 監査で提示された$u=1$ m/s、$w=0.414213562373$ m/sと上限$pi/8$ | 内側入力を許可し、閉境界を許可し、直外を拒否する |
| envelope境界 | alpha・beta・動圧の下限／上限 | 各閉境界を許可し、その外側を拒否する |
| 微小・極小速度 | 正のsubnormalを含む局所速度 | 定義済みflow angleと有限な荷重を返し、逆数overflowやNaNを生成しない |
| load error伝達 | 範囲外の翼要素をRK4 load providerへ接続 | `DynamicsError::Load`から元の`AeroError`と`AerodynamicRole`を取得できる |
| 公開評価型 | 5 roleを持つ検証済みmodelとzero/nonzero flow | role lookupは必ず値を返し、角度は`Zero`またはalpha/beta両方の`Defined`で表す |
| 二次抗力のRK4接続 | $\dot v=-kv^2$、$v(t)=v_0/(1+kv_0t)$、$x(t)=\ln(1+kv_0t)/k$ | velocity・positionが解析解に収束し、step半減で誤差が減少する |

| BPG | 検証 |
|---|---|
| 002 | frame往復、quaternion不変量、機体・パイロットの運動量収支、一般3次元の内部移動、重力、刻み半減収束、参照case比較 |
| 003 | 5要素role、解析揚抗力、alpha/beta係数、wind subtract、回転局所速度、pilot非加算、moment arm、要素姿勢、zero-speed・envelope・数値境界 |
| 004 | 無風、一様風Galilean invariance、固定ground launchのheadwind、crosswind、鉛直風、解析shear、grid trilinear解析値・閉境界・範囲外error、WindField→空力→RK4接続 |
| 005 | 対称grid上昇流とroll相殺、右翼上昇流の解析roll、水平尾翼の解析pitch、垂直尾翼の解析yaw、回転局所速度、二重計上回避 |
| 006 | lifecycle、身体位置指令・移動限界、接触補間と同時刻終端state、authority両端・Shared混合、actuator飽和・rate limit、決定的scenario、入力replay、空力・FBW・pilot motionを含む100/200 Hz step-halving収束 |
| 026 | position targetの加速・制動・収束、最大速度・加速度・移動範囲、復帰不能境界のtyped error、同条件決定性、6DoF internal-mass接続 |
| 027 | actuator deflectionの全RK4 load stageへの伝播、UniformAir/WindField結合、neutral互換、stage error時の状態不変 |
| 028 | Manual / Shared / Automaticの統合tick、pilot target・actuator・6DoF一括更新、決定性、load error時の不変性、tick overflow |
| 029 | 水面非接触・境界・tick内一時接触・接線接触、複数接触点の最早fraction、同時刻physical補間・actuator保持、geometry・tick・actuator error |
| 030 | controlled tickからcontact終端への統合、airborne state返却、post-contact state非公開、contact/dynamics typed error、正負3軸の全RK4保持値・正の接触fractionでの終端actuator一致・fraction 0での旧state保持、record終端・exact時刻・区間内Replayの保持規則 |
| 031 | configurable body-rate feedbackの符号・axis別飽和・極端な有限rate・無効設定、および合成roll momentを介した6DoF減衰。実機tuningの検証とは区別する |
| 032 | course-distance score v1の北・東・斜行・逆行・高度不変性、cross-track/net horizontal解析値、極端軸正規化、無効軸・差分overflowのtyped error |
| 033 | CG launchからdatum stateへの静止閉形式、3D attitude/angular rate/pilot motionを含む位置・速度復元、pilot range・non-finite・datum translation overflowのtyped error |
| 034 | 固定tick input列の決定的再生、最初のfractional WaterContactとscore v1の一致、TimeLimit/empty input、load/contact/score errorの型付き伝播、Contact後のtick非実行 |
| 007 | legacy `SyntheticPlayableFlight`のneutral glideが150–300 m・15–35秒で接触すること、native core参照軌道とWASM adapter snapshotの許容差、生成WASMの実Node実行、30/60/120 FPS独立性、pause/resume、keyboard/gamepad binding・切断・中立確認、NED pose変換、tick入力からsnapshot・Screen表示契約までのsynthetic試験。この距離条件を新hybrid mockへ適用しない。実ブラウザーのWebGLで滑空時間・操作応答・着水を別途確認する |
| 008 | playable fixtureへのborrowed WindField注入、3操縦modeでの定数gridと既存一様風の軌道一致、重心位置の風速が0となるshear gridのroll応答と解析場との一致、RK stageのgrid範囲外error・要素role保持・部分state非公開、fixture/Scenarioの借用期間、WASMのimmutable環境所有/typed cache、metadataのsource/完全identity/seed words/bounded input/legacy値/未知archive再生保持。環境assetの選択activation・描画接続と実ブラウザー受入は別途検証する |
| 014 | engine import/型境界、全8 Scene/overlay、anchor別追従、共通操作、backend切替、単一loop、recenter、resource解放 |
| 015 | 全SceneのWebXR UI、session拒否・終了、reference space、実WebXRManagerと模擬browser APIを通るPilotEye/身体offset/runtime IPD、全anchorのgaze/controller一致、reset共役写像、Screen復帰。実HMDのpose/projectionは別途受入 |
| 016 | 全SceneのPhone VR UI、同期absolute権限要求・重力較正待ち・relative-only起動失敗・ModelのScreen復帰・不正sample・無通知中の姿勢保持、左右aspect、実Phone backend/StereoEffectの初回pitch/roll・非Flight水平復帰・Eye mount・全anchor/gaze・yaw-only recenter・listener例外時の全cleanup。relative重力sourceのbrowser接続と実スマートフォン/GPUは別途受入 |
| 017 | Rust GameSession遷移・開始/Pause/終了/Retry規則、preset決定性、Custom、三軸の境界と軌道不変性、WASM snapshotと全backend接続 |
| 018 | HUD項目とFlight Pilot camera固定、Replay/Attract rig選択、未定義telemetry、全backendの表示と操作。ゲーム値はRust snapshotを表示 |
| 019 | Rust record sample/finalize/metrics、身体状態・目標列、終端一致、capacity、allocation、集計、format schema、条件別PB、adapter保存I/O |
| 020 | Rust analysis series/queryと既知値、map軸・風断面、速度/高度、共有cursor、欠損、Screen/VR描画・操作 |
| 021 | Rust replay clock/seek/interpolation query、snapshot補間、quaternion符号、全backendのResult復帰、原record不変性 |
| 022 | camera director、Attract、短いrecord、FPS差、demoとplayer記録の分離 |
| 023 | Boot updateの純粋性、排他遷移、request ID、stale permission、backend開始失敗とScreen復帰、pagehide disposal、実`XRFrame.getPose()`境界のnull |

非有限値は入口と積分途中で拒否する。比較は解析解、不変量、独立した基準caseを用いる。
同じ実装から期待値を生成するだけの試験を検証根拠としない。
一様風不変性の試験では、位置依存の地面効果や接触を除外して比較する。
評価対象、初期条件、入力列、許容差、duration、seed、実行環境、commitを記録する。
100 Hzの適合性は空力・actuator・FBW・身体移動を含む統合系で、軌道・姿勢・接触時刻・scoreを
刻み半減と比較する。BPG-035では通常の入力範囲でのフライト成立率、失敗理由の表示、
採用値の出典・仮定・適用範囲、操作性を受入条件に含める。M3〜M6のplayable synthetic game受入と
実機同定・機体固有fidelity検証は別のgateとして扱う。

## 表示・性能・配布

BPG-010の湖面描画は地形・空に依存しない簡易world上で先行受入する。静止・前進・横移動とpitch/roll変化を与え、world-spaceの波面・反射・表面模様から移動方向・速度変化・姿勢変化を識別できることを目視確認する。描画と静水面接触判定が独立していることも確認する。続くBPG-009では対岸稜線・cockpit/wing基準・近景optic flow・中景landmarkを段階的に有効化し、pitch・roll・yaw・速度・高度・scaleの手掛かりと500 m移動時のparallaxを確認する。BPG-011では稜線視認性を維持するhazeをcloud detailより先に調整し、reflectionへの空・雲の反映も評価する。
`lake-venue-parallax.test.ts` はOSM由来の多景島detail patchとAW3D30北西側稜線の位置を三次元camera projectionへ通し、500 m移動時に近景のscreen displacementが遠景より大きいことを回帰確認する。`lake-venue-mesh.test.ts` は岸線transition meshの沖側underlapからDEM outer ringまで各頂点が三角形に接続されること、上向きface、離陸海岸のnodata band、対岸北岸の広いDSM欠損帯、主要4島すべての連続した岸線遷移を検査する。水域マスクで有効DSM標高を除外し、疎な標本しかない多景島・オコノ洲はOSM輪郭内を半格子間隔で補間する回帰条件も含む。2026-09-30、local 5198のTitleとReplay Pilot/Chase初期視点を画面確認し、水面・湖岸・対岸地形が同時表示され、岸線underlap拡張後のTitleでも見える範囲に明瞭な隙間がないことを確認した。この画面確認はパネル外余白へ多景島を合わせるカメラ変更前のbuild `4753456b` に対する証拠である。現行Title cameraはOSM島輪郭の中心方位272.17°とviewport寸法からyawを決め、多景島を中央パネル左端から24 px離して配置する。`three-renderer-camera.test.ts` は320〜1920 px幅の5画面寸法で投影位置を検査するが、実画面は新カメラ変更後に未確認である。幾何・投影テストだけでは姿勢・高度を変えた実画面の視認性、500 m飛行中の実描画parallax、全湖岸の稜線誤差受入を満たさない。地形・湖岸・島はTitle背景でもFlight/Replayと同じworldに表示する。
2026-09-30に岸線の短いオープン端を閉じる処理を追加し、`lake-venue-mesh.test.ts`で該当する北西岸の全サンプルとmesh vertex接続を検査する。local 5198のTitleとDemo Replayの開始直後・飛行約16秒の1280×720表示で水面・湖岸・対岸稜線を再確認し、確認した範囲では背景へ抜ける明瞭な隙間がない。全湖岸を近接飛行する画面確認と、傾斜・高度を変えた系統的なvisual sweepは未実施である。
湖岸meshの回帰試験には、OSMの短いline fragmentで陸側metadataが局所反転した西岸も追加した。明示的なOSM land maskを使って陸側を再探索し、同区間の全sample profileと頂点接続を確認する。

2026-09-30、対岸4峰のAW3D30 30 m patchに含まれる最高標高sampleの周囲90 m四方を、国土地理院[サーバーサイド標高API](https://maps.gsi.go.jp/development/elevation_s.html)で独立照合した。各patchの最高sampleを中心に南北・東西±30 mの9点を取得し、`build_biwa_world.py`と同じ局所投影（緯度111,132 m/度、経度111,320×cos(35.294075°) m/度）で照合した。9点すべてでAPIの`hsrc`は「1m（レーザ）」だった。中心値を各面から差し引いた相対プロファイルRMSEとAPI側最大点のずれは次の通りである。

| AW3D30 patch | 相対profile RMSE | API側最大点のずれ（北, 東） |
|---|---:|---:|
| 北西 | 3.89 m | +30 m, +30 m |
| 北 | 2.56 m | 0 m, 0 m |
| 北西遠方 | 6.82 m | 0 m, 0 m |
| 西 | 2.74 m | 0 m, 0 m |

この9点照合は山頂付近の短距離な形状比較であり、稜線全体の位置・標高誤差ではない。中心差し引きにより一定の鉛直datum差は除かれるが、DSMとAPI側DEMの表面定義、空間解像度、取得時期の差は残る。したがって値をAW3D30へ上書きせず、地形の独立確認が部分的に済んだ証拠として扱う。GSI APIは返却内容を変更・停止する可能性があるため、実行時依存にはせず、出典と取得日をこの検証記録に残す。
2026-09-30、local 5198の現行worktreeを新規ブラウザータブで開き、1280×720のTitle画面とDemo Replayの9.5秒・約93 m地点を目視確認した。Titleでは対岸地形が表示され、Replayでは湖面と対岸稜線が連続して見え、確認した範囲に明瞭な水面・地形間の背景抜けはなかった。既存の撮影記録はカメラ位置変更前のため、今回の確認を現行画面の証拠として追記する。傾斜・高度・全湖岸を網羅した検査ではない。
2026-09-30、全40本のopen shorelineと17島輪郭をサンプル単位で集計した結果、標高transitionが作れない41サンプルは沖の白石を構成する4輪郭だけに集中していた。対応AW3D30画素は有効な地形クラスではなく、既存DSMから岩の高さを作れない。これらはOSMの4輪郭と高島市公表の最高14 mを使う描画専用岩礁meshで覆い、他の高さは輪郭面積による推定値とした。transition試験は通常のopen coastと標高を持つ全島でサンプルを完全接続し、別試験で4岩礁の輪郭・水面下underlap・14 m上限を検査する。離陸地点の湖岸では遷移帯を構成する各パネルの中心点が実際の三角形内に入ることも回帰検査する。変更後のlocal 5198・1280×720 Titleでは見える範囲の水面と湖岸が連続し、目視できる背景抜けはなかった。沖の白石を正面から捉えた近接画面と全湖岸のvisual sweepは未実施である。
2026-09-30、実会場の `platform`・`shore`・`telephoto` camera pointと記録飛行開始位置から生成したcamera poseで、沖島・多景島・対岸西側稜線・北側稜線の標高ピークが指定画角に入ることを `lake-venue-parallax.test.ts` で検査した。これは実際のworld座標・camera pointを用いた投影試験であり、mesh rasterization、遮蔽、実画面上の視認性を証明しない。画面撮影を伴うmulti-view visual sweepは引き続き必要である。

2026-09-30、local 5198のbuild `4753456b`を1280×720の新規タブで確認し、TitleとDemo Replay 2.55秒・5.95秒時点の画面確認を行った。画面上で水面・対岸地形は連続し、確認範囲に明瞭な背景抜けは見えなかった。これは遠景の斜め視点であり、島岸や特定の湖岸区間を近接確認した結果ではない。同日に陸側の遷移距離を全40 shoreline fragmentで監査し、断片33の一部で最大2,398 mに達することを確認した。該当位置の岸線サンプル（N=-4,954.7 m, E=-5,531.3 m）から外端（N=-3,546.1 m, E=-3,590.6 m）までの遷移面が接続され、外端標高は約3.04 mだった。これは描画上の接続を示すもので、欠測帯の実地形を裏付ける測量値ではない。高密度の近接visual QAは残る。

2026-09-30、同じlocal 5198を再確認し、Demo Replay 16.31秒・161.4 m地点で水面、機体、対岸の同時表示を撮影した。画面上では水面と対岸の間に明瞭な空抜けは見えなかった。これは遠距離視点の一例であり、個別湖岸の近接状態や全方位を検査したものではない。

2026-09-30、変更中worktreeで `npm.cmd run verify` が成功した。typecheck、lint、30 test files / 264 tests、repository checks、production buildが通過し、Three.js renderer chunkの500 kB超過警告だけが残った。全Issueの視覚受入や上記欠測帯の実地形一致を証明する検査ではない。
2026-09-30、水面と地形の継ぎ目対策として、離陸地点周囲の30 m DEM patchを3 km角から12 km角へ拡張した。これにより近傍の低密度 shoreline transition が通常沿岸に重ならない範囲へ広がり、陸側の近接地形探索はOSM land maskで水域のDSM値を避ける。open shorelineの端点付近ではこの探索を止め、隣接する面との連続性を維持する。全shoreline sampleの幅監査では、12 km local patch範囲内の最大transition幅が335.44 m、600 m超が0件となった。離陸海岸のfragment 33は従来最大約2,398 mだった箇所が約335 mまで短縮した。一方、patch外の遠隔海岸には最大約2,404 mの広いtransitionが残る。これは今回の近岸の隙間対策が全湖岸の地形形状を解決した意味ではない。現行worktreeで `npm.cmd run verify` はtypecheck、lint、30 test files / 264 tests、repository checks、production buildまで成功した。local 5198のTitleおよびDemo Replay開始直後を1280×720で確認し、表示範囲には明瞭な水面・地形間の隙間が見えなかった。高密度の近接・全方位visual sweepは未実施である。

2026-09-30、`lake-venue-parallax.test.ts` の北・西・北西の稜線投影検査を、runtimeの `flightRelativePose` と `pilotEyePoseThree` を通すPilot cameraへ変更した。北西稜線のpeakはheading 310–320°、pitch ±5°、roll ±5°、高度10–30 mの各条件で、視野中央のNDC ±0.45以内かつnear/far clip内に入る。北西へ500 m移動したときに多景島の投影移動量が遠景稜線の3倍を超えることも確認する。対象test fileは6件成功した。これは姿勢・高度変更時の幾何投影と相対parallaxのsoftware回帰検査であり、地形meshの遮蔽、画素上のコントラスト、実画面の判読性を証明しない。実画面の新規camera条件での確認、近接する島岸・全湖岸の視認性、全稜線の誤差受入は未完了である。
BPG-012ではframe-time分布、画質振動、physics allocationと実行時間、download量を計測する。
BPG-013ではsubpath、WASM MIME、cache、asset帰属、keyboard/gamepad、browser smoke testを検証する。
未確認の端末・browserは明記する。実機同定・物理HIL検証をこれらの合格に含めない。
VR基盤の開始前に試験用HTTPS配信、HMDとbrowser、スマートフォンとviewerを確保し、
全Scene fixtureと実フライト接続後の再試験を分けて記録する。

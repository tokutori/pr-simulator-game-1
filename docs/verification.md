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
browser上の物理実行はBPG-007以降、Pages配信はBPG-013で検証する。
BPG-007ではplayable synthetic flightのkeyboard/gamepad入力からWASM tick・snapshot・Screen描画までを検証する。決定性検証用`SyntheticFlight`とブラウザー用`SyntheticPlayableFlight`は分離する。playable fixtureは無風・neutral入力で150–300 mを15–35秒で飛行するRust core受入試験を持つ。これらの係数はplayability用であり、実機性能を示さない。
正式なGameSession遷移と実機受入は、それぞれBPG-017、BPG-015/016で検証する。

### CLI検証flight

`cargo run -p birdman-game-cli --locked -- verify-flight all` はRust coreの`FlightScenario::run_feedback`を使い、Manual・Shared(0.5)・AutomaticでTimeLimitとWaterContactの両終端を再現する。各modeは同じ固定tick pilot intent列を二度実行し、同一終端を確認する。FBWは各tickでcoreが直前stateからbody-rate feedbackを生成し、mode別の状態遷移へ適用する。空力は各element位置で固定空間wind gradientをsampleする。個別modeは `manual`、`shared`、`automatic` を指定する。scenarioの空力係数・wind gradient・feedback gainは統合経路のsoftware fixtureであり、実機同定値・公開機体のtuning・通常操縦でのゲーム成立を示さない。この再現可能なsynthetic flightがBPG-006のCLI受入条件であり、機体固有modelのsource調査・fidelity検証はBPG-035でM6完了後に行う。BPG-035はM3〜M6のsynthetic game開発をblockしない。

## 後続の物理検証

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
| 029 | 水面非接触・境界・tick内一時接触・接線接触、複数接触点の最早fraction、同時刻physical/actuator補間、geometry・tick・actuator error |
| 030 | controlled tickからcontact終端への統合、airborne state返却、post-contact state非公開、contact/dynamics typed error |
| 031 | configurable body-rate feedbackの符号・axis別飽和・極端な有限rate・無効設定、および合成roll momentを介した6DoF減衰。実機tuningの検証とは区別する |
| 032 | course-distance score v1の北・東・斜行・逆行・高度不変性、cross-track/net horizontal解析値、極端軸正規化、無効軸・差分overflowのtyped error |
| 033 | CG launchからdatum stateへの静止閉形式、3D attitude/angular rate/pilot motionを含む位置・速度復元、pilot range・non-finite・datum translation overflowのtyped error |
| 034 | 固定tick input列の決定的再生、最初のfractional WaterContactとscore v1の一致、TimeLimit/empty input、load/contact/score errorの型付き伝播、Contact後のtick非実行 |
| 007 | playable synthetic fixtureのneutral glideが150–300 m・15–35秒で接触すること、native core参照軌道とWASM adapter snapshotの許容差、生成WASMの実Node実行、30/60/120 FPS独立性、pause/resume、keyboard/gamepad binding・切断・中立確認、NED pose変換、tick入力からsnapshot・Screen表示契約までのsynthetic試験。実ブラウザーのWebGLで滑空時間・操作応答・着水を別途確認する |
| 014 | engine import/型境界、全8 Scene/overlay、anchor別追従、共通操作、backend切替、単一loop、recenter、resource解放 |
| 015 | 全SceneのWebXR UI、session拒否・終了、reference space、実HMDのpose/projection |
| 016 | 全SceneのPhone VR UI、sensor権限・null・timeout・stale、左右aspect、実スマートフォン |
| 017 | Rust GameSession遷移・開始/Pause/終了/Retry規則、preset決定性、Custom、三軸の境界と軌道不変性、WASM snapshotと全backend接続 |
| 018 | HUD項目とcamera許可、未定義telemetry、全backendの表示と操作。ゲーム値はRust snapshotを表示 |
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

BPG-009以降では稜線誤差、500 m移動時のparallax、reflection、雲、hazeを目視・数値で評価する。
BPG-012ではframe-time分布、画質振動、physics allocationと実行時間、download量を計測する。
BPG-013ではsubpath、WASM MIME、cache、asset帰属、keyboard/gamepad、browser smoke testを検証する。
未確認の端末・browserは明記する。実機同定・物理HIL検証をこれらの合格に含めない。
VR基盤の開始前に試験用HTTPS配信、HMDとbrowser、スマートフォンとviewerを確保し、
全Scene fixtureと実フライト接続後の再試験を分けて記録する。

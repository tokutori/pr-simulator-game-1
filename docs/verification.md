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
BPG-007ではplayable synthetic flightのkeyboard/gamepad入力からWASM tick・snapshot・Screen描画までを検証する。決定性検証用`SyntheticFlight`とブラウザー用`SyntheticPlayableFlight`は分離する。playable fixtureは無風・neutral入力で200–300 mを15–35秒で飛行するRust core受入試験を持つ。WASM browser integrationはManual modeで100 msのpilot-position keyboard入力後に180–230 mで着水することも検証する。これらの係数はplayability用であり、実機性能を示さない。
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

static部品のゼロ流はalpha未定義errorであり、hybridの全局所点静止特例はBPG-039で検証する。
neutral増分0、局所流の微係数、mock trim、公開終端・旧record扱いはBPG-039〜043の試験とする。
単体polarの合格を、実機精度・wake・失速・Re依存・地面効果の検証として扱わない。
公開mockはBPG-041で定義・検証し、既定切替はBPG-042の公開型・identity・記録version更新と同時に行う。
旧BPG-007の距離・時間は既存fixtureの履歴・回帰条件として扱い、新hybridの空力調整targetにしない。

2026-10-05、Windows / Rust 1.97.0の独立したworktreeで上記12件と借用期間のdoctestが成功した。
repo外の計測用Rust programで`System` allocatorのalloc・alloc_zeroed・reallocを計数し、
polar・provider・モデル構築のallocation 0、static評価10,000回と既存RK4 step 10,000回のallocation 0を確認した。
測定は非ゼロ流、P offset、空間wind gradient、moving pilotを含むdebug buildで行った。
coreのunsafe禁止と依存は維持し、計測用allocatorをrepoのsourceやruntimeへ追加しない。

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
| 007 | playable synthetic fixtureのneutral glideが150–300 m・15–35秒で接触すること、native core参照軌道とWASM adapter snapshotの許容差、生成WASMの実Node実行、30/60/120 FPS独立性、pause/resume、keyboard/gamepad binding・切断・中立確認、NED pose変換、tick入力からsnapshot・Screen表示契約までのsynthetic試験。実ブラウザーのWebGLで滑空時間・操作応答・着水を別途確認する |
| 008 | playable fixtureへのborrowed WindField注入、3操縦modeでの定数gridと既存一様風の軌道一致、重心位置の風速が0となるshear gridのroll応答と解析場との一致、RK stageのgrid範囲外error・要素role保持・部分state非公開、fixture/Scenarioの借用期間、WASMのimmutable環境所有/typed cache、metadataのsource/完全identity/seed words/bounded input/legacy値/未知archive再生保持。環境assetの選択activation・描画接続と実ブラウザー受入は別途検証する |
| 014 | engine import/型境界、全8 Scene/overlay、anchor別追従、共通操作、backend切替、単一loop、recenter、resource解放 |
| 015 | 全SceneのWebXR UI、session拒否・終了、reference space、実WebXRManagerと模擬browser APIを通るPilotEye/身体offset/runtime IPD、全anchorのgaze/controller一致、reset共役写像、Screen復帰。実HMDのpose/projectionは別途受入 |
| 016 | 全SceneのPhone VR UI、sensor権限・初回timeout・不正sample・無通知中の姿勢保持、左右aspect、実Phone backend/StereoEffectのEye mount・全anchor/gaze・recenter・Screen復帰。実スマートフォンは別途受入 |
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

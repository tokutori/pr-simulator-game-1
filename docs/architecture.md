# アーキテクチャ契約

## 目的と範囲

短時間の無推力滑空を通じた広報を目的とする。実行環境はWeb browser、配布先はGitHub Pages。
physicsはRust/WASM、描画adapterの初期実装はTypeScript/Three.js/WebGLとする。
ゲームと描画engineの交換境界は `render-boundary.md` に従う。
RP2040、virtual I/O、sensor fault、HIL、firmware timingは対象外である。
機体モデルの実機同定と予測精度は、数値計算の正しさとは別に評価する。

## 依存方向

矢印は「左が右に依存する」を表す。

```text
CLI  ──→ format ──→ core
WASM ──→ format ──→ core
CLI/WASM ─────────→ core
Web ──→ WASM adapter
Web app / camera / UI ──→ engine-neutral render contracts
Three.js adapter ──→ engine-neutral render contracts / Three.js
composition root ──→ concrete adapter factory
Web presentation ──→ Screen / WebXR / Phone VR platform adapters
world-build ──→ offline input data / asset format
```

coreは `#![no_std]` とし、wasm-bindgen、serde、I/O、clock、network、乱数源、GISへの依存を禁止する。
必要な乱数は外部でseedから生成し、確定したパラメータを渡す。
`alloc` はモデル構築に限って許容する。simulation step中のheap allocationは0を要件とする。
この性能要件の検証は実装段階でallocation計測により行う。

formatは外部表現をdeserializeして検証し、coreのconstructorから有効なモデルを構築する。
core型の内部不変条件を保持するため、数値フィールドを無条件に公開しない。
filesystemアクセスとエラー表示はCLI、browser APIと表示はWeb/WASM adapterの責務である。
CLI/WASMはcoreを直接使用してよい。format経由で物理計算を呼ぶ必要はない。

## 責務とディレクトリ

| 境界 | 責務 |
|---|---|
| `crates/birdman-game-core` | math、frames、dynamics、aerodynamics、environment、control、scenario、simulation、game session、record集計・再生query |
| `crates/birdman-game-format` | aircraft/scenario/world/recordの外部表現、version、検証・codec |
| `crates/birdman-game-wasm` | WASM ABI、所有権、coreへの変換、session command、RenderSnapshot・分析query |
| `crates/birdman-game-cli` | native/WASI実行、CSV出力、profiling入口 |
| `web/src` | device入力、browser/presentation状態、共通UI、camera pose、画質、WASM呼出し |
| `web/src/render/contracts` | engine非依存の描画・UI・view・resource識別子の契約 |
| `web/src/render/engines/three` | Three.js固有のscene graph、camera、shader、GPU/XR結合 |
| `tools/world-build` | offline地理・気象処理とasset生成 |
| `tools/asset-check` | 配布assetの登録・出典・hash・利用条件の検査 |

moduleは実装時に追加する。空の細分化crateを増設しない。
Webはstrict TypeScriptとし、外部APIのnull/undefinedを最小境界で検査する。
Web実行コードとNode.js用build/toolコードは別tsconfigで型検査し、WebからNode.js moduleへのimportをlintで拒否する。
配布assetの入口とVite build graphの検査は `data-sources.md` に従う。
失敗は判別可能な型で保持し、状態遷移はdiscriminated unionと網羅的分岐で表現する。

Web UIはThe Elm Architectureの原則を採用する。単一の不変`AppModel`を状態の正本とし、
純粋な`update(model, message)`が次状態と副作用要求を返す。DOMと各backendのviewはModelから導出し、
event handlerはMessageのみを送信する。非同期完了にはrequest IDを付け、現在状態と一致しない結果を破棄する。
物理状態はcore/WASMの正本を参照し、UIに複製しない。外部API・描画engine・物理実行の副作用はeffect portへ隔離する。
このAppModelはbrowserのresource lifecycle、表示状態、編集途中のform、focus等を所有する。
GameSessionの開始可否、domain phase、pause理由、終了・再試行規則、score、record確定値はRust coreが所有する。
Webはimmutableなsession snapshotからScene・HUD・button availabilityを導出し、session変更intentをWASMへ送る。
Rust coreへ複製したゲーム状態を置かず、WASM境界はsession操作とsnapshot/query単位にまとめる。

## 実行契約

simulation tickは100 Hz。clock、tickへの入力割当、pause/resumeはplatform adapterが管理する。
物理状態をrendererへ可変参照として公開しない。RenderSnapshotを補間し、描画は独立に実行する。
Rust coreの `advance_flight_tick` は一つの入力sampleからauthority・actuator・pilot motion・6DoFを原子的に進める。
`advance_flight_tick_with_contact`は水面接触時にfractional terminal sampleだけを返し、接触後stateを公開しない。
`FlightScenario`は検証済みAircraftModel・launch・aerodynamics・WindField・actuator・contact geometry・course axisを
一つの不変構成へ組み立て、native CLIとWASM adapterが共通利用する。外部format decode・I/O・controller入力生成はadapter境界に置く。
同一モデル、scenario、機体・身体の初期状態、tickごとの舵・身体位置指令列に対する決定性を保つ。
パイロット前後移動の力学・入力境界は `pilot-motion.md` に従う。
FBW authority混合と舵actuator stateは `flight-control.md` に従う。
Desktop・WebXR・Phone VRはpresentation layerに配置する。head trackingは視線のみへ適用し、
physicsやFBWを変更しない。権限、reference space、光学profile、loopの契約は `presentation.md` に従う。
Information・Assistance・Weatherの三軸難易度は `difficulty.md` に従う。
format境界でpresetを解決し、HUD設定・ControllerConfig・Environmentを分離する。
coreにDifficulty型を追加しない。難易度による空力係数・actuator特性の変更を禁止する。
native/WASM間のbitwise一致は要求せず、量ごとの許容差で比較する。

主要8 GameSceneとoverlayは `game-flow.md`、視点は `camera.md` に従う。
Result/Analysis/Replayは `flight-record.md` の記録を共用し、表示契約は `result-analysis.md` に従う。
最初のbrowser版からPilot視点を採用する。sceneごとのThree.js worldの再生成を避ける。
開発順は「全Scene共通Screen/VR基盤→物理→ゲーム骨格→記録・解析・Replay→実環境→描画品質→配布」とする。
実気象・地形の取得をゲーム進行の前提にしない。実行順と完了条件は[GitHub Milestones](https://github.com/tokutori/pr-simulator-game-1/milestones)、作業の依存関係はGitHub Issuesで管理する。

エラーはcoreでenumとして返す。NaN、無限大、不正な質量・慣性・quaternion、適用範囲外、
grid範囲外を黙って補正しない。失敗時は直前の有効状態を保持し、部分更新を外部へ公開しない。
Webは有効recordがあれば終了理由付きResultへ遷移し、boot等の初期化失敗は所属Scene内のfailed状態で扱う。
再試行・復帰の選択肢を提示し、表示言語はcoreから分離する。

## 参照元と現状

2026-09-27確認: ローカル `simulator1` のoriginは
`https://github.com/tokutori/simulator-prototype-1.git`、参照HEADは
`20118837706611db84b70daa024ab2741cf8fc2d`。
`flight-dynamics-core/src/lib.rs`、`dynamics.rs`、viewerの `coordinates.ts` を確認した。
no_std、FRD/NED、body-to-NED quaternion、明示的wind入力、RK4を設計参考とする。
既存stateの並進速度はbody表現、本ゲームはNED表現を採用するため、式を直接転記しない。
係数と検証ケースはBPG-002以降で適用範囲を再確認する。coreの依存はno_std対応で役割の明確なものに限り、
crate DAGとtarget buildをCIで検証する。`libm`は数学関数のno_std実装として許可する。

BPG-001は契約とbuild可能な境界のみを含む。BPG-002/003の6DoF・空力coreは実装済みである。
GameSession、record、metrics、analysis/replay queryとそのWASM commandは後続BPGで実装する。
BootのAppModelはbrowser/presentation状態のみを持ち、Rust側のgameplay state実装を代替しない。
開発原則は[設計指針](https://zenn.dev/bem130/articles/1b352797de94e7)に基づく。

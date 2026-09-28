# 広報用鳥人間シミュレーターゲーム

琵琶湖を舞台とする短時間の無推力滑空ゲーム。
Rust / WebAssembly による物理計算と TypeScript / Three.js による描画を分離する。

松原水泳場付近から湖中央方向への約500 m、数十秒のフライトを対象とする。
keyboard / gamepad、manual / shared / automatic、FBWを備える計画である。

## 現在の実装範囲

BPG-014のScreen表示shell、BPG-015のnative WebXR sessionとVR操作、BPG-016のPhone VR stereo・端末姿勢tracking・head-gaze・Gamepad操作を実装し、自動試験を追加した。Boot画面でScreen表示を確認できる。実HMD・スマートフォン・browser・viewerの組合せによる表示・操作確認は未実施であり、各受入Issueを完了扱いしない。

BPG-002〜005・024で6DoF、5要素空力、空力境界の数値検証、空間風場を実装した。BPG-025〜034ではRust coreにauthority mixer、actuator dynamics、pilot position policy、決定的flight tick、fractional water contact、score、launch変換、固定tick flight sequenceを追加した。Rust coreの`FlightScenario`はlaunch・機体・空力・空間風・actuator・contact・score設定を統合する。

BPG-006のCLIは決定性検証用`SyntheticFlight`を使い、全control modeの再現可能なflightを実行する。BPG-007のWASM browser用`SyntheticPlayableFlight`は、約94 kg・主翼面積18 m²級・約9.7 m/s・無風の初期条件を持つ。neutral入力で約273 m・約28秒の飛行をRust試験で確認する。このfixtureの係数はplayability用であり、実機性能やcontroller tuningを示さない。

現在の作業差分では、keyboard/gamepad入力、fixed-tick更新、HUD、fractional water-contactに加え、Rust `GameSession`のTitle・Setup・Briefing・Countdown・Flight・Result・Replay遷移をWASM／Screen UIへ接続している。SetupではInformation・Assistance・Weatherの各軸とpresetを選択でき、解決結果をRustのflight制御・scenarioへ反映する。Rust coreは初期sample・tick入力・終端状態を記録し、`birdman-game-format`のJSON schema version 1とWASM exportを提供する。WebはResult確定時にIndexedDBへ保存し、Rust由来summaryを表示する。保存recordはTitleに最新3件を表示し、選択時はRust GameSessionでarchiveを検証してReplayへ遷移する。Analysis graphは水平map、altitude/speed系列、共有cursor、固定高度の5×5 wind queryを備える。模式的な湖岸・platform・地物は非地理データとして管理する。Replayは確定recordのseek、Result Analysis cursor同期、Rust補間pose、連続再生・速度選択、Screen上のPilot/Chase選択を実装した。追加Replay rig、生成WASM宣言の更新、実ブラウザー受入は未完了であり、BPG-007は未完了である。

FlightRecordの永続record読出しUI、追加Replay rig、Attract、実環境データ、Pages公開は後続BPGで実装する。Replayには連続再生・速度選択とScreen上のPilot/Chase切替がある。機体固有modelのsource調査・fidelity検証はBPG-035でM6後に行い、M3〜M6の開発をblockしない。ゲーム進行・記録・解析queryはRust coreの責務、DOM・WebXR・Phone VR・browser表示状態はTypeScript側の責務である。
各段階の完了条件と依存関係は[実装計画](docs/implementation-plan.md)を参照する。

## 開発環境

Rust 1.97.0、Node.js 24.14.1以上の24系、npmを使用する。
Rustのtargetとcomponentは `rust-toolchain.toml` に固定する。
PowerShellでは `npm.cmd` を使用できる。テキストI/OにはUTF-8を明示する。

```sh
npm ci
npm run verify
cargo fmt --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo doc --workspace --no-deps --locked
cargo build -p birdman-game-core --target wasm32v1-none --locked
cargo build -p birdman-game-wasm --target wasm32-unknown-unknown --locked
cargo build -p birdman-game-cli --target wasm32-wasip2 --locked
cargo run -p birdman-game-cli --locked
cargo run -p birdman-game-cli --locked -- verify-flight all
npm run dev
```

Web起動ページは `http://localhost:5173/pr-simulator-game-1/` で確認できる。開発時に`npm run dev`・`npm run typecheck`・`npm test`・`npm run build`を実行すると、`wasm-bindgen` bindingを生成する。現行差分にはGameSessionを含む基本Scene遷移があるが、生成bindingの更新、実ブラウザー上のWebGL操作確認、全gameplay loopの受入、実機検証は未完了である。

## 設計資料

- [アーキテクチャ](docs/architecture.md)・[座標系](docs/coordinates.md)
- [物理契約](docs/physics.md)・[空力](docs/aerodynamics.md)・[風場](docs/wind-field.md)
- [描画](docs/rendering.md)・[データ候補](docs/data-sources.md)
- [Desktop / WebXR / Phone VR](docs/presentation.md)
- [VRの参照座標系](docs/vr-spaces.md)・[3D engine交換境界](docs/render-boundary.md)
- [多軸難易度・Game flow・Replay](docs/difficulty.md)
- [8 Sceneとoverlay](docs/game-flow.md)・[Pilot/Cinematic camera](docs/camera.md)
- [Flight record](docs/flight-record.md)・[Result Analysis](docs/result-analysis.md)
- [検証](docs/verification.md)・[開発手順](CONTRIBUTING.md)

参照元は [`tokutori/simulator-prototype-1`](https://github.com/tokutori/simulator-prototype-1)
である。ローカル名は `simulator1`。数式・設計・検証ケースを参照し、runtime dependencyにはしない。
ソースコードはMIT License。第三者データは[個別の利用条件](THIRD_PARTY_DATA.md)を維持する。

開発は全Scene共通のScreen/VR表示基盤を早期に確立し、簡易worldでゲーム骨格と記録・Replayを完成させる。
実環境データと景観品質はその後に実施する。湖面は機体の速度・姿勢を把握する重要な視覚的手掛かりであるため、水面描画を地形・会場および空・雲より先行させる。BPG IDは識別子であり、数値順は実装順を表さない。

# 広報用鳥人間シミュレーターゲーム

琵琶湖を舞台とする短時間の無推力滑空ゲーム。
Rust / WebAssembly による物理計算と TypeScript / Three.js による描画を分離する。

松原水泳場付近から湖中央方向への約500 m、数十秒のフライトを対象とする。
keyboard / gamepad、manual / shared / automatic、FBWを備える計画である。

## 現在の実装範囲

BPG-014のScreen表示shell、BPG-015のnative WebXR sessionとVR操作、BPG-016のPhone VR stereo・端末姿勢tracking・head-gaze・Gamepad操作を実装し、自動試験を追加した。Boot画面でScreen表示を確認できる。実HMD・スマートフォン・browser・viewerの組合せによる表示・操作確認は未実施であり、各受入Issueを完了扱いしない。

BPG-002〜005・024で6DoF、5要素空力、空力境界の数値検証、空間風場を実装した。BPG-025〜028ではRust coreにauthority mixer、actuator dynamics、pilot position policy、全RK4段階への舵状態伝播、100 Hzの決定的flight tickを実装した。BPG-029では水面接触fractionと同時刻の終端state補間を実装し、BPG-030では接触時に終端sampleだけを返すtick統合を追加した。BPG-031ではRust coreに明示gainを用いるbody-rate feedback primitiveを追加した。BPG-032ではRust coreにversioned course-distance scoreを実装した。BPG-033ではCG基準launch条件をdatum基準FlightStateへ変換する。BPG-034では固定tick input列を再生し、contact・score・TimeLimitを一つのRust operationで確定する。Rust coreの`FlightScenario`はlaunch・機体・空力・空間風・actuator・contact・score設定を統合し、CLIのsynthetic検証flightから利用する。BPG-006のCLIではsynthetic値による全control modeの再現可能なflightを実行する。これはソフトウェア統合fixtureであり、実機・公開機体の性能やcontroller tuningを示さない。BPG-007では同じscenarioをRust/WASMとScreen表示へ接続し、keyboard/gamepad入力、fixed-tick更新、HUD、fractional water-contactを実装した。生成WASMを用いるNode統合試験と入力・描画契約試験は成功している。実ブラウザー上のWebGL操作確認は未実施である。GameSession遷移、正式なgameplay規則、FlightRecordは後続BPGで実装する。機体固有modelのsource調査・fidelity検証はBPG-035でM6後に行い、M3〜M6の開発をblockしない。ゲーム進行・記録・解析queryはRust coreの責務、DOM・WebXR・Phone VR・browser表示状態はTypeScript側の責務である。各Sceneのゲーム機能、実環境データ、Pages公開も未実装である。Boot UIのTEA状態整理とbackend切替失敗の修正はBPG-023で行う。
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

Web起動ページは `http://localhost:5173/pr-simulator-game-1/` で確認できる。開発時に`npm run dev`・`npm run typecheck`・`npm test`・`npm run build`を実行すると、`wasm-bindgen` bindingを生成する。GameSessionを含む完全なgameplay loop、実ブラウザー上のWebGL操作確認、実機受入は未完了である。

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
実環境データと地形・水面・空の高品質化はその後に実施する。BPG IDは識別子であり、数値順は実装順を表さない。

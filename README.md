# 広報用鳥人間シミュレーターゲーム

琵琶湖を舞台とする短時間の無推力滑空ゲーム。
Rust / WebAssembly による物理計算と TypeScript / Three.js による描画を分離する。

松原水泳場付近から湖中央方向への約500 m、数十秒のフライトを対象とする。
keyboard / gamepad、manual / shared / automatic、FBWを備える計画である。

## 現在の実装範囲

BPG-001の設計契約とworkspaceを基盤に、BPG-014のScreen表示shell、8 Scene共通のUI契約、VR panel描画契約、Three.js adapter境界を実装中である。Boot画面でScreen表示を確認できる。
WebXR・Phone VR session、各Sceneのゲーム機能、物理計算、実データ、Pages公開は未実装である。
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
npm run dev
```

Web起動ページは `http://localhost:5173/pr-simulator-game-1/` で確認できる。現在はBoot表示shellを起動する。
BPG-001のWASM検査はRust artifactのbuildまでである。JavaScript binding生成、
browser接続、nativeとの軌道比較はBPG-007で実装する。

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

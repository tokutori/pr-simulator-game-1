# 広報用鳥人間シミュレーターゲーム

琵琶湖を舞台とする短時間の無推力滑空ゲーム。
Rust / WebAssembly による物理計算と TypeScript / Three.js による描画を分離する。

広報版の設計目標は、松原水泳場付近から湖中央方向への約500 m、数十秒のフライトである。
keyboard / gamepad、Manual / Shared / Automatic、FBWを備える。設計目標は架空mockの性能保証と区別する。

## 現在の実装範囲

BPG-014のScreen表示shell、BPG-015のnative WebXR sessionとVR操作、BPG-016のPhone VR stereo・端末姿勢tracking・head-gaze・Gamepad操作を実装し、自動試験を追加した。Boot画面でScreen表示を確認できる。実HMD・スマートフォン・browser・viewerの組合せによる表示・操作確認は未実施であり、各受入Issueを完了扱いしない。

BPG-002〜005・024で6DoF、5要素空力、空力境界の数値検証、空間風場を実装した。BPG-025〜034ではRust coreにauthority mixer、actuator dynamics、pilot position policy、決定的flight tick、fractional water contact、score、launch変換、固定tick flight sequenceを追加した。Rust coreの`FlightScenario`はlaunch・機体・空力・空間風・actuator・contact・score設定を統合する。

BPG-038でborrowed非線形全機static polarと、moment軸・固定参照点・PWL補間を追加した。
BPG-039 / [#218](https://github.com/tokutori/pr-simulator-game-1/issues/218)で全機staticとcurrent-reference局所normal-force差分、
検証済みgeometry・TailIncidence・閉境界・RK stage付きcause・排他的Scenario入口をRust coreへ追加した。
[#219](https://github.com/tokutori/pr-simulator-game-1/issues/219)は二系統actuator・authority・q/r FBW、
[#220](https://github.com/tokutori/pr-simulator-game-1/issues/220)は公開架空mockとtrim・launchを実装する。
数式と近似範囲は[空力契約](docs/aerodynamics.md)に従う。
[#221](https://github.com/tokutori/pr-simulator-game-1/issues/221)のアプリ統合は、このmockを既定として入力・表示・保存・Replayを接続する。
physical controlsは水平尾翼・垂直尾翼の二系統であり、姿勢・body rateのroll/pitch/yaw三成分と区別する。
以下の飛距離条件は既存fixtureの検証記録であり、新しいhybrid mockの合否基準には使用しない。実機数値・非公開xlsxは導入しない。

BPG-006の旧CLIは決定性検証用`SyntheticFlight`を使い、全control modeの再現可能なflightを実行する。BPG-007の旧browser用`SyntheticPlayableFlight`は、約94 kg・主翼面積18 m²級・約9.7 m/s・無風の初期条件を持つ。neutral入力で約220 m・約23秒、100 msのpilot-position keyboard入力を与えたManual flightで約193 mの飛行を確認した。これらはlegacy fixtureの検証記録であり、新hybrid mockの飛距離・安定性や実機性能を示さない。

Rust `GameSession`はTitle・Setup・Briefing・Countdown・Flight・Result・Replayと独立demo recordのAttractを管理する。SetupでInformation・Assistance・Weatherを選択し、Briefingで準備結果を確認する。二系統のモデルIDは`bpg041-rectangular-hybrid-mock`、controller IDは`bpg040-tail-rate-feedback`であり、各versionは1とする。ID・物理制限・環境・終了理由はRustから供給する。

新規flightはJSON record schema 6へ保存する。保存v1〜5は元の三軸snapshotとして閲覧し、v6の二系統と排他的に扱う。旧recordを新mockで再積分しない。Result・Analysis・Replayは同じ保存record、元causeと終端stampを参照し、再生clockと集計値をRustが所有する。WebはIndexedDB保存、layout別Personal Best selector、水平map・高度/速度graph・共通cursor・理由付き風queryを接続する。未知保存環境へ現在の環境・地図を流用しない。

Screen・WebXR・Phone VRは共通ModelからUIを導出し、同じsession ownerを使用する。VRのHead HUDとMenu anchorを分離する。実ブラウザー/GPU・実HMD・スマートフォンによる全gameplay loopの受入は未検証である。機体固有modelのsource調査・fidelity検証はBPG-035でM6後に行い、ゲーム機能の開発をblockしない。
各段階の完了条件と依存関係は[実装計画](docs/implementation-plan.md)を参照する。

## 飛行ログの出力

Resultおよび保存記録のReplay/Analysisには「飛行ログ CSV」「元記録 JSON」を用意する。
CSVはRustが全保存標本の数値と単位・欠損理由を出力し、加速度は保存された速度からのfinite-difference推定値として区別する。
保存していない空力荷重などを補完しない。推定値には差分方式・標本時刻・availabilityを併記する。
JSONは選択中の元FlightRecordを出力する。保存記録のschemaとmetadataを維持し、表示cursorによる切り出しや再simulationは行わない。
ScreenとVRは共通の操作を使用する。ブラウザーのdownload要求が制限される場合はScreenのボタンから再操作する。
通知はdownload要求の発行を示し、端末への保存完了を保証しない。成功要求のobject URLは60秒の猶予後、または非復帰page teardownで解放する。保持上限は8件である。失敗した要求のURLは即時解放する。

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
cargo run -p birdman-game-cli --locked -- verify-hybrid-flight all
npm run dev
```

Web起動ページは `http://localhost:5173/pr-simulator-game-1/` で確認できる。開発時に`npm run dev`・`npm run typecheck`・`npm test`・`npm run build`を実行すると、`wasm-bindgen` bindingを生成する。型・自動試験・buildの合格と、実ブラウザー上のWebGL操作・全gameplay loop・実機の受入を区別する。

`main`へのpush後はGitHub ActionsがUbuntu・Windows双方の検査を通したビルドをGitHub Pagesへ配信する。公開先は [GitHub Pages](https://tokutori.github.io/pr-simulator-game-1/) である。手動実行も`main`から可能である。現段階は試験公開とし、BPG-013の配布条件と実ブラウザー受入は未完了のまま追跡する。

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

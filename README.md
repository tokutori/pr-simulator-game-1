# 広報用鳥人間シミュレーターゲーム

琵琶湖を舞台とする、短時間の無推力滑空ゲームである。
物理計算とゲーム進行は、共通のRust実装を使用する。
表示と操作には、WebAssembly・Three.jsによるWeb版と、Bevyによるネイティブ版を用意する。

広報版では、松原水泳場付近から湖中央方向へ約500 m、数十秒間の飛行を設計目標とする。
**注意:** この目標と、現在の架空機体モデルの性能は分けて扱う。
Web版はキーボードとゲームパッドによる操作に対応する。
操縦方式には`Manual`・`Shared`・`Automatic`があり、FBWによる操縦支援を備える。

## 現在の実装範囲

### 表示と操作

- `Screen`：通常のディスプレイに表示する。
- `WebXR`：ブラウザーのWebXRセッションを利用し、HMDで表示・操作する。
- `Phone VR`：スマートフォンで左右眼の映像を表示し、端末姿勢、視線、ゲームパッドを入力に用いる。
- Bevyネイティブ版：`Screen`表示を提供する。起動方法とWeb版との差分は[ネイティブ版の説明](docs/bevy-native-screen.md)を参照する。

BPG-014〜016で、各表示方式の基盤と自動試験を実装した。
Web版は起動画面から共通のゲーム進行へ接続する。
実HMD・スマートフォン・ブラウザー・ビューアーの組合せごとの表示と操作には、未確認の条件が残る。

Windows・macOS・Linuxの各`x86_64`/`arm64`への対応は、[#543 / PR #544](https://github.com/tokutori/pr-simulator-game-1/pull/544)で実装している。
6構成のビルド結果と配布物の確認状況は、同IssueとReleaseに記録する。
今回の変更に対するGPU描画と実機操作の受入は未完了である。

### 物理と操縦

`birdman-game-core`は、次の処理を提供する。

- 6自由度の剛体運動（6DoF）、パイロットの重心移動、空間風場。
- 全機の静的空力特性と、現在の流れを基準とする局所荷重の差分を組み合わせたハイブリッド空力モデル。
- 水平尾翼・垂直尾翼の操舵権限配分、取付角の変化速度制限、FBW。
- パイロット位置の制御、固定周期の飛行更新、発進条件の変換。
- 更新区間内の接触時刻を求める着水判定、飛距離の採点。

これらを`TailFlightScenario`へ接続している。
操舵系統は水平尾翼と垂直尾翼の二系統である。
機体姿勢と角速度は、roll・pitch・yawの三成分で表す。

空力モデルの実装は、次の作業に分かれる。

- BPG-038：借用データによる非線型の全機体静的空力特性表、モーメント軸、固定参照点、区分線型（PWL）補間。
- BPG-039 / [#218](https://github.com/tokutori/pr-simulator-game-1/issues/218)：全機体静的空力特性表と局所法線力の差分、検証済み形状、`TailIncidence`、適用範囲の閉境界、RK段階付きの原因情報、排他的なシナリオ生成経路。
- [#219](https://github.com/tokutori/pr-simulator-game-1/issues/219)：二系統のアクチュエーター、操舵権限配分、pitch・yaw角速度（q/r）のFBW。
- [#220](https://github.com/tokutori/pr-simulator-game-1/issues/220)：公開の架空機体モデル、トリム、発進条件。
- [#221](https://github.com/tokutori/pr-simulator-game-1/issues/221)：架空機体モデルを既定とする、入力・表示・保存・再生のアプリケーション統合。

数式と近似の適用範囲は[空力契約](docs/aerodynamics.md)に従う。
合成モデルの定義と限定的な操縦受入は[Playable契約](docs/playable-hybrid-model.md)を参照する。

**モデルに関する注意**

- 現在のモデルは架空機体の合成モデルであり、実機の性能を保証しない。
- 過去の検証用データにおける飛距離は、現在のハイブリッドモデルの合否基準へ流用しない。
- 現在の架空機体モデルに、実機係数や非公開の`xlsx`は導入しない。
- 実機モデルの出典調査と精度検証は、BPG-035としてM6の後に実施する。ゲーム機能の開発をその前提にしない。

### ゲーム進行と既定モデル

Rustの`GameSession`が、次のシーンの進行を管理する。

- `Title`
- `Setup`
- `Briefing`
- `Countdown`
- `Flight`
- `Result`
- `Replay`

独立したデモ記録の再生は`Attract`として管理する。
`Setup`では、表示情報（`Information`）、操縦支援（`Assistance`）、気象条件（`Weather`）を選択する。
`Briefing`では、発進前に準備結果を確認する。

新規飛行の既定モデルは次のとおりである。

| 項目 | 識別子 | バージョン |
|---|---|---:|
| 機体モデル | `bpg041-playable-hybrid-mock` | 3 |
| コントローラー | `bpg040-tail-rate-feedback` | 3 |
| カタログ | 現行の共通カタログ | 5 |
| シナリオ | 現行の登録シナリオ | 5 |

保存記録は、記録時点の識別情報を保持する。
識別子、物理的な制約、環境、終了理由はRustから取得する。

### 記録・解析・再生

- 新規飛行と保存記録の閲覧には、二系統尾翼のJSON記録形式であるschema 6を使用する。
- `Result`・`Analysis`・`Replay`は同じ`FlightRecord`を参照し、元の原因情報と終端時刻を保持する。
- 再生時計と集計値はRustが所有する。
- Web版は、IndexedDBへの保存、Personal Bestの選定、水平地図、高度・速度グラフ、同期カーソル、理由情報を伴う風の取得を提供する。

未知の保存環境へ、現在の環境や地図を適用しない。
schema 1〜5には未対応の通知を表示する。
既存の保存データは、自動で削除・変換しない。

### 共通UIと検証範囲

`Screen`・`WebXR`・`Phone VR`・Bevyは、Rustの共通UI定義から表示内容を導出する。
Web版の各表示方式は、同じセッション所有者を使用する。
VRでは、頭部に追従するHUDとメニューの配置基準を分離する。

確認済みの実行条件と、未確認の受入条件を分けて記録する。

- コミット`1093d4cb`では、実Chromium/WebGLによる`Screen`の操作・保存・再生を確認した。
- `Phone VR`では、合成センサーを用いて左右眼表示の`Title`・`Menu`・`Setup`・`Briefing`を確認した。
- 上記の`Phone VR`検査では、飛行中の頭部追従HUD、実センサー、実HMDを確認していない。
- 自動試験・ビルドの成功、実ブラウザーでの操作、実GPU描画、実機受入は、それぞれ別の検証結果として扱う。

詳細は[検証契約](docs/verification.md)、各段階の完了条件と依存関係は[実装計画](docs/implementation-plan.md)を参照する。

## 飛行ログの出力

`Result`と、保存記録の`Replay`・`Analysis`には、「飛行ログ CSV」と「元記録 JSON」を用意する。
`Screen`とVRは、共通の出力操作を使用する。

### CSV

- Rustが、保存された全標本の数値・単位・欠損理由を出力する。
- 加速度は、保存された速度から求めた有限差分による推定値として出力する。
- 推定値には、差分方式、標本時刻、値の有効性を併記する。
- 保存していない空力荷重などの値は補完しない。
- CSV出力形式のバージョン2では、尾翼の取付角と、算出方法を明示した推定加速度を出力する。

### JSON

- 選択中の元`FlightRecord`を出力する。
- 保存形式とメタデータを保持する。
- 表示カーソルの位置による切り出しや、再シミュレーションは行わない。
- 保存済みschema 6のJSONは、検証済みの原文を維持する。
- 記録時に保存していないソースハッシュを、現在のビルドから補完しない。

### ブラウザーでのダウンロードに関する注意

- ダウンロード要求が制限された場合は、`Screen`のボタンから再操作する。
- 通知はダウンロード要求の発行を示す。端末への保存完了は保証しない。
- 要求に使用したオブジェクトURLは、成功時には60秒後、失敗時には即座に解放する。
- 復帰を伴わないページ終了時には、保持中のURLを解放する。
- オブジェクトURLの保持上限は8件である。

## 開発環境と起動

Rustは`1.97.0`を使用する。
ターゲットとコンポーネントは`rust-toolchain.toml`に固定する。
PowerShellのテキスト入出力では、UTF-8を明示する。

### Bevyネイティブ版

```sh
cargo run -p birdman-game-bevy --release --locked
```

ネイティブ版の起動には、Node.jsやブラウザーを必要としない。
OSごとの前提環境と操作は[ネイティブ版の説明](docs/bevy-native-screen.md)、配布物の確認手順は[Native previewの配布](docs/native-release.md)を参照する。

### Web版と共通の開発コマンド

Web版の開発には、Node.js `24.14.1`以上の24系とnpmを使用する。
PowerShellでは`npm.cmd`を使用できる。

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
cargo run -p birdman-game-cli --locked -- verify-hybrid-flight all
npm run dev
```

Web版の開発ページは`http://localhost:5173/pr-simulator-game-1/`で確認できる。
`npm run dev`・`npm run typecheck`・`npm test`・`npm run build`は、実行前に`wasm-bindgen`のバインディングを生成する。
型検査、自動試験、ビルドの成功と、実ブラウザー上での一連のゲーム操作の確認は分けて扱う。

### 形式検証

```sh
powershell -File tools/verus/check-pause-reasons.ps1
```

実際の`birdman-game-core`を検証する固定Verusでは、ジェネリックな派生`Clone`の仕様が未生成であることを示す警告が2件残る。
対象の保持契約は、`Clone`による同値性を仮定しない。
[保証範囲と未証明の境界](docs/formal-verification.md)を参照する。

## CIとGitHub Pages

`main`へのpushは、UbuntuとWindowsの検査を実行する。
GitHub Pagesは、明示的な公開指示を受けた後に更新する。

1. `main`の`Check`ワークフローを手動実行する。
2. `deploy_pages`を有効にする。
3. 同じ実行で検査した成果物を配信する。

公開先は[GitHub Pages](https://tokutori.github.io/pr-simulator-game-1/)である。
現段階は試験公開とし、BPG-013の配布条件、VRと実端末の未完了受入を追跡する。

## 設計資料

- [アーキテクチャ](docs/architecture.md)・[座標系](docs/coordinates.md)
- [物理契約](docs/physics.md)・[空力](docs/aerodynamics.md)・[風場](docs/wind-field.md)
- [描画](docs/rendering.md)・[データ候補](docs/data-sources.md)
- [Desktop / WebXR / Phone VR](docs/presentation.md)
- [VRの参照座標系](docs/vr-spaces.md)・[3Dエンジンの交換境界](docs/render-boundary.md)
- [多軸難易度・ゲーム進行・再生](docs/difficulty.md)
- [8シーンとオーバーレイ](docs/game-flow.md)・[パイロット視点とシネマティックカメラ](docs/camera.md)
- [飛行記録](docs/flight-record.md)・[結果解析](docs/result-analysis.md)
- [検証](docs/verification.md)・[開発手順](CONTRIBUTING.md)

## 開発方針と利用条件

- 全シーンで共通の`Screen`・VR表示基盤を早期に確立する。
- 簡易な世界表示で、ゲーム進行と記録・再生を完成させる。
- 実環境データと景観品質は、その後に整備する。
- 水面は速度と姿勢を把握する重要な手掛かりである。水面描画を、地形・会場や空・雲より先行させる。
- BPG IDは作業の識別子であり、数値順は実装順を表さない。

参照元は[`tokutori/simulator-prototype-1`](https://github.com/tokutori/simulator-prototype-1)である。
ローカルでの名称は`simulator1`とする。
数式・設計・検証ケースを参照し、実行時の依存には含めない。

ソースコードのライセンスはMIT Licenseである。
第三者データには、[個別の利用条件](THIRD_PARTY_DATA.md)を適用する。

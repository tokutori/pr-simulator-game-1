# 広報用鳥人間シミュレーターゲーム

琵琶湖の彦根・松原水泳場付近を舞台とする、短時間の無推力滑空ゲームである。
物理・空力・ゲーム進行を共通のRust実装で処理し、Web版とBevyネイティブ版から利用する。

**[公開Web版を試す](https://tokutori.github.io/pr-simulator-game-1/)** · [Native版の起動・操作](docs/native-guide.md)

機体・空力パラメータはゲーム用の合成モデルであり、特定の実機性能を表すものではない。

## 主な機能

- 6自由度運動、パイロットの前後移動、空間風場、水面接触を100 Hzの固定周期で計算する。
- 全機静特性と局所荷重差分による空力モデル、二系統尾翼、FBW操縦支援を備える。
- 設定・発進・飛行・結果・解析・Replay・再試行を共通セッションとUIで管理する。
- 飛行記録の保存、条件別Personal Best、軌跡・高度・速度チャート、JSON/CSV出力に対応する。
- WebはScreen / WebXR / Phone VR、NativeはデスクトップのScreen表示を提供する。

- 共通基盤: Rustの物理・進行・記録を共用し、UI・cameraの導出と描画資産の生成を独立した層で担う。
- プラットフォーム: 各adapterが表示・入力・保存I/Oを担当する。
- 検証状況: 対象環境ごとの確認範囲は[検証仕様](docs/verification.md)と各Issueで管理する。

## クイックスタート

### Web版

Rust（`rust-toolchain.toml` で固定）、Node.js 24系（24.14.1以上）、npmを用意する。

```sh
npm ci
npm run dev
```

ブラウザーで `http://localhost:5173/pr-simulator-game-1/` を開く。
PowerShellでスクリプト実行が制限される場合は `npm.cmd` を使用する。

### Bevyネイティブ版

リポジトリのルートから起動する。Node.jsやブラウザーは不要である。

```sh
cargo run -p birdman-game-bevy --release --locked
```

Windows・macOS・Linuxのx86_64 / ARM64向け配布ファイル、OS別の前提環境、
操作キー、保存先は[Nativeガイド](docs/native-guide.md)を参照する。

## 開発・検査

Webの型検査・リント・単体試験・本番ビルド:

```sh
npm run verify
```

Rustの書式・リント・試験:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Verusの実行方法は[形式検証](docs/formal-verification.md)、
数値検証・各ターゲットのビルド・CUIオフスクリーン描画は[検証仕様](docs/verification.md)を参照する。
開発手順とIssue/PRの記述規則は [CONTRIBUTING.md](CONTRIBUTING.md)に記載している。

## ドキュメントとライセンス

| 分野 | 文書 |
|---|---|
| 全体構成・データ | [アーキテクチャ](docs/architecture.md) · [出典・環境・記録形式](docs/data-and-formats.md) |
| 物理・操縦 | [飛行力学・空力・風・接触](docs/flight-dynamics.md) · [操縦制御](docs/flight-control.md) |
| ゲーム・表示 | [ゲーム進行・UI・カメラ](docs/gameplay-and-ui.md) · [描画とプラットフォーム](docs/rendering-and-platforms.md) |
| 利用案内 | [Nativeガイド](docs/native-guide.md) |
| 検証 | [試験・数値・CUI描画](docs/verification.md) · [形式検証](docs/formal-verification.md) |

- ソースコード: [MIT License](LICENSE)。
- 第三者データ・フォント: [個別の利用条件](THIRD_PARTY_DATA.md)。
- 設計・数式の参照元: [`tokutori/simulator-prototype-1`](https://github.com/tokutori/simulator-prototype-1)。

# 機体JSON viewer

機体JSONの検証結果、設計値、導出諸元、定常滑空トリム、静特性と平面形を確認するデスクトップツールである。
検証と導出にはゲームと同じ `birdman-game-content`・`format`・`core` を使用する。

## 起動

リポジトリ直下で実行する。Rust版は親ディレクトリの `rust-toolchain.toml` に従う。

```powershell
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target'
cargo run --manifest-path tools/aircraft-viewer/Cargo.toml --locked
cargo run --manifest-path tools/aircraft-viewer/Cargo.toml --locked -- C:/models/my-aircraft.json
```

- 入力: 無指定時は登録機体を表示する。「機体JSONを開く」、ドラッグ&ドロップ、起動引数で別の機体を開く。
- 検証: 読み込みごとに共通の機体検証とトリム計算を行い、失敗時は元の型付きエラーを表示する。
- 識別: SHA-256は読み込んだJSONのバイト列から算出する。
- 操作: タブで設計・特性・平面形を切り替え、グラフはドラッグ・ホイールで移動・拡大できる。
- 保存: 本ツールは閲覧専用である。

## 構成と検査

独立workspaceと `Cargo.lock` を持ち、egui依存をツール内に閉じる。
`app` は読み込み結果、`ui` は画面構成、`design` と `characteristics` は表示を担当する。

```powershell
cargo fmt --manifest-path tools/aircraft-viewer/Cargo.toml --check
cargo clippy --manifest-path tools/aircraft-viewer/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path tools/aircraft-viewer/Cargo.toml --locked
```

| 依存 | 固定版・互換性 |
| --- | --- |
| eframe | [0.33.3](https://github.com/emilk/egui/blob/0.33.3/Cargo.toml)、egui 0.33系、MSRV 1.88 |
| egui_plot | [0.34.1](https://github.com/emilk/egui_plot/blob/0.34.1/Cargo.toml)、egui 0.33系、MSRV 1.88 |
| rfd | [0.15.4](https://docs.rs/rfd/0.15.4/rfd/struct.FileDialog.html)、ネイティブファイル選択 |

日本語には同梱の M PLUS 1p Regular を使用する。利用条件は [SIL OFL 1.1](../../assets/fonts/MPLUS1p-OFL.txt) を参照する。

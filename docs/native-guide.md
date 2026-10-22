# Native版の利用案内

Web版と同じ飛行計算・操縦支援・記録を利用するデスクトップ版である。Windows・macOS・Linuxのx86_64とARM64に対応する。

## ダウンロードと起動環境

OSとCPUに一致する配布アーカイブを展開し、実行ファイルを起動する。日本語フォント・シェーダー・登録データを組み込んでおり、オフラインで利用できる。

| 環境 | ファイル名のCPU表記 | 前提・初回起動時の注意 |
|---|---|---|
| Windows | `x86_64` / `aarch64` | 対応GPUドライバー。ランタイム不足のエラーが出る場合は、対象CPU用の[Microsoft公式VC++再頒布可能パッケージ](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)を導入する |
| macOS | Intel: `x86_64` / Apple Silicon: `aarch64` | 対応GPU。試用版は無署名のため、Gatekeeperの警告が表示される場合がある |
| Linux | `x86_64` / `aarch64` | Ubuntu 24.04基準。X11またはWayland、GPUドライバー、Vulkan loaderが必要 |

- Windows: `.\birdman-game-bevy.exe`
- macOS・Linux: `./birdman-game-bevy`
- Linuxの実行時パッケージ: `libudev1`、`libx11-6`、`libxcursor1`、`libxrandr2`、`libxi6`、`libwayland-client0`、`libxkbcommon0`、`libxkbcommon-x11-0`、`libvulkan1`。
- ハッシュ照合: PowerShellは `Get-FileHash -Algorithm SHA256 <archive>`、Linuxは `sha256sum -c SHA256SUMS`、macOSは `shasum -a 256 -c SHA256SUMS` を使用する。
- ビルド来歴: 配布物の `SOURCE.json` とアーカイブ内の `BUILD-INFO.json` に記録する。
- 画面の最低対応解像度: UI倍率を適用した後の表示領域で1280×720。推奨は1920×1080以上である。

## 最初のフライト

1. タイトルの「飛行を設定」から「飛行設定」を開く。
2. プリセット・計器表示・操縦支援・気象を選ぶ。既定は「すべて（Full）」「軽い支援（Light Assisted FBW）」「標準（Typical）」、3D描画品質は「Auto」である。
3. 「飛行準備へ進む」で「発進前の確認」を開き、飛行条件と操作方法を確認する。
4. 「発進カウントダウンを開始」で発進する。取り消すと「発進前の確認」へ戻る。
5. 飛行終了後、結果画面で距離・終了理由・記録を確認する。
6. 「同じ条件で再試行」で同じ条件の発進前確認へ戻る。条件を変える場合は「飛行設定」を開く。

- 一時停止: 「一時停止」のメニューから画質設定や操作方法を確認できる。「飛行を再開」で続ける。
- 自動停止: 非アクティブ化や処理遅延で停止した場合は、原因が解消してから「飛行を再開」を選ぶ。
- 条件変更: 計器表示・操縦支援・気象は飛行設定で、画質は一時停止中にも変更できる。
- クレジット: タイトルと一時停止メニューの「クレジット・利用条件」から開く。

## 操作

| 入力 | 操作 |
|---|---|
| ↑ / ↓ | 機首上げ / 下げ |
| ← / → | 左 / 右旋回 |
| J / L | パイロットを後方 / 前方へ移動 |
| C | 操縦席 / 追尾の視点切替 |
| P / Esc | 飛行の一時停止・再開 |
| 右ドラッグ | 視点を回転 |
| Tab / Shift+Tab | メニューのフォーカスを移動 |
| Enter / Space | 選択中の操作を実行 |
| 時刻スライダー上の矢印 / Home / End | 解析・記録再生の時刻を変更 |
| F12 | 作業ディレクトリへスクリーンショットを保存 |

- パイロット移動: キーを離すと目標位置を保持する。
- ゲームパッド: 左スティックで機首と旋回、右スティック上下でパイロットの目標位置を操作する。接続・再開時は一度中央へ戻し、右スティックを現在の目標位置へ合わせると操作を引き継ぐ。
- 記録再生の視点: 再生画面の視点選択を使用する。
- 終了: ウィンドウを閉じる。

## 画質と飛行記録

- 計器: 選択した表示設定に応じ、速度・高度・姿勢・方位・風・迎角などを表示する。
- 画質: 「Auto」で描画負荷に応じて自動調整する。手動では描画解像度100%・75%・50%と水面画質Low・Medium・Highを選べる。
- 解析: 飛行軌跡・高度・速度のグラフを、共通の時刻カーソルで確認できる。
- 保存: 結果と記録再生の画面から、元記録JSONと飛行ログCSVを出力する。保存先は起動時の作業ディレクトリ内の `flight-logs` で、保存ごとに新しいファイルを作る。
- 一覧: タイトルに条件別の自己ベストを優先して最大3件表示する。JSON保存後と次回起動時に更新する。
- ファイル管理: 削除や名前の変更はOSのファイル管理機能を使う。

## ソースからの起動とオプション

固定Rustツールチェーンと[Bevyのビルド環境](https://bevy.org/learn/quick-start/getting-started/setup/)を準備し、リポジトリのルートで実行する。初回のReleaseビルドには時間を要する。

Linuxではウィンドウ関連の開発パッケージに加え、ゲームパッド用の`libudev-dev`を使用する。

```sh
cargo run -p birdman-game-bevy --release --locked
```

| オプション | 説明 |
|---|---|
| `--help` / `--version` | 引数一覧、またはバージョン・ビルド情報を表示して終了 |
| `--no-vsync` | 垂直同期の解除を要求。GPU・OSにより選択される同期方式が異なり、画面のちらつきや消費電力が増える場合がある |
| `--export-dir DIR` | 飛行記録の保存先を変更 |
| `--font PATH` | 組込みのM PLUS 1p Regularを指定フォントへ変更 |

ソースから起動する場合は `--` の後ろへオプションを付ける。

```sh
cargo run -p birdman-game-bevy --release --locked -- --export-dir ./my-flight-logs
```

## 利用条件と詳細

- データ出典: [THIRD_PARTY_DATA.md](../THIRD_PARTY_DATA.md)。同梱ライブラリの利用条件は `THIRD-PARTY-LICENSES.html`、フォントの利用条件は `assets/fonts/MPLUS1p-OFL.txt` を参照する。
- 記録形式: [データ形式](data-and-formats.md)。
- ゲーム進行・計器・視点: [ゲームとUI](gameplay-and-ui.md)。
- 開発者向けの描画・VR仕様: [描画とプラットフォーム](rendering-and-platforms.md)。
- 自動描画検査と受入条件: [検証](verification.md)。

# Native版の利用案内

- Native版はBevyで描画するデスクトップ用Screen版である。
- Web版と同じRustの飛行計算、操縦支援、ゲーム進行、記録形式、共通UIを使用する。
- Windows・macOS・Linuxのx86_64とarm64を対象とし、VRはWeb版で扱う。

## 起動環境

- OSとCPUに一致するarchiveを展開する。
- Windowsは`birdman-game-bevy.exe`、macOSとLinuxは`./birdman-game-bevy`を起動する。
- shader、日本語フォント、実行に必要な登録データは実行ファイルに含まれる。
- Node.js、ブラウザ、ソースツリーの配置は起動条件に含まれない。

| 環境 | 前提 |
|---|---|
| Windows | 対応GPU driver。VC++ runtime不足が報告された場合は、対象CPU用の[Microsoft公式VC++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)を導入する |
| macOS | 対象CPU用の実行ファイルと対応GPU。試用配布は無署名であり、Gatekeeper・notarizationの受入状況を確認する |
| Linux | Ubuntu 24.04のglibcを基準とする。X11またはWayland、対応GPU driver、Vulkan loaderを用意する |

Linuxのruntime packageには`libx11-6`、`libxcursor1`、`libxrandr2`、`libxi6`、`libwayland-client0`、`libxkbcommon0`、`libxkbcommon-x11-0`、`libvulkan1`がある。build成功と実GPUでの受入を区別し、SmartScreen等の配布保護機構への対応状況も確認する。

- 配布物の`SHA256SUMS`を照合してから展開する。
- PowerShellでは`Get-FileHash -Algorithm SHA256 <archive>`、Linuxでは`sha256sum -c SHA256SUMS`、macOSでは`shasum -a 256 -c SHA256SUMS`を使用できる。
- `SOURCE.json`とarchive内の`BUILD-INFO.json`にはsource SHA、target、実行ファイル等のhashが記録される。

リポジトリから起動する場合は、ルートで次を実行する。

```sh
cargo run -p birdman-game-bevy --release --locked
```

- 固定Rust toolchainと[Bevyのnative build環境](https://bevy.org/learn/quick-start/getting-started/setup/)が必要である。
- WindowsはMSVC toolchain、macOSはXcode Command Line Toolsを使用する。
- 依存関係を含む初回Release buildには時間を要する。
- 操作性やFPSを比較する場合はReleaseを使用する。

## 最初のフライト

1. タイトルの「飛行を設定」からFlight Setupへ進む。
2. プリセット、表示情報、操縦支援、気象を選択する。既定はFull・Light Assisted FBW・Typical、3D描画品質はAutoである。
3. 「飛行準備へ進む」でBriefingを開き、飛行条件、代表風、操作方法を確認する。versionやseedは「技術情報」で確認できる。
4. 「発進カウントダウンを開始」で発進する。取消操作はBriefingへ戻る。
5. 飛行を操作し、着水または終了後のResultで距離と終了理由を確認する。
6. 「同じ条件で再試行」は同じ機体・シナリオ・初期条件のBriefingへ戻る。条件変更はFlight Setupへ戻って行う。

- 表示情報、操縦支援、気象、操縦bindingは飛行中に固定する。
- 湖面画質や3D描画品質は一時停止中にも変更できる。
- 設定画面を閉じた後はPauseへ戻り、明示的な再開操作で飛行を再開する。

## 操作

| 入力 | 操作 |
|---|---|
| ↑ / ↓ | pitch入力 |
| ← / → | 旋回入力 |
| J / L | パイロット重心の目標位置を前後へ移動 |
| C | NativeのFlight表示をPilot / Chaseへ切替 |
| P / Esc | 飛行の一時停止・復帰 |
| マウス右ボタンを押してドラッグ | 視点操作 |
| Tab / Shift+Tab | メニューの操作順にフォーカス移動 |
| Enter / Space | フォーカスした有効な操作を実行 |
| Range上の矢印 / Home / End | 解析・Replayの時刻を変更 |
| F12 | 作業ディレクトリへスクリーンショットを保存 |

- 機体は二系統尾翼モデルであり、独立したroll入力は設けない。
- パイロット移動入力を解放した後も目標位置を保持する。
- Replayの視点は画面の共通camera操作から選択する。
- 終了はウィンドウを閉じて行う。

- 非アクティブ化や処理遅延でも飛行は停止する。
- 停止理由が解消した後にResumeを実行する。
- 停止中の物理時刻、記録、水面のsimulation時刻は保持される。
- 重い処理の直後に停止した場合も、再開可能な状態になるまで待つ。

## 表示と画質

- HUDには設定に応じて速度、高度、時間、距離、姿勢、方位、風、迎角、パイロット目標位置を表示する。
- 未定義の物理量は数値を補完せず理由を示す。
- 適用範囲外や計算失敗の終了では最後の有効状態を表示し、Resultの短い説明と「技術情報」に元の原因を保持する。
- 操縦支援の保護範囲は[飛行制御](flight-control.md)を参照する。

- Autoは有効なGPU計測に応じ、解像度、shadow、水面品質を順に調整する。
- 手動の100%・75%・50%では3Dと反射の内部解像度を変更し、UIと画角を維持する。
- 水面Low・Medium・Highと描画倍率は飛行計算や記録を変更しない。
- Autoの対応状況・適用済み画質・失敗理由は設定表示で確認する。

表示同期を解除する要求は次のとおりである。

```sh
cargo run -p birdman-game-bevy --release --locked -- --no-vsync
```

- GPUとOSに応じて同期方式が選択されるため、同期解除とFPS増加は保証されない。
- tearingや消費電力の増加を伴う場合がある。
- 画質、処理能力、表示同期による上限は分けて評価する。

## 記録・解析・保存

- ResultのSummary、Analysis、Replayは同じ確定記録を使用する。
- Analysisには水平軌跡、高度、対気・対地速度のグラフがあり、共有cursorで同じ時刻を参照できる。
- ReplayからResultへ戻ると選択したtabと時刻を保持する。

- Resultの保存操作からJSONとCSVを出力できる。
- 既定の保存先は起動時の作業ディレクトリ内の`flight-logs`である。
- 既存ファイルを上書きしない。
- 別の保存先は次のように指定する。

```sh
cargo run -p birdman-game-bevy --release --locked -- --export-dir ./my-flight-logs
```

- JSONは保存・再生用の完全な記録であり、CSVは位置、速度、加速度、姿勢、角速度、角加速度、入力、操縦・環境diagnostic等のsample列を扱う。
- 収録項目と単位は[データ形式](data-and-formats.md)を参照する。
- 欠損値と元の失敗原因を保持する。

- タイトルは保存先の有効な`flight-log-*.json`を読み、Personal Bestを優先して最大3件表示する。
- JSON保存後と次回起動時に一覧を更新する。
- CSV、別名ファイル、symbolic linkは一覧の対象外である。
- 保存記録のReplayはTitleへ戻る。
- 削除・rename・file dialogは提供しないため、保存ファイルの管理はOSで行う。

## フォント・利用条件・詳細

- 日本語表示には組込M PLUS 1p Regularを使用する。
- `--font PATH`で利用条件を確認済みのローカルフォントへ変更できる。
- 実行時のWebフォント取得は不要である。

- `--help`は引数、`--version`はversion、source SHA、OS、CPUを表示する。
- この2操作はウィンドウやGPU、保存記録を初期化しない。
- source SHA未指定の開発buildは`unrecorded`と表示する。

第三者Rustライブラリの条件は同梱の`THIRD-PARTY-LICENSES.html`、公開データの帰属は[THIRD_PARTY_DATA.md](../THIRD_PARTY_DATA.md)、フォントの全文licenseは`assets/fonts/MPLUS1p-OFL.txt`を参照する。描画・VRの契約は[描画とplatform](rendering-and-platforms.md)、ゲーム進行は[ゲームとUI](gameplay-and-ui.md)、検査手順と未検証範囲は[検証](verification.md)に記載する。

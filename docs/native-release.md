# Native previewの配布

Windows・macOS・Linuxのx86_64とarm64を対象とする。自分のOSとCPUに一致するarchiveを展開し、
Windowsでは `birdman-game-bevy.exe`、macOS/Linuxでは `./birdman-game-bevy` を起動する。
起動方法と操作は同梱の `docs/bevy-native-screen.md` を参照する。
shaderと日本語フォントは実行ファイルへ埋め込む。ソースツリー、Node.js、ブラウザは起動に不要である。
同梱の `assets` は登録済み公開データと利用条件を含む。

試用配布は無署名である。Windows SmartScreenとmacOS Gatekeeper/notarizationの受入は未確認である。
WindowsでVC++ runtime不足が報告された場合は、対象CPUに対応する
[Microsoft公式VC++ Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist/)
を導入する。実行ファイルのDLL依存は最終成果物の検査結果に従う。
LinuxはUbuntu 24.04のglibcを基準とし、X11またはWayland、対応GPU driver、Vulkan loaderを必要とする。
必要なruntime packageには `libx11-6`、`libxcursor1`、`libxrandr2`、`libxi6`、`libwayland-client0`、
`libxkbcommon0`、`libxkbcommon-x11-0`、`libvulkan1` がある。GPUとdriverの適合は別途確認する。
CPU-onlyの `--help` / `--version` 成功とGPU描画・操作・80 FPSの受入を区別する。

## 配布物の確認

`SOURCE.json` と各archiveの `BUILD-INFO.json` にsource SHA、target、Cargo.lockと実行ファイルのhashを記録する。
`SHA256SUMS` を照合してから展開する。例えばPowerShellでは `Get-FileHash -Algorithm SHA256 <archive>`、
Linuxでは `sha256sum -c SHA256SUMS`、macOSでは `shasum -a 256 -c SHA256SUMS` を使用する。
第三者Rust crateのlicenseは `THIRD-PARTY-LICENSES.html`、公開assetの出典は `docs/data-sources.md`、
日本語フォントの条件は `assets/fonts/MPLUS1p-OFL.txt` に記載する。

## maintainerのbuild手順

`Native preview artifacts` workflowを手動実行する。`source_sha` の省略時は開始時のmainを一度解決し、
全構成を同じ完全SHAへ固定する。mainへの統合前は、同じリポジトリのPRへ `native-preview-build` labelを付与して
固定headのbuildを先行できる。PRの仮merge SHAと配布source SHAを混同しない。

6つの実hostでRelease buildとbinary architecture検査を行い、各実行ファイルをソースツリー外の一時directoryへ
コピーして `--help` / `--version` を実行する。CLIがGPU・font・ログ保存を初期化しないことも確認する。
license生成は固定cargo-aboutを用いる単一job、archive生成は全6build成功後の単一jobで行う。
巨大なtarget cacheは保存しない。通常の3gateとPages公開条件は独立して維持する。

最終 `native-preview-<SHA>` artifactには6archive、checksum、来歴をまとめる。
全構成を同じSHAで取得できた場合だけdraft Releaseへ添付する。workflow自体はtagやReleaseを作成しない。
review済みPR headがそのままmainへfast-forwardされた場合、同一SHAのartifactを再利用できる。
SHAが変化した場合は新しいsourceでbuildする。描画・実機操作は別の受入事項である。

転送Actionの保守は `Artifact transfer check` workflowで検査する。既存の未失効 `native-licenses-<SHA>`
artifact一個を選択し、download→upload→再download後のfile pathとSHA-256を照合する。
手動実行ではartifact IDを指定でき、省略時は最新の候補を使用する。対象2workflowを変更したPRでも実行し、
通常のソース変更には追加待機を導入しない。Nativeの再コンパイル、artifact内データの実行、Release操作は行わない。

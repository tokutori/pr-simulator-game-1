# 第三者データとライセンス

プロジェクトのソースコードはMIT Licenseである。
第三者データ・画像・地形・機体assetには各出典の利用条件を適用し、MITへ再ライセンスしない。
ソフトウェア依存のライセンスも各packageの条件を維持する。

配布assetの一覧と個別条件は `assets/manifest.toml` に記載する。候補一覧は `docs/data-sources.md` に記載する。
候補掲載は取得・再配布の許諾確認を意味しない。

## 琵琶湖の局所湖岸

`assets/biwa-shoreline.json` は OpenStreetMap の琵琶湖 relation 63499 の
外周 way 41696803 を発進地点周辺に切り出して簡略化した加工済みデータである。
このデータには [Open Database License 1.0](https://opendatacommons.org/licenses/odbl/1-0/) を適用する。
帰属表示は「© OpenStreetMap contributors」であり、ゲーム中の HUD と
データ内のメタデータに掲載する。ソースコードの MIT License をこのデータへ適用しない。

## 登録要件

`assets/manifest.toml` を正本とし、各assetに次の情報を登録する。

- path: `assets/` 内の相対パス
- sha256: 配布ファイルのSHA-256
- source: 出典URLまたは自作データの生成元識別子
- source_version: 元データのversionまたは取得日と識別子
- license: SPDX識別子または明示的な利用条件名
- license_url: 条件のURLまたはリポジトリ内の条件ファイル
- attribution: 実際に表示する帰属文
- processing: tool versionと処理パラメータ、地理データではoriginと標高基準

自作assetも生成元とライセンスを登録する。`web/public/` の配布assetも同じ管理経路を使用し、
未登録ファイルの別経路配布を禁止する。manifestはファイルの存在、hash、登録漏れ、重複をCIで検査する。
検査はmetadataの存在を保証する。法的な利用可否は採用時の一次資料確認により判断する。
出典ファイルを移植する場合は元のcopyrightとlicense noticeを保持する。

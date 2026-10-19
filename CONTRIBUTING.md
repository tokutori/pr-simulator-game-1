# 開発手順

全媒体の文章には [AGENTS.mdの記述原則](AGENTS.md#全媒体の記述原則)を適用する。
以下はIssue・PR・文書への具体的な適用方法と開発手順である。

## Issue・PR・文書の記述

[Issues](https://github.com/tokutori/pr-simulator-game-1/issues)を作業と依存関係、
[Milestones](https://github.com/tokutori/pr-simulator-game-1/milestones)を段階ごとの達成目標の正本とする。

### Issue

- 利用者向けの達成目標または独立した障害を単位とする。補助関数や小さな試験ごとに分割しない。
- 本文は「目的」「実装方針・対象範囲」「完了条件のチェックリスト」を基本とする。同じ制約を複数の節へ繰り返さない。
- 実装統合後もOPENを維持する場合は、「完了した実装」と「残るClose条件」へ更新する。
- 親Issueには現在の対応表と残課題だけを置く。個別PRの試験件数・CI履歴・内部運用規則を蓄積しない。
- 調査方針は本文へ統合する。同じ調査を複数担当が重複コメントせず、新しい決定・完了・問題だけをまとめて報告する。
- 計画変更は本文とmilestoneへ反映する。既存BPG IDを保持し、新規IDは重複を確認する。

### PR・報告

- 関連する実装、全adapterへの接続、検証、文書更新をまとめる。commitは目的別のConventional Commitsとする。
- タイトルは `<type>(<scope>): <日本語の要約>` とする。
- 冒頭に `Closes #番号`、残件がある場合は `Refs #番号` とその理由を書く。
- 本文は「概要」「主な変更点」「検証結果」を基本とし、最終HEADで確認した結果を記載する。
- 途中のCI失敗・デバッグ日記・ブランチ切替手順・無関係な未変更事項を列挙しない。merge前に古いDraft時点の記述を更新する。
- 進捗報告は前回からの完了・変更点、現在の作業、新たな問題をIssue/PR単位で示す。回答が必要な質問は分離する。

### 文書

- `docs/` は主題別の現行仕様・利用案内とする。Issue/PRごとの境界文書や検証文書を増設しない。
- 仕様変更時は該当節を書き換えて統合し、廃止した仕様と移行経緯を除く。
- 作業履歴の日時・SHA・PR/CI番号・個人のローカルパスはIssue・PR・実行ログへ置く。出典の固定版・取得時点・ライセンス等の必要な根拠は保持する。
- READMEとNativeガイドには起動・操作・前提環境を記載し、内部設計とCI手順は設計・検証文書へ分ける。
- 文書の統合時は相互リンク、ソース内参照、配布梱包も更新する。

テキストはUTF-8、改行はLFとし、PowerShellのI/OにもUTF-8を明示する。
`gh issue` / `gh pr` の本文は `--body-file` で渡し、更新後の内容を確認する。
GitHubの独立数式は `math` fence、文中数式は `$...$` とする。

## ブランチと統合

1. 現在のbranch・worktree・remote・差分と依存先を確認し、無関係な変更を保持する。
2. 独立したbranchで実装し、並行作業には別worktreeを使う。早期にDraft PRを作成する。
3. 局所検査と独立レビューを終えた変更群をまとめてpushし、最新mainとの統合結果で必須CIを確認する。
4. レビュー対象のHEAD、remoteのHEAD、必須検査の成功を確認して統合する。

依存先が独立レビューと必要な局所検査を完了した固定HEADなら、別worktreeで先行開発できる。
Draftには依存先PRと固定HEADを示し、親の統合後にbaseをmainへ変更する。
親が更新された場合は通常mergeで取り込み、影響を確認する。最終的な統合結果のレビューと必須CIは省略しない。

fast-forward可能な場合は、fetch後に `git merge --ff-only <reviewed-head-sha>` と通常pushでmainを更新する。
分岐している場合は通常のmergeを行う。保護規則を迂回せず、force push・rebaseは行わない。
GitHubのMERGED状態とremote mainを確認する。GitHubの「Rebase and merge」はSHAを変えるためFFとは区別する。

共有build targetは同時使用を避ける。Cargoのtarget/build先にソース自身や祖先directoryを指定しない。
worktree削除前には絶対path・所有者・未commit変更・ignored directoryとreparse pointを確認する。
共有npm/cacheのリンクはリンク自体だけを非再帰で解除し、参照先の共有backingを保持してから `git worktree remove` を行う。

## 検査と公開

- 変更箇所のlint・型検査・関連試験を先行し、Rust UI/ABIの生成は必要な範囲でまとめて行う。
- 検査コマンドは [README](README.md) と[検証仕様](docs/verification.md)を参照する。最終CIのUbuntu・Windows・Verusの3ゲートを確認する。
- compiler/linter警告を確認する。外部サービスの未解消警告は原因・影響・追跡先を記録する。
- ソフトウェア試験、GPU描画、物理キー・HMD操作、実機妥当性を区別し、実施済み・失敗・未実施を報告する。
- UI・描画は可能な範囲を再現可能なCUI/headless/offscreen試験で確認する。
- mainへのpushではGitHub Pagesを公開しない。明示的な公開指示の後、mainの `Check` を `workflow_dispatch`・`deploy_pages=true` で実行し、そのrunで検査を通過した成果物を公開する。
- CI cacheの保存はtrusted main pushと非公開手動検査が担当する。PRとPages公開runは読取り専用とする。

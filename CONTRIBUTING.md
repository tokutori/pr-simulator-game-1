# 開発手順

## PRの単位

[GitHub Issues](https://github.com/tokutori/pr-simulator-game-1/issues)を作業計画・依存関係・進捗の正本とする。
Issue本文は「目的・実装方針・完了条件」を基本とし、関連契約や依存先は必要な箇所に記載する。依存先はIssue番号でリンクする。
PR統合後は、完了したPRと残る完了条件のチェックリストへ更新する。実装済みの項目を着手前の計画として残さない。
親Issueの本文には最新の対応表と未完了項目を置き、個別の試験件数・SHA・CI履歴はコメントまたは各PRへ記録する。
共通ルールやエージェントの運用手順は `AGENTS.md` / `CONTRIBUTING.md` で管理し、Issueでは作業固有の条件に絞る。
既存BPG IDは維持し、新規IDはGitHub上で重複を確認する。
実行順と段階ごとの完了条件は[GitHub Milestones](https://github.com/tokutori/pr-simulator-game-1/milestones)で管理する。
計画変更はIssue本文・milestoneへ直接反映し、関連する設計契約を同一作業で更新する。
PRのタイトルは `<type>(<scope>): <日本語の要約>` とする。
本文の冒頭には、Issueの完了条件を満たす場合は `Closes #番号`、残件を追跡する場合は `Refs #番号` と残す理由を記載する。
本文は「概要・主な変更点・検証結果」を基本とし、検証結果は最終HEADの実施済み検査と関連する未達条件に絞る。
途中のCI失敗やデバッグの経緯、先行ブランチのmerge・base変更手順はコメントまたは実行ログへ記録する。
merge前に本文を更新し、Draft時点の古い未完了表記と、差分に関係のない未実施事項の列挙を取り除く。
CIはコード・設計文書・assetを検査する。Issue本文や依存関係の検査・同期は行わず、着手時とレビュー時に確認する。

1. 依存BPGのmerge完了と基底commitを確認する。以下の先行条件を満たす場合は固定した依存HEADを確認する。
2. `feat/bpg-###-description` 等のブランチを作成する。
3. 対象の契約、実装、検証、文書を同一PRに含める。
4. CIと差分を確認し、検査結果と未検証範囲をPR本文に記載する。
5. レビュー完了後、対象head SHAを確認してmergeする。

commitは目的別のConventional Commitsとする。検査未実施の機能を完了として扱わない。
通常の開発ではmainへの変更をPR経由で管理する。

Issueは達成目標または独立した障害を単位とし、補助関数や個別試験ごとの増設を避ける。
PRは関連する機能・全adapter・検証をまとめ、目的別のcommitを保持する。
局所的に整合した変更群をまとめてpushし、最終combined treeのCIを確認する。専用の文書PRや追加gateで待機を増やさない。

feature branchは早期にDraft PRを作成する。`Check`はmain/bevyへのpush、無条件のpull_request、手動実行で起動する。
PRは仮merge tree、pushと手動実行は対象headを検査する。未PR・merge conflict中の先行head検査が必要な場合は、
local検査または対象branchの手動実行（`deploy_pages=false`）で補完する。競合解消後はPRの3gateを確認する。

## 依存作業の先行

依存先が独立review・必要な局所検査を完了した不変のHEADに固定されている場合は、別worktree/branchで依存作業を先行できる。
親branch/sourceは変更しない。親PR未merge中はchild PRのbaseを親branchとし、依存HEADのSHAを明記する。
親PRのmerge後はchild PRのbaseをmainへ変更し、必要な通常mergeで最新mainを取り込み、combined treeと最終gateを確認する。
親HEADが更新された場合はreview・局所検査の状況を再確認し、通常mergeで取り込み、影響範囲を確認する。
親CIの失敗や変更により、childにも修正・再検査が必要となる手戻りリスクを伴う。

main統合前には依存PRのmerge完了、最新mainとのcombined tree、独立review、既存3gateの成功を必須とする。
通常pushとfast-forward優先、既存変更の保護、共有build targetのleaseを維持する。force push・rebaseは行わない。

Windowsでworktreeを削除する前に、対象の絶対path・所有者・未commit変更と、内部のJunction・symbolic link等のreparse pointを確認する。共有npm/cacheへ接続したリンクは、参照先を検証してリンク自体だけを非再帰で解除してから`git worktree remove`を実行する。共有backing directoryと他のworktreeは保持する。cleanなGit状態はignored directoryやリンク先の安全性を保証しない。

## 公開と統合の分離

mainへのpushは検査を実行し、GitHub Pagesを自動公開しない。
公開は明示的な公開指示を受けた後、`Check`の`workflow_dispatch`をmainに対して実行し、`deploy_pages`を有効にする。
Ubuntu・Windowsの検査成功後に、同じrunで生成したartifactだけを公開する。
Rust cacheの保存はmainへのpushと非公開の手動検査（`deploy_pages=false`）が担当し、Pages公開runは復元だけを行う。
保存元のrunが失敗した場合は、同じmainの非公開手動検査でcacheを更新できる。

Windowsは公式[sccache-action](https://github.com/mozilla-actions/sccache-action)と固定sccache v0.18.0で、
既存target cacheを補完する。compiler cacheも同じ条件でのみ書き込み、PR・Pages公開は`READ_ONLY`とする。
既存rust-cacheのroot profile識別を保持し、compiler cacheは実際のcompiler・target・引数・入力が適合する成果だけを再利用する。
既存targetの復元後にWindowsのwrapperを有効化し、rust-cacheが両OSへ適用するincremental無効化を維持する。
Clippyではwrapperを明示解除する。最終link・試験実行・Web検査は引き続き実行する。
既存targetを保持する読取り専用の導入PRとcompiler側の初回cold、main保存後の自然な後続runを区分する。
公式Actionのpost統計と既存step時間・repo cache容量で効果を確認し、初回cold runから短縮率を断定しない。
Windows先行の範囲と、両OS・設定変更後の再利用・容量の残条件は[#511](https://github.com/tokutori/pr-simulator-game-1/issues/511)に保持する。

## Fast-forward優先

GitHubの通常mergeボタンは `--no-ff` 相当である。Rebase and mergeはSHAを変更するため、
元のcommitを保持するfast-forwardとは区別する。
根拠: [GitHub公式: Pull request merges](https://docs.github.com/en/pull-requests/reference/pull-request-merges)。

fast-forward可能で、レビュー・CI・リポジトリ規則を満たす場合は、ローカルで
`git merge --ff-only <reviewed-head-sha>` を実行し、通常のpushでmainを更新する。
実行直前にfetchし、レビュー対象SHAとremote headの一致、作業ツリーのclean状態を確認する。
push競合が発生した場合は再取得して再判定する。force pushは禁止する。
GitHubでPRがmergedとなり、remote mainが意図したcommitを指すことを確認する。
間接mergeではPRの保護条件が自動強制されない場合があるため、必須検査を省略しない。

履歴が分岐している場合は通常のmerge commitを使用する。
保護規則で直接pushが禁止される場合も、規則を迂回せず通常のPR mergeを使用する。
変更済みの統合結果を再検証する。fast-forwardのためだけの履歴改変は要求しない。
この手順はmerge方式の契約であり、個別PRのレビュー完了を意味しない。

## 数式と本文

GitHubの[公式数式記法](https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/writing-mathematical-expressions)
に従い、独立式は言語名 `math` のfenced code block、文中式は `$...$` を使用する。
`math` block内に追加のドル区切りを入れない。
ChatGPT固有の参照マーカーと数式区切りは転記しない。
`gh issue create` / `gh pr create` は `--body-file` でUTF-8本文を渡し、作成後に表示を確認する。

## 検査

READMEのコマンドと `docs/verification.md` を参照する。
ローカルは変更箇所のlint、freshなbindingを用いた型検査、関連試験を先行する。Rust UI/ABI変更時は必要な生成を一回行う。
Cargoのtarget/build directoryにはソースの祖先または同一directoryを指定しない。workspace配下の通常targetまたはソースと兄弟のcacheを使用する。WASM生成入口はCargo metadataの実解決先を検査し、危険配置を生成前に拒否する。
UI-only変更では無関係な数値fixtureの反復を省き、最終CIの全検査を保持する。
Windows CIはworkspace試験からnative packageを除外し、その後native単独の試験とbuildを実行する。
これによりWebやcoreのdev依存によるfeature統合をnative試験から分離する。全packageのtests/doctestsを保持し、
Linux CIは従来のworkspace一括試験を継続する。
compiler・linter警告と、CI基盤の非推奨警告を確認し、解消してからmergeする。
外部サービス起因で解消不能な警告は原因・影響・追跡IssueをPRに記録する。
実機妥当性とソフトウェア検査の合格を区別する。

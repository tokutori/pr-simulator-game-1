# 開発手順

## PRの単位

[GitHub Issues](https://github.com/tokutori/pr-simulator-game-1/issues)を作業計画・依存関係・進捗の正本とする。
各IssueにはPurpose、Scope、Non-goals、Contracts affected、Implementation、Verification、
Completion criteria、Dependenciesを記載する。依存先はIssue番号でリンクする。
既存BPG IDは維持し、新規IDはGitHub上で重複を確認する。
実行順と段階ごとの完了条件は[GitHub Milestones](https://github.com/tokutori/pr-simulator-game-1/milestones)で管理する。
計画変更はIssue本文・milestoneへ直接反映し、関連する設計契約を同一作業で更新する。
PRには対象Issueをリンクし、実施結果と未達条件を記録する。
CIはコード・設計文書・assetを検査する。Issue本文や依存関係の検査・同期は行わず、着手時とレビュー時に確認する。

1. 依存BPGのmerge完了と基底commitを確認する。
2. `feat/bpg-###-description` 等のブランチを作成する。
3. 対象の契約、実装、検証、文書を同一PRに含める。
4. CIと差分を確認し、検査結果と未検証範囲をPR本文に記載する。
5. レビュー完了後、対象head SHAを確認してmergeする。

commitは目的別のConventional Commitsとする。検査未実施の機能を完了として扱わない。
通常の開発ではmainへの変更をPR経由で管理する。

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
compiler・linter警告と、CI基盤の非推奨警告を確認し、解消してからmergeする。
外部サービス起因で解消不能な警告は原因・影響・追跡IssueをPRに記録する。
実機妥当性とソフトウェア検査の合格を区別する。

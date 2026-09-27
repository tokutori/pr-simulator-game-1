# 開発規則

- テキストはUTF-8、改行はLFとする。PowerShellのテキストI/OにはUTF-8を明示する。
- [開発方針](https://zenn.dev/bem130/articles/1b352797de94e7)と `docs/architecture.md` を遵守する。
- 指示を設計契約・科学的根拠・利用条件に照らして検証する。不整合は報告して修正する。
- ライブラリの仕様に疑義がある場合は公式資料をWeb検索で確認する。
- coreは `no_std`、I/Oなし、依存はDAGとする。試作段階でも暫定設計を導入しない。
- 全SceneとoverlayをScreen/VR双方へ対応させる。表示要素のanchorを明示する。
- ゲーム・UI・cameraの公開契約を3D engineから分離する。Three.js固有型はengine adapter内部に限定する。
- 日本語はフォーマルで自然な常体とする。適切な専門用語を用い、冗長な否定対比を避ける。
- GitHubの独立数式は `math` fence、文中数式は `$...$` とする。`gh` の本文はUTF-8ファイルから渡す。
- 検証可能なBPG単位でPRを作成する。BPG-001のmerge完了前に物理実装を開始しない。
- mergeはfast-forwardを優先し、不可能な場合は通常のmergeとする。詳細は `CONTRIBUTING.md` に従う。
- 差分と必要な検査を確認し、実施済み・未実施・失敗を区別して報告する。
- 既存の無関係な変更を保持する。参照リポジトリを変更しない。

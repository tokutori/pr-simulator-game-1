# 実装計画

作業計画・依存関係・進捗は[GitHub Issues](https://github.com/tokutori/pr-simulator-game-1/issues)、実行順と段階ごとの完了条件は[GitHub Milestones](https://github.com/tokutori/pr-simulator-game-1/milestones)で管理する。

BPG IDは既存Issueの識別子として維持する。IDの数値順は実装順を表さない。着手時には対象IssueのDependenciesとmilestoneを確認する。

全Scene共通のScreen/VR対応とゲーム骨格を先行し、実環境データと景観品質は後続段階で扱う。設計契約は本リポジトリの各設計文書、作業手順は[CONTRIBUTING.md](../CONTRIBUTING.md)に従う。

## Critical path

M2は解析解・不変量・収束試験と、全control modeの再現可能なsynthetic CLI flightで完了する。BPG-006はソフトウェア統合fixtureを受け入れ、公開機体の性能検証を主張しない。

M3では同じsynthetic `FlightScenario`を用いてBPG-007のScreen browser vertical sliceへ進み、BPG-017/018でGameSession・gameplay・HUDを接続する。M3以降はFlightRecord・解析・Replay、実環境データ、景観品質・性能調整を順に実装する。機体固有係数・controllerのsource調査とfidelity検証は[BPG-035](https://github.com/tokutori/pr-simulator-game-1/issues/64)へ分離し、M6完了後に着手する。BPG-035はM3〜M6の依存先ではなく、これらの完了を遅延させない。

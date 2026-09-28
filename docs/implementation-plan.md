# 実装計画

作業計画・依存関係・進捗は[GitHub Issues](https://github.com/tokutori/pr-simulator-game-1/issues)、実行順と段階ごとの完了条件は[GitHub Milestones](https://github.com/tokutori/pr-simulator-game-1/milestones)で管理する。

BPG IDは既存Issueの識別子として維持する。IDの数値順は実装順を表さない。着手時には対象IssueのDependenciesとmilestoneを確認する。

全Scene共通のScreen/VR対応とゲーム骨格を先行し、実環境データと景観品質は後続段階で扱う。設計契約は本リポジトリの各設計文書、作業手順は[CONTRIBUTING.md](../CONTRIBUTING.md)に従う。

## Critical path

M2は解析解・不変量・収束試験と、全control modeの再現可能なsynthetic CLI flightで完了する。BPG-006はソフトウェア統合fixtureを受け入れ、公開機体の性能検証を主張しない。

M3ではBPG-006の決定性検証用fixtureと分離したsynthetic playable `FlightScenario`を用いてBPG-007のScreen browser vertical sliceを成立させ、BPG-017/018でGameSession・gameplay・HUDを接続する。M3以降はFlightRecord・解析・Replay、実環境データ、景観品質・性能調整を順に実装する。景観工程では、機体の速度・姿勢に対する視覚的手掛かりとなる湖面表現を優先し、BPG-010の水面描画をBPG-009の地形描画とBPG-011の空・雲描画より先行させる。水面は簡易world上で独立して実装し、地形・空の完成を依存条件としない。機体固有係数・controllerのsource調査とfidelity検証は[BPG-035](https://github.com/tokutori/pr-simulator-game-1/issues/64)へ分離し、M6完了後に着手する。BPG-035はM3〜M6の依存先ではなく、これらの完了を遅延させない。

景観の実装順は、ゲームループ成立後にBPG-010 湖面、BPG-009 地形・会場、BPG-011 空・雲、BPG-012 適応画質とする。水面の初期反射は簡易sky/environmentを用いて成立させ、後続の空・地形追加時に反射対象を拡張する。水面描画の着手・受入を地形・空の完成待ちにしない。

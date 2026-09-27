# 検証契約

## BPG-001の必須検査

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --locked`
- `RUSTDOCFLAGS=-D warnings` を設定した `cargo doc --workspace --no-deps --locked`
- coreの `wasm32v1-none` build（stdを提供しないtarget）
- WASM adapterの `wasm32-unknown-unknown` build
- CLIのnativeと `wasm32-wasip2` build
- Web/Node別のTypeScript typecheck、strict lint、tool unit tests、Web production build
- dependency boundary、asset manifest、出典・license・hash、Viteのasset取込経路、Markdown数式区切りの検査

CIはLinuxとWindowsで実施する。WASI artifactはbuildまでとし、実行を報告に混同しない。
browser上の物理実行はBPG-007以降、Pages配信はBPG-013で検証する。
BPG-001のWebは起動ページのみであり、フライトやThree.jsの動作確認を意味しない。

## 後続の物理検証

| BPG | 検証 |
|---|---|
| 002 | frame往復、quaternion不変量、機体・パイロットの運動量収支、一般3次元の内部移動、重力、刻み半減収束、参照case比較 |
| 003 | 対称空力、揚抗力の解析値、alpha/beta符号、moment arm、低速・適用範囲・非有限値 |
| 004 | 無風、一様風Galilean invariance、同一ground launchのheadwind、crosswind、鉛直風、shear、grid境界 |
| 005 | 左右対称、片翼上昇流、水平・垂直尾翼の局所風、回転局所速度、二重計上回避 |
| 006 | lifecycle、身体位置指令・移動限界、接触補間と同時刻終端state、authority両端、actuator飽和、決定的scenario、入力replay |
| 007 | native/WASM数値許容差、FPS独立性、pause/resume、keyboard/gamepadのbinding・切断・tick入力からsnapshotまでのbrowser試験 |
| 014 | engine import/型境界、全8 Scene/overlay、anchor別追従、共通操作、backend切替、単一loop、recenter、resource解放 |
| 015 | 全SceneのWebXR UI、session拒否・終了、reference space、実HMDのpose/projection |
| 016 | 全SceneのPhone VR UI、sensor権限・null・timeout・stale、左右aspect、実スマートフォン |
| 017 | 全backendの基本ループ、preset決定性、Custom、三軸の境界、軌道不変性、Pause/Retry |
| 018 | HUD項目とcamera許可、未定義telemetry、全backendの表示と操作 |
| 019 | 全tick記録、身体状態・目標列、接触時の位置/姿勢/actuator/身体状態一致、容量、集計、schema、条件別PB |
| 020 | map軸・風断面、速度/高度、共有cursor、欠損、Screen/VR解析操作 |
| 021 | snapshot補間、seek、再生時計、全backendのResult復帰、原record不変性 |
| 022 | camera director、Attract、短いrecord、FPS差、demoとplayer記録の分離 |

非有限値は入口と積分途中で拒否する。比較は解析解、不変量、独立した基準caseを用いる。
同じ実装から期待値を生成するだけの試験を検証根拠としない。
一様風不変性の試験では、位置依存の地面効果や接触を除外して比較する。
評価対象、初期条件、入力列、許容差、duration、seed、実行環境、commitを記録する。
100 Hzの適合性は空力・actuator・FBW・身体移動を含む統合系で、軌道・姿勢・接触時刻・scoreを
刻み半減と比較する。公開機体とscenarioでは通常の入力範囲でのフライト成立率、失敗理由の表示、
採用値の出典・仮定・適用範囲、操作性を受入条件に含める。実機同定の達成とは区別する。

## 表示・性能・配布

BPG-009以降では稜線誤差、500 m移動時のparallax、reflection、雲、hazeを目視・数値で評価する。
BPG-012ではframe-time分布、画質振動、physics allocationと実行時間、download量を計測する。
BPG-013ではsubpath、WASM MIME、cache、asset帰属、keyboard/gamepad、browser smoke testを検証する。
未確認の端末・browserは明記する。実機同定・物理HIL検証をこれらの合格に含めない。
VR基盤の開始前に試験用HTTPS配信、HMDとbrowser、スマートフォンとviewerを確保し、
全Scene fixtureと実フライト接続後の再試験を分けて記録する。

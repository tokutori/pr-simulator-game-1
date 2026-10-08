# 形式検証契約

## 導入範囲と検証時点

[#268](https://github.com/tokutori/pr-simulator-game-1/issues/268)の第一単位として、PauseReasonsの4原因集合とManual除去後のResume判定を検証する。
対象は実際のcoreが使用する整数・集合操作であり、全GameSessionや飛行modelの正しさへ一般化しない。
Bevy統合後に独立PRとして導入し、製品toolchain、公開API、物理係数、保存schemaを変更しない。

Windows 11でtracked checkerを実行し、module-level constructorを含む正driverは`37 verified, 0 errors`、
負driverは指定した`assert(still_present)`で`14 verified, 1 errors`となった。整形後にも同じcheckerを再実行して成功した。
core 301、format 78、session 14、WASM 65 unit testsと関連integration/doctest、Clippy、
no_std/WASM build、rustdocが成功した。CIとWeb全検査の結果はPRで別途記録する。

## 製品と証明が共有するsource

- `crates/birdman-game-core/src/pause_reasons.rs`がPauseReason、PauseReasonsと実行bodyを所有する。
- `game_session.rs`は同moduleの公開型を再exportし、pause、can_resume、resumeから実関数を呼ぶ。
- `proofs/pause_reasons.rs`と`proofs/pause_reasons_negative.rs`は同じtracked moduleをそれぞれ一度includeする。
- `tools/verus/check-pause-reasons.ps1`はinclude先と検査前後のsource/toolchain hashを照合する。

公開の4原因、privateなu8表現、contains/is_emptyを維持する。空集合はprivateなmodule-level
`empty_pause_reasons`の同じbodyから生成する。Resumeの許可は元bitsから導き、独立したboolを保持しない。
失敗したResumeではManualだけを除去して外部原因を保存し、clearだけでは飛行を再開しない既存契約を維持する。

検証属性は`cfg(verus_only)`で有効化する。通常Cargoではvstd import、ghost view、検証属性が除去され、
Rust `1.97.0`、coreのno_std・I/Oなし・通常依存DAGを維持する。vstdを通常依存へ追加しない。
private fieldはghost-onlyのclosed viewを通して仕様から参照し、maskは実bit関数から生成するdual specを使う。
空集合には公式の`verus_spec`属性を用い、内部impl marker、shadow実装、trusted wrapperを設けない。
[公式exec属性](https://verus-lang.github.io/verus/guide/exec_attr.html)、[dual spec](https://verus-lang.github.io/verus/guide/exec_to_spec.html)に従う。

既存WASM build fingerprintはcoreのRust sourceを再帰収集するため、抽出moduleも対象になる。
source配置の変更によるfingerprint差を固定・無視せず、保存済みrecordには元metadataを保持する。

## 証明する性質と通常回帰

正driverは4bit内の任意の原因集合について、挿入・除去の冪等性、選択原因の所属、異なる原因の保存、
同一原因の操作列の吸収則、4bit閉包を検証する。Manual除去後はDocumentHidden、TrackingSuspended、ProcessingDelayを保存し、
いずれかが残ればResume判定はfalseとなる。空集合の生成と判定も同じ実bodyを検証する。
bitvector queryへ渡す前提は通常の検証で実関数の契約から確認する。
負driverは原因を除去した後も所属すると意図的に主張し、その指定assertが拒否されることを検査する。

通常unit testは16集合×4原因を列挙する。実GameSessionを使う別回帰は重複pause/clear、失敗Resume後の原因保存、
外部原因が残る間の拒否、明示Resume、pause中のstate/record不変を確認する。
非Pause phaseのResume拒否とResultの確定record保持も対象とする。これらはcallerの通常回帰であり、
GameSession全体の形式証明ではない。

## 固定ツールとCI

- Verus release: `0.2026.10.04.426d8b0`。
- 公式Windows ZIP: `verus-0.2026.10.04.426d8b0-x86-win.zip`。
- ZIP SHA256: `b36968a0f333036e7e1770bb73fdd407a35c8fd163124c1d309972574150da61`。
- 証明専用Rust: `1.98.1`。rustupで追加し、default/overrideと製品`rust-toolchain.toml`を変更しない。

[固定release](https://github.com/verus-lang/verus/releases/tag/release/0.2026.10.04.426d8b0)、
[導入手順](https://github.com/verus-lang/verus/blob/release/0.2026.10.04.426d8b0/INSTALL.md)、
[専用toolchain](https://github.com/verus-lang/verus/blob/release/0.2026.10.04.426d8b0/rust-toolchain.toml)を基準とする。
scriptは解凍・実行前にdigestを照合し、compiler/verifier versionを確認する。
全義務を`--no-cheating --num-threads 1`で検証し、対象を限定・免除するflagは使用しない。
正検証はexit 0、verified件数が正、0 errorsを必須とする。負検証は非zero exit、verified件数が正、1 error、
指定assertのファイル・行と`assertion failed`を必須とし、コンパイル・起動失敗を成功と数えない。

Check workflowの独立したWindows jobが次のscriptを実行する。
Pagesは従来のmain/manual/boolean条件に加え、VerifyとPauseReasons proofの双方の成功を必要とする。

```powershell
./tools/verus/check-pause-reasons.ps1
cargo +1.97.0 test -p birdman-game-core --locked pause_reasons
cargo +1.97.0 test -p birdman-game-core --locked every_pause_set
cargo +1.97.0 test -p birdman-game-core --locked resume_rejects_nonpaused
cargo +1.97.0 clippy -p birdman-game-core --all-targets --locked -- -D warnings
cargo +1.97.0 build -p birdman-game-core --target wasm32v1-none --locked
```

## 全体への適性監査と優先順

| 優先度 | 契約・対象body | 適性と現在の根拠 | 残る境界 |
|---|---|---|---|
| P0 | PauseReasonsの4bit集合・Resume predicate | 同一bodyのtracked checkerで正37義務と指定負検証を確認した | 外部原因の発生条件は未証明 |
| P0 | GameSessionのphase遷移・caller・確定record保持 | 離散状態遷移は次の形式化候補。今回の実GameSession通常回帰で接続を確認する | phase遷移全体、入力admission、終端原子性は未証明 |
| P1 | record/finalization・codec・identity・PB分離 | 純粋な整合性判定とschema別分岐は候補となる。保存値とlayoutを仕様として固定する必要がある | 現在の通常回帰は形式証明ではなく、既存archive全体も未証明 |
| P1 | tick原子性・bounded保護候補探索 | 副作用commit条件・探索回数は離散契約として扱える | 浮動小数点、全RK stageの荷重域、安全な候補の存在は未証明 |
| P1 | owner世代・stale query・資源解放 | immutableな観測tokenと有限lifecycleは候補となる | browser/Bevyの再入・非同期・OS資源は環境仮定が必要であり未証明 |
| 別検査 | 数値収束・局所安定性・空力適用域 | 解析解、量別誤差、刻み・domain回帰を個別に評価する | bit集合の証明から数値精度・全飛行安定性を導かない |
| 別検査 | model fidelity・GPU/実入力・VR・80FPS | 一次資料・実機・実表示・性能計測で評価する | 本形式証明の対象外。試験未実施や目標未達を合格と扱わない |

この表は適性と実施順の監査であり、未着手対象の導入・完了を宣言するものではない。
新しい証明ごとに対象の実body、事前条件、公開仕様、環境仮定を定める。

## 信頼境界

Verus frontend/attribute macro、専用rustc、bundled vstd、SMT solverと配布物の供給経路をTCBに含める。
ZIP digestは配布物の同一性を確認するものであり、その意味論やsolverを独立に証明するものではない。
製品bodyにassume/admit/external_body/assume_specificationによる免除を設けない。
`--no-cheating`、同一source確認、指定した負検証でもTCB自体は除去されない。
[公式TCB](https://verus-lang.github.io/verus/guide/tcb.html)に従い、仕様の十分性と製品callerの適用条件を別に監査する。

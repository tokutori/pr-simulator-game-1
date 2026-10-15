# 形式検証契約

## 導入範囲と検証時点

[#268](https://github.com/tokutori/pr-simulator-game-1/issues/268)の第一単位として、PauseReasonsの4原因集合とManual除去後のResume判定を検証する。
対象は実際のcoreが使用する整数・集合操作であり、全GameSessionや飛行modelの正しさへ一般化しない。
Bevy統合後に独立PRとして導入し、製品toolchain、公開API、物理係数、保存schemaを変更しない。

#268導入時にWindows 11でtracked checkerを実行し、module-level constructorを含む正driverは`37 verified, 0 errors`、
負driverは指定した`assert(still_present)`で`14 verified, 1 errors`となった。整形後にも同じcheckerを再実行して成功した。
core 301、format 78、session 14、WASM 65 unit testsと関連integration/doctest、Clippy、
no_std/WASM build、rustdocが成功した。CIとWeb全検査の結果はPRで別途記録する。

#382の初回cutではWindowsの同checkerで全体`41 verified, 0 errors`、必須4群をそれぞれ`12 / 8 / 3 / 3 verified, 0 errors`で確認した。
renameと旧宣言コメントを含むfixtureのexact inventory拒否、必須名欠落の診断、指定assert負例2件の各`15 verified, 1 error`も確認した。
通常RustのPauseReasons 2件、GameSession 30件、共有application 6件の選択回帰、core all-targets Clippy、
`wasm32v1-none`のno_std build、formatとdiff検査が成功した。GPUやGameSession全体の証明へ一般化しない。

## 製品と証明が共有するsource

- `crates/birdman-game-core/src/pause_reasons.rs`がPauseReason、PauseReasonsと実行bodyを所有する。
- `game_session.rs`は同moduleの公開型を再exportし、pause、can_resume、resumeから実関数を呼ぶ。
- 正driverと二つの負driverは同じtracked moduleをそれぞれ一度includeする。
- `tools/verus/check-pause-reasons.ps1`はinclude先と検査前後のsource/toolchain hashを照合する。

公開の4原因、privateなu8表現、contains/is_emptyを維持する。空集合はprivateなmodule-level
`empty_pause_reasons`の同じbodyから生成する。Resumeの許可は元bitsから導き、独立したboolを保持しない。
失敗したResumeではManualだけを除去して外部原因を保存し、clearだけでは飛行を再開しない既存契約を維持する。

[#382](https://github.com/tokutori/pr-simulator-game-1/issues/382)の初回cutでは、実行関数`PauseReasons::request_resume`が
`Result<(), PauseReasons>`で認可結果を返す。`GameSession::resume`はOkをRunning、Errの残存理由をPausedへ投影し、
`can_resume_after_manual`も同じ認可結果から導出する。OkはManual以外の理由が空であることと同値であり、
ErrはManualだけを除去した非空の理由を保持する。拒否後の再要求でも残存理由と拒否を維持する。
この認可結果の証明をGameSession全体の証明へ一般化しない。以降のcutでCountdownと実行状態の本番遷移へ接続する。

### Countdownの本番遷移

Countdownのcutは`crates/birdman-game-core/src/countdown.rs`の本番`CountdownState`を直接includeする。
正の秒数と非wrap token、obsolete tokenの無視、時刻検査と可用性の優先順、取消、待機、時間到達による発進認可を対象とする。
待機中の公開残秒は正であり、観測後の経過時間が所定時間へ到達した場合に発進を認可する。
公開Countdownが残秒0で存在することを前提としない。

待機状態だけを`u64` nanosecondsで保持する。所定時間は最大`u32::MAX`秒なので待機値はその範囲に収まる。
入力deltaの全受理域、f64からnanosecondsへの丸め、加算overflowは既存の標準`Duration`境界に維持し、
比較する加算後の候補は`u128`で渡す。入力の`u64`へのcast、deadlineでのclamp、Duration上限を超える通常整数加算は行わない。
同じ時刻状態をDurationと整数の双方に保持しない。

GameSessionの`observe_countdown`は本番の認可結果を使用し、telemetryと`record.begin_tail`が成功した後だけRunningへcommitする。
認可計算は候補値を返すため、record開始失敗時は直前のCountdownのtoken・elapsed・phaseを保持する。
標準Durationとf64変換、wrapperのphase代入・物理payload・tick・record保持、古いcallbackの発進後拒否は形式証明範囲に含めず、
実GameSessionの接続回帰を維持する。Retry、Replay、異常tickからの終端化も未証明である。

既存runnerでPauseReasonsに加えてCountdownの全体positive、必須4群の名前別検証、exact compiler inventory、
renameと旧宣言コメントを含むfixture、missing-target診断、指定assert負例を実行する。
Countdownの負例は期限前の発進とobsolete tokenによる進行を意図的に主張し、指定assertで拒否されることを要求する。
検証結果は対象headのPRへ記録し、sourceを追加した段階で合格を主張しない。

### FlightExecutionの本番遷移

`crates/birdman-game-core/src/flight_execution.rs`はRunning/Pausedと停止理由を単独所有する。
GameSessionのFlight variantは同moduleの`ActiveFlight<TailFlightTickState>`を保持する。
このprivateな実行型は実payloadとexecutionの組だけを所有し、pause、clear、resumeの本番methodを接続する。
各methodは任意の不透明な`State`を更新せず、executionだけへ既存操作を委譲する。
この実bodyについてpayload保持と成功・NotPaused・PauseConditionsRemainの分類を同時に検証する。
飛行payload内の物理stateとtickは非更新の対象だが、その数値の妥当性や浮動小数点演算の精度は保証しない。

本番の可変参照更新をそのままincludeし、pauseによるPaused遷移と理由追加、clear後のPaused維持、
Manual解除後の再開認可、外的理由が残る場合のPaused維持、Runningでのclear/resume拒否と実行状態保持を検証する。
実行遷移の必須4群とpayloadの必須3群にexact inventory・名前別検証・rename fixture・missing-target検査を適用する。
負例は外的理由を残したResumeの成功と、最後の理由をclearした時点の自動再開を意図的に主張する。
payload負例は実pauseの後にpayloadが変化したと意図的に主張し、指定assertの拒否を要求する。
GameSession wrapperの全分岐、configuration・record保持、record初期化・Retry・Replay・異常tick終端化は引き続き未証明であり、#382の残範囲とする。

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

FlightExecutionのcutでは固定Verusをreadonlyで再利用し、既存runnerの検証部分で正driverの`24 verified, 0 errors`、
必須4群の各`1 verified, 0 errors`、exact inventoryとrename/missing検査、指定負例2件の各`20 verified, 1 errors`を確認した。
既存PauseReasonsとCountdownのsuiteも同時に成功した。ZIP取得・digest照合・展開を含むbootstrap全体はCIで検査する。
GameSessionの通常回帰32件、core all-targets Clippy、`wasm32v1-none`のno_std build、formatとdiff検査が成功した。

## 固定ツールとCI

- Verus release: `0.2026.10.04.426d8b0`。
- 公式Windows ZIP: `verus-0.2026.10.04.426d8b0-x86-win.zip`。
- ZIP SHA256: `b36968a0f333036e7e1770bb73fdd407a35c8fd163124c1d309972574150da61`。
- 証明専用Rust: `1.98.1`。rustupで追加し、default/overrideと製品`rust-toolchain.toml`を変更しない。

[固定release](https://github.com/verus-lang/verus/releases/tag/release/0.2026.10.04.426d8b0)、
[導入手順](https://github.com/verus-lang/verus/blob/release/0.2026.10.04.426d8b0/INSTALL.md)、
[専用toolchain](https://github.com/verus-lang/verus/blob/release/0.2026.10.04.426d8b0/rust-toolchain.toml)を基準とする。
scriptは解凍・実行前にdigestを照合し、compiler/verifier versionを確認する。導入済みの専用Rustは再利用し、未導入の場合だけ追加する。
全義務を`--no-cheating --num-threads 1`で検証する。全体のpositive検証を維持したうえで、
`verify_set_operations`、`verify_resume_predicate`、`verify_resume_admission`、`verify_empty_set`を名前別にも実検証する。
必須名は`--verify-only-module pause_reasons --verify-function <name>`で選択し、各対象のexit 0、正のverified件数、0 errorsを要求する。
全体positiveの公式JSON `func-details`からcompilerが解決した完全修飾名をexactに確認し、selectorのsubstring fallbackを通じた別名関数の受理を防ぐ。
JSON stdoutとstderr diagnosticsを分離し、PowerShell標準`ConvertFrom-Json`を使用する。sourceのregex inventory、追加scope anchor、独自parserは導入しない。
必須関数を別名へ変更して旧宣言をblock comment中へ残した一時fixtureは、全体proofが成功してもexact inventoryで拒否する。
総件数や名前の文字列存在だけを検証範囲の保証に使用しない。
同じproduction moduleをincludeし、必須driverを持たない既存の負driverへ名前選択を適用し、指定functionの欠落診断で拒否することも確認する。
構文・起動失敗や別のassertion失敗は、この欠落検出の成功に含めない。
正検証はexit 0、verified件数が正、0 errorsを必須とする。負検証は非zero exit、verified件数が正、1 error、
指定assertのファイル・行と`assertion failed`を必須とし、コンパイル・起動失敗を成功と数えない。
`proofs/resume_admission_negative.rs`は実関数へDocumentHiddenを渡しても再開できると意図的に主張し、
指定した`assert(admitted)`だけが拒否されることを同じrunnerで確認する。

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
| P0 | GameSessionのphase遷移・caller・確定record保持 | Countdown認可、FlightExecutionの本番更新とActiveFlightのpause/clear/resume時payload保持を直接検証し、実GameSession通常回帰で接続を確認する | wrapper全体、configuration・record保持、Retry/Replay、終端原子性は未証明 |
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

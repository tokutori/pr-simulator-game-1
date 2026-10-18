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
標準Durationとf64変換は未証明であり、実GameSessionの接続回帰を維持する。
phase代入・payload・tick・record保持、発進後の再観測拒否とRetryのcaller接続は後述の実所有境界で検証する。
数値producerとReplay clockを含むwrapper全体の正しさとは区分する。

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
GameSessionのconfiguration・record保持、Retryと異常tick終端化の離散的接続は後述の所有境界で検証する。
数値callback、allocatorによるrecord新規化、全wrapper分岐を一括した保証は#382の残範囲とする。

### Result / Replay の本番遷移

`configured_phase.rs` は従来の全 ConfiguredPhase variant と、Result / Replay の実更新を所有する。
GameSession は Flight state・SessionResult・ReplayClock を具体化した private alias を使用し、
current-session の `enter_replay` / `leave_replay` は同 module の二つの method を呼ぶ。
不透明な payload の型 parameter はこの内部 enum に限定し、別の状態機械や transaction framework を設けない。

本番 enum を正負 driver が直接 include し、Result だけが Replay に入ること、Replay だけが Result に戻ること、
成功時の結果 payload 完全保持と指定 clock の格納、拒否時の enum 全体保持を検証する。
必須三群、exact compiler inventory、rename / missing-target 拒否、発進前の Replay 認可と結果改変を誤って主張する負例を既存 runner に追加する。
外側の record / configuration はこの method へ借用せず、Rust の所有分離と GameSession の通常回帰で保持を確認する。
これらの外側 payload、GameSession wrapper 全体、archived Replay / Attract、clock の浮動小数点演算は Verus の直接証明範囲に含めない。

### Retry の本番所有体

`configured_session.rs` は製品の実 `ConfiguredSession` を単独所有し、configuration・record・既存 ConfiguredPhase を保持する。
GameSession は従来の具体型を private alias に割り当て、同じ owner method で Result 認可、factory 呼出し、record / Ready の commit を実行する。
configuration と record に Copy / Clone を要求せず、既存 phase の三つの payload と合わせて不透明な値として扱う。

成功時は configuration 全体を保持し、factory が返した record を格納して Ready に移る。
Header 生成または予約を含む factory の失敗時は owner 全体を保持し、原因を分類して返す。
Result 以外では factory の前提条件を要求せず拒否する。正 driver の拒否群は前提条件 false の factory を渡し、認可前の呼出しを検証で拒否する。
sealed configuration の非更新は scenario・seed・初期状態・tick 上限の保持を含むが、その浮動小数点の妥当性や物理精度を証明しない。

Header 生成と実 `FlightRecord::try_new` は製品の同じ closure に接続する。
allocation と record の空性・metadata は factory の形式保証として追加仮定せず、実 factory の接続回帰で確認する未証明境界とする。
Title 等の owner 不在拒否と GameSession wrapper 全体、Countdown 初期化、異常終端化、archived Replay / Attract は本 cut の直接証明範囲に含めない。
既存 runner に成功・予約失敗・認可拒否の必須三群、exact inventory / rename / missing-target、認可と失敗保持の指定負例を追加する。

### FlightRecord の離散所有

`flight_record_storage.rs` は実 header・Vec sample・sample_capacity・finalization の4fieldと、元の FlightRecordError を所有する。
FlightRecord は従来の3実型を用いた具体 alias であり、storage module、field、低水準 push / finalize は外部へ公開しない。
公開 domain 操作は従来の時刻・finite・metadata 検査を維持し、compile-fail の使用例で低水準操作の不可視性を確認する。

実 Vec::push の容量判定、prefix と他 field の保持、成功時の1 sample追加、容量拒否時の全保持を直接検証する。
finalize は AlreadyFinalized、実 Vec.last による NotStarted、不変 sample の検証、metadata commit の順序を所有する。
成功は検証出力を一度だけ保持し、sample / header / capacity を変更しない。全 Err は owner 全体を保持する。
検証 callback は &Sample のみを受け取り、Err 原子性や有効 metadata を仮定する契約は追加しない。
意味検査は製品の同じ validate_finalization を呼び、その数値仕様自体は本 cut の直接証明範囲に含めない。

固定 Verus の既定は usize / isize の32 / 64 bitであり、本 driver と runner に幅固定を置かない。
len < capacity と usize::MAX から1要素追加の安全性と論理容量上限を検証する。
Vec の論理更新は bundled vstd の仕様に依存し、実 reserve / allocator・wasm codegen の証明と区別する。
既存 constructor の checked conversion / checked_add / try_reserve_exact と通常回帰を維持する。
低水準 push の証明は公開 append の全認可を含めず、確定後追加の拒否は既存 domain 回帰の対象である。
GameSession の previous / report 選択、元 failure と Result の対応、正常 / 異常終端化の縦断保証、Retry factory の空性・metadata は後続範囲である。

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
| P0 | GameSessionのphase遷移・caller・確定record保持 | Countdown・FlightExecution・Result / Replay、公開owner選択と停止操作、Prepared/Countdown/Retry callerの離散的保持を直接検証する | 数値callback、allocatorによる新規化、Duration・Replay clock、archived Replay / Attractの全経路、wrapper全体は未証明 |
| P1 | record/finalization・codec・identity・PB分離 | 純粋な整合性判定とschema別分岐は候補となる。保存値とlayoutを仕様として固定する必要がある | 現在の通常回帰は形式証明ではなく、既存archive全体も未証明 |
| P1 | tick原子性・bounded保護候補探索 | 副作用commit条件・探索回数は離散契約として扱える | 浮動小数点、全RK stageの荷重域、安全な候補の存在は未証明 |
| P1 | owner世代・stale query・資源解放 | immutableな観測tokenと有限lifecycleは候補となる | browser/Bevyの再入・非同期・OS資源は環境仮定が必要であり未証明 |
| 別検査 | 数値収束・局所安定性・空力適用域 | 解析解、量別誤差、刻み・domain回帰を個別に評価する | bit集合の証明から数値精度・全飛行安定性を導かない |
| 別検査 | model fidelity・GPU/実入力・VR・80FPS | 一次資料・実機・実表示・性能計測で評価する | 本形式証明の対象外。試験未実施や目標未達を合格と扱わない |

この表は適性と実施順の監査であり、未着手対象の導入・完了を宣言するものではない。
新しい証明ごとに対象の実body、事前条件、公開仕様、環境仮定を定める。

## 信頼境界

### 実core crateの限定直接検証

固定426d8b0の`cargo-verus`とproof用Rust 1.98.1から、実`birdman-game-core`を直接読み込む。
`verus-proof`は固定`vstd`の`alloc`だけを有効にする明示的optional featureであり、通常の依存は従来どおりである。
製品Rust 1.97と`no_std`を維持し、closure仕様に必要な`proc_macro_hygiene`は`cfg(verus_only)`に限定する。
公開`finalize`/`finalize_with_failure`、実`finalize_record`、実`validate_finalization`を検証する。
Err時の実owner保持、header・sample列・capacity保持、成功時のreason/tick/fraction/score/元failureの転送を扱う。
成功metadataの`disposition`対応、数値の合法性、allocatorは未証明である。

実`ConfiguredSession`のprivate終端methodは、実recordの確定後に同一state・時刻・reason・score・元failureで
`SessionResult`を構築する。record拒否はhelper入口のowner全体を保持し、configurationは全経路で保持する。
score単独失敗はFatal Result確定後にScore Errを返し、元failureがあるscore失敗は元reason/failureとscore Noneを保持する。
公開record APIの成功commitは既存getterから確認でき、ghost-only closed predicateは実header・sample列・capacityを参照する。
時刻getterのspecは固定Verusの`dual_spec`で実bodyから生成し、その同値も検証する。
Running認可と実tickの制御bodyは後述の契約へ接続する。外側GameSessionのowner選択と戻り値も直接検証するが、
数値callbackの具体bind/body、数値関数が生成する評価結果の妥当性、数値score評価は未証明の境界である。
これらを仮定して終端化全体を完了扱いにせず、親#382を継続する。

### GameSessionの所有境界

実`GameSession`と`SessionLifecycle`を検証し、同じconfiguration・record・physical stateと無関係なownerの保持を接続する。
到達するscenario、polar、geometry、wind等の既存データ型へ検証属性を付けるが、その数値関数の本体を免除しない。
公開pause/clear/resumeは実ActiveFlightへ委譲し、拒否分類と外的停止理由の保持を保証する。
privateな停止エラーの変換は単一のnamed methodへ集約し、dual_specで同じ本番matchを参照する。
変換用のtrait仕様の複写や外部仕様を追加せず、入力のprivate型を公開しない。
ResultとReplayの往復は同じconfiguration・record・resultを保持し、ArchivedReplayからの退出だけがTitleへ戻る。
公開snapshot/recordの投影は`dual_spec`により実bodyへ接続する。Countdownの秒数投影には、実start_countdownが生成する
有効なCountdown状態を要求し、Duration変換や全ての構築経路の検証済みを意味しない。

公開advanceは数値closureのbindだけを残し、同じGameSession上のprivate methodでowner認可、封印済みtick上限/identity、
既存Running呼出し、返すsnapshotを扱う。callbackのrequiresは従来の呼出し可能性だけであり、latestとpreviousの相関を追加しない。
Result.scoreは同じconfigurationとterminalを受け取ったscore callbackの返却値へ接続し、scoreの数式の正しさとは区分する。
公開数値closureのbody、control modeと物理関数の具体bind、allocator、Duration変換、Replay clockの浮動小数点演算は未証明である。
既存のowner回帰は同じGameSession境界を通し、callback順序・元エラー優先順・戻りsnapshotを確認する。

### 準備・Countdown・Retryのcaller接続

実GameSessionのSetup・Briefing・Countdownの認可と遷移を検証し、拒否時のowner全保持、
同じconfiguration・attract記録・attempt tokenの保持を接続する。実header constructorのvalidatorを
dual_specで参照し、封印済みidentityとtick上限から生成したheaderを同じrecord factoryへ渡す。
準備と二種類のRetryはfactory成功後だけrecordを交換し、失敗時は旧ownerを保持する。
factoryが返した同じrecordへの置換を保証し、allocatorが生成する空recordの全形式保証とは区分する。

公開observe_countdownは従来のDuration変換・初期状態生成・telemetryをbindし、同じowner上のprivate methodが
既存CountdownState::observeとrecord.begin_tailを呼ぶ。非Countdownはcallbackを呼ばずIgnoredとなる。
古いtokenの拒否優先順、有効な整数deadline、同じconfigurationからのcallback出力転送、begin成功後だけの
FlightRunning commitを保証する。Launch後の再観測はownerを保持してIgnoredとなり、再発進しない。
合成clientは実prepare・ready・start・observe・beginとRetryを呼び、空recordやtick0を事前条件に追加しない。
Titleからの発進拒否は公開APIを用いるpositiveと指定negativeでも確認する。

Duration/IEEE変換、初期状態とtelemetryの数値関数本体、allocatorとtry_reserve_exactは未証明である。
公開prepare/retryの具体的なFlightRecord::try_new bindとrecord新規化は通常Rust回帰で確認し、
private factoryの返却値転送の形式保証からallocatorの正しさを導かない。scenario・seed・初期条件の保持は
同configurationの構造的保持であり、物理妥当性や数値計算全体の証明ではない。

### Running tickの制御body

GameSessionのprivate owner境界は、既存ConfiguredSessionのprivate methodへ制御を委譲する。
実ConfiguredPhaseのrunning_stateだけがRunningのstateを返し、拒否時は数値callbackを呼ばずowner全体を保持する。
この認可getterは同じproduction moduleを用いる既存suiteの必須positiveと指定negativeへ接続する。
認可getter単体の保証と、実tick bodyのactual core直接検証を区分する。

評価・telemetry・scoreの三つのFnOnceは共有ConfigurationとCopy値だけを受け取り、record・phaseへの可変参照を受け取らない。
実bodyは同じprevious/inputを評価へ一回渡し、その返却reportのstateをtelemetryへ渡してから、同report/telemetryをappendする。
callbackのrequiresは呼出し可能性だけを要求し、state保持・有効なrecord・数値結果の相関を仮定しない。
標準FnOnceのrequires/ensuresと、実record view上のpredicateにより、同じ出力の転送、sample列のprefix、最大1sampleの追加を検証する。
[公式FnOnce仕様](https://verus-lang.github.io/verus/guide/reference-signature-fnonce.html)の呼出し契約を用いる。
ghost-onlyのproof_decl/proofは実値を参照し、新しい実行状態・数値model・runtime APIを追加しない。

tick失敗はpreviousと元TailFlightTickErrorをResultへ保持する。元tick failureがある場合のscore失敗も同じfailureを保持する。
telemetry失敗とappend失敗はpreviousを終端化し、finalize自体のRecord/Scoreエラーを先に返す既存順序を維持する。
ScoreエラーはFatal Result、score None、failure Noneを保持し、Record拒否は入口owner保持、previousのFatal Result、またはappend済みRunningを区分する。
正常継続、TimeLimit、WaterContactの実report・record・Resultのstate/stamp/metadataも同じbodyで接続する。
実finalizationの整数guardから、確定recordの最新sample tickとResult terminal tickの一致を本番postconditionへ伝播する。
NoElapsedIntervalの必須clientは同じ公開append/finalizeを呼び、sample列の非変更と元の最新tick保持を結合する。
これはfloat fractionのIEEE意味論、物理state等号、数値producerの本体を保証する契約とは区分する。
通常Rust回帰は各callbackの呼出し順序、実scenarioの一回評価、telemetryの入力state、エラー優先順、拒否とResult保持を通す。
finalize拒否の回帰には内部不整合を明示的に注入し、利用者操作から発生した障害とは扱わない。

外側の実callerは既存scenario/telemetry/score関数をbindし、同じconfigurationのtick上限・identityを渡す。
封印済み上限/identityとownerへの転送は所有境界の契約へ接続する。
数値callbackの具体bind/body、任意reportと前sampleの等値、wrapper全体は形式証明の対象外である。
実producerは数値評価後のreport包装を必ず同じ`evaluated_tick_report`へ渡す。この本番bodyでAdvancedのnext/control転送、
接触途中のsample/control転送、実fraction-zero分岐で同じpreviousのstate/incidence/target保持と新control除去を検証する。
分岐に使うIEEE比較の数学的意味は標準仕様の境界に残し、実数上のゼロ判定の証明へ拡張しない。

RunningとGameSessionのpostconditionは、既存recordがpreviousと対応し、評価callbackのreportがunelapsed payloadを保持する場合に、
Resultの最新sampleとterminalの整数tick・FlightState・incidenceが一致することを保証する。これは追加requiresではない。
合成clientは任意長recordへの実Advanced appendからpreviousとの対応を確立し、同じ本番包装を用いたcallbackを実ownerへ渡す。
相関を仮定せず、正常継続・接触・score失敗を含むResultのpayload保持へ接続する。NoElapsedでは保存済みinput/telemetryを変更しない。
任意の別previousに由来するreportや不整合な内部ownerへ、この条件付きpostを無条件適用しない。
fractionのIEEE等値とspec値等号の区別、数値evaluation、allocatorの未証明境界を維持する。
0-fractionの本番private producerは同じpreviousのFlightState・incidence・pilot targetを構造的に保持し、
newly applied controlsを持たない。その実bodyと、同じpreviousによる公開begin/NoElapsed append/finalizeを呼ぶ
private clientをactual coreで検証する。初期sampleと終端reportのFlightState・incidenceが一致し、
sampleのinput/telemetryを保持することを結合する。初期sampleには独立したpilot target fieldがないため、
targetはreportのprevious保持に限定する。
実Vec上のlatest_matches_tail_stateはtick・整数fraction・FlightState・held incidenceを結合し、begin成功とAdvanced append成功が確立する。
同じ本番Running bodyは、正常に受理された継続Runningのstateと最新sampleの対応を保証する。
別のprivate clientは任意長recordへ実Advanced appendを行い、その同stateから既存ContactAtStart producerを呼び、
NoElapsed append/finalizeまで物理payloadと保存済みinput/telemetryを保持する。相関をrequiresへ追加しない。
数値接触検出と外側callback wiringは未証明であり、任意の別previousに由来するNoElapsed reportを同じstateとして扱わない。
初期controlsとNoElapsed分類は既存の純粋関数の`dual_spec`を`open`とし、同じ実bodyから生成した定義を展開する。
[公式dual_spec仕様](https://verus-lang.github.io/verus/guide/exec_to_spec.html)に従い、数値仮定や模倣modelは追加しない。

公開`begin_tail`/`append_tail_report`は同じ実bodyの認可、実reportからstate/control/telemetryへの転送、
bounded Vecへの追加、拒否時のowner全保持を検証する。NoElapsedIntervalは結果の成否にかかわらずrecord全体を保持する。
ghost-onlyのsample sequenceとlogical limitは実Vec/fieldのviewであり、別の記録モデルやruntime APIを追加しない。
整数tickの隣接・上限は直接の契約である。fractionの実`!= 0.0`比較は、固定vstdの`ne_ensures(..., false)`へ接続する。
この標準関係は未解釈であり、IEEE比較の数学的意味やspec上の値等号を導かない。
[固定vstdのne_ensures宣言](https://github.com/verus-lang/verus/blob/426d8b0/source/vstd/std_specs/cmp.rs#L243-L245)と
[f64のne仕様](https://github.com/verus-lang/verus/blob/426d8b0/source/vstd/std_specs/cmp.rs#L295-L299)に従う。

同じtelemetry validatorではfinite判定を有限閉区間へ等価展開する。固定Verifierはstdの`is_finite`とMIN/MAX定数に
未対応のため、privateな有限端点2定数を用い、製品Rustでstd MIN/MAXとのbit一致、NaN/±∞/±0/最大値近傍/subnormalを検査する。
validatorの実bodyを検証し、エラー分類・record保持を接続する。数値比較の意味やtelemetry数式の証明は主張しない。
接触fraction 0/1は対応する実stateを直接保持する。補間/detectの非自明state・signed zero回帰と、
実tail producer→record→GameSession Resultの接続回帰を維持する。NoElapsed reportを任意の別previousから生成して
公開appendへ渡す場合の相関は保証しない。最新sampleとreportの等値を未証明のrequiresへ追加しない。

append成功後のfinalize拒否には一般的な全Err保持を要求しない。WaterContactでは旧Runningと追加sampleが残り得る。
公開経路は同sampleのtick/fractionを確定へ渡すため、有効producerのfractionが[0,1]なら同guardの拒否例は確認していない。
既存の拒否回帰は内部不整合を注入する。IEEE数値producerとwrapperの到達不能証明を、この正常継続の契約から導かない。

独立proof clientは同じ検証で生成したcore/vstdの`rmeta`/`vir`とbuiltinを、固定配布物の
`rust_verify`へ明示して読み込む。実行bodyや実型の複写、外部仕様による実装免除、新たな数値公理は追加しない。
Vector3/Point3のgeneric derived Cloneは固定Verusが仕様を生成できず、warningが2件発生する。
今回の保持契約は実ownerと値の同一性に接続し、Clone equalityを仮定しない。
Cloneの仕様化とGameSession全体の証明完了を、この検査の成功から導かない。

`RangeInclusive::contains`はRust 1.97の実装で両端との比較を行い、NaN・無限大も区間外として拒否するため、
重複する`is_finite`項だけを除去した。公開finalizeの非有限値・符号付きゼロ・端点・直外値と
既存の失敗原因順を通常Rust回帰で確認する。
[固定Rust実装](https://github.com/rust-lang/rust/blob/1.97.0/library/core/src/ops/range.rs#L833-L846)、
[固定Cargo Guide](https://github.com/verus-lang/verus/blob/426d8b0/source/docs/guide/src/cargo_verus.md)に従う。

Verus frontend/attribute macro、専用rustc、bundled vstd、SMT solverと配布物の供給経路をTCBに含める。
ZIP digestは配布物の同一性を確認するものであり、その意味論やsolverを独立に証明するものではない。
製品bodyにassume/admit/external_body/assume_specificationによる免除を設けない。
`--no-cheating`、同一source確認、指定した負検証でもTCB自体は除去されない。
[公式TCB](https://verus-lang.github.io/verus/guide/tcb.html)に従い、仕様の十分性と製品callerの適用条件を別に監査する。

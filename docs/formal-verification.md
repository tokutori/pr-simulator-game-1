# 形式検証

## 目的と適用範囲

- Verusで、本番Rustが実行する離散的な認可・状態遷移・所有権・記録保持を検証する。
- 証明用の別シミュレーターや別の状態機械を正本にしない。
- 数値関数、実機適合性、allocator、GPU、外部APIを含むシステム全体の証明は完了していない。

- 本書は証明対象・契約・信頼境界を定める。
- 各revisionの実行結果はCI/PRに記録し、本書へ検証件数や実行日記を蓄積しない。
- 通常の数値・I/O・描画試験は[検証](verification.md)を参照する。

## 本番コードとの接続

- `cargo-verus`で実core crateとruntimeのDisplay所有操作を直接検証する。単体契約を同じ事後条件のラッパーから呼び直す方式は用いない。
- private fieldの仕様はghost-onlyのclosed predicate/viewから実ownerを参照する。ghost sequenceは実Vecのviewであり、別の記録storageを持たない。
- getterと純粋な分類関数は`dual_spec`、または上位の所有契約に必要な意味観測へ接続する。ghost参照は実ownerのfieldを読み、独立した更新modelを持たない。
- 複数操作を跨ぐ保持条件は、同じcrate内の証明専用clientで実際の操作列へ接続する。

- 検証注釈とghost-onlyの参照は`cfg(verus_only)`へ限定する。
- 通常Cargoで注釈・vstd importを除去し、coreの`no_std`、I/O分離、製品toolchainと依存DAGを維持する。
- `verus-proof`は固定vstdのallocを有効にする明示的optional featureである。
- closure仕様の`proc_macro_hygiene`と、同じconstructor impl内の条件付き注釈を先に展開する[`cfg_eval`](https://doc.rust-lang.org/std/prelude/v1/attr.cfg_eval.html)を証明時だけ有効にする。

本番bodyへ`assume`・`admit`・`external_body`・`assume_specification`による免除を追加しない。証明対象の追加時は実body、事前条件、公開事後条件、未証明の境界を明示する。

## 対象別の契約

### 停止理由と実行状態

`PauseReasons`はManual、DocumentHidden、TrackingSuspended、ProcessingDelayの4bit集合を所有する。空集合、所属、挿入・削除の冪等性、他原因の保持、4bit閉包を同じ実bodyで検証する。

- `request_resume`のOkはManual除去後の外的理由が空であることと同値である。
- ErrはManualだけを除去した非空集合を返す。
- 拒否後の再要求も残存理由を保持する。
- clear操作は自動再開しない。
- 外的理由の実際の発生・解消を検知するbrowser/OS処理はこの証明の対象外である。

- `FlightExecution`と`ActiveFlight<State>`はRunning/Paused、停止理由、不透明な飛行payloadを所有する。
- pause・clear・resumeがpayloadを変更しないこと、成功・NotPaused・PauseConditionsRemainの分類、Runningでのclear/resume拒否を検証する。
- 物理payloadの非更新と、その数値の妥当性・精度は別の性質である。

### Countdownと準備

本番`CountdownState`の正の秒数、非wrap token、古いtokenの無視、観測時刻・可用性の優先順、取消、待機、期限到達による発進認可を検証する。待機中の公開残秒は正であり、残秒0のCountdown状態を認可の前提にしない。

- 待機値はu64 nanoseconds、観測後の加算候補はu128で扱う。
- deadlineは最大u32秒の範囲である。
- 入力をu64へ縮小したり、deadlineでclampしたりしてoverflowを隠さない。
- 同じ時刻をDurationと整数の二重状態として保持しない。
- Duration生成・f64との変換・その丸めは未証明の標準境界である。

- GameSessionの準備・Ready・Countdown開始・観測は、認可拒否時のowner保持、同じconfiguration・独立demo・attempt tokenの保持を接続する。
- 実header validatorは`dual_spec`で参照する。
- 記録factory成功後だけrecordを交換し、factory失敗では旧ownerを保持する。
- 公開`prepare_flight`は本番`FlightRecord::try_new`へ接続し、同じheader・空sample列・未確定状態・最大tick数に1を加えた論理上限を持つrecordだけをPreparingへ格納する。

- Countdown観測の本番callerは、同じconfigurationから得た初期状態・telemetryを`record.begin_tail`へ渡し、その成功後だけRunningへcommitする。
- 非Countdownと古いtokenは数値callbackを呼ばない。
- 発進後の再観測で再発進しない。
- 初期状態・telemetryの数値関数と、その具体callbackの接続は通常回帰で確認する。

### Result・Replay・保存済み記録

- 本番`ConfiguredPhase`でResultだけがReplayへ入り、Replayだけが元のResultへ戻ることを検証する。
- 成功時は結果payloadを完全保持し、指定clockを格納する。
- 拒否時はenum全体を保持する。
- Replayのclock数値計算は対象外である。

実GameSessionの`open_archived_replay`、`take_finalized_result_record`、`install_attract_record`、`leave_attract`、`playback_record`を直接検証する。許可phaseと入力recordの確定状態を認可条件とし、header・sample・finalizationを含む同じrecordの移譲・参照を保証する。

- 拒否時はsession全体を保持する。
- archived Replayの退出はTitleへ戻り、通常のResult/Replay往復と区別する。
- archiveの閲覧・Result取り出しは独立demoとCountdown tokenを保持し、demoの再登録は既存記録を上書きしない。
- `enter_attract`のdurationと再生clockの浮動小数点演算は直接証明の対象外である。

### Retry

- 本番`ConfiguredSession`はconfiguration、record、ConfiguredPhaseを所有する。
- Result認可、factory呼出し、recordとReadyへのcommitを同じowner methodで検証する。
- configurationやrecordへCopy/Cloneを要求する別の状態機械を設けない。

- 成功時はconfiguration全体を保持し、factoryが返した同じrecordへ交換する。
- header生成・予約を含むfactory失敗はowner全体を保持する。
- Result以外はfactoryの前提を要求せず、呼出し前に拒否する。

- scenario・seed・初期状態・tick上限の保持は同じconfigurationの構造的保持である。
- 公開`retry`と`retry_briefing`は本番constructorを同じfactoryへ渡し、同じconfigurationから生成した空recordをそれぞれReadyとPreparingへ格納する。InvalidHeader・AllocationFailedと認可拒否は旧owner全体を保持する。

### Displayの非同期認可と資源所有

本番runtimeの`DisplayWorkflow`が所有するstate・active資源・permission資源と、同じ`request`・`complete`・`close`・`release_unused`の更新bodyを直接検証する。modeはpresentationの実`GameUiPresentationMode`を共有し、その構造的な等値関係を固定Verusの`Structural`で接続する。

- 要求待機中または同じ表示modeへの要求は、stateと保持資源を変更せずI/O要求を返さない。
- 受理した要求は入力modeとrequest IDを次のpending stageへ保持し、返すprimitive要求と一致させる。Stopはactive資源、Startは保持permission資源を参照し、Screenはpermissionなしで開始できる。
- request ID不一致または成功completionのstage不一致は、stateと両保持資源を変更しない。Failedは各pending stageの合法な失敗遷移として扱う。
- 認可されたStartedは実資源IDをactiveとして保持し、permissionの所有を解消する。同IDの引継ぎを許容し、保持中IDをReleaseへ返さない。
- 不一致completionが取得した未保持資源は、そのIDのReleaseを1件返す。`release_unused`も未保持Someに対して1件、保持IDに対して0件を返す。
- closeは旧保持IDだけを昇順・重複なしでReleaseし、Closedと空の所有へ移る。

- 検証接続: 証明時のcfgはDisplayの実型・更新・返却を選び、通常ビルドは同じ更新bodyと既存serde/schemaを使う。
- 外部I/O: aggregate入力、shutdown ledger、Releaseの実行と終端、重複遅着通知を跨ぐexactly-once、browser/OSの実handle寿命は、通常接続試験と実環境の受入で確認する。

### FlightRecordの容量・確定

`FlightRecordStorage`は実header、Vec sample、論理capacity、finalizationを所有する。低水準push/finalizeはprivateであり、公開操作が時刻・有限値・metadataを検査する。

- push成功は既存sample列をprefixとして保持し、1件だけ追加する。他fieldは変更しない。
- 容量拒否はowner全体を保持する。usizeの32/64bitを固定せず、長さと上限から追加の安全性を検証する。
- finalizeはAlreadyFinalized、NotStarted、最後のsample検査、metadata commitの順序を持つ。
- finalize成功は検査出力を一度だけ格納し、header・sample・capacityを保持する。全Errはownerを保持する。

- 検査callbackは不変なsample参照だけを受け取り、recordへの可変参照を持たない。
- 実Vecの論理更新は固定vstdの仕様へ依存する。
- 本番`try_new`はheaderの検査後に`try_reserve`を一度実行し、成功時の同じheader・空列・未確定状態・論理上限と、失敗の分類を直接検証する。Headerのエラー分類は同じ`dual_spec`本体から証明した補題でmodule間へ渡す。
- 物理容量は標準Vecの予約保証に従って論理上限以上を確保する。予約容量は通常容量試験、飛行中の追加割当は既存allocation計測、WASM codegenは通常ビルドで確認する。予約の成否と物理割当量は実行環境のallocatorに従う。

- 公開`begin_tail`と`append_tail_report`は実bodyの認可、state/control/telemetry転送、bounded Vecへの追加、拒否時のowner保持を検証する。
- NoElapsedIntervalでは成功・失敗にかかわらずrecord全体を保持する。
- 初期controlsとNoElapsed分類は本番の純粋関数から生成するspecを用いる。

- 公開`finalize`・`finalize_with_failure`、実`finalize_record`・`validate_finalization`は実coreへ直接接続する。
- Err時のowner保持、成功時のreason/tick/fraction/score/元failure転送を検証する。
- dispositionと数値の意味的対応を含む全数値仕様の証明には拡張しない。

### Running tickと終端化

- 実ConfiguredPhaseのRunning認可だけが飛行stateを返す。
- 認可拒否では数値callbackを呼ばず、ownerを保持する。
- 評価・telemetry・scoreのFnOnceはconfigurationとCopy値を受け取り、record・phaseへの可変参照を持たない。

- 同じprevious/inputを評価へ一回渡し、返却reportのstateをtelemetryへ渡し、同じreportとtelemetryをappendする。
- 呼出し可能性以外の数値的相関をcallbackの事前条件へ追加しない。
- 実bodyによる出力転送、sample列のprefix、最大1件の追加を検証する。
- [FnOnce仕様](https://verus-lang.github.io/verus/guide/reference-signature-fnonce.html)に従う。

| 経路 | 保持・commitの契約 |
| --- | --- |
| 評価失敗 | previousと元のTailFlightTickErrorをResultへ保持する |
| 元failureを伴うscore失敗 | 元reason/failureを保持し、scoreをNoneとする |
| score単独失敗 | Fatal Result、score None、failure Noneを保持する |
| telemetry/append失敗 | previousを終端化し、finalize自体のRecord/Score errorを既定の優先順で返す |
| 正常継続・TimeLimit・WaterContact | 同じreport・record・Resultのstate/stamp/metadataを転送する |

- Record errorには入口owner保持、previousのFatal Result、append済みRunningがある。
- append後のfinalize拒否へ一律の全Err不変を要求しない。
- 内部不整合を注入した回帰と、利用者操作から到達する障害を区別する。

- 本番`evaluated_tick_report`は、Advancedと正fraction接触のstate/control、fraction-zero分岐のprevious state・incidence・pilot target保持、新control除去を検証する。
- 実begin/Advanced appendが最新sampleとRunning stateの対応を確立する。
- 合成clientは実producerと同じpreviousからNoElapsed append/finalizeを呼び、sampleのinput/telemetry、整数tick、FlightState、held incidenceの保持へ接続する。

- この相関は実begin/appendと本番包装から導き、任意の別previousから得たreportへ適用しない。
- fractionのIEEE比較、数値接触検出、非自明な補間、数値evaluation、外側callbackの具体wiringは未証明である。
- 初期sampleに独立したpilot target fieldはないため、targetはreportのprevious保持に保証を限定する。

## runnerと固定ツール

検査入口は既存の一つのrunnerである。

```powershell
./tools/verus/check-verus.ps1
```

runnerは一回の`cargo-verus verify`でcoreとruntimeをrootとして選択し、各rootのJSON成功を確認する。coreの本番契約・操作列とDisplayの実所有操作は同じtargetの依存を共有し、coreの検証を重複実行しない。その後、直接検証したcoreを使う統合負例を一回実行する。

| 項目 | 固定値 |
| --- | --- |
| Verus | `0.2026.10.04.426d8b0` |
| Windows archive | `verus-0.2026.10.04.426d8b0-x86-win.zip` |
| archive SHA-256 | `b36968a0f333036e7e1770bb73fdd407a35c8fd163124c1d309972574150da61` |
| 証明用Rust | `1.98.1` |
| 製品Rust | `rust-toolchain.toml`の`1.97.0` |
| vstd | `0.0.0-2026-10-04-0306`、非default・allocのみ |

- [固定release](https://github.com/verus-lang/verus/releases/tag/release/0.2026.10.04.426d8b0)と[導入手順](https://github.com/verus-lang/verus/blob/release/0.2026.10.04.426d8b0/INSTALL.md)を基準とする。
- 解凍・実行前にdigestを照合し、compiler/verifier versionを確認する。
- 証明専用toolchainを追加し、製品のdefault/overrideを変更しない。

runnerは以下を要求する。

- `--no-cheating --num-threads 1`で、各rootの証明対象を一括検証する。coreは本番crate、runtimeはcfgで分離したDisplay所有操作を対象とする。
- positiveはexit 0、正のverified件数、0 errorsを要求する。
- compiler出力でcrate全体が対象となったことを確認する。内部補助関数名の固定表や関数ごとの別プロセスへ依存しない。
- 統合negativeは非zero exitと、指定した三つのassertionのファイル・行・診断を要求する。構文・起動・別assertの失敗を成功に数えない。
- 検査前後のsource/toolchain hashを照合する。negativeは直接検証した同じcore/vstdのrmeta/virと固定builtinを用いる。

- 保証範囲は本番関数の契約と複数操作のclientを正本とする。
- 削除や範囲縮小は通常の差分レビューで確認する。
- CIのVerus jobは同runnerを実行し、通常Rustのbuild・lint・関連回帰・no_std検査は別の保証として維持する。

## 信頼境界と残る検証

- TCBにはVerus frontend・attribute macro、専用rustc、bundled vstdの標準仕様、SMT solver、配布物の供給経路を含む。
- archive hashは同一性を検査し、意味論やsolverを独立に証明するものではない。
- [公式TCB](https://verus-lang.github.io/verus/guide/tcb.html)に従い、仕様の十分性とcallerの適用条件を別に確認する。

- 固定Verifierでf64比較の標準関係は未解釈である。
- 実際の`!= 0.0`の分岐とspec値等号を同一視せず、[固定vstdの比較仕様](https://github.com/verus-lang/verus/blob/426d8b0/source/vstd/std_specs/cmp.rs)の範囲を保持する。
- telemetry validatorの有限閉区間とprivate端点定数は、製品RustでMIN/MAXのbit一致、NaN/±∞/±0、端点近傍、subnormalを検査する。
- `RangeInclusive::contains`の境界も[固定Rustの実装](https://github.com/rust-lang/rust/blob/1.97.0/library/core/src/ops/range.rs)へ依存する。

Vector3/Point3のgeneric derived Cloneは固定Verusで等価仕様を生成できない。現在の保持契約は実owner・同じ値の転送へ接続し、Clone equalityを仮定しない。

| 対象 | 現在の位置付け |
| --- | --- |
| 認可・離散遷移・本番record保持 | 上記の実bodyと公開callerへ限定して証明する |
| Duration、Replay clock、allocator、数値callback、wrapper全分岐 | 未証明の境界を保持し、通常回帰を併用する |
| Displayのrequest認可・保持ID・返却Release | 上記の実所有操作と契約を直接証明する |
| record codec、PB identity、他の非同期generation・外部資源の実寿命 | 証明拡張の候補であり、現状の試験を形式証明済みと扱わない |
| 浮動小数点収束、局所安定性、空力適用域、保護候補の存在 | 解析解、量別誤差、刻み・domain検査を用いる |
| model fidelity、実GPU・入力・保存、VR、FPS | 実データ・実行環境・性能計測を必要とする |

- 有限集合の完全列挙にはその範囲での保証がある。
- 証明へ移した同一性質の重複は整理できるが、通常compilerでの接続、数値、資源解放、stale結果、GPU、配布物起動を一括削除しない。
- 証明は仕様への適合を扱い、仕様の物理的妥当性や環境接続を無条件には保証しない。

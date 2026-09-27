# GameScene・overlay・sessionの契約

## 状態機械と描画world

主要GameSceneはBoot、Title、FlightSetup、Briefing、Countdown、Flight、Result、Replayの8種とする。
GameSceneは `web/src/app` のdiscriminated unionで管理し、許可されたeventのみで遷移する。
描画worldはGameSceneと独立に保持する。Three.jsのScene等の具体objectはengine adapterが所有する。
水面・地形・空・雲・機体の共有assetは通常遷移で破棄せず、camera、visibility、UI、sessionを変更する。
world差し替え・context復旧・終了時には所有者がresourceを明示的に解放する。

| GameScene | sessionの処理 | 主な遷移 |
|---|---|---|
| Boot | WASM・最低限asset・renderer初期化 | 成功→Title、失敗→Boot内のerror状態 |
| Title | 独立したAttract再生のみ | Flight→FlightSetup、Demo→Attract substate |
| FlightSetup | 三軸設定を検証しFlightConfigurationを構築 | Start→Briefing、Back→Title |
| Briefing | 必要asset取得、初期状態・記録bufferを準備 | Ready→Countdown、Back→FlightSetup |
| Countdown | 初期状態を固定し、物理時刻を進めない | 完了→Flight、取消→Briefing |
| Flight | 100 Hz physics、入力、記録 | 終了→Result、Pause→同じFlight内で停止 |
| Result | 確定済みrecordを参照 | Replay、Retry→Briefing、Setup、Title |
| Replay | 確定済みrecordの再生・seek | 戻る→Result |

SummaryとAnalysisはResult内のtabである。Replayボタンは主要Scene遷移を行う。
Retry、Attract、camera種別、解析グラフごとのGameSceneは追加しない。

## 準備と開始

BootではWASM、機体・scenario manifest、最小world、renderer、capability調査、初期画質推定を扱う。
高品質assetは必要時にlazy loadする。端末権限やXR sessionは自動要求しない。
Bootのloading/failed、Briefingのpreparing/ready/failedは各Scene内のunionとし、
準備未完了の値をPreparedFlightとして扱わない。
失敗時は原因と再試行・復帰操作を表示し、読み込み成功を装って進行しない。
非同期loadの世代を識別し、Scene退出後の完了通知による巻き戻りを防ぐ。

FlightSetupではphysics stateを積分しない。Briefingで設定を解決し、
WindField、波・空・雲、launch条件、asset hashを確定する。
Countdown開始時にconfigurationをsealし、launchまで物理・controller・actuatorの時刻を固定する。
操縦deviceの最新値は取得するが、Countdown中の入力を事前積分しない。
launch eventを一度だけ処理し、tick 0の初期snapshotを保存してから、tick 0の入力で最初のstepを実行する。
Countdown時計はUI用であり、Flightのsimulation timeと分離する。
非表示化やtracking中断でCountdownを停止し、明示的な再開なしに発進しない。

## Overlayと停止

Settings、Pause、Help、Creditsはoverlayとして管理する。HUDは状態に応じて合成する表示である。
単純なGameScene×Overlayの直積は採用せず、Sceneごとに有効な組合せを型で表現する。
例えばFlightはrunningまたはpausedのsubstateを持ち、pausedだけがPause menuとそのSettings/Helpを開ける。

| 所属 | 許可するoverlay |
|---|---|
| Boot | error/retry表示のみ |
| Title | Settings、Help、Credits |
| FlightSetup | Settings、Help、Credits |
| Briefing | Help、Credits |
| Countdown | 取消・停止表示 |
| Flight running | HUDと警告のみ |
| Flight paused | Pause menu、その子のSettings/Help |
| Result | Help、Credits |
| Replay | 再生停止状態でSettings/Help/Credits |

FlightのSettingsを閉じるとPause menuへ戻る。overlay終了だけでphysicsを再開しない。
手動pause、非表示tab、処理遅延、XR session中断の理由を保持し、全停止条件の解消とResume操作を要する。
pause中はtick・controller・actuator・記録sample・波のsimulation timeを進めない。
描画、UI操作、必要なhead trackingは継続してよい。
再開時にwall-clock差を加算せず、accumulatorと入力境界を再設定して次tickから進める。
非表示化時の押下状態を解除し、device再接続後の入力を再取得する。

Settingsは画質、音量、表示サイズ、binding設定等を扱う。
初期版ではInformation内容、camera許可・安定化、Assistance、Weather、input shapingを飛行中に変更しない。
これらはFlightSetupへ戻って変更する。HUDの文字サイズ・配色等の可読性設定は変更してよい。
操縦bindingはFlight中は固定する。頭部recenterは姿勢の基準化として記録metadataへ残す。

## 終了とResult

終了理由はWaterContact、OutOfValidEnvelope、ManualAbort、FatalSimulationError、TimeLimitを区別する。
TimeLimitはscenarioの明示的な最大tick数に達した場合であり、物理的な着水とは区別する。
同一tickで競合したeventは一度だけfinalizeする。coreの失敗は未確定stateを採用せず、最後の有効stateを保存する。
着水を計算できた有効stepでは接触eventを確定して以降のstepを行わない。
手動中断は次step前の入力境界で確定し、終了時刻と理由をrecordへ保存する。
致命的なboot/assetエラーは架空のFlightResultを生成しない。

Resultの背景は最後の有効snapshotで固定する。Summary、Analysis、Replayは同じimmutable recordを参照する。
着水以外は終了点をEndとして表示し、Splashという名称を使用しない。
初期のPersonal Best登録対象は検査済み完全recordのWaterContactのみとする。
保存容量不足等は型付きの保存結果として通知し、記録保存の失敗を成功と表示しない。

Retryはsealed configuration・初期状態・scenario seed・波の位相基準を維持し、
controller/actuator内部状態、時刻、入力buffer、recordを初期状態へ戻してBriefingへ遷移する。
同一分類からのscenario再抽選は行わない。条件を変更する場合はFlightSetupへ戻る。
Replayから戻る際はResult tabと選択時刻を復元する。

## Attract・Credits

TitleのAttractは記録済みdemo flightとCameraDirector、cinematic HUDを使用する。
一定時間の無操作またはDemo操作で開始し、利用者の操作でTitleへ戻る。
demoはplayer session・Personal Best・Replay履歴を更新しない。
新規入力をAttract終了とFlight開始へ二重使用しない。overlay表示中は無操作タイマーを停止する。
自動操縦によるlive demoを追加する場合も独立sessionとし、記録再生とは状態を区別する。

Creditsは同梱済みassetだけを対象とし、manifestから表示用creditsと第三者帰属一覧を生成する。
候補データの提供者を使用済みとして表示しない。ソースコードとpackage licenseの表示も含める。
生成物と `THIRD_PARTY_DATA.md` の登録一覧の一致を配布CIで検査する。
API権限、退出、エラー表示はInformation設定にかかわらず利用できる。

## 検証

BPG-017で遷移表、不許可event、二重launch/finalize、load競合、Pause→Settings→Pause、
非表示復帰、同条件Retry、三軸固定を検証する。
Replayの再生操作はBPG-021、AttractとCameraDirectorはBPG-022で接続する。
Scene追加とcamera追加を独立に扱い、world resourceの再生成・listener重複を検査する。

## 段階的な完成範囲

全8 SceneとoverlayはScreen・WebXR・Phone VRの共通表示基盤を使用する。
初期のBPG-014〜016で全Scene用の型付きview modelと操作経路をfixtureで検証し、
BPG-017で実sessionへ接続する。fixtureによるUI検証をフライト実装の完成と混同しない。
ゲームの基本ループは実地形・実気象・高品質rendererに依存させない。
BPG-017では合成scenarioと簡易worldでBootからResult、Retry/Setup/Titleへの復帰を成立させる。
Replay操作はBPG-021、Attract操作はBPG-022で有効にする。
実装前の機能へ遷移する操作を公開せず、架空のrecordや成功結果を返さない。
簡易world・合成scenarioも正式なasset/model形式を使用し、出典に自作の検証データと明記する。

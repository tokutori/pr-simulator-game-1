# ゲーム進行・共通UI・視点

## 状態と責務

- ゲームの状態と操作の正本は共有Rustの`ApplicationSession`と`GameSession`である。
- Web/WASMとNativeは同じcommand executorへ委譲する。
- 表示先は不変snapshotから画面を導出し、遷移可否、再開条件、設定解決、scoreを再実装しない。

| Scene | 役割と遷移 |
|---|---|
| Boot | WASM・renderer・最低限asset等を準備する。成功でTitle、失敗時は原因と再試行を表示する |
| Title | 新規Flight、保存記録、独立したAttract、設定・案内への入口 |
| FlightSetup | プリセットとInformation・Assistance・Weatherを編集する。確定でBriefing、戻る操作でTitle |
| Briefing | 確定条件と操作方法、準備結果を確認する。開始でCountdown、設定変更でFlightSetup |
| Countdown | 初期状態を保持して発進時刻を待つ。完了でFlight、取消で同じBriefing |
| Flight | 固定100 Hzで入力・物理・記録を進める。停止は同Sceneのsubstate、終了でResult |
| Result | 確定結果と記録を表示する。Summary・Analysis、Replay、Retry、Setup、Titleを提供する |
| Replay | 確定記録を時刻指定で再生する。現行記録はResult、保存archiveはTitleへ戻る |

- SummaryとAnalysisはResult内のtabである。
- 結果画面は通知、「概要 / 分析」の選択tab、選択した内容の順に表示する。下部にはReplay・同条件の再試行・設定変更・Titleへの共通操作列を配置する。
- 概要は距離・時間・終了理由と副指標を二段に分ける。「詳細・ログ出力」で飛行条件・CSV/JSON・技術情報を展開する。
- camera種類、解析グラフ、Retry、AttractごとのSceneは追加しない。
- 描画worldはSceneと独立して保持し、通常遷移では共有assetを再生成しない。
- world交換、context復旧、終了時の解放は資源所有者が実行する。

- Bootの非同期読込み、権限要求、DOM focus等はplatform境界で管理する。
- 要求には所有者・世代を付け、退出後の完了通知で画面を巻き戻さない。
- 古い要求が取得した資源も解放する。
- 準備失敗を成功と表示せず、未実装機能への有効な遷移や架空のrecordを公開しない。

## Flight Setupと難易度

難易度は三軸で構成する。

```math
D=(D_{information},D_{assistance},D_{weather})
```

- 公開アプリの既定はFull・Light Assisted FBW・Typicalである。
- Lightは共有の0.2 FBW authorityへ解決する。
- 3D描画品質の既定はAutoであり、難易度の三軸から独立する。

| 軸 | 選択と責務 |
|---|---|
| Information | Full・Standard・Minimal・Realistic・Custom。HUDと情報補助だけを変更する |
| Assistance | Strong・Assisted・Light・Manual・Custom。検証済みcontroller設定へ解決する |
| Weather | Calm・Mild・Typical・Challenging・NearLimit。登録scenarioと環境を選択する |

- 各設定で6DoF方程式、係数、風の作用、actuator、積分刻み、質量特性の算出規則を共用する。
- 同じ機体・身体初期状態、aircraft、environment、舵actuator入力列、身体目標列、刻みから同じ物理応答を得る。
- 難易度による揚力増幅、抗力低減、物理姿勢の直接修正は禁止する。

- 外部設定の型、validation、preset展開は`birdman-game-format`を正本とする。
- 共有application層がscenario/controllerを解決し、coreへ物理configurationを渡す。
- Information metadataは表示境界に保持し、physics APIへDifficulty型を追加しない。
- scenario選択はcatalog versionと明示したseed・選択規則で決まり、時計や暗黙乱数を使用しない。

### 選択と確認

- Flight Setupではプリセットの一括選択と各軸の直接選択を視覚的に分離する。
- 候補、現在値、説明を同時に確認できる設定フォームとし、候補を順送りする操作へ依存しない。
- 手動変更後のCustomは変更済み状態を示す。

| プリセット | Information | Assistance | Weather |
|---|---|---|---|
| Beginner | Full | Strong | Mild |
| Standard | Standard | Assisted | Typical |
| Expert | Minimal | Light | Challenging |
| Realistic | Realistic | Manual | Typical |

- Realisticは対象機に近い情報・操縦条件を意図する名称であり、実機の計器構成が未確定の段階で実機と同一と表示しない。
- 通常警告のWarnings cueはproducer未接続の能力として非対応を明示する。
- アプリケーションエラー、権限状態、退出操作、必須のcontroller failureはInformation設定にかかわらず表示する。

- Fullは利用可能な主要telemetry、姿勢、方位、風、迎角、flight-path等を扱い、Standardは主要計器、Minimalは高度・距離・時間を中心にする。
- Customはtelemetry・attitude・wind・flight-path・AoA・warningsの各cue設定を保持する。
- ゼロ対気速度等の未定義量は型付きの欠損理由を維持し、表示値を捏造しない。

- Briefingは最終確認に特化し、飛行条件、操縦方法、準備結果を表示する。
- 主要操作は「発進カウントダウンを開始」、戻る操作は設定変更とする。
- version・seed等は初期状態で閉じた技術情報へ格納し、開閉は共有UI状態で扱う。
- 設定・確認・発進の進行表示を設ける。

- Weatherではscenario名、代表風向風速、その地点・高度、空間変動、鉛直流、ゲーム上の発進条件を表示する。
- 代表風を全fieldの値と解釈しない。
- 気象風向は真北を0°とする時計回りで、吹いてくる方向を示す。
- 水平無風は方向なしとし、鉛直流は上昇・下降を別記する。
- 画面上の風ベクトル矢印が示す移動先方向と区別する。

- Weather分類はWindField・WaveState・SkyStateを一貫して確定する。
- 名称から風速や発進可否を推定せず、未搭載scenarioは非対応理由を返す。
- NearLimitはゲーム上の分類であり、大会の公式運用限界を保証しない。
- 合成scenarioと登録気象の根拠は[データ形式と出典](data-and-formats.md)に従う。

Assistanceはpilot舵指令とFBW舵指令のauthorityを解決し、共通actuatorへ渡す。

```math
u_{surface}=(1-a)u_{pilot,surface}+a u_{FBW,surface},\qquad 0\le a\le1
```

- Manualは$a=0$、Automaticは$a=1$、Sharedは混合設定である。
- 身体の目標位置は別経路で扱う。
- actuator特性を難易度で変更しない。
- 実装済みの入力・尾翼保護と、その限界は[飛行制御](flight-control.md)を参照する。

## 準備・Countdown・停止

- Briefingで解決済みconfiguration、seed、初期状態、環境・asset identityを確定する。
- Countdown中は物理・controller・actuator時刻を固定し、操縦入力を取得・積分しない。
- launch時にtick 0の初期snapshotを記録してから最初の入力を適用する。
- 開始前のkeyboard押下は破棄し、gamepadは新規飛行とPause復帰時に中立確認を行う。

- Countdownは共有coreがduration、試行token、受理済みelapsedを保持する。
- adapterは同じ単調時計の連続観測の差分を秒単位で渡す。
- callback回数や累積時刻を時間の正本にしない。
- 残り秒は切上げで導出し、3秒Countdownは800 msで3、1秒で2を示し、3秒に達した最初の有効観測で一度だけlaunchする。

- 同時刻の観測は差分0とする。
- 負値・非有限値・Duration変換/加算overflowは状態を変更せず拒否する。
- 取消・launch済みtokenは無作用であり、再開始は新tokenを使用する。
- tokenはwrapさせず、枯渇時は開始を拒否する。
- Webは`performance.now()`と起床用timer、Nativeは既存`Time<Real>`のframe差分を使用する。
- 同frame中に開始・取消・再開始した試行へ、それ以前の差分を転用しない。

- 非表示化、tracking中断、共有上限150 msを超えるframeでCountdownを取り消し、同じBriefingへ戻す。
- 中断後の発進には明示的な再開始を要する。
- Flightもtickと入力取得の前にProcessingDelayで停止する。
- 1秒timerの起床間隔をframe処理遅延と扱わない。

| Scene / substate | 表示・許可するoverlay |
|---|---|
| Boot | 読込み失敗と再試行 |
| Title | Credits |
| FlightSetup・Briefing | 通常の設定・確認画面 |
| Countdown | 取消・停止案内 |
| Flight running | HUD・必須警告 |
| Flight paused | Pause、その子のSettings・Help・Credits |
| Result・Replay | 通常の結果・記録再生画面 |

- Sceneとoverlayの有効な組合せを型で表す。
- Settingsを閉じるとPauseへ戻る。
- Manual・非表示・処理遅延・XR中断等の停止理由を保持し、条件解消と「飛行を再開」の両方を必要とする。
- Pause中はtick、controller、actuator、sample数、水面simulation時刻を保持する。
- 描画・UI・head trackingは継続できる。

- 復帰時はwall-clock差を加算せず、accumulatorの残余と入力境界を再初期化する。
- Flight中は解決済み三軸、input shaping、操縦bindingを固定する。
- HUDの可読性と描画品質は変更できる。

- WebのBFCache退避ではsession・record・rendererを保持し、Countdown取消、Flight停止、Replay/Attract clock停止を行う。
- 復帰時は同じsnapshotを同期してScreenへ戻り、FlightとReplayの再開には明示操作を要する。
- Attractは同じcursorから再開できる。
- 保留中のXR/sensor要求と古いtimerを失効させ、通常のpage退出で最終解放する。

## 終端・Result・保存

- 終了理由はWaterContact、OutOfValidEnvelope、ManualAbort、FatalSimulationError、TimeLimitを区別する。
- 同一tickの終端は一度だけ確定する。
- 有効な着水stepは接触時刻を確定して以後を進めず、計算失敗では未確定stateを採用しない。
- 手動中断は次step前に確定する。
- bootやassetの失敗から架空のFlightResultを生成しない。

- Resultの背景は最後の有効snapshotで固定する。
- 確定score・元cause・終端stampと、解析・Replayの任意cursorを分離する。
- terminal通知は描画・HUD・入力cleanupの成否から独立して一度送る。
- resetやdispose後へ旧frameの描画・cleanupを適用しない。

- Summaryの主表示はversion付きcourse-distance scoreと飛行時間である。
- cross-track、net horizontal distance、最大対気速度、最大重心迎角、最大絶対roll、解決済み設定、model/scenario/controller identity、seed、代表風を併記する。
- 異常終了を着水と区別し、取得不能値はunavailableと示す。
- 内部diagnosticは閉じた技術情報に配置する。

Retryはsealed configuration、初期状態、scenario seed、波の位相基準を維持し、controller・actuator・時刻・入力buffer・recordを初期化してBriefingへ戻る。同じ分類からscenarioを再抽選しない。

- Personal Bestは検査済み完全recordのWaterContactを対象とする。
- 比較条件とbuild来歴の定義、JSON/CSVと保存一覧の境界は[データ形式](data-and-formats.md)へ集約する。
- 画質やbackendを物理比較条件に含めない。
- 保存結果は記録単位で保持し、同じ記録の結果画面とリプレイへ表示する。資源の再準備中も保持し、記録の交代と終了時に退役する。保存失敗の原原因は診断へ保持する。

### AnalysisとReplay

- Screenの分析画面は基準フォントサイズの72倍を最大幅とする中央カードとし、外周余白と縦スクロール領域を設ける。共有Rustが画面高・文字高からチャートの目標高さを決め、adapterの実測値から表示座標を更新する。
- 「軌跡 / 高度 / 速度」は共通の選択tabで切り替える。「選択地点の値」は時刻・北/東位置・高度・対気/対地速度・風の3成分を、ラベル・数値・単位に分けて表示する。

| グラフ | 意味 |
|---|---|
| Horizontal map | 合成重心の軌跡、Start・終端、方位、scale、会場地物、風 |
| Altitude vs time | 静水面基準の合成重心高度、静水面0、発進高度、共有cursor |
| Airspeed / Groundspeed vs time | 重心の対気・対地3D速度norm、共有cursor |

- 地図は東を右、北を上とする等縮尺local NED平面である。
- 共有Rustが記録座標を論理pxへ投影し、2Dのy反転と風矢印の線分を導出する。Screen・Bevyはその座標を描画し、空間Menuは同じgeometryをpanel矩形へ配置する。
- 軌跡は記録済み合成重心を使用し、body stateから再計算しない。
- 接触時の機体重心位置と物理接触点を区別する。
- 終端のマーカーと凡例には共通の日本語終了理由を表示する。

- 静止・極短・一直線の軌跡でも表示範囲を退化させない。
- 距離目盛・scale bar・time marker・legendを備え、色だけに依存しない。
- Y軸名は等縮尺プロットの左側に揃え、方位とscale barはプロット上部の専用余白に配置する。近接する時刻ラベルは表示を間引く。
- 会場地物は同じorigin/versionを使用する。
- 地図tile取得を必須にせず、未登録の地理原点を実会場へ補完しない。

- 風断面は高度と範囲を明示した固定NED水平面のqueryである。
- 初期条件では発進時重心高度の5×5 gridを使用する。
- 矢印は空気の移動先を示し、鉛直流を別記する。
- 選択地点の風矢印の縮尺は凡例に表示し、時刻選択に合わせて更新する。
- 時刻・位置・高度の異なる局所風を同一断面へ混合しない。
- 登録環境が欠ける場合は疎gridを非表示とし、記録上の重心風を使用する。

- 高度は$h=-D$、速度は3D normと単位を明示する。
- 横風・鉛直流がある場合、対気・対地速度の大小だけで向かい風を判断しない。
- 根拠のない速度安全域や高度警戒帯を追加しない。
- 追加の力・局所迎角等は対応diagnosticがある記録だけで表示する。

- ResultとReplayは秒単位の単一record-time cursorを使用する。
- 時刻スライダーに加え、高度・速度チャートのプロット内から主ボタンでドラッグして時刻を選択できる。捕捉中は左右の範囲外を先頭・末尾へ丸め、ボタン解放・取消・捕捉喪失・画面切替で操作を終える。
- 分析・再生の時間軸は記録の実際の終了時刻までとし、「先頭へ」「末尾へ」で正確な端点を選択する。0秒の記録は選択地点の値を表示し、時刻変更の操作を無効にする。
- seek範囲、補間sample、再生速度・状態はcore queryを正本とし、adapterは時計の起床と入力だけを担当する。
- Resultの時刻照会はReplay遷移を必要とせず、phase、score、record、physics、再生clockを変更しない。
- clock操作はReplay/Attractへ限定する。

- 描画用間引きは端点・極値・eventを保持する。
- cursorの数値は原sampleから導出し、間引きpolylineを正本にしない。
- resizeや画質で集計値を変えない。
- keyboard・touch・VR入力でもtab、scroll、時刻選択、戻る操作へ到達できる。
- current ReplayからResultへ戻ると元tabとcursorを復元し、保存archiveのReplayはTitleへ戻る。

## 共通UIの構造

- 共有Rustが文言、値、単位、可視条件、許可されたaction、相対順序を所有する。
- 通常menuは順序付きsectionのColumn・Row・ChoiceGridを正本とし、座標やID文字列からgroupや順序を復元しない。
- Statusと数値項目は型付きsourceから動的値へ接続する。数値項目は桁位置が安定する書体で描画する。

- HUDは画面下端を基準とする透明な領域へ配置する。主要数値と計器を横並びの行へまとめ、右下の操作領域と前方中央を空ける。
- 共有RustがReadouts・Instrumentsの行、領域全体の寸法、各nodeの正規化boundsと文字高に対する計器の高さを決定する。
- nodeはID、label、Text・Attitude・HeadingTape・Direction・LinearGaugeの内容を持つ。パイロットの実位置と目標位置は一つの値として表示する。
- 姿勢計は透明な線画、ピッチ目盛、飛行経路マーカーを使用する。地図の出典表示は地図とクレジットへ配置する。
- adapterは種類を網羅的に投影し、同型計器の追加で項目別の表示実装を増やさない。
- node IDは保持対象の識別だけに使用する。

- 静的な構造revisionと動的frameを分離する。
- 文言変更によってnode数、group、順序を変更せず、表示文字列のprefixから計器を選ばない。
- chartは系列、色、gradient、legend、cursorの意味を保持し、全backendでswatchと系列の対応を維持する。

- ScreenはDOM/SVG、VRはCanvasとpanel、Nativeはretained ECSへ投影する。
- adapterは実viewport・文字計測・入力・focus・pointer捕捉とGPU資源のI/Oを担当する。共有Rustは論理pxとpanel寸法への配置、scroll・入力の認可、実矩形からチャート時刻への変換を所有し、adapterは実提示revisionと要素IDを添えて観測を返す。
- 同じSceneの値更新でnode・focus・scrollを保持し、Scene変更で適切に初期化する。
- menu内の画面外の操作はscrollとkeyboard focusで到達可能にする。HUDはスクロールを使用しない。
- Screenの最低対応解像度はUI倍率適用後の1280×720、推奨は1920×1080以上とする。

VRを含むanchor、両眼への収まり、透明度、前方clear regionは[描画とplatform](rendering-and-platforms.md)に従う。保存全件の索引とTitle最大3枠の表示queryは分離し、毎frameの共通UI入力を有界に保つ。保存一覧の取得に失敗した場合は、Titleの「保存記録の一覧を再試行」から取得し直す。取得中の重複操作と古い取得結果を拒否し、正常な空一覧と失敗を区別する。

## Camera

- Webの通常FlightはPilotを使用し、Information・Assistance・presentation種別から独立する。
- Nativeはpresentation操作としてPilot/Chase切替を提供する。
- いずれもcamera操作からphysicsやrecordを変更しない。

PilotEyePointは機体構造datumからの基準位置・取付姿勢を持つmetadataであり、左右眼の中点を表す。実身体移動$x_p$に対し、基準位置から前後方向の眼移動を導出する。

```math
r_{eye}^{B}=r_{eye,0}^{B}+(x_p-x_{p,0},0,0)
```

- 表示上の眼位置は共有cockpit形状から導出する。
- 現synthetic形状のFRD基準offsetは`[0.06, 0, 1.04] m`であり、実測の搭乗者位置を表さない。
- Screenのvertical FOVは共有60°、VRは眼別projectionを使用する。
- forwardだけでrollを決めず、正規直交basisまたは単位quaternionを使用する。

```math
T_{world,view}=T_{world,aircraft}T_{aircraft,pilot}T_{pilot,head}T_{head,eye}
```

- 描画basisはFRD/NEDの$(forward,right,down)$を$(right,-down,-forward)$へ写す。
- 位置と姿勢に同じbasisを使い、$q_{view}=q_{basis}q_{FRD}q_{basis}^{-1}$で変換する。
- basis quaternionはwxyz順で$(0.5,0.5,0.5,-0.5)$とする。
- 眼別offsetはstereo境界で一度だけ適用する。

- Briefing/Countdownはsealed configurationの初期状態、Title/Setupは会場由来の展示姿勢を使用する。
- 展示からphysics tickやFlight telemetryを生成しない。
- cameraは数値snapshot、view metadata、視線入力、camera時刻から数値poseを導出し、coreをstepしない。
- nose、wing/cockpitの横線と地平線を同時に視認できる構図を維持する。
- head trackingへcinematic smoothingやhead bobを適用しない。

- Replay/AttractはAuto、Pilot、Chase、Orbit、Platform、Shore、Overhead、Side、Front、Telephotoを扱う。
- Autoは共有CameraDirectorを選ぶmodeである。
- 手動選択では自動cutを停止する。
- VRの自動選択はPilotを維持し、明示した外部rigへはhead trackingを局所姿勢として合成する。

- 定点rigはworld assetのversion・origin・hash付きcamera_pointsを使用し、追従rigはrecord時刻から導出する。
- Telephotoの登録位置はN=300 m、E=700 m、水面上80 mであり、表示用の地形との視線条件を基にした配置である。
- 実測camera塔や任意軌跡の無遮蔽を保証する値として使用しない。

- 定点の向きは角速度を制限する。
- 初期profileの120°/sとrecord sample時刻の球面補間を用い、FPS・seek履歴へ依存させない。
- Chaseのspring-damper等はcamera専用の解析更新または固定刻みを使用し、physicsを変更しない。
- seek・巻戻し・cutでcamera履歴を再初期化する。
- 退化したlook方向、近接、水面・地形との交差には有効なfallbackを使用する。

- Directorはlaunch、距離、高度、終端event、record時刻からshotを選び、優先順位、最短継続時間、再発火条件を持つ。
- 乱数を使用する場合はseedとversionを明示する。
- Cinematic・Telemetry・Analysisの表示modeを持つ。再生開始時はCinematicを選択し、技術情報を閉じる。camera選択と分析cursorを保持する。

## Attract・Credits

- Attractは独立したdemo recordとDirectorを使用し、player session、Personal Best、Replay履歴を更新しない。
- 利用者入力でTitleへ戻り、同じ入力をFlight開始へ再使用しない。
- overlay中は無操作timerを停止する。

- Creditsは登録済みassetと第三者licenseから生成する。
- 候補データを使用済みと表示せず、[THIRD_PARTY_DATA.md](../THIRD_PARTY_DATA.md)と配布物の帰属を一致させる。
- 保証対象の試験・形式証明・実機受入の区分は[検証](verification.md)と[形式検証](formal-verification.md)に従う。

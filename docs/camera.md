# Camera subsystem契約

## Pilot view

通常FlightはPilot viewを既定とする。BPG-007の最初のbrowser接続からこの規則を適用する。
視点は機体構造datumからの基準body offsetと取付姿勢を持つPilotEyePointとしてモデルmetadataへ保存する。
身体の前後移動による眼位置変化は、`pilot-motion.md` の実身体位置から導出する。
初期眼位置 $r_{eye,0}^B$ と初期身体位置 $x_{p,0}$ をmetadataへ記録し、
$r_{eye}^B=r_{eye,0}^B+(x_p-x_{p,0},0,0)$ とする。Replayも記録された実身体位置を使用する。
forward vectorだけではroll方向が決まらないため、正規直交basisまたは単位quaternionを用いる。
PilotEyePointは左右眼の中点を表し、眼別offsetはpresentation backendが一度だけ適用する。
Pilot viewにはnoseとwing/cockpit横線をbody-fixedな姿勢基準として含め、外界の地平線・対岸稜線と同時に見える構図を保つ。可動camera装飾や過度な視野遮蔽でこの基準を失わない。

```math
T_{world,view}=T_{world,aircraft}\,T_{aircraft,pilot}\,T_{pilot,head}\,T_{head,eye}
```

FRD/NEDから描画座標への変換はadapterで完了させる。
cameraはRenderSnapshot、view metadata、view入力、camera用時刻を受け取り、coreをimport・呼出ししない。
engine非依存の数値poseを生成し、Three.js等のcameraや数学型を公開しない。
機体姿勢を直接反映し、強い追従遅延を追加しない。
head poseとstereo/XR変換は `presentation.md` に従う。
景観・操作panel・cursorの参照系は `vr-spaces.md` に従う。

BPG-007のsynthetic aircraftは `SYNTHETIC_PILOT_EYE_POINT` を明示的なview metadataとして使用する。
実機・別機体modelを追加する際は、各modelのPilotEyePoint metadataへ置き換える。

Informationに許可cameraとvisual stabilization profileを含める。
Fullでは明示設定でchaseを許可できる。Standard/Minimal/RealisticはPilotのみとする。
すべてのpresetの既定はPilot、安定化offとする。Customは有効な組合せだけを受け付ける。
Flight中の許可camera間の切替は許可するが、許可集合と安定化profileは飛行開始前に固定する。
許可集合をPersonal Bestの比較keyへ含め、実際のcamera切替をpresentation eventとして記録する。
Replay/Attractのcamera選択は元フライトの記録・比較keyを変更しない。

Pilotの安定化を追加する場合は視覚的な微小振動抑制に限定し、physics stateへ反映しない。
頭部追跡へcinematic smoothingやhead bobを適用しない。XR/Phoneの初期版では安定化offを維持する。
backend間で同じ設定を再現できない場合はFlightSetupで非対応と表示し、飛行中に黙って補正しない。

## CameraModeとGameScene

| Rig | Flight | Replay | Attract |
|---|---|---|---|
| Pilot | 既定 | 任意 | 任意 |
| Chase | Full/Customで許可時のみ | 手動の既定 | あり |
| Orbit | なし | 手動観察 | なし |
| Platform | なし | あり | あり |
| Shore | なし | あり | あり |
| Overhead | なし | あり | あり |
| Side tracking | なし | あり | あり |
| Front tracking | なし | あり | あり |
| Telephoto shore | なし | あり | あり |

AutoはCameraDirectorを選択するmodeであり、独立したcamera projectionではない。
ReplayはAutoと手動rigを選択でき、手動選択時には自動cutを停止する。
ReplayではScreen/VRの両方でcamera rigを手動選択できる。VRの自動選択はPilotを維持し、手動選択した外部rigではhead trackingをcamera rigの局所姿勢として適用する。Chase poseはengine非依存のFRD座標で定義する。
BPG-022の合成worldでは、定点rigの位置をversion・origin・camera_pointsのSHA-256を持つworld metadataに登録する。追従rigとOrbitは記録時刻から位置を導出し、seek後も同じ時刻で同じposeへ戻る。
CameraModeごとのGameSceneを作らない。実装は `web/src/render/camera/` に配置する。
定点rigの位置とIDはworld assetの `camera_points` に保存し、origin/version/hashを共有する。
Platform/shore/telephotoのtarget追従は共通rigのパラメータとして扱う。
機体追従rigにも位置offset、FOV、追従係数、利用範囲を持たせ、無意味なclass細分化を避ける。

## Cinematic dynamics

定点cameraは位置を固定し、機体への向きに角速度制限を設ける。
合成worldの初期profileでは角速度の上限を120度/sとし、FlightRecordのsample時刻をcheckpointとして球面補間する。描画FPSとseek順序に依存させない。
Chaseは位置のspring-damperと注視方向の制限を用いてよい。

```math
\ddot x_c=k(x_{target}-x_c)-c\dot x_c
```

kとcの単位・適用範囲をprofileへ定義する。描画FPSに依存する単純Euler更新を避け、
解析更新またはcamera専用の固定刻みで安定性を検証する。
これはvisual dynamicsであり、physicsの積分器やtick数を変更しない。
seek・巻戻し・cutではcameraの内部状態を再初期化する。
Autoはrecord時刻とeventから再現可能なshotを選ぶ。seek後の履歴依存を防ぐため、
shot境界の初期camera stateから再構成するか、検証済みcheckpointを用いる。

TelephotoのFOV 15–30度は初期の画角候補とする。
遠近関係は視点位置で決まるため、望遠的構図はcamera距離とFOVを組み合わせて設計する。
近接点・水面・地形との交差、zero-length look direction、画面外逸脱には有効なfallback shotを用いる。
地形が存在する条件ではworldの描画用queryを利用してよいが、physics stateは変更しない。

## CameraDirectorと表示

launch、distance閾値、低高度、終了event、record時刻からshotを選択する。
各shotに優先順位、最短継続時間、再発火条件、cut/fade方式を保持する。
固定秒数だけに依存せず、短いフライト・中断・着水でも終端shotへ到達する。
AttractはPlatform→Side/Chase→Shore/Telephoto等のsequenceを使用する。
乱数選択を導入する場合はseedとdirector versionを明示する。

ReplayのViewModeはCinematic / Telemetry / Analysisとする。
Cinematicは距離・時刻を中心に表示し、Analysisでは軌跡・風・力・操舵等を重ねられる。
追加diagnosticのないrecordでは該当表示をunavailableとする。再生中に値を捏造しない。
Replay/Attractの自動cutはDesktopを基本対象とし、XR/Phoneでは明示選択までPilotを維持する。
外部視点のXR表示を提供する場合はhead trackingを維持したrigとして検証し、通常cameraで上書きしない。

## 検証

Pilotの位置offset、basis、機体回転合成、IPD二重加算、camera許可集合、視覚補助の条件保存を検証する。cockpit/nose/wingのbody-fixed方向と地平線の相対姿勢がpitch・rollに一致することも確認する。
Cinematicは30/60/120 FPS、seek・巻戻し、手動選択、短いrecord、定点追従、最短shot時間を検証する。
camera変更がphysics state・保存済みFlight recordを変更しないことを確認する。

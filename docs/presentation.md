# Presentation backendとVR契約

## 適用範囲

Desktop、native WebXR、Phone VRをWeb側のpresentation backendとして分離する。
physics・WindField・FBW・操縦authorityは全backendで共通とする。
本書はBPG-001の設計契約であり、VR機能の実装・実機確認はBPG-014〜016で行う。
head trackingは視線入力であり、操舵入力へ自動的に割り当てない。
PilotEyePoint・camera許可・Replay/Attractのrigは `camera.md`、主要Sceneとoverlayは `game-flow.md` に従う。
Desktopというbackend名は通常のScreen表示を表し、スマートフォンの単眼表示も含む。
追従規則は `vr-spaces.md`、engine交換境界は `render-boundary.md` に従う。
本書のThree.js APIは初期engine adapterの実装候補である。

| Backend | 表示・追跡 | 実行系 |
|---|---|---|
| Desktop | 単眼、Flight既定はPilot、mouse等の視線操作 | WebGLRenderer |
| WebXR | immersive-vr、runtime提供の眼別pose/projection | WebXRManagerとXR session |
| Phone VR | 左右分割、端末姿勢による3DoF | DeviceOrientationEventとStereoEffect/StereoCamera |

WebXRで利用可能なtracking自由度は端末に依存する。controllerやhand trackingは初期要件に含めない。
Phone VRは位置追跡・reprojection・XR compositorを提供しない。
WebXR失敗時にPhone VRを選択でき、両方が使用不能ならDesktopを継続できる。

## 全Scene共通のScreen/VR UI

Boot、Title、FlightSetup、Briefing、Countdown、Flight、Result、Replayのすべてと、
Settings・Pause・Help・Credits・HUDをScreen/WebXR/Phone VRで表示・操作できることを必須とする。
「FlightのみVR」は完了条件を満たさない。GameScene変更でXR sessionを終了しない。
presentation modeはGameSceneと独立した状態であり、session lifecycleは全Sceneの外側で管理する。

共通のUiViewModelとUiActionから、Screen用DOM/SVGとVR用panelを生成する。
画面遷移、validation、設定値、focus、disabled状態は共有し、表示adapterへゲーム規則を複製しない。
VR panelは両眼で読める文字・距離とhit領域を持つ。共通layout/drawing commandをCanvas等へ描き、
texture化したpanelへ適用できる構成とする。通常DOMの自動取り込みは前提にしない。
graphも同じrecord・軸・cursorモデルからVR panelへ描く。VR表示専用の集計を作らない。

DOM Overlayは選択機能であり、対応機種でのみ使用する。非対応でも主要操作をVR panelで完結させる。
根拠: [WebXR DOM Overlays仕様](https://immersive-web.github.io/dom-overlays/)。
DOMとXR入力を併用する場合は同じ選択が二重発火しないよう共通actionで処理する。
Screenはkeyboard/pointer/touch、VRはgamepad focus操作またはhead-gazeによる選択を基本とする。
head-gazeには選択確定時間と取消を設ける。XR controllerがあればray/selectを同じactionへ変換してよい。
tracked controllerとhand trackingを必須にしない。装着中にスマートフォンへのtouchを要求しない。
hoverだけに依存せず、数値変更、tab、scroll、graph cursor、戻る、pause、終了を操作できるようにする。

Bootでも、最小rendererとVR入口が準備できた後はWASM・assetの読み込み中表示をVRで継続できる。
初回XR session開始やsensor権限は実際の利用者操作を要するため、ブラウザの権限UIと最小bootstrapはScreenに置く。
根拠: [WebXR Device API](https://immersive-web.github.io/webxr/)。
renderer/session自体が利用不能な場合はScreenで原因を表示する。対応するVR実行系がある状態で
BootやResultのためだけにScreenへ戻すことを禁止する。
XR終了・tracking喪失では現在のGameSceneを保持し、Flight/Countdown/Replayは停止して復帰先を表示する。

BPG-014〜016をBPG-001直後に実施する。全Sceneのview model fixtureで表示・選択・復帰を検証し、
後続のゲーム進行、HUD、Analysis、Replayの各PRでもScreen/VR双方の受入試験を必須とする。
fixtureは表示基盤の検証データであり、未実装physicsの代用としてゲームへ組み込まない。

## 状態とcapability

backend種別、capability、session状態、tracking状態を独立したdiscriminated unionとする。
状態はidle / requesting / active / stopping / failed、trackingはvalid / unavailable / staleを基本とする。
start/stopに加え、capability調査、resize、recenter、frame描画、disposeの責務を持つ。
非同期開始と終了の競合、二重開始、session end、permission denied、context lossを明示的に扱う。
停止時にlistener、animation callback、render targetを解放する。

`navigator.xr` の存在確認後、`isSessionSupported('immersive-vr')` のfalseとrejectを処理する。
support確認の成功はsession開始成功を保証しない。
参考: [WebXR仕様](https://immersive-web.github.io/webxr/)、
[isSessionSupported](https://developer.mozilla.org/en-US/docs/Web/API/XRSystem/isSessionSupported)。

Phone VRではAPI存在、許可状態、有効な姿勢eventの受信を別々に判定する。
nullや非有限値を0度として採用しない。受信timeout・stale判定を設ける。
センサーが利用できない場合は理由を表示し、再試行・Desktopへの復帰を可能にする。
sensor権限とXR sessionは利用者の明示操作から要求する。
非同期capability調査は開始操作より前に実施し、await後にuser activationが残ると仮定しない。
別backendへ切り替える際も、必要な権限要求を新たな開始操作から実行する。

fullscreenとlandscape lockは補助機能とする。非対応・拒否・中断を処理し、
手動の横向き配置や通常表示へ復帰できる。複数のAPIがuser activationを消費し得るため、
センサー許可後に必要なら別の明示操作でfullscreenを要求する。
根拠: [Device Orientation and Motion](https://www.w3.org/TR/orientation-event/)、
[Screen Orientation](https://w3c.github.io/screen-orientation/)。

## 視点の階層

```text
scene
└── aircraft presentation root（補間済みphysics pose）
    └── cockpit mount（構造datumからの基準offsetと身体位置による移動）
        └── tracking origin（基準化済みtracking空間）
            └── head pose
                └── view cameras
```

```math
T_{world,eye}=T_{world,aircraft}\,T_{aircraft,cockpit}\,
T_{cockpit,tracking}\,T_{tracking,head}\,T_{head,eye}
```

Phone VRではheadの並進を固定し、基準化した頭部回転を合成する。
deviceorientationのZ-X'-Y''回転、度からrad、端末axes、screen orientation、
camera前方軸を明示的に変換する。alpha/beta/gammaを航空機のyaw/pitch/rollへ直接代入しない。
絶対方位を常に取得できるとは仮定せず、利用者の正面を基準にrecenterする。
基準姿勢・画面方位補正後のquaternionに対して、例えば次の相対回転を用いる。

```math
Q_{head}(t)=Q_{calibrated}(t_0)^{-1}Q_{calibrated}(t)
```

基準姿勢は利用者が正面・水平を確認した時点で取得する。
画面回転時にはtracking基準を再評価する。符号・積順序・recenter前後の連続性を試験する。
sensor姿勢は現実の端末空間に属する。航空機の仮想姿勢をsensor補正へ重複適用しない。
WebXRではreference spaceの原点・高さとcockpit mountを対応付け、床高やIPDを二重加算しない。
XR管理cameraのpose/projectionをPhone VRの値で上書きしない。

## Stereoと光学profile

Three.js公式の [StereoEffect](https://threejs.org/docs/pages/StereoEffect.html) と
[StereoCamera](https://threejs.org/docs/pages/StereoCamera.html) を候補とする。
2026-09-27確認時点でStereoEffectはWebGL向けaddonとして提供され、
StereoCameraのeyeSep既定値は0.064である。本ゲームは1 unit=1 mとし、IPDはmで管理する。
0.064 mは調整可能な初期値であり、全利用者・viewerへの適合値とは扱わない。

[公式実装](https://github.com/mrdoob/three.js/blob/master/examples/jsm/effects/StereoEffect.js)
は左右viewportへの描画を行い、内部StereoCameraのaspectを0.5とする。
親cameraのaspectはcanvas全体のwidth/heightとし、二重に半減させない。
描画前に親階層のworld matrixを更新し、終了時にviewport/scissor等を復元する。
StereoEffectはlens distortion補正を含まない。

[StereoCamera実装](https://github.com/mrdoob/three.js/blob/master/src/cameras/StereoCamera.js)
はoff-axis projectionを構成する。eyeSepだけでなくfov、focus、near/farの意味と値をprofileで規定する。
既定focusをviewer用の適合値とみなさない。眼別FOVや非対称lens中心が必要な場合は、
StereoEffectを直接拡張せずbackend内部の眼別render targetと補正passへ切り替えられる構造にする。

ViewerProfileにはversion、単位、IPD、眼別FOV、lens center、projection/focus設定、
distortionモデルを保持する。distortionはdisabled / radial等のunionとし、係数の座標基準・単位を定義する。
初期実装は補正なしの実験表示を許容する。特定viewerで光学適合を確認するまで、
そのviewerへの正式対応を表示しない。profile databaseとpolyfillへの依存は初期要件に含めない。
DeviceOrientationControlsは使用しない。
[Three.js migration guide](https://github.com/mrdoob/three.js/wiki/Migration-Guide#133--134)ではr134での削除が記録されている。

## 時間・性能・操作

rendererのanimation loop所有者は常に1つとする。backend切替でphysicsを二重stepしない。
physicsは100 Hzを維持し、snapshot補間とhead pose取得を別に扱う。
XRではruntimeのframe schedulingと眼別projectionを使用し、tracking poseは描画直前に反映する。
Desktop/Phoneの画質目標とXRのrefresh rateを区別する。
Auto品質はbackend別のframe budgetを使用し、XRで16.67 msを固定基準にしない。
reflection・shadowは両眼共有の可否を判断し、眼別水面reflectionの不整合を試験する。
post-processingとrender target解像度をbackendごとに計測し、physics品質を変更しない。

Phone VRでは開始・recenter・終了を装着前後に操作できる導線を設ける。
操縦にはgamepadまたはautomaticを利用できる。頭部姿勢を操舵として使用する機能は別途設計する。
HUDとメニューは左右眼で読める距離・位置へ配置する。

## 検証と完了条件

- BPG-014: engine境界、全Sceneのview model、anchor別追従、backend切替、単一loop、transform合成、resource解放。
- BPG-015: WebXR capability/reject/session end、reference-space reset、pose/projection、実HMD。
- BPG-016: 権限許可・拒否、event未受信/null/stale、縦横画面、resize、左右aspect、IPD/FOV、実スマートフォン。
- BPG-012: 二眼描画負荷、XR refresh rate、thermal負荷、画質振動、physics一致。
- BPG-013: HTTPS、permission導線、終了復帰、backend別creditsと対応端末一覧。

モック試験と実機確認を区別する。HMD、スマートフォン、browser、OS、viewerの組合せと
補正有無を記録し、未検証の組合せへの互換性を保証しない。

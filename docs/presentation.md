# Presentation backendとVR契約

## 適用範囲

Desktop、native WebXR、Phone VRをWeb側のpresentation backendとして分離する。
physics・WindField・FBW・操縦authorityは全backendで共通とする。
本書はBPG-001の設計契約であり、VR機能の実装・実機確認はBPG-014〜016で行う。
head trackingは視線入力であり、操舵入力へ自動的に割り当てない。
FlightのPilot固定cameraとReplay/Attractのrigは `camera.md`、主要Sceneとoverlayは `game-flow.md` に従う。
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

単眼DOMの可視性は`PresentationUiState`から純粋関数で導出し、Sceneと独立に適用する。

| Presentation状態 | Screen DOM / Flight HUD |
|---|---|
| uninitialized / initializing / failed | Screen DOMを表示する。HUDはFlight/Pauseだけで表示する |
| ready(Screen) | Screen DOMを表示する。HUDはFlight/Pauseだけで表示する |
| requesting、from=Screen | 権限要求中のScreen DOMを維持する |
| requesting、from=VR/null | 非表示とする |
| starting / stopping | from/toを問わず非表示とする。BFCache復帰のfrom=nullも含む |
| ready(WebXR/Phone VR) / cached / hidden | 非表示とする |

VR起動の副作用を実行する前に単眼DOMを隠し、Screenへの復帰完了後に表示する。
Snabbdomの安定mountへ`hidden`・`inert`・`aria-hidden`を反映し、専用CSSで`display: none`を指定する。
author CSSの`display: contents`が`hidden`の表示抑制を上書きしないようにする。
根拠: [HTML hidden](https://html.spec.whatwg.org/multipage/interaction.html#the-hidden-attribute)、
[HTML inert](https://html.spec.whatwg.org/multipage/interaction.html#the-inert-attribute)。
非表示中も最新Viewをpatchし、Flight HUDの既存nodeとsnapshot更新・render通知を保持する。
DOMのclick/change/inputは非表示中に拒否する。表示復帰時にlistenerを再登録しない。
focusとscrollはbrowser adapter内で非表示への遷移時に一度だけ退避する。
同一Scene/overlayへ戻るときにscrollを復元し、focusは同一の接続中・有効なnodeに限って`preventScroll`付きで復元する。
非表示中に別要素へ移動したfocusと、除去・無効化された要素のfocusは復元対象から除外する。

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
BPG-015ではnative WebXR sessionへ接続する。BPG-017でgame flowを実flight stateへ接続する際、
HUD、Analysis、Replayを含むScreen/VR双方の受入試験を必須とする。
fixtureは表示基盤の検証データであり、未実装physicsの代用としてゲームへ組み込まない。

## 状態とcapability

backend種別、capability、session状態、tracking状態を独立したdiscriminated unionとする。
状態はidle / requesting / active / stopping / failed、trackingはvalid / unavailableを基本とする。
Phone VRのvalidは最後に受理した姿勢を保持することを表す。
start/stopに加え、capability調査、resize、recenter、frame描画、disposeの責務を持つ。
非同期開始と終了の競合、二重開始、session end、permission denied、context lossを明示的に扱う。
停止時にlistener、animation callback、render targetを解放する。

Boot UIは単一の不変Model、純粋なupdate、Modelから導出するview、副作用effect portを用いる。
permission要求・backend切替は一件ずつ直列化し、完了Messageにはrequest IDを付ける。
古い非同期完了はModelを更新せず、取得済みsession等の資源を解放する。
切替対象の起動に失敗した場合、停止済みのVR backendを再起動せずScreenを開始する。
Screen復帰も失敗した場合はactive backendなしを明示し、render loopを停止する。
unexpected session end・tracking喪失は同じ状態遷移を経てScreen復帰を行う。

`navigator.xr` の存在確認後、`isSessionSupported('immersive-vr')` のfalseとrejectを処理する。
support確認の成功はsession開始成功を保証しない。
参考: [WebXR仕様](https://immersive-web.github.io/webxr/)、
[isSessionSupported](https://developer.mozilla.org/en-US/docs/Web/API/XRSystem/isSessionSupported)。

Phone VRではAPI存在、許可状態、重力参照を持つ有効な姿勢eventの受信を別々に判定する。
API存在は対応候補の判定であり、tracking成功を表さない。Modelは許可要求中、重力参照と方位の較正待ち、
追跡中、起動失敗を区別する。nullや非有限値を0度として採用しない。初回姿勢取得にはtimeoutを設け、取得後は最新有効姿勢を保持する。
センサーが利用できない場合は理由を表示し、再試行・Desktopへの復帰を可能にする。
sensor権限とXR sessionは利用者の明示操作から要求する。
非同期capability調査は開始操作より前に実施し、await後にuser activationが残ると仮定しない。
別backendへ切り替える際も、必要な権限要求を新たな開始操作から実行する。

## BPG-015 WebXR実装

WebXR sessionはユーザー操作内から要求し、Three.js `WebXRManager`へ接続する。reference spaceは`local`とし、
左右眼のpose/projectionとXR animation loopをruntimeから利用する。XR select rayとhead-gaze dwellは共通`UiAction`へ変換する。
`XRReferenceSpace`のresetではevent transformをengine非依存のposeへ変換してWorld/Cockpit/Menu anchorを新referenceへ写像する。
transformが取得できない場合は選択を停止し、Screenへ退出して再開始を促す。
native viewerはruntimeのcenter-head poseである。片眼やXR union cameraのposeへ置換しない。
`WebXrSessionPort.transformTrackingPose`はraw tracking poseをpresentation frameへ写像する。
Pilot Flightでは機体local frameのEye mount、外部Replayではworld frameのcamera rig、
non-flightではidentityを $M$ とし、viewerとcontroller rayへ同じ $M$ を一度だけ合成する。
Menu/Headはこのviewer frameから配置する。Cockpit/World panelはphysical frameを保持する。
Phone VRの`RendererAdapter.transformTrackingPose`は独立した入口であり、同じ $M$ を使用する。
cameraへ渡すraw head poseを維持し、Menu/Head配置とgazeへmountを一度だけ適用する。
Menu recenterはmounted headから配置し、tracking recenterは $M H^{-1}M^{-1}$ でretained anchorを写像する。
`three-renderer-panel-reference.test.ts`は実Phone backendとStereoEffectを使用し、
3身体位置・全anchor・gaze dwell・recenter・Screen復帰の同一basisを検査する。
WebGL driverを代替したソフトウェア試験であり、実スマートフォンの受入は別途実施する。

Three.jsはXR cameraのlocal poseをruntime値へ更新するため、Eye mountをcameraの親tracking originへ保持する。
各眼のworld poseは $A E H_{eye}$ となり、身体移動、runtimeのhead poseとIPDを各一度だけ含む。
Screen/Phoneの既存 $A E H$ と、外部Replayの $C H_{eye}$ を維持する。
根拠: [Three.js WebXRManager](https://threejs.org/docs/pages/WebXRManager.html#updateCamera)。

reset eventの $R$ は新native originを旧referenceで表したposeである。
portのreset callbackにはviewerと同じpresentation frameで $M R M^{-1}$ を返し、
backendはその逆をretained anchorへ適用する。旧panel $P$、旧viewer $H$ は次を満たす。

```math
(M R^{-1} H)^{-1}(M R^{-1} M^{-1}P)=(M H)^{-1}P
```

根拠: [WebXR XRReferenceSpaceEvent](https://www.w3.org/TR/webxr/#xrreferencespaceevent-interface)。
`three-webxr-mount.test.ts`は実WebXRManagerと模擬XRSession/XRFrame/XRWebGLLayerを接続し、
両眼、3身体offset、全anchorのgaze/controller、回転と並進を含むreset、session終了後のScreenを検査する。
GPU driverと非対象のvenue I/Oを代替する。runtime合成経路の回帰と実HMD受入を区別する。
全Scene/overlayのfixture、自動session lifecycle試験、production buildは実施可能である。HMD/browser上のpose・projection・操作確認は
対象実機未確保のため未実施として扱い、実機検証完了までBPG-015を完了扱いしない。

fullscreenとlandscape lockは補助機能とする。非対応・拒否・中断を処理し、
手動の横向き配置や通常表示へ復帰できる。複数のAPIがuser activationを消費し得るため、
センサー許可後に必要なら別の明示操作でfullscreenを要求する。
根拠: [Device Orientation and Motion](https://www.w3.org/TR/orientation-event/)、
[Screen Orientation](https://w3c.github.io/screen-orientation/)。

## BPG-016 Phone VR実装

Phone VRはsecure context上の`DeviceOrientationEvent`と`ScreenOrientation`を使用する。
センサー許可要求は開始操作の同期区間で`requestPermission(true)`を呼び出し、absolute orientationに必要なmagnetometerも要求する。
開始時にstereo資源を準備し、重力参照と非退化の水平方位を較正するまでbackendの開始成功と描画を保留する。
`alpha`・`beta`・`gamma`のnull、非有限値、初回event timeout、不正sample時刻を失敗状態として扱い、0度へ置換しない。
画面角度はScreen Orientation仕様の自然向きからのcounter-clockwise角としてZ軸補正へ適用する。
重力で水平を定義し、最初の較正可能なsampleと明示的なrecenterではyawだけを基準化する。初回pitch/rollも保持する。
tracking喪失時はstereoを解除しScreenへ復帰する。

browser adapterは`deviceorientation`と`deviceorientationabsolute`のうち最初に`absolute=true`を受け取った経路を選択し、
そのsession中は別経路を混合しない。Earth参照系の上向きを重力の根拠とし、選択経路の参照喪失は失敗として扱う。
`absolute=false`の任意参照系には上向きを推定しない。relative-only browserでは較正待ちの後にtimeoutとなり、
失敗理由付きでScreenへ復帰する。これはPhone VR開始成功として扱わない。
独立sensor portは同じsampleと整合し、session中に固定された`relative-reference-up`を供給できるが、現browser adapterはこのsourceを提供しない。
`webkitCompassHeading`だけで水平を保証しない。`DeviceMotionEvent`の`accelerationIncludingGravity`と`acceleration`の差分を
利用する経路は、欠測、符号、姿勢sampleとの同期と参照系の一貫性を検証する残作業である。
根拠: [Device Orientation参照系・権限・Device Motion](https://www.w3.org/TR/orientation-event/)、
[Apple DeviceMotionEvent](https://developer.apple.com/documentation/webkitjs/devicemotionevent)。

`deviceorientation`は有意な姿勢変化を通知し、freshness維持の追加通知は任意である。
有効sample取得後の無通知だけでは、静止と通知経路の停止を識別できない。heartbeat間隔は仮定せず、
通知がない間は最新有効姿勢を保持する。明示的な不正sample・画面方位の喪失ではtrackingを停止し、
利用者の終了操作とpage/session lifecycleによるlistener・stereoの解放も維持する。
このAPIによる無通知hardware failureの検出は保証しない。
根拠: [Device Orientation §6.1](https://www.w3.org/TR/orientation-event/#deviceorientation)。

sample時刻は`DeviceOrientationEvent.timeStamp`を保持する。受信境界では同一Window/time originの
単調時計`performance.now()`を用い、受信時刻・sample時刻の有限性と非負性、sampleが未来を指さないこと、
前回sampleからの非減少性を検査してから姿勢を更新する。試験用の`nowMs`注入も同じtime originを要する。
rendererのrAF時刻はrendering opportunityを示し、callback直前に受信したsampleより古い場合がある。
その時刻差をtracking障害と判定しない。rAF時刻は描画とdwellに使用する。
同一sample時刻と受信時刻に等しいsampleは許容する。受信後の経過時間は失敗条件に含めない。
根拠: [DOM event生成時刻](https://dom.spec.whatwg.org/#concept-event-inner-create)、
[High Resolution Time](https://www.w3.org/TR/hr-time-3/#dom-performance-now)、
[HTML rendering update](https://html.spec.whatwg.org/multipage/webappapis.html#update-the-rendering)。

head-gazeは既存のdwell selectorを使用する。標準mappingのGamepadが接続されている間は、左stickをpanel cursor、
button 0を選択、button 1を戻る、右stickをscrollへ対応付ける。Gamepad入力とhead-gazeが同時に同一controlへ
actionを発火させない。これらは共通`UiAction`へ変換する。標準mappingでないGamepadは選択対象とせず、head-gazeを継続する。
mappingは[W3C Gamepad API](https://www.w3.org/TR/gamepad/)のStandard Gamepad配置に従う。
初期optical profileはIPD 0.064 m、vertical FOV 60°、focus distance 10 m、distortion disabledとし、
`generic-unverified-v1`として明示する。これらの値はviewer適合を保証しない。

自動試験は全Scene/overlay fixture、permission拒否、初回event欠落、不正sample、無通知中の姿勢保持、screen rotation、recenter、listener解放、
head-gazeと標準Gamepadのfocus/選択/back/scrollを対象とする。実スマートフォン、browser、viewerでの表示・操作・
Screen復帰は未確認であり、実機受入完了までBPG-016を完了扱いしない。fullscreen・screen lockと非標準Gamepadは
現行実装の対象外である。

sensor browser adapter・backend・runtime・App updateを接続する模擬API試験では、rAF時刻より新しいsampleを受理し、
1秒・60秒の無通知中もPhone VRと最新姿勢を維持する。次のsample・画面回転・recenterを適用し、
明示的な不正sampleでScreenへ復帰する経路を検査する。
この試験は実スマートフォンで報告された即時Screen復帰の原因確定と受入確認を代替しない。

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
端末の自然な画面方位に固定された姿勢を $D=R_z(\alpha)R_x(\beta)R_y(\gamma)$、
`ScreenOrientation.angle`を $s$、重力と初回水平方位から求めたtracking基底を $B$ とすると、head回転は $H=B D R_z(-s)$ とする。
補正は端末姿勢の右から一度だけ合成する。画面方位を変更してもsensorの端末座標系は変化しない。
根拠: [Device Orientation §3.1](https://www.w3.org/TR/orientation-event/#device-orientation)、
[Screen Orientation angle](https://w3c.github.io/screen-orientation/#dom-screenorientation-angle)。
`three-renderer-phone-orientation.test.ts`は0°・±90°・180°・270°について、実sensor adapter、
Phone VR backend、Three.js camera、StereoEffectを接続し、左右・上下・rollの回転方向と
固定世界点の両眼投影、画面回転、yaw-only head/Menu recenter、非零初回pitch/roll、全Sceneの非Flight水平復帰を検査する。
Phone非FlightはTitleの位置・yawを維持してpitchを0°とし、ScreenのTitle pitch −6°を維持する。
Flightは既存の航空機・Eye mount・headの積 $A E H$ を維持する。WebGL driverには記録用代替を使用する。
この検査は描画driverの実GPU動作と実スマートフォンのsensor精度・装着時受入を保証しない。
較正とrecenterは利用者の水平方位を基準とし、北向きをゲームの正面へ固定しない。
manual recenterの基底差分はtrackingの上向き軸周りの回転となる。

```math
B_{new}=R_y(-\psi) B_{old},\qquad H_{new}=R_y(-\psi)H_{old}
```

pitch/rollを基準化しない。初回方位が退化すると較正を保留し、追跡中の上向き・下向きでは基底を維持する。
退化したmanual recenterは直前の基底を維持する。画面回転は同じ基底へ右側補正を適用する。
recenterの基底差分をEye mountで共役変換してWorld/Cockpit/Menuへ適用し、cameraとpanelの相対関係を維持する。
符号・積順序・recenter前後の連続性を試験する。
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
各animation冒頭で前フレームのpending描画入力を適用し、同フレームのraw head poseと眼別projectionを捕捉する。
続くphysics callbackはcamera選択とintrinsicsを維持する非null poseだけを即時更新する。
Runtimeはphysics後に`beginViewFrame()`を呼び、以後のview導出、canvas計測・描画、gaze、3D描画を同じ入力で完了する。
camera切替、FOV、stereo、viewport、poseの明示取消は捕捉後に適用せず、次animationへ送る。
view開始後のpose・lake入力も次animationへ送る。pendingとidleの新setterは最後の入力を優先し、
例外時もprivate frame phaseを解放する。pendingはstop/startをまたいで次animationまで保持し、disposeで破棄する。
animation外のnative selectは最後に描画した操作panelとmountを使う。未描画入力をrayへ混合せず、
初描画前、操作panelの非表示、loop停止、session終了時はselectを無効にする。
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
- BPG-016: 権限許可・拒否、初回event未受信、不正sample、無通知中の姿勢保持、縦横画面、resize、左右aspect、IPD/FOV、実スマートフォン。
- BPG-012: 二眼描画負荷、XR refresh rate、thermal負荷、画質振動、physics一致。
- BPG-013: HTTPS、permission導線、終了復帰、backend別creditsと対応端末一覧。

モック試験と実機確認を区別する。HMD、スマートフォン、browser、OS、viewerの組合せと
補正有無を記録し、未検証の組合せへの互換性を保証しない。

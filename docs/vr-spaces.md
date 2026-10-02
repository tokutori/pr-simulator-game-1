# VRの参照座標系と追従規則

## 用語と基準

「追従」は対象の座標変換を変更する処理と定義する。
頭部回転に応じて背景の見える方向が変わることは、眼のpose更新による投影変化である。
worldに固定した地形・湖面・雲の姿勢を頭部回転へ同期させない。
各表示要素はanchor種別を明示する。

| Anchor | 基準 | 頭部運動に対する挙動 |
|---|---|---|
| World | 地理的なlocal world | 対象は固定され、眼のposeに応じて見え方が変化する |
| Cockpit | 航空機と固定取付位置 | 機体に追従し、頭部からは独立する |
| Menu | menu用tracking reference内の配置位置 | 開いた位置を維持し、頭部運動で視野内位置が変化する |
| Head | 現在の頭部poseと固定offset | 視野内の位置を維持する |

interactive panelの提案既定値はMenuである。機体の実物相当計器はCockpit、非modal Flight情報板はHeadに配置する。
視野内固定を要するcontrolはHeadとして明示し、gamepad等の頭部から独立した選択方法を必須にする。
head-gazeだけでHead固定panel上の複数項目を選ぶ構成は採用しない。

## 要素ごとの規則

| 要素 | Anchor / 処理 |
|---|---|
| 湖面、湖岸、地形、world-anchored clouds | World。頭部へ追従しない |
| 太陽・無限遠の空 | 方向をWorldへ固定。sky geometryを眼の位置へ平行移動しても回転は同期しない |
| 機体、cockpit、実機相当の計器 | 機体poseと固定取付位置。計器はCockpit |
| Boot/Title/Setup/Briefing/Result/Replayのpanel | Menu。背景のcinematic cameraから独立する |
| Countdown表示 | Cockpit内の所定位置。頭部正面へ自動追従させない |
| 非modal Flight情報板 | Head。前方中央を空け、周辺へtelemetryを配置する。操作controlを含めない |
| Pause/Settings/Help/Credits | Menu。開いた時点の配置を維持する |
| head-gaze cursor | Head方向からrayを生成し、hit点へ表示する |
| controller ray | controller poseから生成する |
| tracking喪失・退出の最小案内 | Head固定を許容し、利用可能な選択方法を確保する |

flight-path marker等は定義された方向を眼へ投影し、固定panelの文字項目と区別する。
Screen HUDはviewport基準であり、VRのanchorをpixel位置へ直接置換しない。

## Head情報板と操作panelの分離

`UiViewModel.headHud`は`absent | visible`の排他型とする。visibleはHead anchor、local pose、
大きさ、中央clear region、背景alpha、foreground alpha、非操作の表示要素を持つ。
文字、ADI、heading、pilot position、wind、迎角、flight-pathの型を区別し、`UiControl`と`UiAction`を含めない。
flight-pathは姿勢計を非表示にしたCustom設定でも独立表示できる。interactive Menuは従来の`panels`へ残す。
HUDの背景alphaを下げる際も文字・警告・計器foregroundのalphaを独立して維持する。

clear regionと表示要素のboundsは左上原点の正規化矩形とし、要素の範囲・重複ID・clear regionとの交差を検査する。
この矩形検査に加え、profileの角寸法、左右projection、身体前後移動、head姿勢を使って実際の前方可視領域を確認する。
`BackendFrame.headHud`は表示内容とraw center-head poseを一組として保持し、tracking欠損時はabsentとする。
Phone VRは基準化したsensor姿勢、WebXRは`XRFrame.getViewerPose().transform`を使用する。
XRのuser camera用identity、片眼pose、frustum union用cameraをcenter-headの代替にしない。
表示用のworld basisはengine adapterが一度だけ合成し、Flightでは機体・PilotEye・head・HUD localの順とする。
左右眼のIPDはstereo projectionだけへ適用する。外部rigと非Flight背景基底もheadとの合成を一度だけ行う。

Three.jsのHUD surfaceは独立したCanvasTextureとmeshを所有し、transparent、opacity 1、depthWrite false、
depthTest falseで描画する。背景alphaとforeground alphaはcanvas側で別に適用する。
HUDを水面反射用layerへ追加せず、終了時はtexture・material・geometryを一度ずつ解放する。
非表示時もmeshを保持し、変更された不変viewだけをtexture更新の入力にする。

`createHeadHudView`は同一の`FlightHudModel`と当該frameの`ViewerFrame`から、
`absent | unavailable(reason) | visible(layer, textHeightMeters)`を純粋に導出する。
Information全5設定を共用し、Customの各cueを独立に扱う。全cueを意図的に非表示にしたabsentと、
geometry・可読領域の不足によるunavailableを区別する。物理値の正本とInformation設定を変更しない。

layoutの設計値は距離2.4 m、最大幅2.16 m、最大高2.4 m、前方中央の接平面角で左右各15°・上下各10°である。
上下のbandへreadoutと計器を配分し、狭い左右視野では列数と上下配分を変える。
距離と中央空白角を保ち、実際の左右view/projectionでplane四隅がclip margin内へ収まる大きさを探索する。
収容できない場合は要素や中央空白を縮小して継続せず、理由を持つunavailableを返す。
各眼の光学軸から左右各15°・上下各10°の保護錐をHead planeへ交差させ、頭部接平面の保護領域との和集合を上下bandから除く。
設計font emの中心位置での角寸法は0.60°とする。全非空描画行の実ink bounding boxについて、
左右端の上下ray間の角高を両眼で測り、0.35°以上かつclip内へ収まることを事前評価する。
これは個別の句読点等の最小字高や実機可読性の保証を意味しない。代表cap/x-height・数字と実rasterの可読性を別途確認する。
cardの内余白は0.5 em、card間隔は0.6 emとし、純粋な必要面積の導出とCanvasの事前評価・描画で同じ内余白を使う。

専用Canvas painterはplaneの物理aspectに合わせたpixel寸法を使う。
`prepareHeadHudPaint`が固定font・left align・middle baselineで全cardの文字を測定し、
advanceとglyphの実bounding box、改行後の行数と高さを検査して不変のpaint planを返す。
`fillText`の`maxWidth`による字形の圧縮を使用しない。
根拠: [HTML Canvasの文字描画とTextMetrics](https://html.spec.whatwg.org/multipage/canvas.html#textmetrics)。
測定はCanvas adapterの副作用境界で行う。AppModel・status・DOMへ測定結果を書き戻さない。
composition rootはframe開始時に取得したmodel・FlightSnapshot・ViewerFrameを一組として保持し、
事前評価の成功・失敗をpureなgame viewへ渡して、Head layerとMenu案内を同じ結果から確定する。
その後`drawHeadHud`がCanvas全面をclearし、成功planだけを描く。欠損metricsや文字収容失敗は部分描画を公開しない。

Flight runningのVR操作はHead情報板から分離した小型MenuのPauseに配置する。
Pause・そのSettings/Helpは通常サイズのMenuとし、Head情報板をabsentにする。
ScreenのHUDと操作は従来のDOM adapterを使用する。非Flightの全SceneもHeadをabsentとする。
小型Menuの字高・配置、Head surfaceへのゲーム接続、実WebXRManager境界、GPU視認性は後続のconsumer接続単位で検証する。
現時点のcomposition rootはHead viewを供給せず、ゲームviewの既定値はabsentである。

Menuの予約帯はMenuPlacement.openまたはrecenter時のposeと現在のHead poseが整列した場合に適用する。
この条件はFlight開始そのものを意味しない。Menuの配置はScene間で保持され、開始前の頭部運動も相対basisを変える。
整列時はHead最下段cardを小型Menuの上端から離し、両眼投影の非重畳を確認する。
任意の頭部運動ではMenuの視野内位置が変化し、操作MenuをHeadより前へ合成するため短時間の情報遮蔽を許容する。
Head自体の視野内位置と光学中心保護は維持する。全姿勢の非重畳や操作Menuの中央排除は保証しない。
通常前方姿勢、Menuへの視線移動、頭部を戻したときの情報再表示を実Canvas/GPUで検証する。
GPU、実HMD・スマートフォン受入は数値・SDK境界試験と区別する。

## 同一frameの眼別geometry

`RendererAdapter.startLoop`はtimestampと不変の`ViewerFrame`を渡す。
`ViewerFrame`は`configured`、`runtime-derived`、`unavailable`を排他的に表現する。
Runtimeは同じframeをview導出関数へ渡し、そのraw center-headをbackendへ供給する。
AppModel、DOM、window上に眼別geometryの正本を追加せず、前frameのgeometryを再利用しない。

現frameの順序は前frameのpending入力適用、geometry取得、physics・機体pose更新callback、
`beginViewFrame()`、view導出、backend frame、描画とする。
physics callbackはcamera選択とintrinsicsを変えない非null poseを同frameへ反映する。
geometry取得後のprojection設定・光学profile・camera cut・viewportとposeの明示取消は次animationへ送る。
view開始後のpose・lake入力も次animationへ送り、取得projectionと実描画の構成を同frameで一致させる。
外部XRFrameやreference spaceが欠けた場合はraw geometryを利用不能として扱い、
XR終了後の通常rAFはScreen/Phoneの構成済み光学状態から処理する。
WebXRでは一つの`XRFrame.getViewerPose()`結果からcenter transformと各viewのtransform・projectionをコピーする。
`headFromEye`はcenter transformの逆変換とeye transformから導出する。
projectionは16要素のcolumn-major行列のまま保持し、非対称frustum、眼の回転、shearをFOV単一値へ変換しない。
公開型へWebXR・Three.jsの具体型を含めない。配列とposeはコピー・freezeし、browser所有の配列を保持しない。
初期対応はleft/right各1眼の組である。入力順序を正規化し、他のview数・重複・未知の眼編成は利用不能理由を返す。
非有限値、非単位pose、退化projectionも理由を保持する。有効なcenter-headがある場合、geometry失敗時もtrackingは維持する。
参照: [WebXRのview geometry](https://www.w3.org/TR/webxr/#xrviewgeometry-interface)、
[viewer pose](https://www.w3.org/TR/webxr/#xrviewerpose-interface)。

Phone VRでは実`StereoCamera`を描画と同じcamera projection設定、aspect倍率0.5、eye separationで評価する。
fov、aspect、zoom、near/far、focusを個別に近似せず、左右projectionをコピーする。
これは構成済みのソフトウェア光学モデルであり、実端末・viewerの較正値を意味しない。
Screenおよび取得不能なXR frameは`unavailable`とする。raw sensor headは従来どおりPhone backendが所有する。
`headPlaneFitsViews`はhead基準の矩形四隅を各眼のview/projectionへ写像し、両眼のclip範囲を検査する。
eye offsetは配置検査に使用し、Head HUD meshへ追加しない。描画時のIPD適用はstereo rendererが一度だけ行う。

`viewer-frame.test.ts`は非対称・回転・shearを含む模擬XR入力、コピー、reference変換、欠損・不正入力と純粋な投影判定を検査する。
`three-renderer-panel-reference.test.ts`は実renderer adapterのanimation-loop境界と、実`StereoEffect`の両眼出力を検査する。
後者のXR portは模擬APIである。native consumerの投影証明は、#169の実`WebXRManager`を用いた回帰と
Head HUDの左右眼投影を接続する後続単位へ残る。GPU視認性と実スマートフォン・実HMD受入も独立した未達条件である。
## Panelの深度合成

`UiPanel`の合成規則はanchorから純粋に導出する。World/Cockpitはphysical、Menu/Headはoverlayとする。
physical panelは世界と同じ深度判定・深度書込みに参加する。Menu/Head panelは世界の後へ合成し、
水面・機体・透明な景観によって必須操作を隠さない。panelとcursorは世界の深度を変更せず、cursorをpanelの後へ描く。
これは合成規則であり、anchor pose、左右眼の有限距離・視差、camera、終端Flight poseを変更しない。

Three adapterはoverlay panelをtransparent queueへ入れ、depthTest/depthWriteを無効にする。
描画順は世界、非操作Head情報板、interactive panel、panel cursorの順とし、adapter内の予約順序を使用する。
世界とoverlayの親GroupはrenderOrder 0を維持し、世界objectへUI用の描画順序を流用しない。
Groupの順序は子meshの順序より優先されるため、両者を合わせて検査する。
opaque queueのrenderOrderだけで透明景観より後へ描けるとは扱わない。
同じmeshをphysical panelへ戻す際はopaque queueと深度判定・書込みを復元する。

Result Menuの可視性とdwellの初期確定は別の受入条件である。可視化だけで中立姿勢からの意図しないRetryが
解消したとは扱わず、通常entryでResult到達後に新規操作なしで滞在できるかを独立に確認する。

## Menuと入力

Menu用referenceと景観のcamera rigを分離する。
panelを開いた時点の頭部方向から配置を決定し、その後のhead poseでは更新しない。
距離、幅、文字サイズ、水平化の基準をprofileに保持する。
Titleのcamera移動、Attractのcut、Replayのseekでもpanelの位置を変更しない。
利用者の「正面へ配置」操作で再配置する。遅延した自動追従は初期版に導入しない。

menu表示中も眼のposeと選択rayを更新する。
head-gazeの確定時間を表示し、視線を外すか取消操作で確定を中止できる。
Head固定controlはgamepad focusまたは独立controller rayで選択する。
phone viewerでもtouchなしで戻る・閉じる・再配置・退出を操作できるようにする。
GameScene変更時もXR sessionを維持し、必要なpanel内容だけを更新する。

## 座標変換

共通reference Rで定義した眼と対象Oに対し、眼から見た対象は次式で求める。

```math
T_{eye,object}=T_{R,eye}^{-1}T_{R,object}
```

World/Cockpitは仮想機体・camera rigを通して眼と同じreferenceへ変換する。
Menuはbackground rigの変換を含まないUI layerとして合成できる。
左右眼のview/projectionを各々適用し、同じpanelの有限距離と視差を維持する。
head poseの逆変換をpanelへ重複適用しない。
これらのpose・anchorはengine非依存の数値データとし、scene graphへの変換はengine adapterが担当する。

Flight poseを持たないBoot/Title/FlightSetup/Briefing/Result/CreditsのPhone VRでは、景観構図用の描画基底を眼とMenu/Head panelへ一度ずつ適用する。
panel配置・head-gaze・recenterは共通tracking referenceで計算し、構図用の高度・yaw/pitchを混入させない。
眼とpanelへの共通基底を $B$、tracking reference内のposeを $H,P$ とすると、
相対poseは $(BH)^{-1}(BP)=H^{-1}P$ となり、描画とhit testが一致する。
viewport変更によるTitle構図の更新でもMenuの相対位置を維持する。
Screenの景観構図、WebXRのreference space、Flight/Cockpitの機体変換、Replayの外部rigは既存の基底を使用する。
Title基底を`transformTrackingPose`へ追加し、panel側でも再度合成する二重変換を禁止する。

head-gaze hit testは左右眼の中点に対応するhead基準、controller hit testはcontroller基準とする。
描画と同じanchor変換・frame poseを使用し、不可視・disabled・別overlayの対象を選択しない。
UI anchor状態をphysicsやFlight sampleへ混入させず、操作履歴はpresentation metadataとして記録する。

## Recenter・停止・tracking

頭部基準recenter、panel再配置、WebXR reference-space resetを別eventとする。
reference resetではanchorを新referenceへ写像して見かけの位置を保持する。
変換不能の場合は停止状態と再配置案内を明示する。
Phone VRは3DoFとして扱い、未取得の頭部並進を推定で追加しない。
physics pause中もhead trackingを維持する。視点を固定して背景を頭部へ追従させる実装は禁止する。
tracking喪失時は選択を停止し、復帰またはScreenへの退出を可能にする。

## 受入試験

BPG-014〜016で以下を検証する。実機試験と変換fixture試験を区別して記録する。

1. 頭部yaw/pitch/rollだけを変更したとき、World/Cockpit/Menuのposeは不変であり、投影が変化する。
2. 機体だけを移動・回転したとき、計器は機体に追従し、地形は移動しない。
3. Titleのcamera cutやReplay seekでMenu panelが移動しない。
4. Head固定要素は視野内位置を維持し、対応する独立入力で選択できる。
5. 左右眼の視差とhit位置、recenter/reset前後の位置関係が一致する。
6. Pause/Settings中も周囲を見回せ、physics tickとrecordは進まない。
7. 全8 Sceneとoverlayでanchor規則を満たし、graph cursor・数値変更・退出まで操作できる。

`three-renderer-panel-reference.test.ts`は実`createThreeRenderer`・Three.js scene graph・`StereoEffect`を実行し、
WebGL driverだけを描画要求の記録用doubleへ置換する。非Flight Menu/Headの両眼相対pose・IPD・投影、
Phone VR backendのgaze/recenter、Screen構図、WebXR分岐、Flight/Cockpitと外部Replay rigの変換保持を検査する。
landscapeはpanel四隅、portraitはpanel中心の視野内投影を確認する。非Flight World/Cockpitは既存変換の非回帰を確認する。
この数値結合試験に加え、実ブラウザ/GPUで両眼panel・可視controlの選択・Credits・退出を検証する。
native XRの実HMD pose/projectionとPhone viewerの光学特性は独立した実機受入項目である。

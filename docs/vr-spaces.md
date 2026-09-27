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

interactive panelの提案既定値はMenuである。Flightの計器はCockpitに配置する。
視野内固定を要するcontrolはHeadとして明示し、gamepad等の頭部から独立した選択方法を必須にする。
head-gazeだけでHead固定panel上の複数項目を選ぶ構成は採用しない。

## 要素ごとの規則

| 要素 | Anchor / 処理 |
|---|---|
| 湖面、湖岸、地形、world-anchored clouds | World。頭部へ追従しない |
| 太陽・無限遠の空 | 方向をWorldへ固定。sky geometryを眼の位置へ平行移動しても回転は同期しない |
| 機体、cockpit、実機相当の計器 | 機体poseと固定取付位置。計器はCockpit |
| Boot/Title/Setup/Briefing/Result/Replayのpanel | Menu。背景のcinematic cameraから独立する |
| Countdown表示、Flight HUD | Cockpit内の所定位置。頭部正面へ自動追従させない |
| Pause/Settings/Help/Credits | Menu。開いた時点の配置を維持する |
| head-gaze cursor | Head方向からrayを生成し、hit点へ表示する |
| controller ray | controller poseから生成する |
| tracking喪失・退出の最小案内 | Head固定を許容し、利用可能な選択方法を確保する |

flight-path marker等は定義された方向を眼へ投影し、固定panelの文字項目と区別する。
Screen HUDはviewport基準であり、VRのanchorをpixel位置へ直接置換しない。

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

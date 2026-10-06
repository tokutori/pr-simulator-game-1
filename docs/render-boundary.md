# 3D engine交換境界

## 目的

最初の3D engineはThree.js/WebGLとする。
GameScene、難易度、camera rig、UiViewModel、Flight record、画質選択はengine非依存とする。
別engineへの置換はadapterとそのasset/shader変換を対象とし、物理・ゲーム進行・記録形式を維持する。
runtime中のengine交換は初期要件に含めない。composition rootでadapterを選択できればよい。

## 二つの独立した選択

- Presentation mode: Screen / WebXR / Phone VR
- Rendering engine: Three.js、将来の別engine

Presentation modeは眼・tracking・表示先・入力方式を決定する。
engine adapterはGPU resource、描画、engine固有のXR結合を担当する。
modeとengineの組合せでcapabilityを判定し、未対応機能を黙って省略しない。
WebXR adapterとThree.jsのWebXRManagerが同時にframe loopを所有する構成は禁止する。
native XRのloop所有・frame取得は結合部へ集約し、上位へengine objectを返さない。

## 依存方向

```text
game state / difficulty / flight record
                  ↓
UiViewModel / RenderSnapshot / camera rig / quality policy
                  ↓
engine-neutral render contracts
                  ↑
Three.js adapter + WebXR/Phone VR integration
                  ↑
composition root（adapterの選択・注入）
```

renderer契約は `web/src/render/contracts/`、初期adapterは `web/src/render/engines/three/` に置く。
camera rigは `web/src/render/camera/` で数値poseを生成する。
web appと共通rendererからThree.jsのScene、Object3D、Vector3、Quaternion、Camera、Material、Texture、
WebGLRendererをimport/re-exportしない。型だけのimport、動的import経由の漏出も検査する。
CLI/core/formatは描画engineに依存しない。
composition rootだけが具体adapterのfactoryをimportし、返却値は共通interfaceとして扱う。

## 最小契約

| 契約 | engine非依存の内容 |
|---|---|
| RenderSnapshot | 描画座標へ変換済みの機体pose、actuator表示値、simulation時刻 |
| WorldDescriptor | asset ID、local origin、配置、環境parameter、camera_points |
| CameraRigPose | 位置・単位quaternion・投影設定。engine camera instanceを含めない |
| FrameViews | 眼別view/projection数値、viewport、表示frame識別子 |
| UiViewModel / UiAction | 文言、値、可否、focus、graph/cursor、操作event |
| AnchorPose | World/Cockpit/Menu/Headと各referenceの数値変換 |
| QualityProfile | 項目別品質の意味と要求値。engine独自定数を含めない |
| EngineCapabilities | 対応mode、stereo、UI panel、reflection等の利用可否 |
| RenderError | unsupported、asset失敗、context lost等の判別可能な失敗 |

共有ベクトル・quaternion・matrixの要素順、単位、referenceは契約で固定する。
検証済みのreadonlyな数値型と識別子を用い、engine型への型aliasやanyで境界を迂回しない。
巨大なSceneを毎frame直列化せず、world/asset作成とtick/frame更新を分離する。
GPU resource handleはadapter内部に保持し、上位ではasset IDを使用する。
dispose・resize・world load・frame描画・capability取得の所有者と失敗時状態を定義する。
必要な操作だけを契約化し、汎用game engine全体を再実装しない。

## UI・camera・XR

全SceneのScreen/VR UIは同じmodel/actionを使用する。
2D panelのlayoutとhit領域は共通データとし、VR texture/mesh生成をengine adapterへ閉じ込める。
camera rig、director、anchor変換はengine scene graphを参照せず、数値poseとworld metadataを使用する。
XR runtime由来の眼別projection・trackingはplatform側で取得し、engineとの結合部で共通契約へ変換する。
同じ不変ViewerFrameをview導出とbackendのcurrentFrameへ渡し、Menu配置と実描画で眼別projectionを共有する。
BackendFrameのPanelFrameはabsent、visible、unavailableの排他型とする。visibleだけがUiPanel・共通pose・cursorを保持する。
unavailable frameには以前のpanelや選択領域を流用しない。viewer欠損時の配置data保持と実表示・選択の可否を分離する。
Three.jsのStereoEffect/StereoCamera/WebXRManagerは初期adapterの実装候補であり、公開契約に含めない。
APIごとのnative XRFrame等はplatform結合部に保持し、GameSceneやFlight recordへ公開しない。

## Assetとshader

地形・機体・配置・環境parameterはengine非依存のsourceとmetadataを保持する。
実行用asset変換、material、reflection pass、水面shader、post-processingはengine adapterの責務とする。
shaderを別engineへ無修正移植できることは保証しない。視覚的な要件と物理parameterを共通契約とする。
色空間、depth、単位、handedness、alpha、投影行列の差はadapterで変換して検証する。
特定engineのserialized Sceneをworld assetの正本にしない。

## 検証と開発順

BPG-014で境界と最小Three.js adapterを実装し、engine packageのimport規則と公開型の検査を追加する。
engineをimportしないcontract test doubleで全Sceneのview model、camera pose、描画要求、入力eventを検証する。
test doubleはGPU互換性や別engineの実表示を保証しない。
BPG-015/016で全SceneのXR/Phone結合を確認し、以降の描画機能もadapter境界を維持する。
engine追加時は同じcontract testsと実機の表示・操作試験を適用する。
画像のpixel一致を要求せず、座標・投影・操作・必要な視覚機能・性能の適合を評価する。

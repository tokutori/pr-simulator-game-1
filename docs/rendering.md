# 描画・性能契約

視覚的手掛かりとしての優先順位はWater、Aircraft、Venue、Far Terrain、Sky/Cloudsとする。ゲームループ成立後の景観工程では、湖面描画を地形・会場および空・雲より先に実装する。
WebGL描画はRenderSnapshotを入力とし、physics stateを変更しない。
Three.js固有の実装はengine adapterへ集約する。共通契約とassetの正本は `render-boundary.md` に従う。
Desktop・WebXR・Phone VRのbackendと光学profileは [presentation契約](presentation.md) に従う。
二眼描画ではreflection、cloud billboard、HUD、post-processingの両眼整合を検証する。
全Scene・overlayのScreen/VR対応を高品質描画に先行して確立する。
簡易worldでもPilot視点、ゲーム進行、Result/Analysis/Replayを完成させてから景観品質を追求する。その後は水面反射・波面・表面模様が速度と姿勢の把握に与える情報を先に成立させる。湖面描画は簡易worldで独立して開発し、地形・空の完成を待たない。

## 湖面

湖底は表示せず、不透明水面を採用する。基調色は `#3c5057`、`#5f7279`、`#2d3f43` 付近。
空・雲・山・湖岸・太陽のreflectionとFresnel効果を含める。
太陽は微小波面法線によるsun glitterとして表現する。水面にはworld-spaceの波面・反射・表面模様を用い、画面内の流れから対地移動と姿勢変化を読み取れる状態を受入条件に含める。
Lowでも低解像度environment reflectionを維持し、reflectionの更新頻度と解像度を調整する。

波浪パラメータはscenario生成時に固定し、位相のみsimulation timeで進める。
局所水面から風上へrayを飛ばしてfetchを推定する。
fetchは風向と湖岸形状で決まり、湖中央からの距離だけでは決定しない。
風速・fetchのみから実際の波浪状態が一意に決まるとは仮定せず、風の継続時間や観測との較正を記録する。

大波はGerstner waveを候補とする。単なる正弦波の高さ和は鉛直変位モデルであり、
Gerstner waveの水平変位を含まない。実装時に水平変位、法線、steepness上限、自己交差回避を定義する。
Low 4–6、Medium 8–12、High 12–24成分を暫定予算とする。
微小波はnormal map/procedural normal。白波は波高・steepness・風速に応じて限定する。
水面の見た目と静水面の着水判定は初期版では分離する。

## 地形・会場

松原会場は湖岸線、砂浜、platform、松林、観客エリア、仮設構造物を優先する。
彦根近傍は中精度、対岸・山地は稜線とpeakを保持するlow-poly geometryとする。
500 m移動でのparallaxを保持するため、全遠景をskybox画像へ置換しない。
screen-space errorと複数視点の稜線誤差で簡略化を検証する。
多景島のcanopy補正は描画専用とし、湖岸で0へ滑らかに減衰させる。
入力がDSMの場合は既存の樹冠高との二重計上を確認する。collisionへ適用しない。

## 空・雲・haze

analytic sky、haze、world-anchoredな積雲のbillboard/impostor clusterを基本とする。
雲は明るい上面、暖色寄りの太陽側、青灰色の下面を持つ。
山の稜線は識別可能に保持し、山下部は距離に応じて淡くする。
雲を水面reflectionへ反映する。雲影はHigh以上の任意機能とする。
volumetric raymarchは初期版の対象外である。

## 画質

Auto / Low / Medium / High / Ultra / Customを用意する計画である。
項目はresolution scale、terrain LOD、water、reflection、shadow、cloud、particles、post-processing。
Autoは起動時benchmarkと実行時frame-time percentileを用い、hysteresisとcooldownを設ける。
p95が18.5 msを超えた場合の低下、12.5 ms未満での上昇は初期候補値とし、計測後に確定する。
適応画質は水面反射を無効化せず、速度・姿勢の判読に必要な波面と反射の最低品質を維持する。縮退時はresolution scale、shadow、cloud、terrain、particles、post-processing、水面の高周波detailの順を基準とする。反射解像度・更新頻度は最低品質を下回らない範囲で調整する。
手動画質とAutoを区別し、Customを無断で上書きしない。
Desktop/Phoneでは60 FPSを目標とする。XRはruntimeのrefresh rateに対応するframe budgetを使用する。
CPU/GPU/転送量のbudget、対象端末、継続時間はBPG-012で数値化する。
画質変更は物理tickと物理モデルへ影響しない。

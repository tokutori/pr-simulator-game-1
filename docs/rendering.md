# 描画・性能契約

空間把握のための視覚優先順位は、湖面optic flow、地平線・対岸稜線とcockpit/wing/noseの自己基準、近距離固定物、中距離landmark、haze、装飾的なcloud・terrain textureとする。ゲームループ成立後は湖面を最初に実装し、以後も見栄えより速度・姿勢・高度・進行方向・scaleを復元する手掛かりを優先する。
WebGL描画はRenderSnapshotを入力とし、physics stateを変更しない。
Three.js固有の実装はengine adapterへ集約する。共通契約とassetの正本は `render-boundary.md` に従う。
Desktop・WebXR・Phone VRのbackendと光学profileは [presentation契約](presentation.md) に従う。
二眼描画ではreflection、cloud billboard、HUD、post-processingの両眼整合を検証する。
全Scene・overlayのScreen/VR対応を高品質描画に先行して確立する。
簡易worldでもPilot視点、ゲーム進行、Result/Analysis/Replayを完成させてから景観品質を追求する。その後は水面反射・波面・表面模様を先行させ、地平線・対岸稜線、cockpit/wing/nose、近距離固定物、中距離landmark、hazeの順に追加する。湖面描画は簡易worldで独立して開発し、地形・空の完成を待たない。

## 湖面

湖底は表示せず、不透明水面を採用する。空の明るさを維持しながら、水面の最終色は参照写真の暗い青灰色へ調整する。
空・雲・山・湖岸・太陽のreflectionとFresnel効果を含める。
太陽は微小波面法線によるsun glitterとして表現する。現段階は青みのある晴れ間、全天の約5%を覆う小さな薄い低層雲、水平線付近の淡い霞を持つ静的な全天テクスチャを背景空と水面反射で共有する。太陽と雲は全Weather共通の固定照明であり、環境入力が導入された際に可視性と雲量を接続する。水面にはworld-spaceの波面・反射・表面模様を用い、画面内の流れから対地移動と姿勢変化を読み取れる状態を受入条件に含める。水体色の暗化は空の反射に適用せず、反射成分の水平線側の色を背景空に合わせる。反射強度は参照画像に合わせて45%に抑えた試作値である。Fresnel係数は幾何法線を基準とし、可視の微小面の係数を限定的に混合する。反射方向は視線側を向く微小面の法線を用い、裏向きの面では幾何法線へ戻す。遠方の微小波反射を視線角で一律に消さないため、反射方向とFresnel混合の可視条件を分ける。この処理は可視微小面を積分した厳密な粗面BRDFや空と水面の輝度一致を表すものではない。
Lowでも低解像度environment reflectionを維持し、reflectionの更新頻度と解像度を調整する。

試作湖面は有限fetch向けのJONSWAP形状と方向分散を持つ離散波浪成分、位相固定、Gerstner水平変位、非正弦波形、有限長の局所起伏、共有全天テクスチャの反射で構成する。全画質で同じ18成分の基準スペクトルを生成し、Low/Medium/Highで6/10/18成分を幾何波として描く。低画質の成分は全周波数帯を代表し、画質を上げると帯域ごとの方向成分を追加する。中短波の起伏は局所テクスチャで補い、画質に応じて水面mesh密度も変える。合成scenarioのWeather 5段階は描画専用の風向・風速・細部振幅・模様seedへ対応させ、天候選択時に波況を更新する。Mild以降の向きはscenario風に概ね合わせ、風速には過去の吹送で残った起伏を表す仮定を加える。Calmにも既存波を残す。現行シナリオから風向別fetch・吹送時間・水深が得られないため、600 m fetchは全段階共通の仮定とする。飛行・リプレイでは確定した天候から同じ波浪状態を作り、simulation timeのみで位相を進める。この描画入力は琵琶湖の実測状態や瞬間風から計算した波浪予報ではない。シナリオ環境契約が加わった時点でその入力へ置き換え、太陽・空・地形reflectionも環境descriptorへ接続する。
水面meshは6 km四方とし、視点付近へ頂点を集中させる。局所頂点間隔で幾何波をfilterする。中短波には、有限範囲の山と周囲の谷を多数重ねた2種類の周期テクスチャを用い、同じ高さ場から頂点変位と画素法線を作る。さらに近層を異なる回転・縮尺で2回参照し、細かな法線成分を重ねる。頂点で表せない距離では高さを減衰させ、画素法線はmipmapで平均傾きと傾きの二次モーメントを集約する。頂点法線に含めた勾配を画素へ渡して差し引き、幾何形状と画素法線で同じ起伏を二重に数えない。画素内の4位置で法線を評価して平均する。遠景の霞は弱く適用し、波の明暗を一律に消さない。側方視点でmeshの端や過度に早い細部の消失が見えないようにする。全ての波の位相はworld-spaceで評価し、水面meshが機体へ追従しても波形を動かさない。meshの表示範囲と波浪生成に仮置きした600 m fetchは別の入力である。
局所起伏は各テクスチャ辺長の1/96を特徴尺度とし、波頂片を6割、周囲の谷を伴う丸い起伏を4割配置する。波頂片は進行方向より波頂方向へやや長く、緩い曲率を持たせる。波片数を面積に応じて増やし、前後の設定で局所起伏の被覆率を概ね保つ。波片の中心はテクセルより細かい位置に置き、近層の頂点変位は格子間隔が0.12～0.32 mの範囲で減衰させる。
遠層のテクスチャ辺長は288 m、特徴尺度は3 mとし、遠景の投影画素でも有限長の波頂片を残す。
回転させた高周波2層は移動方向も同じ角度で回転させる。4層の移動速度の比は[深水重力波の分散関係](https://uhslc.soest.hawaii.edu/ocn620/lectures/surface-gravity-waves-1.html)にある位相速度 $c \propto \sqrt{\lambda}$ に近づける。この移動は帯域ごとの平行移動による視覚上の近似であり、波列内の分散や水深の変化はまだ表現しない。
微小法線の強度は描画カメラの高度から独立させ、スタート画面と飛行画面で同じ高さ場と風況を使用する。近層の局所起伏の振幅を抑え、細かい二層の法線変化を強めて短い波頂を描く。この調整は描画専用とし、飛行物理を変更しない。
波頂の局所振幅には下限を設け、微小法線は水面全体へ連続的に適用する。波のない斑状領域を作る強度マスクは使用しない。

将来のシナリオ波浪パラメータはscenario生成時に固定し、位相のみsimulation timeで進める。
局所水面から風上へrayを飛ばしてfetchを推定する。
fetchは風向と湖岸形状で決まり、湖中央からの距離だけでは決定しない。
風速・fetchのみから実際の波浪状態が一意に決まるとは仮定せず、風の継続時間や観測との較正を記録する。

大波はGerstner waveを候補とする。単なる正弦波の高さ和は鉛直変位モデルであり、
Gerstner waveの水平変位を含まない。実装時に水平変位、法線、steepness上限、自己交差回避を定義する。
Low 4–6、Medium 8–12、High 12–24成分を暫定予算とする。
微小波はnormal map/procedural normal。白波は波高・steepness・風速に応じて限定する。
波頂の鋭さはGerstner水平変位と平均高さが0の三乗波形で表現し、水平写像のJacobianが正となる範囲に抑える。三乗波形は山を狭く、谷を広くする。波頂に沿う位相と振幅は2段の二次元値ノイズで変化させる。局所起伏は非周期的な配置と山・谷の短い形で長い筋を減らすが、64 mと288 mのテクスチャ反復は残る。白波は波頂の局所圧縮に限定し、現在の局所風速だけで既存の波群の砕波を判定しない。写真上の明るい波筋をすべて砕波とみなさない。写真だけで実際の波高・fetch・吹送履歴は同定できない。描画専用の振幅調整はflight physicsと着水判定を変更しない。根拠は[NVIDIA GPU GemsのGerstner波の説明](https://developer.nvidia.com/gpugems/gpugems/part-i-natural-effects/chapter-1-effective-water-simulation-physical-models)、[NOAAの風浪・砕波の説明](https://oceanservice.noaa.gov/education/tutorial_currents/03coastal1.html)、[米国気象局の風浪とうねりの区別](https://www.weather.gov/marine/faq)による。
岸から沖への水深増加は水深分布が入力された時点で分散関係・浅水変形・砕波の条件へ反映する。現行scenarioは水深分布を持たないため、画面内の位置から水深を推定しない。
水面の見た目と静水面の着水判定は初期版では分離する。

## 地形・会場

## 空間手掛かりの層

遠景の対岸稜線をworld-fixedなhorizon/referenceとして保ち、Pilot視点のpitch・roll判読を支える。機体のnose・wing/cockpit横線はbody-fixedな自己基準とし、外界と同時に視認できる。
近景はplatform・buoy等の固定objectでoptic flowを生成し、中景は島・湖岸・識別可能なpeakでheading・横流れ・scaleを補う。近・中・遠景のparallaxを維持し、均一なskyboxやtextureだけで距離感を代替しない。
地形・物体・機体の描画は同一world scaleと姿勢変換を用いる。decorative detailより先に、pitch・roll・yaw・速度・高度の変化を読み取れることを受入条件とする。

松原会場は湖岸線、砂浜、platform、松林、観客エリア、仮設構造物を優先する。
彦根近傍は中精度、対岸・山地は稜線とpeakを保持するlow-poly geometryとする。
500 m移動でのparallaxを保持するため、全遠景をskybox画像へ置換しない。
screen-space errorと複数視点の稜線誤差で簡略化を検証する。
多景島のcanopy補正は描画専用とし、湖岸で0へ滑らかに減衰させる。
入力がDSMの場合は既存の樹冠高との二重計上を確認する。collisionへ適用しない。

## 空・雲・haze

全天テクスチャ、haze、world-anchoredな積雲のbillboard/impostor clusterを基本とする。
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

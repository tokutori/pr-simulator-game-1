# 描画・platform・VRの契約

## 境界と空間把握

- 描画は数値snapshotと共有表示モデルの投影である。
- 純粋な波・空・会場・機体・camera・UIの規則は共有Rustが所有し、Three.jsとBevyの型、mesh、shader、sampler、描画pass、資源寿命は各adapterへ閉じ込める。
- 依存方向は[アーキテクチャ](architecture.md)に従う。

受入の中心は、速度・pitch・roll・高度・進行方向・scaleを外観から復元できることである。優先する視覚基準は次のとおりである。

1. 水面の波、glitter、反射と非一様な模様によるoptic flow。
2. world固定の地平線・対岸稜線と、body固定のnose・wing/cockpit横線。
3. platform、桟橋、ブイ等の近距離固定物。
4. 島、湖岸、識別できるpeak等の中距離landmark。
5. 近・中・遠景のparallaxとhaze。装飾はこれらの基準を維持する。

- 全Sceneとoverlayで前方視界と計器の判読性を確保する。
- 近景・遠景の奥行きを保持し、移動に伴うparallaxを表現する。

- 共通の長さはm、物理位置はlocal NED、機体座標はFRDである。
- 描画basis、quaternion、眼位置、camera rigは[ゲームとUI](gameplay-and-ui.md#camera)に従う。

## Screenの解像度とAuto

- Flight SetupとPause SettingsはAuto・100%・75%・50%を提供する。
- 既定はAuto、初期の適用済み解像度は100%である。
- 各軸50%は3D画素数のおよそ25%に対応し、UIの論理解像度、画角、入力座標、物理tick、記録を維持する。

| Scene | 有効な品質方針 |
|---|---|
| FlightSetup・Briefing・Countdown・FlightとPause | 保持したフライト用の選択を適用する |
| Boot・Title/Attract・Result・Replay | Autoを使用し、フライト用の手動三値を保持する。背景の水面上限はHigh |

- 共有UIが全Sceneをフライト設定用・背景用のscopeへ分類し、描画policyはscopeと品質組を受け取る。同じscope内のScene切替では計測窓を継続する。

- 共有`ScreenRenderAutoController`が、解像度100%→75%→50%、shadow map 2048→1024→512、水面High→Medium→Lowの順で縮退する。
- 回復は逆順に三値の組で試行する。上位で予算を超えた場合は試行元の解像度・shadow・水面へ戻し、同じ回復辺の再探索を6秒、12秒、最大24秒へ間引く。
- 下位のp95が試行時の9.5/12.5未満へ低下した場合は通常の3秒cooldown後に早期再探索する。待機期限後も成熟・鮮度・受領率を満たす窓で再探索し、画質の回復機会を保つ。
- 保持した手動の解像度・shadow・水面、Auto/Manual選択、要求中の値、適用済み値を区別する。

- Auto中の水面選択は上限を変更する。
- 適用済みが上限より低い場合は保持し、上限を超える場合に下げる。
- 明示ManualとAuto解除では保持した三値へ戻す。
- 要求とACKはepochと三値を照合し、全資源を準備してからcommitする。
- 準備失敗は既存資源と原因を保持する。
- commit後の旧資源cleanup失敗では新しい適用済み値と原因を保持し、準備失敗と区別する。

初期窓は最初の有効GPU source時刻から2秒とする。8 ms区間ごとの最大GPU spanを代表値とし、そのp95を使用する。

| 判断 | 条件 |
|---|---|
| 縮退 | p95 > 12.5 ms、最低8代表値、source span ≥ 1秒、鮮度 ≤ 500 ms |
| 回復 | p95 < 9.5 ms、最低30代表値、span ≥ 1.8秒、鮮度 ≤ 100 ms、source frame受領率 ≥ 90% |
| 次の交換 | 適用後3秒のcooldownを経過する |

- 計測窓は実際に受領した有効sampleで構成し、高負荷履歴を保持する。
- coverage不足時は回復を保留する。
- resize、backend変更、context復帰、環境・外部品質変更、中断、disjointでは古い窓と回復試行を失効する。自身の資源交換・ACK・pipeline準備では回復試行を保持する。
- 計測の履歴や時刻をUI構造cache keyへ含めない。

- 初期計測の完了は、同contextで受領率90%以上の成熟窓を評価し、要求資源をACKした事実として保持する。
- 継続適応中のReady/Measuring/Applyingと区別する。
- pipelineの再準備では現在窓を失効させる。
- 明示Manual、新Auto、外部context失効、未対応、適用・cleanup失敗では完了事実も破棄する。
- Nativeの初期完了は、ACK後のepochに対応したpipeline Readyと500 ms以内のGPU sourceも必要とする。

- Webは反射と主描画を含むcommand spanを`EXT_disjoint_timer_query_webgl2`で非同期測定する。
- NativeはBevyのRecorderでsource frame・source時刻・epochが一致するspanを受け取る。
- GPU upload、screenshot/readback、present待機、CPU全体の時間と区別する。
- 使用するpipelineの準備が完了したframeを有効sampleとする。Auto用queryはAuto選択中に発行する。
- Webは最初の描画前にGPU計測能力を確認する。GPU計測非対応のAutoは解像度50%・shadow 512・水面Lowの軽量固定profileを要求し、交換ACK後に原因付きで表示する。Manualへ戻すと保持した三値を復元する。
- Webの資源準備はtextureと反射targetを初期化し、`compileAsync`の完了後にdefault framebufferと反射targetで初回使用を確認する。ゼロ面積scissorで画素書込みを抑え、近景・遠景水面の使用とshader原原因を確認してから準備完了を返す。後発のXRなどのprogram失敗も実描画時に共有ownerへ通知する。
- 背景内部program・shadow pass・後着decalのuploadは準備時の初回使用と後続描画のCPU/GPU観測へ含める。shadow描画量はscissorによる画素書込み抑制と区別する。

- WebのCSS寸法は保持し、元のDPRと選択倍率の積をThree.jsの公開`setPixelRatio`へ渡す。
- Nativeは整数物理寸法へ共有`physical_size`を適用する。
- DPRの整数化順による両adapterの差は各軸1 pixel以下とする。
- 0寸法では交換を保留し、正の寸法へ戻った後に適用する。

- Nativeの縮小targetでもUIとcameraの論理投影を維持する。
- 反射targetは有効3D寸法に追随する。shadowのみの変更では既存の解像度targetを保持する。
- shadow調整はmap寸法だけを変更し、cascade、bias、太陽条件を保持する。

- Phone VRは元の描画倍率、WebXRはruntimeのcompositor解像度を維持する。
- VR中はScreen Autoを停止し、復帰時はepoch/readinessを再準備して保持設定へ戻る。
- Webのrenderer共有shadow適用値はVR切替後も保持する。

## 共有水面モデル

- `birdman-game-scene::lake_water`は環境条件から波スペクトル、品質別投影、周期画像を純粋に生成する。
- Web移植基準の式と成分順を使用し、`lake_sky`の全天画像も共用する。
- 波は描画モデルであり、位相の進行には共有セッションのsimulation timeを使用する。

- 品質選択はformatのWaterQualityを共用し、lake_waterのprofileが波数・mesh予算を定める。基準spectrumは18成分で、Low・Medium・Highは6・10・18成分を使用する。
- fetch、JONSWAP重み、方向分散、正規化、steepness上限0.52を保持する。
- quality投影はFloat32 upload、帯域energy補償、実mesh spacing、解析的微分上界、choppiness reserveを使用する。
- geometryの位相と方向jitterは固定hash、`patternSeed`はdetail画像だけへ作用する。

- Weatherの波浪入力はscenario準備時に固定する。
- Calmにも準備時の波浪を保持する。
- 合成条件の600 m fetch等は描画上の仮定であり、琵琶湖の実測波浪や波浪予報を表さない。
- 位相はsimulation timeだけで進める。

### Geometryとdetail

- 近景meshは6 km四方の非一様gridで、Low・Medium・Highを96・144・224分割とする。Webは元の頂点属性と三角形を保った4×4のchunkに分け、視錐台カリングを適用する。
- chunkの境界球はshaderのXZ座標に合わせ、全波成分・detail画像による水平/鉛直変位とFloat32丸めの余裕を含める。Highの各chunkは最大3,249頂点で、境界共有により全chunkの頂点総数は51,984となる。
- camera水平位置を1 m格子へsnapして追従し、波はworld-spaceで評価する。
- 遠景ringは18万m四方を覆い、近景領域を除外する。
- meshの範囲と波浪生成のfetchを区別する。

- 有限wave packet、二段noiseのphase/振幅変調、Gerstner水平変位、平均高さ0の非正弦波形を使用する。
- 解析勾配はpacketとLOD visibilityの両方を含む。
- local XZからの頂点間隔は$s=s_0+(14.4/N)\max(|x|,|z|)$とし、中心勾配0、同率の軸は両側勾配の平均とする。
- fragmentの補間間隔は微小法線の表示配分にだけ使用する。
- Webの頂点出力はcrest・compression・spacingをvec3、基準XZ・表現済み勾配をvec4にまとめる。反射投影座標は補間後のworld座標からfragment側で算出する。

- CPUは実Float32係数とmeshの間隔域から水平変位の微分上界$B_i$を計算する。
- 方向norm、noise、LOD境界とstationary pointを含め、全域非表示成分は0とする。
- choppinessは$q=\min(4.5,0.56/\sum_i B_i)$にFloat32用の余裕を加える。
- 実数解析では水平変位のLipschitz上界を1未満へ抑える。
- GPUでの数値・描画結果は[検証](verification.md)の本番shader試験で確認する。
- Web水面は共有Rustの確定波成分数をcompile定数へ転写し、近景・遠景の配列とloopを実成分数へ特殊化する。波式・画素微分・detailの平均と分散は同じ投影を使用する。

visibilityと実空間勾配の両channelがすべて厳密0の成分は、packet・位相・三角関数の評価を省略できる。非減衰のslope energyは元の成分順ですべて加算し、crestの正規化を保持する。

- detail画像は512×512の周期RGBAである。
- R/Gは勾配、Bは勾配variance、Aは静水面基準の高さを格納する。
- uint32 LCG、解析勾配、加算時Float32丸め、byte量子化を共有する。
- nearは64 m・3150 wavelets、farは288 m・5400 waveletsと900個のbroader waveletsを使用する。
- 生成境界は合計65535 wavelets、broader raster radius最大512 texelsとし、範囲外は型付きerrorとする。

- 局所起伏は辺長1/96の特徴尺度を基本とし、波頂片6割と谷を伴う丸い起伏4割を配置する。
- nearの頂点変位は間隔0.12〜0.32 mで減衰し、farには特徴尺度3 mと低振幅4.5 mの帯域を使用する。
- 64 mと288 mの周期反復は残る。
- 帯域別の移流は深水重力波の$c\propto\sqrt{\lambda}$に近づけた視覚近似であり、帯域内の分散・水深変化を表さない。

- near/farと回転・縮尺を変えたfine/rippleの4層micro-normalは、解像する帯域を4点で積分する。texel footprintの短軸を保守的に評価し、2〜4texelの区間で採取位置を中心へ連続的に縮め、十分に縮小された帯域はmipの平均傾斜と二次momentを1回採取して共用する。
- Fragmentの波loopは実成分数を上限とし、各成分の画素微分を全laneで評価する。法線寄与が0の画素では、その後のcrestとgradientの演算を省略する。
- 回転は移動方向、texture勾配、法線勾配へ一貫して適用する。
- mipmapに平均勾配と二次モーメントを保持し、頂点法線が表現した勾配を差し引いて二重計上を避ける。
- 未解像varianceからroughnessを導出する。
- 微小法線の強度をcamera高度へ連動させず、水面全体に連続適用する。

### 色・反射・shadow

- 湖面は不透明とする。
- 暗い青灰色の水体色、facet Fresnel、sun glitter、crest/compression由来の明暗・白波、height tint、距離hazeを合成する。
- 水体色と天空反射を個別に計算し、水平線側の反射を背景空と対応させる。
- 反射強度45%は表示上の試作値であり、厳密な粗面BRDFや輝度較正を表さない。

- 反射方向は視線側を向く微小面の法線を用い、裏向きでは幾何法線へ戻す。
- Fresnelは幾何法線を基準に可視微小面を限定混合する。
- 白波は波頂の局所圧縮から導出する。

- 空と水面radianceは同じ全天画像を使用する。
- Lowでも低解像度反射を維持し、背景の輝度、水面optic flow、反射の最低品質を守る。
- 太陽shadowのcasterは機体の不透明な被覆・骨格とする。

- 機体反射は水面鏡映cameraと半解像度の透明targetを使用する。
- 中央眼による近似であり、厳密な両眼反射、局所波面の鏡映幾何、太陽の有限角による半影は含まない。
- Nativeは水面上0.05 mを超えたcameraだけでpassとsamplingを有効にする。
- targetは各軸1 texel以上、長辺最大1024 texelsへ同縮尺で制限する。

- NativeのmirrorはBevy公式mirrorの鏡映行列、反転culling、oblique near clipを使用する。
- Web基準の鏡映lookAtとはright基底とtexture Y原点が逆となるため、slope offset`(0.006,0.006)`をNativeでは`(-0.006,-0.006)`へ写す。
- alpha合成は`clamp(0.16 + Fresnel * 0.7, 0, 0.72)`を維持する。
- 丸め後の実target aspectでprojective matrixを構成する。

- Native反射layerは機体と照明に限定する。
- Webの既存機体・雲反射passでは、shadow receiverがない範囲の再更新だけを抑え、例外時もrenderer共有flagを復元する。
- 直後の主描画のshadow更新を維持する。

Nativeは既存DirectionalLightの2048 texel・30〜600 mの3段cascadeから、水面の変位後world positionとgeometric normalへBevyのshadow samplingを適用する。水面はshadow receiverとして登録する。

### Adapterと資源交換

- Webは環境・品質変更時にWASMからowned typed arrayをbulk取得し、bridgeを`finally`で解放する。
- 通常frameは準備済み画像を再利用する。
- Three.jsはDataTexture、sampler、GLSL、mesh、反射passを所有する。

- Nativeは同じ24-slot uploadを使用し、近景gridのUV1へspacingを格納する。
- detailはlinear RGBA、periodic wrapping、anisotropy 8とする。
- 全天画像は色変換してlinear`Rgba16Float`へ投影し、S=Repeat、T=ClampToEdge、linear/trilinear filteringを使用する。
- 品質変更は同じdetail・天空・反射targetを保持する。

- mipmapはBevyの公式生成を利用する。
- カラーdownsampleの対象formatと既知entry pointを確認し、2×2平均のsampling位置を初回Extract前に補正する。
- 補正は対象のカラーformatに限定し、依存更新で既知sourceとの不一致が生じた場合は原因付きで拒否する。

- active画像のAssetId/GPU TextureIdとcommand記録を管理し、未準備画像は再queueする。
- dispatch済みと退役済みの画像を待機列から除外する。commandの記録状態とGPU完了を区別する。
- 画像はimmutableとし、環境交換は新AssetIdを使用する。

- 環境交換は必要な全resourceを検証してから画像、material、bounds、太陽を一括更新する。
- Titleの展示は共有appが選ぶTypical環境の波・天空・太陽・会場を使用する。Setupの選択と展示のidentityを分け、adapterは共有projectionの会場可用性を表示へ写す。
- 全登録気象は同じ琵琶湖会場と時刻の登録天空・座標系を使用し、気象固有の物理風と波を重ねる。天空と座標系の由来は登録documentの仮定を継承し、保存記録は元identityと原文を保持して現在の登録providerから描画条件を解決する。
- 登録環境の解決に失敗した場合は、以前の波・天空資源と元原因を保持し、未登録記録では背景保持の通知と技術情報の元identityを表示する。
- 品質交換はnear/farへ同じ波投影を原子的に適用する。
- 反射resizeは新targetをcameraと全materialへ接続してから旧targetだけを退役させる。
- 非同期pipeline失敗はCPU準備失敗と区別し、対象shader・entry point・元原因を保持する。

## 空・cloud・haze

- 太陽方位は北0°の時計回り、太陽高度は水平0°である。
- 背景、水面、DirectionalLight、glitterへ同じ登録方向を適用する。
- skyの入力は太陽方位・仰角、雲量、visibilityである。
- 全登録気象で同じ天空条件から太陽照明・glitter・cloudを投影する。未知identityは資源準備のUnavailableとして扱い、直前の背景と元原因を保持する。

- `LakeSceneHaze`はsRGB`0x9aafb1`とvisibility由来のnear/farを所有する。
- hazeの開始距離は登録visibilityの0.175倍、終端はvisibilityそのものという表示近似を使用する。
- 地形はview-depth、水面はradial距離をsmoothstepへ渡す。
- Native地形はPBR照明後にWeb通常Screen基準のsRGB hazeを混合してlinearへ戻す。
- Web縮小targetのworking-linear fogとの差をpixel一致と扱わない。

- 上空の薄い雲層を無限遠の遠景として近似し、World方向固定の1024×512・sRGB RGBA天空画像へ合成する。
- 初回投影の基準高さ6 kmに水平雲層を置き、視線rayとの交点のXZ座標で固定seed`0x4f5a2c91`のnoiseを評価する。代表3.6 kmの雲片・24 kmのまとまり・1.2 kmの輪郭変化を作り、地平線ほど細かく扁平に投影する。雲量は被覆と透明度へ対応する。
- 天空画像のtexelが雲層上で覆う範囲から微細noiseを平均化し、地平線側の圧縮によるaliasを抑える。
- 雲量0は晴天gradientを使用する。下半球のhazeと、地平線・経度継ぎ目・天頂の連続性を保持する。
- 背景・水面sky sampler・Web/Native・左右眼で同じ画像を参照する。雲専用のmesh・atlas・material・drawは持たない。
- 天球用テクスチャは同じ天空条件で再利用し、条件変更時だけ生成する。不要になった資源は退役させる。
- 記録のcloud baseは気象metadataとして保持する。描画は遠景近似を使用する。

- Nativeの照明は共通のWeb基準色・irradianceを使用する。
- Directの18,000 lux換算基準と`Exposure = 2^(-EV100)/1.2`、Ambientの`E = πL`を使用し、主cameraと反射cameraへ同じExposureを与える。
- NoToneMappingを共用する。
- これらは描画規約であり、実測気象光量を表さない。

## 会場・地形・機体

- Titleは明示的な会場展示である。
- live session、Result、Replay、Attractは登録identityとlocal NED origin・water datumを照合する。
- 登録済みarchiveは全気象で同じ会場地形・湖岸・島・platformを表示する。未知identityでは直前の背景と元identityを保持して記録を再生する。
- 展示と登録済み記録は同じ登録providerの会場座標系を使用する。

- 地形、40岸線、17島、湖岸・構造物は共有Rustから完成NED位置/法線・linear色を取得する。
- adapterは共有出力の標高と法線をそのまま投影する。
- 登録構造物は桟橋5、岸壁4、防波堤3件で、景観用meshとして扱う。
- 湖岸・地形の出典と解像度は[データ形式と出典](data-and-formats.md)に従う。

- DSMの水域標本を地表へ使わず、陸地の欠損だけを補う。
- 島の地表は再sampleしたgrid、樹冠は元DSMを使用する。
- shoreline transitionは実描画三角形へ接続し、水域側へ240 m・水面下8 mの重なり、陸側5 m・地形下15 cmの潜り込みで隙間を防ぐ。
- 広い岸線欠損の探索上限は粗格子8セル・最大2.4 kmとする。

- 機体は公開synthetic構造から、藍色被覆、透明film、水色foam、薄茶rib/縁材、白いcockpit frame、透明nose、白い後部fairing、T字の黒い桁・胴体管を描く。
- 外観と物理接触は同じ構造形状と採用済み曲げ係数・尾翼角からそれぞれ導出する。材質、UV、表示samplingは描画層が所有する。

- 主翼のsurfaceは`WingFrame`、全構造材はcontentの固定頂点・三角形を投影する`AirframeStructure`を使用し、同じ採用係数を消費する。
- live・prepared・Result・Replayは共有poseの採用済み曲げ係数を主翼へ渡す。係数が変化した場合だけ、同じspanの頂点列でたわみ評価を共有して保持position bufferを更新し、topologyを保持する。
- 描画補間では身体位置・姿勢を補間し、尾翼角と曲げ係数は記録queryと同じ区間保持を用いる。
- 未記録係数は主翼の表示、未登録形状はcockpitと桁を含む機体全体の表示を利用不可にする。表現範囲を超える変形は失敗として通知し、次の有効な採用係数で表示を復元する。
- 全構造材は共通のf64頂点からFloat32へ投影し、尾翼batchは物理pivotを原点とするlocal座標を保持する。
- 法線は完全な半翼topologyにarea-weighted計算を適用し、材質分割後も共有する。
- 表示たわみの定数と実機諸元の区分はデータ文書へ集約する。

- 259部材をattachmentとmaterialの11 batchへまとめ、Web/Nativeは同じ管周方向24分割・輪郭・三角形を描画する。構造材の接触点と表示頂点は同じproducerから得る。
- 管の変形は中立frameからの最小回転で周方向位相を保ち、foamは中央stationの曲げ変位を共有する。
- 主翼被覆は非光沢Lambertを基準とし、Nativeのreflectanceを0として不適切なspecularの持上げを抑える。
- linear-light mipを生成してsRGB textureへ格納する。
- PBR/Lambertの明度差や縮小時の細部は実描画で評価する。

- 尾翼は登録physical geometryのquarter-chordと全surfaceを共有し、水平・垂直それぞれ独立した親でphysical incidenceに従う。
- FRD変換後は水平+X、垂直−Yの回転を用いる。
- geometryは登録されたmodel identityから解決し、未知のidentityは明示的な未対応として扱う。
- 被覆・材質は描画モデル、部材寸法・配置・変形則はcontent、採用曲げ係数・空力・慣性・操縦応答は飛行モデルが所有する。

## Presentation backend

| Backend | 表示と追跡 |
|---|---|
| Web Screen | 単眼。スマートフォンの通常表示も含む |
| WebXR | immersive-vr。runtimeの眼別pose/projectionを使用する |
| Phone VR | 左右分割、Device Orientationによる3DoF。位置追跡・reprojection・XR compositorを提供しない |
| Native Screen | Bevyのデスクトップ描画 |

- Native OpenXRは未接続であり、runtime・frame提出・swapchain、pose/FOVと入力の観測、共有描画の提出を接続対象とする。
- Webでは全8 SceneとSettings・Pause・Help・Credits・HUDを同じ共通UIから表示する。
- XR sessionはScene間で共有する。
- DOM Overlayは任意機能であり、非対応機種でも装着中の主要操作をpanelで完結できるようにする。

- backend種別、capability、session状態、tracking状態は独立した排他型で扱う。
- 開始・終了・resize・recenter・frame描画・disposeを明示し、二重開始、権限拒否、unexpected session end、context loss、古い非同期完了を処理する。
- 起動失敗時は停止済みVRを再起動せずScreenを開始する。
- Screen復帰も失敗した場合はactive backendなしを示し、loopを停止する。

- XR/sensor権限は明示した利用者操作の同期区間で要求する。
- capability調査を事前に済ませ、利用者操作の同期区間で権限を要求する。
- Bootの最小権限UIはScreenに置き、rendererが利用できる場合は読込み中やResultだけを理由にScreenへ戻さない。
- tracking喪失や退出時はSceneを保持し、Countdown取消・Flight停止・Replay/Attract clock停止を共通sessionへ送る。

### Screen DOMの保持

- Screen・初期化中・失敗・Screenからの権限要求中はScreen DOMを維持する。
- VR開始/終了処理、VRからの要求、active VR、BFCache退避、非表示中は隠す。
- VR起動副作用より先に`hidden`・`inert`・`aria-hidden`と対応CSSを反映し、Screen復帰完了後に表示する。

- 非表示でも最新Viewを同じ安定nodeへpatchし、click/change/inputを拒否する。
- listenerは所有者の初期化時に一度登録する。
- focusとscrollは非表示への遷移時に一度退避し、同じScene/overlayへ戻る場合だけ復元する。
- focusは接続中・有効で、別要素への明示移動がない場合に`preventScroll`付きで復元する。

- Screen HUDはFlight/PauseとReplay Telemetry、VR Head HUDはactive Flightに表示する。
- Result、Replay Cinematic/Analysis、Attractは共有HUD観測に従って非表示とする。
- FPSはHUD観測の有無から独立する。
- Screen HUDは下端の透明な領域へ配置し、主要数値、計器、右下の操作を分離する。各cardは共有boundsへ投影し、全体背景とスクロールを使用しない。姿勢計は透明な線画と共有ピッチ目盛を描く。
- Head HUDは両眼の前方中央と操作panelを避け、下側の帯へ優先して配置する。下側に収まらない場合は行単位で先頭の情報を上側へ移す。FPSは前方clear領域の直上へ右寄せし、上側の情報行とはgapで分離する。

## VRの座標・anchor

| Anchor | 基準と挙動 |
|---|---|
| World | 地理world固定 |
| Cockpit | 航空機と取付位置へ固定し、頭部から独立する |
| Menu | 開いたときのtracking reference内に固定し、頭部運動で視野内位置が変わる |
| Head | 現在head poseと固定offsetに追従し、視野内位置を維持する |

- 湖面・地形はWorld、天空と上空の雲は方向をWorldへ固定する。
- sky geometryは眼位置への平行移動とworld基準の回転を組み合わせる。
- 実機相当計器とCountdownはCockpit、全SceneとPause等の操作panelはMenu、非modal Flight情報板はHeadとする。

- Head情報板は前方中央を空け、非操作telemetryを周辺へ配置する。
- 操作control/actionを含めず、interactive Menuとはtexture、pose、寸法、cursor、hit-testを分離する。
- Head固定controlを設ける場合は独立gamepad/controller選択を必要とし、head-gazeだけで複数項目を選ばせない。

共通referenceでの相対poseは次とする。

```math
T_{eye,object}=T_{R,eye}^{-1}T_{R,object}
```

- 機体・PilotEye・head・HUD localを一度ずつ合成する。
- IPDはstereo projectionで一度だけ適用する。
- head基準のrayとcontroller基準のrayをそれぞれ同じ描画anchorへ変換する。
- controller rayは観測空間のorigin・directionと、そのframeのreference-from-space poseを渡す。共有Rustがpanel参照系へ合成し、照準とtriggerに同じ交差判定を使う。triggerには最後に描画したtracking mountを対応付ける。
- 選択対象は、現在のoverlayに属する可視・有効・clip内のcontrolとする。

- World/Cockpit panelはphysicalな深度判定・書込み、Menu/Headは世界描画後のoverlayとする。
- 順序は世界→非操作Head情報板→interactive panel→cursorで、overlayはdepthTest/depthWriteを無効にする。
- anchor poseや有限距離・両眼視差は維持する。
- physicalへ戻る際はqueueと深度状態を復元する。

### 同一frameと両眼geometry

- animation loopの所有者は常に一つとし、backend間で同じphysics tickを使用する。
- 順序は前frameのpending入力適用→raw head/眼別geometry取得→physics/機体pose更新→`beginViewFrame()`→View導出/計測/描画/gaze→3D描画とする。
- 取得後のcamera cut、FOV、viewport、光学profile、明示pose取消、View開始後のpose/lake入力は次frameへ送る。

- presentationの`XrViewerFrame`はconfigured・runtime-derived・unavailableを区別し、同一frameのraw center-headと各眼のheadFromEye/projectionを保持する。Webは生成された同じ型へ観測をコピーする。
- WebXRのcenter-headと各viewは、同じ`getViewerPose()`から`XrViewerObservation`へコピーする。共有Rustの`capture_xr_viewer`が眼構成・pose・projectionを検査し、左右眼を順序付けてHead相対poseを導出する。
- 16要素column-major projectionを保持し、非対称frustum、眼回転、shearをそのまま扱う。clip-depthはWebGLの`[-1, 1]`とVulkan等の`[0, 1]`を型で区別し、reversed Zも同じ区間で判定する。

- 初期対応はleft/right各1眼である。
- 重複・未知の眼編成、非有限値、非単位pose、退化projectionは原因付きで利用不能とする。
- geometry失敗時も有効なcenter trackingを保持する。WebXRの眼geometryは各frameのbrowser配列から所有データへ取得する。

- Phone VRの眼配置とprojectionはruntimeの`phone_viewer`が生成する。描画幅の切捨て半分を左眼、残りを右眼へ割り当て、各眼の実幅と高さから非対称projectionを導出する。
- Web adapterは共有の完成eye pose・projectionとviewportをcamera・viewport/scissorへ転写する。
- 両眼の描画後はviewport/scissor等を復元する。
- Phone VRの左右分割表示はlens distortionを含まない。

### MenuとHeadのlayout

- 共有Rustの`fit_xr_menu_view`が、Menuを開いたframeの両眼projectionとhead-to-eyeからpanel全体を収容する距離を導出する。adapterは返されたhead-local poseを固定anchorへ写す。
- head motion、telemetry、Attract cut、Replay seekでは移動しない。
- 「正面へ配置」、再opening、viewport・光学profileの実変更で再fitできる。

- 同じMenu表示中のviewer欠測では配置を保持し、描画・選択を停止する。
- 収容不能projectionは型付き失敗とし、同frameの描画を止め、世代付きScreen回復へ接続する。
- font読込み等のpendingでは旧入力contextを失効させ、回復を要求せず新geometryを待つ。

- 共通の順序付きsectionと型付きnodeを実font・localeで計測し、共有Rustが同じView/FrameからMenuの描画命令・clip・ページ操作・入力面を一括導出する。adapterは文字計測と、返された矩形・線分・文字の描画を担当する。
- 文字幅は折返し・再配置で収め、指定したフォント寸法を保持する。
- advanceとactualBoundingBox、描画のfont・locale・baseline・alignを一致させ、符号付きink metricsから実矩形を復元する。

- 描画、clip、gaze/controller hit、range、chart cursor、ページ操作は同じviewportとoffsetを使用する。
- portraitと長文はscrollで収容する。
- 共有Rustの`MenuSession`がscroll/focusを所有し、Scene/overlay・panel ID・view keyでscopeを区別する。開閉・scroll変更・配置失効でgenerationを更新し、対応する入力を受理する。
- scope/geometry変更・閉じる/再表示では古い入力を拒否する。
- 通常head motionやtelemetryでgenerationを更新しない。

- `prepare_game_ui_head_hud`が保持済みView/Frameと実フォント観測から光学面・group配置・描画命令・実測ink視角を一括で導出する。既定profileと予約Menu寸法はRustから生成する。
- 左右眼の前方水平±15°・垂直±10°を保護領域として写像し、cardと予約Menu領域を分離する。
- 背景alphaとforeground alphaを独立に保持する。
- painterが実測したglyph端をsurface-local metreで共有Rustの`fit_xr_text_ink`へ一括入力し、両眼clipと角高さ0.35°以上を検査する。Canvasの文字計測・描画とGPU資源はadapterが所有する。
- この閾値はソフトウェア配置の基準とし、実HMDの可読性は端末別の受入で評価する。

- Head surfaceは独立CanvasTextureとmeshを保持し、非表示中も寿命を維持する。
- 変更Viewだけを更新し、終了時はtexture/material/geometryを一度ずつ解放する。
- 水面反射layerへ入れない。
- Menu pending中も有効なHeadを独立描画し、利用不能時の回復要求は所有者・request世代の一致中だけ有効とする。

## Phone VRとWebXR

### Phone VRの重力基準

- secure contextのDevice OrientationとScreen Orientationを使用する。
- 明示的な権限要求が不要な環境では、起動時の1.5秒以内に有限な絶対姿勢を受信してPhone VRの利用可否を判定する。確認用の購読とtimerは結果確定時に解放する。
- `requestPermission`を提供する環境は切替操作から権限を要求し、実値の受信を開始条件とする。タッチ対応と姿勢センサーの利用可否は独立して扱う。
- 開始操作で`requestPermission(true)`を要求し、stereo準備・重力参照・有効な水平較正が揃った時点で開始する。
- null、非有限値、不正時刻、初回event timeoutは原因付きの追跡エラーとして扱う。

- browserは`deviceorientation`/`deviceorientationabsolute`のうち最初に`absolute=true`となった経路をsession中固定する。
- Earth参照系の上向きを水平基準に使用する。
- 独立sensor portが同期したrelative-reference-upを提供する余地は保持する。

自然画面向きに固定された姿勢を$D=R_z(\alpha)R_x(\beta)R_y(\gamma)$、Screen Orientation角を$s$、重力と初回水平方位の基底を$B$とすると、head回転は次となる。

```math
H=B D R_z(-s)
```

- 度/rad、端末axes、Z-X′-Y″、camera前方を明示して変換する。
- 較正とrecenterはyawだけを基準化し、pitch/rollを保持する。
- 画面が鉛直の正面姿勢で水平cameraとなる重力基準を使い、ゲーム正面は較正時の水平向きから定める。

- 上向き・下向きで水平方位が退化した場合は既存基底を維持する。
- 初回退化は較正を保留する。
- 画面回転は同じ基底へ右側補正を適用する。
- recenter基底差分は上向き軸周りの回転として、Eye mountで共役変換してretained anchorへ適用する。

- sampleはeventのtimeStampと同Window/time originのperformance.nowで、有限・非負・未来でない・非減少を確認する。
- 同時刻は許容する。
- rAFとsensorの時刻を区別する。静止中など次のeventを待つ間は、最新の有効姿勢を保持する。
- 明示不正sample、方位/参照喪失、退出・page lifecycleではtrackingと資源を停止・解放する。

- 初期光学profileは`generic-unverified-v1`、IPD0.064 m、vertical FOV60°、focus10 m、distortion disabledである。
- 単位は1 world unit=1 mとし、利用者・viewerへの適合値として扱わない。
- profileはversion、単位、眼別FOV、lens center、focus/projection、disabled/radial等のdistortionモデルを保持できる。

### WebXRのmountとreset

- 明示操作からimmersive-vr sessionを要求し、reference spaceはlocalとする。
- runtimeのhead pose、眼別projection、XR loopを使用する。
- Three.jsのXR cameraはruntime値で更新されるため、機体とEye mountを親tracking originへ保持し、各眼を$A E H_{eye}$として構成する。
- 外部Replay rigは$C H_{eye}$とする。

- 共有Rustが機体・pilot・camera mountと累積tracking補正$C$を合成する。head・左右眼・controller rayは同じmount$M C$を一度適用する。
- 論理referenceのWorld poseを$R$、機体poseを$A$とし、World anchorは$R^{-1}$、Cockpit anchorは$R^{-1}A$、Head anchorは$R^{-1}M C H$で解決する。全panelは$R P$として描画し、選択も同じreferenceで行う。
- Menuは開いた時のreference内poseを保持し、scene・overlay・実測寸法・両眼光学・camera基底の変更時に再配置する。機体の通常移動は同じ基底として扱う。
- Menu開口の光学比較は、眼位置0.1 mm・眼回転0.1 mrad、projection成分の差が`1e-4 × max(1, |前値|, |現在値|)`以内を同値とする。float32相当の観測揺れを吸収し、これらを超えるIPD・視錐台変化で再fitする。
- [reference-space reset](https://www.w3.org/TR/webxr/#dom-xrreferencespaceevent-transform)の$S$は新originの旧tracking内poseである。共有ownerが$C'=C S$を保持し、更新後の$H'=S^{-1}H$と合成してWorldと眼の位置を保つ。

```math
M C' H'=M C S S^{-1}H=M C H
```

- reset変換が得られない場合は選択を停止してScreenへ戻し、再開始を案内する。
- reset直後は次の描画が完了するまでselectを停止する。Phone VRの明示tracking recenterは視線のyawを更新し、保持Menuを専用操作で輸送する。Menuの正面再配置と受動resetは別操作である。
- 床高・IPD・projectionはXR runtimeの出力を使用する。

### VR入力と安全

- Screenはkeyboard/pointer/touch、VRはhead-gaze dwellまたは標準mappingのgamepadを用いる。
- WebXRとPhone VRは同じframe・参照空間の頭部poseを共有Rustのpointer ownerへ渡し、正面方向の視線rayを導出する。WebXRは各入力源のray・handedness・gamepad生値を同frameで渡し、Phone VRの標準gamepadと同じownerが実測・clip済み矩形への交差、focus、1秒dwell、押下edgeを処理する。左右controllerの照準とselectイベントは同じ交差判定を使う。
- panelへ向いたcontroller、接続済みgamepad、gazeの順にfocusを解決する。gamepadは接続slotと世代で識別し、初回・接続交代・focus復帰では保持中ボタンを観測してから次の押下を受理する。
- `visibilitychange`・blur・reference reset・recenterは入力を失効させる。Scene/overlay/layout交換はfocusとdwellを更新し、同じmenuのscrollによる矩形変更ではgamepadのカーソル積分と押下履歴を保持する。入力源の喪失後は新たなgaze dwellを開始する。
- dwellとtriggerの時刻は同じ単調な受信時計へ揃える。XR rAFの現在時刻とselect配送時の`performance.now()`を使用し、予測表示時刻はpose観測に限定する。照準はpanelの非等方scaleを相殺して実寸座標へ配置する。
- gamepadの左stickはpanel cursor、button0は選択、button1は共有UIのBack、右stickはviewport単位のscrollへ接続する。同時押下ではBackを優先する。
- `xr-standard` controllerはグリップの新規押下を「戻る」へ割り当て、thumbstick Yでscrollする。deadzone内ではtouch中のtouchpad Yを使用する。照準中もnavigationを受理し、全入力源の新規Backをscrollより優先、同種操作の競合は右手を優先する。trigger選択はselectイベントだけから受理し、Frameとの実到着順で処理する。

- dwellの進行と取消を示し、視線を外して確定を中止できる。
- Scene切替後の操作には、新しい入力開始を必要とする。
- hoverや装着中touchに依存せず、戻る、pause、退出、tab、scroll、数値変更へ到達できるようにする。
- head trackingは視線、keyboard・gamepadは操縦を担当する。

- Phone VRの並進は固定し、Pause中もhead trackingを継続する。
- 背景はworld基準を保持し、頭部回転による見え方の変化を表現する。
- WebXRのtracking自由度は機器に従う。
- fullscreen/landscape lockは補助であり、拒否や非対応から通常表示へ復帰できる構成とする。

## 性能・受入の区分

PC ScreenはWeb/Nativeとも安定80 FPS相当・12.5 ms/frame、スマートフォンは60 FPS、XRはruntime refreshに対応するframe budgetを目標とする。達成状況は指定端末・実描画負荷で測定する。

- 性能計測はReleaseでOS/GPU、物理解像度、DPR、画質、refresh、VSync、計測区間を固定し、平均とframe-time percentileを記録する。
- 60 Hz同期の実表示FPSと80 Hz以上の性能受入を区別する。
- WebのFPSは成功したrender callback、Nativeは実時間Update頻度の750 ms窓であり、100 Hz物理tickやGPU present完了の計数ではない。

- 数式・Float32 upload・CPU生成画像の検査、本番shaderのGPU実行、画面の視認性、物理入力、実Phone/HMDの光学受入を区別する。
- CPU数値、native/WASM転送、GPU画像、実端末操作は、それぞれ対応する検証結果で評価する。
- 時間的aliasing、optic flow、parallax、80 FPS、各viewerでの可読性は対象条件で確認する。
- 実行コマンドと現在の未達事項は[検証](verification.md)へ集約する。

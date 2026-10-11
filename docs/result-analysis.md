# Result Summary・Analysis契約

## 画面構成

ResultはSummaryとAnalysisのtab、およびRetry / Replay / Setup / Titleの操作を持つ。
主要GameSceneは追加しない。Replayへ移動する場合は選択時刻を渡し、復帰時にtabとcursorを維持する。
すべての表示は `flight-record.md` のimmutable recordを参照する。
WebのAnalysis Modelはcontext付きnamed datasetの元値だけを保持する。
グラフとcamera用の共通Viewは純粋な投影で導出し、named sample・Summary・風断面・cursorの保存contextを照合する。
保存済み合成重心とtelemetryを使用し、元finalization/causeおよびAoA・score・風断面の欠損理由を保持する。
風断面の未要求と未登録環境・登録領域外を区別する。Attractの確定距離と再生cursor位置も区別する。
終了理由、確定可能性、Retry blueprint、score、Summary metricsはRust coreの結果を正本とする。
WebはResult/Analysisのtab・focus・表示範囲を管理し、ゲーム規則やmetricsを再計算しない。

SummaryはRust coreが返すversioned course-distance scoreとFlight Timeを主表示とし、cross-track displacement、net horizontal displacement、最大対気速度、最大重心AoA、最大絶対roll、
解決済みpreset・三軸設定、scenario catalog／scenario／aircraft／environment／controllerのversion、seed、代表風を併記する。
異常終了と着水を区別し、欠損値はunavailableと表示する。

## 最小Analysis

| 表示 | 内容 |
|---|---|
| Horizontal map | 軌跡、Start/終端、方位、scale、湖岸・platform・主要地物、風 |
| Altitude vs time | 重心高度、静水面0、発進高度、共有cursor |
| Airspeed / Groundspeed vs time | 重心の対気・対地3D速度normを同時表示、共有cursor |

3D worldはThree.js、HUDはHTML/CSS/SVG、グラフはSVGまたはCanvasとする。
これはScreenでの表示方法である。VRでは同じview modelと2D描画結果を両眼で読めるpanelへ適用する。
Result、Analysis、Replay操作はVR sessionを維持して実行できる。DOM Overlay対応を必須にしない。
グラフ描画のためにThree.js Sceneを追加しない。
SVG/Canvas選択はsample数、操作性、アクセシビリティの測定後に決定する。
pointer hoverだけでなくtouch・keyboardでも共有時刻を選択でき、数値の代替表示を提供する。
軌跡・高度・速度・風の元系列とSummary集計はcoreから受け取り、軸変換・layout・描画・hover hit-testをWebが担当する。

## Horizontal map

東を右、北を上とする等縮尺のlocal NED平面を用いる。実描画のy反転は2D adapterに限定する。
軌跡は各record sampleに保存された合成重心 $G$ のNED水平位置で統一する。Webでbody stateから重心位置を再計算しない。
着水点とscoreは接触時刻の終端sampleから算出し、接触点位置との区別を表示する。
全体軌跡に余白を加え、静止・極短距離・同一直線軌道でも表示範囲が退化しないようにする。
湖岸line、platform、多景島等はworld assetと同じorigin/versionを使用する。
座標・利用条件はmanifestに登録する。外部の地図tileや衛星画像取得を必須にしない。
近距離表示で範囲外となる地物はoverviewまたは注記で扱い、軌跡を過度に縮小しない。

Start、WaterContactならSplash、それ以外は理由付きEndを示す。
100 m等の距離目盛とscale bar、1秒または5秒等のtime markerはzoomと表示密度で間引く。
軌跡の色は時刻または高度とし、凡例を表示する。色だけに依存せずmarker・数値を併用する。
共有cursorのmarkerは原sampleから補間し、間引き後のpolylineを数値の正本にしない。

代表風はsample位置・高度とともに表示する。粗い5×5程度の風gridは選択高度を明記し、
矢印を空気の移動先方向に描く。気象の「吹いてくる風向」と矢印の意味を区別する。
gridは固定NED高度の水平断面であり、軌跡上の各高度における局所風とは区別する。
高さの異なるsampleを同一平面の風として混合しない。鉛直流は別の色・数値等で明示する。
map用の風断面はvalidated scenarioからWASM queryで取得してcacheできる。
Flight記録上の重心風は変更しない。scenarioが欠けるrecordでは疎gridを非表示にし、
保存済みの軌跡上風を表示する。queryはFlightのsimulation stateを更新しない。
最初の実装では発進時の重心高度を固定断面とし、record軌跡と合成world地物を含む等方範囲へ5×5点を配置する。
水平成分は同一scaleの矢印で示し、鉛直成分の範囲と断面高度を数値で表示する。

## 高度と速度

高度は静水面基準の重心高度h=-Dとし、接触点高度と区別する。
空力・運用根拠が未確認の警戒帯や速度安全域を任意に追加しない。
速度系列は単位と3D normであることを明示する。横風・鉛直流を含む場合、
対気速度と対地速度の大小だけではheadwind/tailwindを判定できない。
説明には発進軸への風投影や、同方向の一様風など前提を明記する。

風の時系列は追加機能とし、記録済み重心風から風速、発進軸平行・直交成分、鉛直成分を生成する。
AoA、roll/pitch、pilot/FBW/actuatorの系列はAdvanced Analysisとして追加できる。
局所翼AoAや荷重は追加diagnosticが保存されたrecordだけで提供する。

## Cursor・性能・検証

ResultとReplayで単一のrecord-time cursor値を共有し、seek範囲・補間sample・再生速度・再生状態はRust core queryが返す。Webはwall-clock schedulerと入力adapterを担当し、再生時刻を独立計算しない。
coreは有効record区間外・非有限時刻・不正recordを拒否する。Webは受信値でcursor位置と描画を更新する。
Resultの初回cursorとrange操作は、Replayへの遷移を経ず、確定済みrecordの秒単位queryを使用する。
`GameSession::playback_sample_at_seconds`はResult/Replay/Attractのread-only queryであり、
Attractでは独立demo record、それ以外では保持中のflight recordを参照する。
queryはphase・score・record・physics state・再生clockを変更しない。
seek・play/pause・rate・advanceのclock操作はReplay/Attractだけに許可する。
graph側のhover/dragはResultの照会時刻またはReplayの再生時刻だけを変更し、元record・physics・Personal Bestを更新しない。
大量sampleは表示用に間引いてよいが、端点・極値・eventを保持する。
画面resizeや端末画質で集計値を変えない。

BPG-020で既知軌道・wind・高度・速度のfixture、軸方向と等縮尺、短いrecord、欠損diagnostic、
終端種別、cursor同期、touch/keyboard、表示用間引きの極値保持を検証する。
VRのgamepad/head-gaze等によるtab・scroll・cursor・Replay遷移も同じ受入条件で検証する。
BPG-021で同じ時刻のmap marker・graph・3D poseの一致とResultへの復帰を検証する。
`wasm-screen-transition.test.ts`は実WASMを使用し、Result進入時の解析取得から初回cursor effect、
Analysisのrange effect、成功応答までを接続する。整数終端とfractional着水終端、
不正時刻のtyped error、Resultでのclock操作拒否、query前後の不変性を検査する。
最初のmapはversion付き`synthetic-training-basin` assetを用い、実地理assetの取得を待たず完成させる。originはscenario発進時の合成重心を基準とするlocal NEDである。湖岸・platform・小島の座標は模式的な非地理データであり、既知のsynthetic scenario catalogだけに適用する。実地理形状は別versionのworld assetで管理し、このfixtureから推定しない。

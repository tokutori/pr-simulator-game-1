# 多軸難易度とフライト設定

## 不変条件

難易度はゲーム設定であり、Information・Assistance・Weatherの三軸で構成する。

```math
D=(D_{information},D_{assistance},D_{weather})
```

すべての設定で6DoF方程式、空力係数、風の作用、actuator dynamics、数値積分、
機体・パイロットの質量特性算出規則を共通とする。慣性の実値は身体位置に応じて変化する。
同一の初期機体・身体状態、AircraftModel、Environment、舵actuator入力列、
身体目標位置列、積分刻みからは同一の物理応答を得る。
native/WASM間は `verification.md` の数値許容差で比較する。
難易度に応じた揚力増幅、抗力低減、姿勢の直接修正を禁止する。

```text
DifficultySettings → deterministic configuration resolution
                   ├─ Information → HUD/presentation
                   ├─ Assistance  → controller → actuator → physics
                   └─ Weather     → scenario → Environment → physics
```

physics APIは機体・身体state、舵actuator状態、身体位置指令、aircraft、environment、dtを受け取り、Difficulty型を持たない。
HUD非表示でも物理量の計算・記録規則は共通とする。ゼロ対気速度等でAoAが未定義となる場合は
既存の型付き未定義状態を維持し、表示のために数値を捏造しない。

## 型と配置

`birdman-game-format` のconfiguration境界に、外部設定の型、検証、preset展開を配置する。
CLIとWASMは共通の変換を使用する。Web側で同じpreset定義を重複実装しない。
解決結果はInformation設定、coreのControllerConfig、有効なScenario/Environment参照に分離する。
coreにDifficulty/Preset型を追加しない。Flight Setupと画面遷移は `web/src/app` に配置する。
formatが保持するInformation metadataをcoreへ渡さない。

設定はenum/structとvalidated constructorで扱う。文字列や範囲外authorityをcoreへ通さない。
scenario分類からの選択にはcatalog versionと明示的seed/選択規則を使用し、clockや暗黙乱数を参照しない。

## Information

Full / Standard / Minimal / Realistic / Customを用意する。
Fullは対気速度、対地速度、高度、AoA、風、姿勢、flight-path marker、警告を表示する。
Standardは主要計器、Minimalは高度・距離・時間、Realisticは対象機の実際の計器構成に近づける。
対象機の計器構成が未確定の段階では、Realisticを実機と同一と表示しない。
Customはtelemetry、attitude、wind、flight-path、AoA、warning cueの表示設定を保持する。
Flight Setupはプリセットによる三軸の一括選択と、各軸の直接選択を分離する。
候補一覧・現在値・説明をScreen/VR共通のviewへ投影し、設定変更は既存GameSession setterへ送る。
Custom presetは個別変更済みの表示とし、選択候補に含めない。
Briefingは飛行条件・操縦方法・準備結果を確認する画面とする。version/seedは初期状態で閉じた技術情報へ格納し、
開閉はAppModelと純粋なupdateで管理する。代表風・地点・高度・空間変動はWASMのenvironment snapshotを表示し、
取得不能を明示する。風速・風向・発進可否をWeatherの名称から推定しない。
代表水平風速と風向は公開されたN/E成分の純粋な表示導出とする。風向は真北を0°とする時計回りの方位で、
[NOAA/NWSの定義](https://www.weather.gov/ggw/GlossaryW)に従い吹いてくる方向を示す。水平風が0の場合は方向なしと表示し、鉛直流は上昇・下降を別記する。
FlightはPilot視点に固定する。Replay/Attractのcamera選択はInformation presetから独立させる。
情報補助の低減でアプリケーションエラー、permission状態、退出操作を隠さない。

## Assistance

Strong / Assisted / Light / Manual / Customを用意する。実際の制御は検証済みControllerConfigへ解決する。
初期版ではauthority $a$ を使用する。

```math
u_{surface}=(1-a)u_{pilot,surface}+a u_{FBW,surface},\qquad 0\le a\le1
```

manual modeはa=0、automatic modeはa=1、shared modeは混合設定を使用する。
このauthorityは舵指令へ適用する。身体の目標位置は `pilot-motion.md` に従い別経路で扱う。
Assistance presetはこれらのmodeと有効なパラメータへ展開する。
Strong/Assisted/Lightの具体値は閉ループ検証で決定し、例示値を確定値として登録しない。
共通actuatorがcommandを受け取り、飽和・rate limit・遅延を計算する。
actuator特性を難易度で変更しない。
将来のpitch stabilization、roll stabilization、yaw damping、input shaping、AoA protectionは
明示的なcontroller設定として追加し、物理荷重やstateを直接変更しない。
Automaticの展示運用も同じ解決済み設定として結果へ記録する。

## Weather

Calm / Mild / Typical / Challenging / NearLimitはscenario metadata上の分類とする。
分類から実在するversioned scenarioを選択し、WindField・WaveState・SkyStateを一貫して確定する。
難易度の数値から風速だけを生成しない。
空間勾配、鉛直成分、crosswind、変動幅と発進方向により、同じ平均風速でも操縦条件が異なる。
数値範囲と「7月末の典型」はBPG-008の根拠確認により確定する。
初期のゲーム骨格は明示的な合成scenario catalogで検証する。実データ導入前にTypical Julyを装って提供しない。
未搭載scenarioを必要とするpresetは非対応理由を表示し、別の気象へ黙って置換しない。
Customで利用するfieldも同じ検証・version管理を通す。
NearLimitはゲーム上の分類であり、大会の公式運用限界や発進可否を保証しない。

Weather変更はEnvironment/scene環境の選択へ限定する。
描画は同じscenarioの波・空・雲・太陽から構成し、weather由来の視認性変化は物理環境の表現として扱う。
Information軸はHUD・情報表示の量を制御する。
Flight Setupにはscenario名、代表風向風速、代表値の地点・高度、空間変動、鉛直流、
ゲーム上の発進条件を表示する。単一点の風を全fieldの値として表示しない。

## PresetとCustom

| Preset | Information | Assistance | Weather |
|---|---|---|---|
| Beginner | Full | Strong | Mild |
| Standard | Standard | Assisted | Typical |
| Expert | Minimal | Light | Challenging |
| Realistic | Realistic | Manual | Typical |
| Custom | 個別設定 | 個別設定 | 個別選択 |

Realisticは現実の搭乗情報・操縦条件への近似を意図する名称であり、最大難易度を表さない。
preset選択で三軸を更新し、任意の軸を手動変更した時点でCustomと表示する。
画質やpresentation backendは難易度の軸に含めない。

## Game flow

主要8 Scene、overlay、停止・Retry契約は `game-flow.md` に従う。
Flight Setupではpresetと三軸の具体値を同時に確認できる。
Briefing開始時に解決済みconfigurationを固定し、Countdownでは維持する。初期版では飛行中の三軸変更を許可しない。
backend切替やpause/resumeは独立した表示・session操作として扱う。
Resultには距離・時間とInformation、Assistanceの実値、scenario名と代表気象条件を保存する。

Personal Bestは表示preset名で分類せず、解決済みconfigurationのcanonical keyで分離する。
keyにはInformation項目、ControllerConfigとversion、scenario ID/version/hash/seed、
AircraftModel/version/hash、physics version、launch条件、score定義versionを含める。
同じ設定へ解決されるpresetとCustomは同じ比較群とする。
画質やbackendは記録metadataとして保持し、物理条件のkeyから分離する。

Replay headerには上記configuration、format version、dt、座標契約versionを保存する。
再現に必要なtick入力列、初期状態、全資産version/hashを保持する。
不明versionや不足assetは型付きエラーとし、別の現行モデルへ黙って置換しない。
入力再現の成立と機体モデルの実機妥当性を区別する。

## BPG-017の完了条件

1. preset展開とscenario選択が同じcatalog・seedで決定的となる。
2. 一軸の手動変更でCustom表示となり、他軸は保持される。
3. Information変更がphysics state・controller出力へ影響しない。
4. Assistance変更はcontroller経路だけへ作用し、aircraft/actuator/environment設定を変更しない。
5. Weather変更はscenario/environmentとその景観だけへ作用する。
6. 同一の初期機体・身体状態、AircraftModel、Environment、舵actuator入力列、身体目標位置列、刻みから、presetに依存しない軌道を得る。
7. 基本Resultまで解決済み三軸とモデルversionを保持する。保存・PB比較はBPG-019、ReplayはBPG-021で同じ契約を検証する。

3〜6はlayer境界を通したregression testとする。6では異なるpreset metadataを伴う入力を解決してから
共通物理入力を与え、数値状態とstep列の一致を確認する。
実データの有無とresolverの正しさを分離し、合成catalogでもすべての解決規則を試験する。

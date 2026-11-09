# 機体JSON作成ガイド

自作の機体JSONを作成し、ゲームへ読み込ませるための案内である。開発者向けの規則は[データと形式](data-and-formats.md)の「機体JSON」節、実行規則は[飛行力学](flight-dynamics.md)に定める。フィールド定義と検証条件の正本は `crates/birdman-game-format/src/aircraft.rs` であり、本ガイドはその読み替えである。

## 作成の流れ

1. 登録機体 `assets/aircraft/playable-glider-v7.json` を複製する（出発点: 検証を通過する完全な文書であり、一部の項目だけを書き換える変更が最も確実である）。
2. `configuration_id` を自分の識別子へ変更する（識別: 機体は `configuration_id` と原文bytesのSHA-256で区別されるため、IDが同じでも内容が変われば別の機体として扱われる）。
3. 後述のリファレンスに従って項目を書き換える。
4. 検証する（CLIまたはviewer、方法は「検証とエラーの直し方」節）。
5. ゲームの飛行前設定画面から読み込み、飛行する（保存先: WebはlocalStorage、Nativeは利用者設定ディレクトリの `birdman-game/aircraft.json`。保持できる自作機体は1件であり、新しく読み込むと置き換わる）。

## 共通の約束

- 形式: UTF-8のJSON、最大256 KiB。先頭のBOMは1個まで無視される。
- 版: `schema_version` は現行の5のみ受理する。旧版・新版は受理せず、警告付きの読み替えも行わない。
- 未知フィールド: 全階層で拒否する。綴りの誤りは意味不明な項目としてエラーになるため、項目名は本ガイドと登録機体の表記に合わせる。
- 座標: 機体固定のFRD（x=前、y=右、z=下）で、原点は datum O である。寸法はSI単位とし、単位は項目名の接尾辞（`_m`・`_kg`・`_rad`・`_mps`・`_mps2`・`_kg_m3`・`_nm2`・`_m2ps`）で示す。
- 数値: 全ての数値は有限値でなければならない（`NaN`・無限大は不可）。
- 検証の段階: 構文・型・未知フィールドの検査、意味検証（値域・順序・整合）、準備検証（トリム解の成立など）の順に行い、意味違反は全件を集めて返す。
- 導出量: 参照面積・スパン・平均空力翼弦・重心まわりの合成慣性・トリム解は文書から導出されるため、項目として指定しない。

## トップレベル

| 項目 | 内容 |
| --- | --- |
| `schema_version` | 文書版。現行は5で、固定する |
| `configuration_id` | 機体の識別子。1〜128文字のASCII英数字・`-`・`_`・`.` |
| `model_version` | 物理パラメータと形状の版。1以上の整数 |
| `mass` | 機体（パイロットを除く）の質量特性 |
| `pilot` | 移動するパイロットの質量特性と移動制限 |
| `surfaces` | 翼面の配列。ちょうど3要素で、`role` が `main_wing`・`horizontal_tail`・`vertical_tail` を各1回ずつ担う |
| `polar` | 全機の静特性表 |
| `structure` | 主桁・コックピット・部材の構造形状 |
| `trim` | 定常滑空の運用点と迎角の探索区間 |

## mass（質量）

| 項目 | 内容 |
| --- | --- |
| `airframe_kg` | 機体質量（kg）。正の値 |
| `airframe_center_of_mass_frd_m` | 機体重心の datum O からのFRD座標（m）。datum は仮の中心であり、正確な重心はこの値で指定する |
| `inertia_about_datum_kg_m2` | datum まわりの慣性テンソル（kg·m²）。3×3の対称行列とし、平行軸換算後の重心まわり慣性が物理的に成立する（正定値である）ことまで検査される |

## pilot（パイロット）

| 項目 | 内容 |
| --- | --- |
| `mass_kg` | パイロット質量（kg）。0以上 |
| `vertical_offset_m` | datum からの下向き固定オフセット（m） |
| `position_bounds_m` | 前後移動の範囲（m）。`[最小, 最大]` で最小 < 最大。重心移動の範囲となる |
| `maximum_speed_mps` | 前後移動の最大速度（m/s）。正の値 |
| `maximum_acceleration_mps2` | 前後移動の最大加速度（m/s²）。正の値 |

## surfaces（翼面）

平面形と上反角は専用項目では表さず、各断面の四分の一翼弦点の置き方で表す。

| 項目 | 内容 |
| --- | --- |
| `role` | `main_wing`・`horizontal_tail`・`vertical_tail` のいずれか |
| `mirror_span` | 断面列をスパン座標0について鏡像にするか。主翼と水平尾翼は通常 `true`、垂直尾翼は `false` |
| `sections` | 断面の配列。スパン方向へ単調な順序で並べ、2〜128個 |
| `strip_fractions` | 空力stripの分割境界。0から始まり1で終わる狭義増加列で、2〜128個。鏡像する面では対称（$a_i + a_{n-1-i} = 1$）であること |
| `lift_anchor` | `[基準の法線力係数, その入射角（rad）]`。入射角は $-\pi/2$ から $\pi/2$ の間（両端を含まない） |
| `tail_aerodynamics` | 尾翼のみ必須で、主翼は `null` |
| `airfoil` | 面に共通の翼型 |
| `actuator` | 尾翼のみ必須で、主翼は `null` |

### sections の各断面

| 項目 | 内容 |
| --- | --- |
| `quarter_chord_frd_m` | 四分の一翼弦点のFRD座標（m）。主翼・水平尾翼はy座標、垂直尾翼はz座標がスパン方向となる |
| `chord_m` | 翼弦長（m）。0以上で、翼端の0は許される |
| `twist_rad` | 断面のねじり角（rad）。後退を含む3Dの四分の一翼弦接線まわりの右手系回転で、範囲は $-\pi$ から $\pi$。水平翼では前縁上げが正、鏡像する面では左右で同じ値にする |

### airfoil（翼型）

NACA式の平均線と厚みのパラメータで、面ごとに1つを指定する。

| 項目 | 内容 |
| --- | --- |
| `camber_ratio` | 最大反り率（翼弦比）。0〜0.5 |
| `camber_position_ratio` | 最大反りの位置（翼弦比）。0より大きく1より小さい |
| `thickness_ratio` | 最大厚み率（翼弦比）。0〜0.5で、0は平板を表す |

### tail_aerodynamics（尾翼の空力係数）

尾翼の合成stripモデルに使う係数である。主翼の空力は全機 `polar` が担う。

| 項目 | 内容 |
| --- | --- |
| `zero_lift_drag_coefficient` | ゼロ揚力抗力係数。0以上 |
| `span_efficiency` | スパン効率。0より大きく1以下 |
| `pitching_moment_coefficient` | 局所四分の一翼弦まわりのピッチングモーメント係数 |
| `reference_airspeed_mps` | 合成形状抗力の基準対気速度（m/s）。正の値 |
| `reference_kinematic_viscosity_m2ps` | 基準動粘性係数（m²/s）。正の値 |

### actuator（尾翼の駆動上限）

尾翼取り付け角の機械的な上限であり、機体の物理量として機体JSONが定める。controllerが使用する範囲はこの内側に別途定められる。

| 項目 | 内容 |
| --- | --- |
| `incidence_bounds_rad` | 取り付け角の可動範囲（rad）。`[最小, 最大]` で最小 < 最大、0を含み、$-\pi$ から $\pi$ の範囲内 |
| `maximum_slew_rad_per_second` | 駆動速度の上限（rad/s）。正の値 |

## polar（全機静特性）

主翼形状を参照とする全機の係数表であり、迎角ごとに独立した区分線型列として補間される。

| 項目 | 内容 |
| --- | --- |
| `method` | 表の出自。`vortex_lattice`・`lifting_line`・`panel`・`software_fixture` のいずれか |
| `moment_axes` | モーメントの軸約束。`body_frd` または `wind_at_beta_zero`（横滑り0の風軸） |
| `reference_point_frd_m` | 表のモーメントの参照点のFRD座標（m） |
| `rows` | 行の配列。2〜128行で、`alpha_rad` は狭義増加とする。`trim.alpha_bounds_rad` の区間全体を覆うこと |

各行の項目は次のとおりである。抗力係数2種は0以上とし、他は符号を問わない。

| 項目 | 内容 |
| --- | --- |
| `alpha_rad` | 迎角（rad） |
| `lift` | 揚力係数 |
| `induced_drag` | 誘導抗力係数 |
| `profile_drag` | 形状抗力係数 |
| `side_force` | 横力係数 |
| `roll_moment` | ロールモーメント係数 |
| `pitch_moment` | ピッチモーメント係数 |
| `yaw_moment` | ヨーモーメント係数 |

## structure（構造）

表示形状と水面接触の形状はこの節から導出される。強度の実証値ではなく、形状と物理形状の定義である。

| 項目 | 内容 |
| --- | --- |
| `main_beam_endpoints_frd_m` | 主桁の前端・後端のFRD座標（m）。前から後ろへ向かう非ゼロの梁であること |
| `main_beam_radius_m` | 主桁の半径（m）。正の値 |
| `wing_flexural_rigidity_nm2` | 主翼の曲げ剛性EI（N·m²）。正の値。翼端のたわみは全備重量を支える等分布荷重の片持ち梁モデルからこの値で決まる |
| `cockpit_stations` | コックピット殻の断面列。2〜128個で、`forward_m` は機首から後方へ狭義減少する |
| `cockpit_rounding_exponent` | 断面の超楕円指数。正の値 |
| `cockpit_frame_radius_m` | コックピット枠の管半径（m）。正の値 |
| `pilot_eye_offset_frd_m` | パイロット視点の取付位置（FRD、m）。前後移動前の位置 |
| `members` | 部材配置（次項） |

コックピット断面（`cockpit_stations` の各要素）:

| 項目 | 内容 |
| --- | --- |
| `forward_m` | 前後座標（m） |
| `center_down_m` | 殻中心の下向き座標（m） |
| `half_width_m` | 半幅（m）。正の値 |
| `half_height_m` | 半高（m）。正の値 |

### members（部材配置）

部材は全体で最大512個まで導出される。各半径は正の値、割合の列は `[0, 1]` の狭義増加列とする。

`wing`（主翼）:

| 項目 | 内容 |
| --- | --- |
| `span_segments` | 半翼あたりの桁・ストリンガ区間数。1〜128 |
| `spar_radii_m` | 桁の根元・翼端の半径（m）。各区間は中点の半径を使う |
| `rib_count` | 半翼あたりのリブ数。0〜128で、根元と翼端の間に等間隔に置く |
| `rib_radius_m` | リブ管の半径（m） |
| `rib_chord_knots` | リブ管を構成する翼弦方向の割合列。2個以上 |
| `stringer_chord_fractions` | ストリンガの翼弦位置の割合列 |
| `stringer_radius_m` | ストリンガの半径（m） |
| `foam_chord_fractions` | フォーム断面の翼弦方向の割合列。2個以上 |
| `foam_half_width_m` | フォームリブの半幅（m） |

`horizontal_tail`・`vertical_tail`（尾翼、いずれも同じ形式）:

| 項目 | 内容 |
| --- | --- |
| `rib_intervals` | スパン方向の区間数。1〜128で、内部の節点にリブが1つずつ置かれる |
| `rib_radius_m` | リブ管の半径（m） |
| `rib_chord_bounds` | リブ管の翼弦方向の両端の割合 |
| `foam_chord_bounds` | フォームパッチの翼弦方向の両端の割合 |
| `foam_half_width_m` | フォームパッチの半幅（m） |
| `foam_normal_offset_m` | 翼面から上面方向へのオフセット（m）。0以上 |

`tail_support_radius_m`: 垂直尾翼の固定支持管の半径（m）。

`cockpit`（コックピット枠）:

| 項目 | 内容 |
| --- | --- |
| `nose_station` | 前方枠の接続先となる `cockpit_stations` の番号（0以上、`cockpit_stations` の個数未満） |
| `upper_station_fractions` | 上部枠端点の、機首から後方への割合の組 |
| `lower_station_fractions` | 下部枠端点の割合の組 |
| `beam_attachment_fractions` | 主桁への取付管の割合の組 |
| `cross_radius_m` | 直立・横枠管の半径（m） |
| `diagonal_radius_m` | 斜材の半径（m） |
| `attachment_radius_m` | 取付管の半径（m） |
| `rim_station` | キャノピー開口の縁を担う `cockpit_stations` の番号（0以上、`cockpit_stations` の個数未満） |
| `rim_segments` | 開口縁の直線区間数。4〜128 |
| `rim_radius_m` | 開口縁の管半径（m） |

## trim（トリム条件）

定常滑空の成立を解く運用点である。トリム解が存在しない文書は検証を通過しても飛行を開始できない。

| 項目 | 内容 |
| --- | --- |
| `airspeed_mps` | 対気速度（m/s）。正の値 |
| `air_density_kg_m3` | 空気密度（kg/m³）。正の値 |
| `gravity_mps2` | 重力加速度（m/s²）。正の値 |
| `alpha_bounds_rad` | トリム探索の迎角区間（rad）。`[最小, 最大]` で最小 < 最大、各値は $-\pi/2$ から $\pi/2$ の間（両端を含まない）。かつ `polar` の表の範囲内であること |

## 検証とエラーの直し方

作成したJSONはゲームへ読み込む前に検証できる。

- CLI: `cargo run -p birdman-game-cli --locked -- validate-aircraft <path>`（実行時と同じ形状・空力・トリムの検査を行う）
- viewer: `tools/aircraft-viewer` で開くと、検証結果に加えて設計値・導出諸元・トリム・平面形を確認できる。

エラーの読み方:

- 構文・型・未知フィールドのエラー: JSONとして受理できない段階の失敗であり、該当箇所の綴りと型を直す。
- 意味検証のエラー: 分類コード・JSON path・入力値・期待条件の組で全件が返る。分類は `SchemaVersion`・`Identity`・`Mass`・`Inertia`・`Pilot`・`Surface`・`Twist`・`Actuator`・`Polar`・`Structure`・`Trim` の11種であり、pathの指す項目を期待条件に合わせて直す（例: `/trim/alpha_bounds_rad/1` が表の範囲外であれば、`polar` の行を増やすか探索区間を狭める）。CLIの表示は `{path}: {value} (expected {expected})` の形であり、分類コード名はviewerの検証結果で確認する。
- 準備検証の失敗: 文書単独では有効でも、トリム解が存在しない等の理由で飛行を開始できない状態を指す。`trim` の運用点と `polar` のピッチモーメント列の釣合を見直す。

読み込みに失敗した保存済み機体は自動では削除されず、既定機体で起動して理由と削除操作が表示される。

## 変更の指針

- 質量: `mass.airframe_kg` と `pilot.mass_kg` の合計が全備重量となり、たわみ量とトリムの双方へ効く。
- 重心: `mass.airframe_center_of_mass_frd_m` の前後位置がトリムと安定性を決める。変更したら必ずトリム成立を検証する。
- 平面形: 断面の追加・移動でテーパー・後退・上反角を表す。参照面積とスパンは主翼から導出され、`polar` の係数はその参照で定義されている点に注意する。
- 剛性: `structure.wing_flexural_rigidity_nm2` を下げるとたわみが増え、水面接触の形状へも影響する。

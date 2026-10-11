# 共通ゲームUIと生成境界

`birdman-game-session::game_ui`をゲームUIの正本とする。`GameSession`がdomain phase・record・playback clockを所有し、
`GameUiState`が表示tab・disclosure・overlay・解析時刻・Replay cameraを所有する。
viewport、presentation resource、保存先、focus、scroll、font、GPU resourceはplatform adapterの責務である。

`game_ui_view`は文言、候補、順序、group、action、表示条件、基本配置、theme、HUDと記録queryからのchartを導出する。
`update_game_ui`は実際に公開した有効な操作を受理し、UI状態と型付きeffectを返す。
WebとBevyは同じViewとeffectをDOM・VR panel・ECSへ投影する。
rendererはゲームのphase、選択条件、score、物理値、解析系列を独立して導出しない。

## Schema生成

Rustのserde型とSchemarsを境界の正本とする。

```sh
npm run generate:game-ui
npm run check:game-ui
```

生成処理はCLIの`export-game-ui-schema bundle`からSerializeとDeserializeの共通定義を取得する。
各groupは独立したSchemarsの`SchemaGenerator`を使用し、同じRust型を一度だけ`$defs`へ登録する。
View、update、frameはSerialize、Action、platform input、TailInputはDeserializeの契約を使用する。
Ajvは各groupを`$id`で登録し、公開validatorを共通定義への参照から生成する。
`allErrors`とnumeric formatsを維持し、同じgroupのvalidatorを再利用する。
runtimeの深いreadonly型とstandalone validatorは`web/src/generated/`へ生成する。
build専用のJSON Schemaとgeneration stampは`tools/game-ui-build/generated/`へ生成する。
ライセンス通知は`assets/game-ui-validators.LICENSE.txt`へ生成し、出典・version・hash・帰属を`assets/manifest.toml`に登録する。
固定dependencyを更新して通知の内容が変わった場合は、生成後にmanifestのversionとhashを更新する。
Webは登録済み通知の静的URLを既存HUDの帰属linkへ接続し、Viteのasset配布経路へ含める。
Ajvはstrict schema・finite number・整数範囲・未知field・未知variantを検査する。
standalone validatorはbrowser用ESMへbundleし、実行時の`eval`を使用しない。
生成物を手編集せず、Rust契約を変更して再生成する。freshness検査は実際のSchemaから全生成物を再導出して比較する。

## Platform差分

メニューは既存Webの設定フォーム・確認・primary CTA・secondary CTA・技術情報を基準とする。
logical pixel、600px breakpoint、最大scroll高、候補列数は共通Viewの値を使う。
OSの表示倍率、safe-area、font metrics、入力イベント、focusとscrollの保持は各adapterで処理する。
未実装機能は明示capabilityから共有Viewが無効化する。adapter固有の候補集合や無言の代替動作を追加しない。

共通chartは軸、系列、segment color、風vector、marker、参照線とcursorを含む。
rendererはGPU・SVG・ECSの座標へ投影する。記録sampleや空力を再計算して別の解析値を生成しない。
Native経路は共有Rustを直接利用し、Schema生成用Node.jsやWASM runtimeを起動条件にしない。

構造Viewは表示条件、記録source、viewport、theme、capabilityが変化した際に更新する。
通常frameは共有`GameUiFrame`からHUD、range、固定readout、再生buttonのlabel、chart cursorと選択地点の風だけを投影する。
`view_revision`を照合し、静的な軸・系列・landmark・風gridを保持する。
WebはSnabbdomのVNodeを直接生成し、入力property、key、focusとscrollを保持してDOMへ反映する。

## 検証範囲

純粋なView/update、生成物のfreshness、境界の拒否条件、各adapterの配置・focus・scroll保持を分離して検査する。
DOM試験とGPUを使わないECS試験は実描画の受入と区別する。GPU起動とユーザー操作は利用可能な環境で別途確認する。

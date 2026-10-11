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

生成処理はCLIの`export-game-ui-schema bundle`からView、Action、platform input、update、frameのSchemaを取得する。
生成物は`web/src/generated/`の深いreadonly型、JSON Schema、standalone validator、ライセンス通知とgeneration stampである。
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

# WebSocketゲームセッションサーバー契約

本書は、Web・Nativeクライアントへ同じゲーム状態を配信するサーバーの実装契約を定義する。サーバーはゲーム進行と操縦入力の認可を所有し、クライアントは自身のcamera・XR・描画資源を所有する。外部飛行コントローラーのWasm・SITL・HITLは[飛行制御](flight-control.md#外部controller)の別契約であり、このWebSocketへ尾翼角やcontrollerバイナリフレームを直接送る方式ではない。

正本は[wire型とcodec](../crates/birdman-game-runtime/src/wire.rs)、[共有サーバー](../crates/birdman-game-runtime/src/server.rs)、[共有Remoteクライアント](../crates/birdman-game-runtime/src/remote.rs)である。入れ子のfield・enum・検証条件もこれらのRust定義に合わせる。

## 接続と同版照合

1. クライアントはゲームを配信するoriginの `GET /api/host` を `Accept: application/json` で取得する。
2. HTTP 200のJSONを下表で照合し、同originの `/api/session/ws` へ接続する。HTTPは `ws`、HTTPSは `wss` を使う。
3. WebSocket subprotocolとして `birdman-session-v1` を要求する。サーバーはこれを選択した接続だけを受理する。
4. クライアントは最初のJSONメッセージとして `hello` を送る。サーバーはidentity・platform・presentationを認可し、`welcome` を返す。
5. 初期roleは `mirror`。サーバーは完全な `view` を送り、対応する `view_ack` を受理してから、そのrevisionの `frame` を配信する。

| `/api/host` のfield | 値・意味 |
| --- | --- |
| `schema_version` | `1` |
| `protocol_version` | `1` |
| `source_sha` | クライアント自身の固定ビルドと一致する、40桁の小文字Git SHA |
| `contract_sha256` | 同じ共有application契約の64桁小文字SHA-256 |
| `web_bundle_sha256` | 配信する `web-bundle-manifest.json` の原文bytesの64桁小文字SHA-256 |
| `websocket_path` | `/api/session/ws` |

`hello.identity` と `welcome.identity` は上表の3つのhash fieldを持つ `ApplicationHostIdentity` である。`source_sha`・`contract_sha256` をクライアント自身のビルドと照合し、bootstrapで取得したidentityと `welcome` のidentityも一致させる。application契約hashの対象はaggregate application schemaであり、wire版は `protocol_version` でも照合する。

HTTP 404は共有起動ownerにおけるLocal実行の判定である。200の不正JSON・不一致identity・接続失敗はエラーとして扱う。サーバーはbootstrapへ `Content-Type: application/json` と `Cache-Control: no-store` を付ける。

HTTP配信・TLS・Origin認可の正本は[Host HTTPアダプタ](../crates/birdman-game-host/src/http.rs)、同梱Webのfile/hash/MIME検証は[Web配布実装](../crates/birdman-game-host/src/web_distribution.rs)を参照する。ブラウザーのupgradeはscheme・host・実効portが同originのときに受理する。Originを持たないNative接続も初期mirrorで接続する。TLS証明書・hostnameの検証とブラウザーのsecure context条件を適用する。

## JSONの共通規則と認可

- 形式: 各JSONメッセージは1つのWebSocket text messageで送る。variantは `kind` にsnake_case名を入れ、構造体の未知fieldは拒否する。メッセージの連結や改行区切りの独自framingは使用しない。
- 整数: `u64` のID・epoch・sequenceはJSON数値である。実装は64 bit整数の精度を保持するJSON codecを使用する。ブラウザーI/Oはwire textをそのまま共有Rustへ通知する。
- `PeerScope`: `{"client_id":1,"connection_epoch":1}` の形で、実socketへ割り当てた接続と照合する。新接続は `welcome` のscopeを使う。scopeの値を別socketの認可へ流用しない。
- `ActorScope`: `{"peer":{"client_id":1,"connection_epoch":1},"role_generation":1}` の形である。canonical入力は現在のoperatorと現在のrole世代に束縛する。
- 入力順序: `input.sequence` は同role世代で厳密に増加させる。role更新でサーバーの受理sequenceは0へ戻る。通常入力の `view_revision` はサーバーがそのpeerへ発行した現在のViewに一致させ、内側のControl・chart測定のrevisionも一致させる。
- 時刻: `input.observed_ms` はクライアントの観測値であり、物理時間を進める根拠にはしない。canonical進行はホストの単調時計からのpulseだけで行う。
- 操作権: 同時operatorは最大1接続。付与・解除・切断・PauseはHost管理のtyped commandで実行する。WebSocketにrole自己昇格メッセージはない。
- mirror: canonical操縦を送らず、自身の表示・camera・XR・資源準備と解放を継続する。公開済みViewのchart測定はpeer自身のpresentationへ反映できる。

role喪失・入力期限切れ・切断では操縦入力を退役し、飛行をPauseへ接続する。新operatorには中立の再取得と明示的なResumeを要求する。操作権は任意のOS programやserial pathを実行する権限を含まない。

## クライアントからサーバーへのメッセージ

次のfield一覧は `kind` を除いた必須fieldを示す。`Option` fieldも現在のproducerでは `null` または値を送る。

| `kind` | field | 受理契約 |
| --- | --- | --- |
| `hello` | `protocol_version`, `identity`, `platform`, `presentation` | 接続ごとに最初の1回。platformは生能力・viewport等、presentationは共有Endpointが出力した `GameUiPlatformInput`。両者のviewportを一致させる |
| `input` | `actor`, `sequence`, `view_revision`, `observed_ms`, `input` | 現role・順序・Viewで認可したcanonical入力 |
| `platform` | `peer`, `observation` | 生の `ApplicationPlatformObservation` の更新 |
| `endpoint_status` | `peer`, `resource_epoch`, `mode`, `visible`, `input_focused`, `presentation` | 実クライアントの表示・GPU資源採用状態。epochの受理は後述 |
| `heartbeat` | `peer` | application接続の生存通知。サーバーは `pong` を返す |
| `view_ack` | `peer`, `view_revision` | 完全Viewを受理し保持した通知 |
| `frame_ack` | `peer`, `frame_id`, `resource_epoch`, `disposition` | Frameとその転送pinを退役。`disposition` は `accepted` または `discarded` |
| `transfer_ack` | `peer`, `transfer_id` | 対象bytes転送を受理した通知。これだけでFrame採用やGPU準備完了とはしない |
| `prepare_assets` | `peer`, `request_id`, `target` | Endpointで割り当てた一意request IDと `ApplicationGraphicsTarget` を指定 |
| `release_assets` | `peer`, `resource_epoch` | そのpeerの対象GPU資源に結び付くサーバーpinを退役 |
| `download_completed` | `peer`, `request_id`, `failure` | 発行済みdownloadの成功は `null`、失敗は原因文字列。role交代後も元peer/requestで完了認可する |
| `activate` | `actor`, `grant_id`, `view_revision`, `control_id` | サーバー発行済みの `ActivationGrant` と完全一致する操作だけを認可 |

`input.input` は次の `CanonicalInput` である。

| `kind` | field | 内容 |
| --- | --- | --- |
| `flight_devices` | `gamepads` | 生の `ApplicationGamepadObservation` 配列。保持した機器観測をホストpulse時に使用する |
| `action` | `action` | 現Viewのenabled actionまたは `back_action` と一致する `GameUiAction` |
| `observation` | `input` | `ApplicationInput` のうち `control`, `keyboard`, `physical_key`, `chart_measured`, `control_pointer` を受理する。XR tracking・描画完了・保存完了はcanonical入力へ転送しない |
| `configuration_loaded` | `document`, `request_id`, `completion` | 発行済みpickerの結果。元request・actor・documentで認可し、現在Viewへの遷移だけでは完了を失効させない |

型の詳細は[集約入力と描画契約](../crates/birdman-game-runtime/src/types.rs)、[UI actionとplatform](../crates/birdman-game-presentation/src/game_ui/types.rs)を参照する。`control_pointer` のcamera/look処理はクライアントEndpoint、touch操縦とchart操作のcanonical処理は共有サーバーへ接続する。

`ConfigurationResult` は `found { transfer }`、`missing`、`cancelled`、`failed { cause }` の直和である。選択開始は `ActivationGrant.operation` の `pick_configuration { document, request_id, maximum_bytes }` に束縛する。もう一方のoperationは `presentation { action }` である。アップロードは元のbytesを転送し、[既存設定検証](../crates/birdman-game-runtime/src/server/transfers.rs)へ渡す。remoteからのControllerExecution選択はWasm guestを受理し、Native実行pathの認可はHostのローカル設定が所有する。

設定uploadのmanifestは `local_id = request_id`、`revision = 0` とし、総長を発行済み `maximum_bytes` 内へ制限する。ControllerExecutionは `guest_program`、その他の設定は `configuration` を指定する。発行済みpickerに対応しないmanifestから領域を確保しない。

## サーバーからクライアントへのメッセージ

| `kind` | field（`kind` を除く） | 内容 |
| --- | --- | --- |
| `welcome` | `protocol_version`, `identity`, `peer`, `role`, `role_generation` | 同版照合後の接続scopeと初期mirror役割 |
| `role` | `peer`, `role`, `role_generation` | 現在の `operator` / `mirror` と新しいrole世代 |
| `view` | `peer`, `view`, `keyboard`, `keyboard_consumption`, `grants` | 完全な `GameUiView`、keyboard設定・共有入力消費能力、発行済みactivation一覧 |
| `frame` | `peer`, `publication` | 対応Viewを参照するFrame・Scene・buffer manifestの一組 |
| `assets` | `peer`, `request_id`, `assets_generation`, `aircraft_identity`, `target`, `assets`, `buffers` | Prepare要求への完全な資源offer。機体identityは32要素のbyte配列 |
| `assets_failed` | `peer`, `request_id`, `cause` | 対象準備requestの失敗 |
| `effect` | `peer`, `effect` | `download_file { request_id, file_name, mime_type, transfer }` を発行 |
| `pong` | `peer` | application heartbeatの応答 |
| `closed` | `peer`, `cause` | サーバーが接続を終端する原原因 |

`publication` の全fieldは `frame_id`, `assets_generation`, `resource_epoch`, `view_revision`, `server_time_ms`, `frame`, `scene`, `display`, `ui_scale`, `buffers` である。`frame` は `GameUiFrame`、`scene` は[共有SceneSource](../crates/birdman-game-runtime/src/scene.rs)、`display` は共有 `DisplayAdjustment` である。サーバーのphysicsやUI状態をクライアントで再構築せず、この完全なpublicationから最新のローカルhead/lookを投影する。

## View・Frame・GPU資源の順序

1. 変更Viewをreliable列で送る。クライアントの `view_ack` がその現在revisionと一致した後、同revisionのFrameを送る。
2. Frame内の全descriptorとmanifestを照合し、必要なbuffer転送が揃ってからpublication全体を採用する。部分受理のbytesを採用済みFrameへ混ぜない。
3. クライアントは `prepare_assets` の `request_id` を、その準備のresource epochとして使う。サーバーはcanonical環境・機体の `assets_generation` と要求targetに結び付けてofferする。
4. クライアントは実bytesからGPU候補を準備・commitし、`endpoint_status.resource_epoch` へ当該request IDを入れる。サーバーは自peerへ発行したpinと現在のassets generationに一致した場合だけReadyを認可し、Frameの `resource_epoch` へそのpeerのepochを返す。資源未採用のepochは0で表す。
5. 入力可能状態は、現在role世代のView・採用済みFrameのView revision・実commit済みresource epochが一致した状態から共有Rustが導出する。Viewだけの先着や旧GPU epochのFrameを、新しい表示の操作可能性として扱わない。
6. 採用・破棄したFrameには実 `frame_id` と `resource_epoch` で `frame_ack` を返す。静的資源はGPU寿命が終わった後の `release_assets` まで保持する。資源Readyの `endpoint_status` と `release_assets` で静的転送の完了をまとめて認可する。

CPU受信ACKとGPU寿命を区別する。クライアント内のDrawing leaseは消費完了まで、Prepare leaseはcommit後のactive resourceへ引き継いでrelease/rollback完了まで不変bytesをpinする。接続断やrole交代後も、自分の既存display・資源・downloadの終端処理を完了する。

| 通知 | 証明する事象 | 保持する寿命 |
| --- | --- | --- |
| `view_ack` | 完全Viewを受理・保持した | そのViewを参照するFrameの送信を許可する |
| `frame_ack: accepted` | 必要な実bytesが揃ったFrame/snapshotを採用した | サーバーのFrame転送pinを退役し、adopted key集合を更新する。GPU upload完了は別に通知する |
| `frame_ack: discarded` | Frameを採用せず破棄した | 対象Frame転送pinを退役する |
| `transfer_ack` | 対象transferを受信した | Frame採用・GPU Readyを兼ねない |
| `endpoint_status` | 発行済みepochの実GPU commitと表示状態 | 静的転送の完了を認可し、静的資源pin自体は保持する |
| `release_assets` | その資源epochを解放した | 対象の静的資源pinと残る転送を退役する |

## Binary転送

各WebSocket binary messageは40 byteのheaderと、その直後のchunk bytesで構成する。整数はすべてlittle endian、offsetは転送本体の先頭を0とする。

| byte範囲 | 型 | 内容 |
| --- | --- | --- |
| 0..4 | 4 bytes | ASCII `BPGN` |
| 4 | u8 | kind: render buffer=1、configuration=2、flight record=3、guest program=4、download=5 |
| 5 | u8 | protocol version `1` |
| 6..8 | 2 bytes | reserved、両方0 |
| 8..16 | u64 | 非零 `transfer_id` |
| 16..20 | u32 | `local_id`。renderではbuffer ID、その他は発行済みrequestに束縛したID |
| 20..24 | u32 | `revision` |
| 24..32 | u64 | `offset` |
| 32..36 | u32 | `total_len` |
| 36..40 | u32 | `chunk_len` |
| 40.. | bytes | `chunk_len` と厳密一致する本体 |

`TransferManifest` は `transfer_id`, `kind`, `local_id`, `revision`, `byte_length` を持つ。JSON側の `kind` は `render_buffer`, `configuration`, `flight_record`, `guest_program`, `download` を使う。manifestを先に認可し、同一identity・総長・現在の連続offsetに一致するchunkだけを追加する。各chunkは非零長、最大262,144 byteで、offset加算のoverflowと総長超過を拒否する。

render用の `BufferManifest` は `key { buffer_id, revision }`, `data_type`, `byte_length`, `transfer_id` を持つ。`data_type` は `f32`, `u8`, `u16`, `u32` で、実descriptorの型・stride・count・長さと照合し、F32値の有限性を確認する。render bytes自身もlittle endianである。

- 不変性: 同じbuffer keyは同じ型・長さ・bytesを表す。async Prepareの途中で同IDの新revisionへすり替えない。
- 省略: `transfer_id: null` は、peerがacceptedとACKした現在のadopted Frameにある完全一致keyだけで使用する。クライアントは対応する同型・同長の不変snapshotを再用する。資源offerは全bufferを転送する。
- 採番: サーバー発のtransfer IDは接続内の全binary kindを通して厳密単調増加・非零・再使用なし。active転送を先に照合し、退役済みIDの遅延chunkは保持領域へ戻さない。クライアント発の設定uploadもpeer単位の単調IDで認可する。
- export: downloadは認可した実producer bytesのexact長をmanifestへ記載する。CSVへrender buffer上限を流用しない。`download_completed` は元requestで完了認可する。

## 上限とフロー制御

| 対象 | 現行上限・動作 |
| --- | --- |
| client → server JSON | UTF-8原文で1 MiB |
| server → client JSON | UTF-8原文で16 MiB |
| binary message | 40 + 262,144 = 262,184 byte |
| 同時peer | 8 |
| reliable列 | peerあたり64枠・16 MiB、全peer合計64 MiB。終端通知用creditを確保する |
| Frame配信 | operator最大60 Hz、mirror最大30 Hz。ACK待ちの採用候補1件と最新候補1件でcoalesceする |
| peerの資源準備 | 未完了・保持中を合わせて8件 |
| render buffer | 単一64 MiB、クライアントのpin上限256 MiB |
| Nativeクライアント生I/O | 合計予約64 MiB、うち送信credit 2 MiB。active/closing socket最大2、1受理batch最大8 messageか16 MiB+1 byte |

上限の正本は[共有定数](../crates/birdman-game-runtime/src/lib.rs)と[サーバーのqueue/pin管理](../crates/birdman-game-runtime/src/server.rs)である。設定文書の総長はdocumentごとの既存format上限を使い、binary chunk上限とは区別する。

送信enqueue・実socket送信完了・application ACKを別の事象として扱う。送信中とqueue内のbytesを計上し、受信は保持中・受理callback中も予約を保つ。低速peerは有限queueと期限で退役する。bulk転送やScene生成でcanonical executorを占有せず、入力・heartbeat・他peerの配信を継続する。

## heartbeat・切断・再接続

- 生存監視: 共有Remoteは250 ms間隔でJSON `heartbeat` を送り、サーバーは `pong` を返す。最終有効受信から5秒を超える接続を退役する。WebSocket制御frameのPing/Pongとapplication heartbeatは区別する。
- 操縦監視: operator入力の有効期限はホスト受理時計で1秒。heartbeatだけでは操縦入力を更新しない。中立観測・入力喪失・明示Resumeの認可は共有ownerが処理する。
- 再接続: 利用者のRetryを共有Rustが受け、旧socketの実終端後に新しいsocket lifetimeで接続する。新たなHello/Welcome、完全View、資源認可を行い、初期mirrorから管理側で再付与する。旧actor・grant・入力sequenceやresource scopeを新接続へ流用しない。
- 失敗: 不正なJSON/型・長さ・binary headerはcodecで拒否してpeerを退役する。古いactor・View・sequenceや未発行操作のAdmission拒否は、実装の認可経路に従って診断を返す。元causeを管理snapshotやクライアント診断へ保持し、失敗を描画Readyや完了ACKへ変換しない。
- 終了: `closed.cause` と実WebSocket closeのcode/reasonを記録する。共有Remoteの明示closeはcode 4000を使用し、実oncloseまで送信queue予算を保持する。listener停止時もHost内のlocal peerとauthorityは継続し、アプリ終了では資源とOS I/Oの有界drainを完了する。

接続scopeの形を確認する最小の完全JSON例を示す。実接続では `welcome.peer` の値を使用する。

```json
{"kind":"heartbeat","peer":{"client_id":1,"connection_epoch":1}}
```

```json
{"kind":"pong","peer":{"client_id":1,"connection_epoch":1}}
```

サーバー自身の実装には[Host service](../crates/birdman-game-host/src/service.rs)と[Native HTTP/WebSocket I/O](../crates/birdman-game-host/src/network_io.rs)、ソフトウェア接続の確認には[実Hostと複数クライアントの旅程](../crates/birdman-game-host/tests/remote_session.rs)を参照する。
